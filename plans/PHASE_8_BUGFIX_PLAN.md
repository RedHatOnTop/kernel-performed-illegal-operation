# Phase 8: Technical Debt Resolution (기술 이슈 수정)

> **상태**: 진행 중 (8-1 완료)  
> **발견 경위**: Phase 7-4 QEMU 부팅 검증 (commit `1efe2d8`) 과정에서 10개 이슈 식별  
> **부팅 환경**: QEMU 10.2.0, UEFI pflash, bootloader 0.11.14, nightly-2026-01-01  
> **부팅 결과**: GDT→IDT→Memory→Heap→Scheduler→Terminal→VFS→Net→APIC 성공, **ACPI에서 page fault로 크래시**

---

## 발견된 이슈 목록

| # | 이슈 | 심각도 | 파일 | 라인 |
|---|------|--------|------|------|
| 1 | ACPI 물리→가상 주소 변환 누락 | Critical | `kernel/src/hw/acpi.rs` | L163 |
| 2 | ACPI `tables()` unsound `&'static` 반환 | High | `kernel/src/hw/acpi.rs` | L455-462 |
| 3 | 부트 순서: `net::init()` → PCI/VirtIO 이전에 호출 | High | `kernel/src/main.rs` | L151 vs L174 |
| 4 | VirtIO net `probe()` 빈 함수 | High | `kernel/src/drivers/net/virtio_net.rs` | L684-690 |
| 5 | QEMU에 NIC 디바이스 미설정 | High | `scripts/run-qemu.ps1` 등 | - |
| 6 | `free_frame()` 미구현 (메모리 누수) | Medium | `kernel/src/memory/mod.rs` | L64-67 |
| 7 | `acpi`/`aml` 크레이트 미사용 + feature 충돌 | Low | `kernel/Cargo.toml` | L38-39 |
| 8 | ~869개 빌드 경고 미관리 | Medium | 크레이트 전체 | - |
| 9 | BIOS 부트 FAT parser overflow | Medium | bootloader 0.11.14 외부 크레이트 | - |
| 10 | VirtIO net init이 부트 시퀀스에서 호출 안 됨 | High | `kernel/src/main.rs` | L178-182 |

> 이슈 #3과 #10은 부트 순서 문제로 통합, #4와 #5는 VirtIO NIC로 통합 → **8개 서브페이즈**

---

## Sub-phase 8-1: ACPI 물리→가상 주소 변환 수정

### 심각도: **Critical** — 커널 크래시 원인

### 근본 원인

`kernel/src/hw/acpi.rs` L163:
```rust
pub unsafe fn parse(rsdp_addr: u64) -> Result<Self, &'static str> {
    let rsdp = unsafe { &*(rsdp_addr as *const Rsdp) };  // ← 물리주소 직접 역참조!
```

부트로더가 제공하는 `rsdp_addr`은 **물리 주소** (예: `0xf52e0`)이지만, 커널은 페이징이 활성화된 상태이므로 `physical_memory_offset`을 더해 **가상 주소**로 변환해야 합니다. 동일한 버그가 다음 함수들에도 존재:
- `parse_rsdt()` L196: `&*(rsdt_addr as *const AcpiTableHeader)`
- `parse_xsdt()` L211: `&*(xsdt_addr as *const AcpiTableHeader)`
- `add_table()` L232: `&*(table_addr as *const AcpiTableHeader)`
- `MadtInfo::parse()` L318: `&*(madt_addr as *const AcpiTableHeader)`

QEMU 시리얼 로그에서 확인된 크래시:
```
EXCEPTION: PAGE FAULT
  Error Code: CAUSED_BY_WRITE
  Accessed Address: VirtAddr(0x1f77e014)
  Instruction Pointer: ...::hw::acpi::...
```

### 목표

ACPI 파서의 모든 물리 주소 접근에 `phys_mem_offset`을 적용하여 page fault 제거

### 수행할 작업

1. `AcpiTables` 구조체에 `phys_mem_offset: u64` 필드 추가 — 내부 메서드에서 자동 변환
2. `AcpiTables::parse(rsdp_addr, phys_mem_offset)` 시그니처 변경
3. `parse()` 내부: `let virt_addr = rsdp_addr + self.phys_mem_offset;` 후 역참조
4. `parse_rsdt()`, `parse_xsdt()` 내부에서 `self.phys_mem_offset` 사용하여 변환
5. `add_table()`: 물리 주소를 `FoundTable.address`에 저장 (기존 유지)
6. `init_with_rsdp(rsdp_addr, phys_mem_offset)` 변경, MADT parse 시 `madt_table.address + phys_mem_offset` 적용
7. `main.rs` 호출부에서 `phys_mem_offset` 전달

> **설계 결정**: `FoundTable.address`는 물리 주소 유지 (디버그/로깅용). 실제 접근 시 항상 `+ phys_mem_offset` 변환.

### QG (Quality Gate) — ✅ ALL PASSED (2026-02-20)

- [x] `cargo build --target x86_64-kpio.json` 성공
- [x] QEMU UEFI 부팅 시 `[ACPI] Parsed 6 ACPI table(s)` 로그 출력
- [x] ACPI 관련 page fault 미발생 (시리얼에 `PAGE FAULT` 없음, 타이머까지 정상 동작)
- [x] MADT 파싱 성공: `[ACPI] MADT: 1 local APICs, 1 I/O APICs, 5 overrides` 출력

---

## Sub-phase 8-2: ACPI `tables()` Unsound 참조 수정

### 심각도: **Medium** — Undefined Behavior (현재 호출자 없음, 향후 사용 시 위험)

### 근본 원인

`kernel/src/hw/acpi.rs` L455-462:
```rust
pub fn tables() -> Option<&'static AcpiTables> {
    unsafe {
        let ptr = &*ACPI_TABLES.lock() as *const Option<AcpiTables>;
        (*ptr).as_ref()  // ← MutexGuard drop 후 dangling reference
    }
}
```

`ACPI_TABLES.lock()`의 `MutexGuard`가 이 줄에서 즉시 drop되면서, 원시 포인터 `ptr`은 뮤텍스 보호 없이 데이터를 참조합니다. 다른 코어에서 동시에 lock하면 데이터 레이스가 발생합니다. 또한 `&'static`은 실제 라이프타임이 아닙니다.

### 목표

Sound한 API로 변경하여 UB를 제거

### 수행할 작업

1. `ACPI_TABLES` 타입을 `spin::Mutex<Option<AcpiTables>>` → `spin::Once<AcpiTables>` 변경
2. `init_with_rsdp()`에서 `ACPI_TABLES.call_once(|| tables)` 으로 초기화
3. `tables()` → `ACPI_TABLES.get()` 반환 (`Option<&AcpiTables>`, 진정한 `&'static`)
4. `table_count()`, `table_signatures()` 등 헬퍼 함수들도 `Once::get()` 사용으로 수정
5. `MADT_INFO`도 동일 패턴으로 `spin::Once<MadtInfo>`로 변경

### QG (Quality Gate)

- [ ] `cargo build` 성공
- [ ] `tables()` 반환 타입이 `Option<&AcpiTables>` (sound lifetime)
- [ ] `unsafe` 블록이 `tables()` 에서 제거됨
- [ ] QEMU 부팅 시 기존과 동일 동작 확인

---

## Sub-phase 8-3: 부트 순서 재배치

### 심각도: **High** — 기능 장애 (네트워크 스택 초기화 실패)

### 근본 원인

`kernel/src/main.rs` 현재 부트 순서:
```
L151: net::init()           ← 네트워크 스택 (DHCP 시도하지만 NIC 없음)
...
L156: APIC init
L161: ACPI init
L174: PCI enumerate         ← 여기서 NIC 발견
L178: VirtIO block init     ← block만 init, network 안 함
```

네트워크 스택이 PCI 열거와 VirtIO NIC 탐지 **이전에** 초기화되므로, NIC 없이 DHCP를 시도하여 실패합니다. 또한 VirtIO network init이 부트 시퀀스에 아예 포함되어 있지 않습니다.

### 목표

부트 순서를 논리적으로 재배치: PCI → VirtIO → Network

### 수행할 작업

> **보정**: PCI/APIC 간 순서 변경은 불필요. 핵심은 `net::init()` 1줄 이동 + VirtIO net probe 1줄 추가.

1. `main.rs`에서 `net::init()` 호출을 VirtIO 초기화 **이후**로 이동 (L153 → ~L190)
2. VirtIO block init 이후에 `drivers::net::virtio_net::probe()` 호출 추가
3. 최종 순서 (APIC/ACPI/PCI 간 상대 순서는 기존 유지):
   ```
   GDT → IDT → Memory → Heap → Scheduler → Terminal → VFS
   → APIC → ACPI → PCI enumerate → VirtIO block + VirtIO net probe → net::init()
   → PS/2 mouse → WASM → Boot animation → Timer
   ```
4. 단계 번호 및 주석 재조정

### QG (Quality Gate)

- [ ] `cargo build` 성공
- [ ] QEMU 시리얼 로그에서 PCI enumerate가 net::init() 이전에 출력
- [ ] VirtIO network probe 로그 출력
- [ ] `[KPIO] Network stack ready` 가 VirtIO 이후에 출력

---

## Sub-phase 8-4: VirtIO Net Probe 구현 + QEMU NIC 디바이스 추가

### 심각도: **High** — NIC 미탐지

### 근본 원인

`kernel/src/drivers/net/virtio_net.rs` L684-690:
```rust
pub fn probe() {
    // Would scan PCI for VirtIO devices:
    // - Vendor ID: 0x1AF4 (Red Hat)
    // - Device ID: 0x1000 (network - legacy) or 0x1041 (network - modern)
    //
    // Or scan MMIO regions for VirtIO MMIO devices
}
```

완전히 빈 함수입니다. 또한 QEMU 실행 스크립트에 `-device virtio-net-pci`가 없어 NIC 자체가 가상 하드웨어에 존재하지 않습니다.

PCI 모듈에는 이미 `find_virtio_network()` (L418-424)와 `PciDevice.bars` 필드가 구현되어 있어, probe에서 이를 활용할 수 있습니다.

### 목표

VirtIO NIC 탐지 및 초기화 파이프라인 완성

### 수행할 작업

> **보정**: VirtIO PIO 모드의 read/write 함수가 미구현 (0 반환/no-op)이므로, `init_pio()` 호출은 `DeviceNotFound` 에러를 반환함. Phase 8은 bugfix 단계이므로 **탐지+로깅** 으로 scope 제한. 전체 PIO VirtIO 드라이버 구현은 별도 phase.

1. `probe()` 구현 (탐지 + 로깅만):
   ```rust
   pub fn probe() {
       let network_devs = crate::driver::pci::find_virtio_network();
       for dev in &network_devs {
           crate::serial_println!(
               "[VirtIO Net] Found device at {} (BAR0={:#x})",
               dev.address, dev.bars[0]
           );
       }
       if network_devs.is_empty() {
           crate::serial_println!("[VirtIO Net] No VirtIO network devices found");
       }
   }
   ```
2. `scripts/run-qemu.ps1`에 `-netdev user,id=net0 -device virtio-net-pci,netdev=net0` 추가
3. `scripts/quick-run.ps1` 동일 적용
4. `scripts/qemu-test.ps1` 동일 적용

### QG (Quality Gate)

- [ ] `cargo build` 성공
- [ ] QEMU PCI 열거 로그에 `1af4:1000` (VirtIO Net) 표시
- [ ] `[VirtIO Net] Found device at ...` 로그 출력
- [ ] 스크립트 3개 모두에 NIC 인자 포함

---

## Sub-phase 8-5: `free_frame()` 구현

### 심각도: **Medium** — 메모리 누수

### 근본 원인

`kernel/src/memory/mod.rs` L64-67:
```rust
pub fn free_frame(_addr: usize) {
    // In a real implementation, this would return the frame to the pool
    // For now, we don't reclaim frames
}
```

할당된 물리 프레임이 절대 반환되지 않아, 장시간 실행 시 물리 메모리가 고갈됩니다.

### 목표

물리 프레임 해제 기능을 구현하여 메모리 재사용 가능하게 함

### 수행할 작업

1. `FreeFrameList` 구조체 추가 (스택 기반 free list):
   ```rust
   struct FreeFrameList {
       frames: Vec<usize>,
   }
   ```
2. `GLOBAL_FREE_FRAMES: Mutex<FreeFrameList>` 전역 변수 추가
3. `free_frame(addr)` 구현: 정렬 검증 후 free list에 push
4. `allocate_frame()` 수정: free list에서 먼저 확인 → 없으면 기존 allocator 사용
5. 이중 해제 방지: debug 모드에서 중복 체크

### QG (Quality Gate)

- [ ] `cargo build` 성공
- [ ] `free_frame()` 호출 후 `allocate_frame()`이 해당 프레임 재사용
- [ ] 비정렬 주소 전달 시 panic 또는 에러 (debug 모드)
- [ ] QEMU 부팅 시 기존과 동일 동작

---

## Sub-phase 8-6: 미사용 의존성 정리

### 심각도: **Low** — 불필요한 빌드 시간/바이너리 크기

### 근본 원인

`kernel/Cargo.toml` L38-39:
```toml
acpi = "5.0"
aml = "0.16"
```

이 크레이트들은 의존성에 선언되어 있지만, `kernel/src/hw/acpi.rs`에서는 자체 파서를 사용하여 **실제로 import하지 않습니다**. 또한 `[features]` 섹션의 `acpi = []` feature가 크레이트명과 충돌할 수 있습니다.

### 목표

미사용 의존성 제거 및 feature 충돌 해소

### 수행할 작업

1. `kernel/Cargo.toml`에서 `acpi = "5.0"` 제거
2. `kernel/Cargo.toml`에서 `aml = "0.16"` 제거
3. `[features]` 섹션의 `acpi = []` → `acpi-tables = []` 로 이름 변경
4. 코드베이스에서 `use acpi::` 또는 `use aml::` 검색하여 참조 없음 확인
5. `cargo build` 후 의존성 트리 확인

### QG (Quality Gate)

- [ ] `cargo build` 성공
- [ ] `cargo tree -p kpio-kernel` 에 `acpi`/`aml` 크레이트 미포함
- [ ] Feature 이름 충돌 없음

---

## Sub-phase 8-7: 빌드 경고 관리

### 심각도: **Medium** — 개발 효율 저하

### 근본 원인

개발 초기 단계에서 많은 코드가 스텁/플레이스홀더로 작성되어 `dead_code`, `unused_variables`, `unused_imports` 등 경고가 ~869개 발생합니다. 이는 실제 문제를 경고 노이즈 속에서 발견하기 어렵게 합니다.

### 목표

개발 단계에 맞는 경고 정책 수립 및 적용

### 수행할 작업

1. `kernel/src/main.rs` 크레이트 루트에 단계별 allow 추가:
   ```rust
   #![allow(dead_code)]        // 개발 중 스텁 코드 허용
   #![allow(unused_variables)]  // 미구현 함수 파라미터 허용
   #![allow(unused_imports)]    // 향후 사용 예정 import 허용
   ```
2. 워크스페이스 `Cargo.toml`에 `[workspace.lints.rust]` 섹션 추가 (향후 점진적 강화)
3. 실제 문제 가능성이 있는 경고 (예: `unused_must_use`) 분류
4. 각 서브 크레이트(`kpio-dom`, `kpio-js` 등)에도 동일 적용

### QG (Quality Gate)

- [ ] `cargo build 2>&1` 경고 수 < 100개
- [ ] `#![deny(unsafe_op_in_unsafe_fn)]` 기존 정책 유지
- [ ] 실제 버그 관련 경고 (예: `unused_must_use`)는 allow하지 않음

---

## Sub-phase 8-8: BIOS 부트 문제 문서화

### 심각도: **Medium** — 대안 존재 (UEFI pflash)

### 근본 원인

bootloader 0.11.14의 FAT 파서에서 debug 빌드 시 정수 오버플로가 발생합니다:
```
panicked at 'attempt to multiply with overflow'
bootloader-x86_64-bios-0.11.14\...\fat.rs
```

이는 외부 크레이트 버그이며 커널 코드로 직접 수정할 수 없습니다. `--release` 모드에서는 오버플로 체크가 비활성화되어 통과할 수 있으나, 다른 문제가 발생할 수 있습니다.

### 목표

알려진 부트 제한사항을 문서화하고 권장 부팅 방법을 명시

### 수행할 작업

1. `docs/known-issues.md` 생성:
   - BIOS 부트 FAT overflow 이슈 설명
   - bootloader 0.11.14 버전 정보
   - 추적 가능한 upstream 이슈 참조
   - UEFI pflash 우회법 안내
2. `docs/QUICK_START.md`에 UEFI pflash가 권장 부팅 방법임을 명시
3. 스크립트 주석에 UEFI pflash 사용 이유 추가

### QG (Quality Gate)

- [ ] `docs/known-issues.md` 존재
- [ ] `docs/QUICK_START.md`에 UEFI pflash 권장 사항 포함
- [ ] BIOS 부트 실패 시 사용자가 문서를 통해 해결책 확인 가능

---

## 실행 순서 및 의존성

```
8-1 (ACPI 주소) ──→ 8-2 (ACPI tables UB) ──→ 8-3 (부트 순서) ──→ 8-4 (VirtIO probe)
                                                                        │
                                                                        ▼
                                                              [QEMU 통합 부팅 검증]
                                                                        │
                                              ┌─────────────────────────┼─────────────────┐
                                              ▼                         ▼                 ▼
                                     8-5 (free_frame)          8-6 (deps 정리)    8-7 (경고 관리)
                                                                                          │
                                                                                          ▼
                                                                                 8-8 (BIOS 문서화)
```

- **8-1 → 8-2**: 동일 파일(acpi.rs) 수정, 8-1이 phys_mem_offset 전달 구조를 만들어야 8-2에서 Once 패턴 적용 가능
- **8-2 → 8-3**: tables() API 변경 후 부트 순서 정리
- **8-3 → 8-4**: 부트 순서가 올바라야 VirtIO probe가 의미 있음
- **8-4 이후 QEMU 검증**: Critical/High 수정이 모두 반영된 상태에서 통합 부팅 테스트
- **8-5, 8-6, 8-7**: 독립적 수정, 순서 무관
- **8-8**: 모든 수정 후 최종 문서화

---

## 커밋 계획

| 서브페이즈 | 커밋 메시지 (예상) |
|-----------|-------------------|
| 8-1 | `fix(acpi): add phys-to-virt address translation for RSDP/RSDT/XSDT/MADT` |
| 8-2 | `fix(acpi): replace unsound tables() with spin::Once for sound lifetime` |
| 8-3 | `fix(boot): reorder init sequence — PCI/VirtIO before net::init()` |
| 8-4 | `feat(virtio-net): implement PCI probe + add QEMU NIC device` |
| 8-5 | `feat(memory): implement free_frame() with stack-based free list` |
| 8-6 | `chore(deps): remove unused acpi/aml crates, rename acpi feature` |
| 8-7 | `chore(lint): add crate-level allow for development-stage warnings` |
| 8-8 | `docs: add known-issues.md, document BIOS boot limitation` |

---

## 예상 결과

Phase 8 완료 후:
1. **커널이 ACPI까지 크래시 없이 부팅** (현재 page fault → 정상 파싱)
2. **PCI → VirtIO → Network 순서 정상화** (현재 역순 → 논리적 순서)
3. **VirtIO NIC 탐지 및 초기화** (현재 빈 probe → PCI 스캔)
4. **물리 메모리 재사용 가능** (현재 leak → free list)
5. **빌드 경고 ~869 → <100** (개발 효율 향상)
6. **알려진 제한사항 문서화** (BIOS 부트 등)

---

## 사전 검증 리뷰 (2026-02-20)

> 실제 코드와 QEMU 시리얼 로그를 대조하여 각 서브페이즈의 타당성, 수정 접근법, 검증 방법을 재검토함.

### 리뷰 방법

- `kernel/src/hw/acpi.rs` (463줄), `kernel/src/main.rs` (550줄), `kernel/src/memory/mod.rs` (228줄), `kernel/src/drivers/net/virtio_net.rs` (721줄), `kernel/src/net/mod.rs` + `dhcp.rs`, `kernel/src/driver/pci.rs` (444줄), `kernel/Cargo.toml` 전문 확인
- `target/qemu-test-serial.log` (4,295 bytes) 크래시 로그 대조
- `spin::Once` API 프로젝트 내 사용 사례 확인 (`browser/memory.rs:583`)

---

### 8-1 검증: ACPI 물리→가상 주소 변환 — **확인됨, 수정 필요**

**이슈 존재 확인**: YES

시리얼 로그 크래시 분석:
```
[KPIO] Initializing ACPI...
EXCEPTION: PAGE FAULT
Accessed Address: Ok(VirtAddr(0x1f77e014))   ← XSDT의 물리 주소
```

부팅 시 `phys_mem_offset = 0x28000000000` (시리얼 로그에서 확인). RSDP 물리 주소 (~`0xf52e0`)는 부트로더가 하위 메모리를 일부 identity-map했기 때문에 우연히 접근 가능했으나, RSDP 내의 XSDT 주소(`~0x1f77e014`)는 매핑되지 않은 물리 주소 → page fault.

**수정 접근법 검증**: 올바름. 단, **설계 개선 사항** 발견:

| 항목 | 계획 | 개선 제안 |
|------|------|----------|
| `phys_mem_offset` 전달 방식 | 모든 함수에 파라미터로 전달 | `AcpiTables` 구조체에 `phys_mem_offset` 필드를 추가하여 내부 메서드에서 자동 변환. 파라미터 반복 제거 |
| `FoundTable.address` 저장값 | 물리 주소 유지 (기존) | **가상 주소로 저장** — 이후 `MadtInfo::parse()` 등에서 재변환 불필요. 또는 `AcpiTables`에 offset을 저장하고 변환 메서드 제공 |
| `MadtInfo::parse()` | `phys_mem_offset` 별도 전달 | `init_with_rsdp()`에서 `madt_table.address`를 이미 가상 주소로 변환 후 전달하면 `MadtInfo::parse()`는 수정 불필요 |

**권장**: `AcpiTables`에 `phys_mem_offset: u64` 필드 추가, `parse()` 내부에서 모든 물리→가상 변환을 수행, `FoundTable.address`는 가상 주소로 저장 (또는 물리 유지하되 getter에서 변환).

**검증 방법**: `scripts/qemu-test.ps1 -Mode linux` 실행. 시리얼 로그에서:
- `[ACPI] Parsed N ACPI table(s)` 출력 확인
- `[ACPI] MADT: N local APICs, M I/O APICs` 출력 확인
- PAGE FAULT 미발생 확인

**라인 번호 보정**: `parse()` 함수 시그니처는 실제 **L160**, `let rsdp = ...`는 **L161** (계획의 L163은 근사치).

---

### 8-2 검증: ACPI `tables()` UB — **확인됨, 단 호출자 없음**

**이슈 존재 확인**: YES, 코드 존재 (L456-462)

**중요 발견**: `tables()` 함수의 **호출자가 0개**임.
- `grep 'tables()'` 결과: `acpi.rs` 정의 외에 호출하는 코드 없음
- `table_count()`, `table_signatures()` 등은 `ACPI_TABLES.lock()` 직접 사용 — `tables()` 미사용
- `terminal/commands.rs:2988`은 `table_count()` 사용, `tables()` 아님

**심각도 조정**: High → **Medium** (dead code의 UB. 현재 트리거되지 않으나 향후 사용 시 위험)

**수정 접근법 검증**: `spin::Once<AcpiTables>` 전환은 올바름.
- 프로젝트 내 선례: `spin::Once<TabManager>` (browser/memory.rs:583)
- `MADT_INFO`도 동일 패턴 적용 가능
- `table_count()`, `table_signatures()` 등 헬퍼도 `Once::get()` 사용으로 단순화
- `Mutex` 제거로 lock contention 가능성도 제거

**추가 고려**: `spin::Once`는 `call_once()` 후 수정 불가. ACPI 재초기화가 필요한 시나리오(예: hot-plug)는 없으므로 문제 없음.

**검증 방법**: `cargo build` 성공 + `tables()` 함수에 `unsafe` 없음 확인 + QEMU 부팅 동일 동작.

---

### 8-3 검증: 부트 순서 재배치 — **확인됨, 설명 수정 필요**

**이슈 존재 확인**: YES

시리얼 로그 증거:
```
[KPIO] Initializing network stack...
[DHCP] Sending DISCOVER...
[Net] DHCP failed (DHCP timeout waiting for message type 2), using static config (10.0.2.15)
...
[KPIO] Initializing APIC...          ← APIC는 net 이후
[KPIO] Initializing ACPI...          ← ACPI는 PCI 이전 (page fault로 PCI까지 도달 못함)
```

**계획 수정 필요**:

| 항목 | 계획의 오류 | 실제 |
|------|-----------|------|
| "PCI enumerate를 APIC 이전으로 이동" | 불필요한 이동 제안 | PCI는 이미 APIC/ACPI **이후**에 있음. 이동 불필요 |
| 실제 필요한 수정 | - | `net::init()`을 VirtIO 초기화 **이후**로 이동하는 것만 필요 |

**정확한 현재 순서 (L151-183)**:
```
L153: net::init()        ← 이것만 밑으로 이동하면 됨
L158: APIC init
L163: ACPI init
L179: PCI enumerate
L183: VirtIO block init
```

**수정 후 순서**:
```
APIC → ACPI → PCI → VirtIO block → VirtIO net probe → net::init()
```

APIC/ACPI/PCI 간의 상대 순서는 변경 불필요. core fix는 **`net::init()` 1줄 이동 + VirtIO net probe 1줄 추가**.

**DHCP 타임아웃 영향**: `dhcp::wait_for_dhcp_reply()` (L275)에서 `max_iters=300`, 각 iter에 100,000 spin loops → NIC 없으면 ~3초 지연. 부트 순서 수정 후 NIC 존재 시 정상 동작, NIC 미존재 시 동일 타임아웃 (QEMU에 NIC 추가는 8-4에서 해결).

**검증 방법**: QEMU 시리얼 로그에서 PCI/VirtIO 로그가 `net::init()` 이전에 출력되는지 확인.

---

### 8-4 검증: VirtIO Net Probe + QEMU NIC — **확인됨, PIO 미구현 주의**

**이슈 존재 확인**: YES (probe() 빈 함수, QEMU NIC 미설정)

**중요 발견 — VirtIO PIO 미구현**:

`virtio_net.rs`의 I/O 함수 검토:
```rust
fn read8(&self, offset: u32) -> u8 {
    if let Some(mmio) = self.mmio_base {
        unsafe { ptr::read_volatile((mmio + offset as usize) as *const u8) }
    } else {
        // PIO mode - would use port I/O
        0                                           // ← 항상 0 반환!
    }
}
```

- `new_pio(io_base)` → `mmio_base = None` → `init()` 시 MMIO 경로 미진입
- PIO 모드의 `read8()/write8()/read32()/write32()` 모두 **no-op 또는 0 반환**
- 결과: `probe()` → `init_pio()` → `init()` 호출 → `read32(MAGIC)` returns 0 → `DeviceNotFound` 에러

**수정 접근법 조정**:

기존 계획의 `init_pio(io_base & 0xFFFC)` 호출은 `DeviceNotFound` 에러를 반환할 것. 두 가지 옵션:

| 옵션 | 내용 | 복잡도 |
|------|------|-------|
| A (최소) | `probe()`에서 PCI 디바이스 발견 로그만 출력, init 시도하지 않음 | Low |
| B (PIO 구현) | `read8_pio()` 등에 `x86_64::instructions::port::Port` 기반 실제 I/O 구현 | High |

**권장**: **옵션 A** (Phase 8은 bugfix 단계, 전체 PIO 구현은 별도 phase). 디바이스 탐지 + 로깅으로 scope 제한.

```rust
pub fn probe() {
    let network_devs = crate::driver::pci::find_virtio_network();
    for dev in &network_devs {
        crate::serial_println!(
            "[VirtIO Net] Found device at {} (BAR0={:#x})",
            dev.address, dev.bars[0]
        );
    }
    if network_devs.is_empty() {
        crate::serial_println!("[VirtIO Net] No VirtIO network devices found");
    }
}
```

**QEMU NIC 추가**: `-netdev user,id=net0 -device virtio-net-pci,netdev=net0` — 올바름.

**검증 방법**: QEMU 시리얼에서 `[VirtIO Net] Found device` 또는 `[PCI]` 출력에 `1af4:1000` 확인.

---

### 8-5 검증: `free_frame()` — **확인됨, 설계 주의사항 추가**

**이슈 존재 확인**: YES (L64-67, 완전 no-op)

**설계 주의사항**:

| 항목 | 상세 |
|------|------|
| `GlobalFrameAllocator` 구조 | bump allocator (next_frame → end_frame). free list 통합 가능 |
| `Vec<usize>` 사용 | Vec grow 시 heap alloc 발생. `free_frame()` 호출 시점에서 안전한지 확인 필요 |
| 이중 해제 | debug_assert로 체크 가능하나, Vec 선형 탐색은 O(n) — alloc_count가 많으면 느림 |
| 실제 사용처 | `free_frame()` 호출자가 있는지 확인 필요 |

```
$ grep -rn "free_frame" kernel/src/
→ 정의 1곳 (memory/mod.rs:64)
→ 호출자 확인 필요
```

**검증 방법**: `cargo build` 성공 + QEMU 부팅 동일 동작. 단위 테스트는 no_std 제약으로 제한적.

---

### 8-6 검증: 미사용 의존성 — **확인됨**

**이슈 존재 확인**: YES
- `use acpi::` / `use aml::` → 프로젝트 전체 0건 (계획 문서 참조 제외)
- `acpi = "5.0"`, `aml = "0.16"` → Cargo.toml에만 존재

**Feature 충돌**: `[features]` 섹션의 `acpi = []`는 `dependencies`의 `acpi` 크레이트명과 Cargo 2021 에디션에서 [충돌 가능](https://doc.rust-lang.org/cargo/reference/features.html#optional-dependencies). 의존성 제거 시 충돌도 해소.

**검증 방법**: `cargo build` + `cargo tree -p kpio-kernel` 확인.

---

### 8-7 검증: 빌드 경고 — **타당함**

경고 수는 이전 세션에서 확인된 근사치. `#![allow(dead_code, unused_variables, unused_imports)]`는 개발 단계에서 표준적 접근.

**검증 방법**: `cargo build 2>&1 | findstr "warning"` 카운트 비교.

---

### 8-8 검증: BIOS 부트 문서화 — **타당함**

외부 크레이트 버그이므로 문서화만 가능. UEFI pflash 우회법은 이미 검증됨.

---

### 종합 리뷰 판정

| 서브페이즈 | 이슈 확인 | 수정법 타당 | 보정 필요 |
|-----------|----------|-----------|----------|
| 8-1 | ✅ 확인 | ✅ 타당 | ⚠️ `AcpiTables`에 offset 필드 추가 권장, FoundTable.address 저장 방식 결정 |
| 8-2 | ✅ 확인 | ✅ 타당 | ⚠️ 심각도 High→Medium (호출자 없음), spin::Once 적용 확인됨 |
| 8-3 | ✅ 확인 | ⚠️ 수정 | ❌ "PCI를 APIC 이전으로 이동" → "net::init()만 VirtIO 이후로 이동"으로 수정 |
| 8-4 | ✅ 확인 | ⚠️ 수정 | ❌ PIO 미구현으로 init 불가 → 탐지+로깅만으로 scope 축소 |
| 8-5 | ✅ 확인 | ✅ 타당 | ⚠️ Vec grow 시 heap 안전성 확인 |
| 8-6 | ✅ 확인 | ✅ 타당 | - |
| 8-7 | ✅ 타당 | ✅ 타당 | - |
| 8-8 | ✅ 타당 | ✅ 타당 | - |

### 추가 발견 사항

**계획에 누락된 검증 인프라 항목**:
- 모든 QG의 QEMU 검증은 `scripts/qemu-test.ps1 -Mode linux` 으로 통일해야 함
- 8-4 완료 후 통합 부팅 테스트 시, 테스트 패턴에 `ACPI initialized`, `Found device` 등 새 패턴 추가 필요
