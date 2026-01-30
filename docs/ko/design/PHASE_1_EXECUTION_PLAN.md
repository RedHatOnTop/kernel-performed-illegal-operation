# Phase 1: 코어 (Core) 실행 계획

## 개요

본 문서는 PHASE_1_CORE.md의 설계를 기반으로 실제 구현 순서와 세부 태스크를 정의합니다.

**시작일:** 2026-01-22  
**목표:** WASM "Hello World" 실행, 기본 WASI 동작, VirtIO-Blk 읽기/쓰기

---

## 현재 상태 분석

### 이미 존재하는 구현 (Phase 0에서)

| 모듈 | 파일 | 상태 |
|------|------|------|
| GDT | `kernel/src/gdt.rs` | 완료 |
| IDT | `kernel/src/interrupts/mod.rs`, `idt.rs` | 기본 구조 완료 |
| PIC | `kernel/src/interrupts/pic.rs` | 기본 구조 완료 (미사용) |
| 메모리 관리 | `kernel/src/memory/mod.rs` | 완료 |
| 힙 할당자 | `kernel/src/allocator.rs` | 완료 |
| 시리얼 출력 | `kernel/src/serial.rs` | 완료 |
| 스케줄러 | `kernel/src/scheduler/` | 기본 구조 존재 |
| Runtime | `runtime/src/` | 스켈레톤 존재 |

### 부족한 구현

| 모듈 | 설명 | 우선순위 |
|------|------|----------|
| APIC | Local APIC / I/O APIC 초기화 | P0 (필수) |
| 타이머 | APIC 타이머 인터럽트 | P0 (필수) |
| 컨텍스트 스위치 | 어셈블리 구현 | P0 (필수) |
| ACPI 파싱 | MADT 테이블에서 APIC 정보 획득 | P1 (중요) |
| PCI 열거 | VirtIO 디바이스 검색 | P1 (중요) |
| VirtIO 공통 | VirtQueue 구현 | P1 (중요) |
| VirtIO-Blk | 블록 디바이스 드라이버 | P1 (중요) |
| WASM 런타임 | wasmi 통합 (no_std) | P1 (중요) |
| WASI 기본 | fd_write, clock_time_get | P2 (후순위) |

---

## 실행 단계

### Stage 1: 인터럽트 인프라 (1-2일)

APIC 기반 인터럽트 처리 시스템 구축

#### 1.1 APIC 모듈 생성

**파일:** `kernel/src/interrupts/apic.rs`

```
- [ ] LocalApic 구조체 정의
- [ ] MSR을 통한 APIC 활성화
- [ ] Spurious Interrupt Vector 설정
- [ ] EOI (End of Interrupt) 구현
- [ ] APIC ID 읽기
```

#### 1.2 I/O APIC 모듈

**파일:** `kernel/src/interrupts/ioapic.rs`

```
- [ ] IoApic 구조체 정의
- [ ] 레지스터 읽기/쓰기
- [ ] IRQ 리다이렉션 테이블 설정
- [ ] IRQ 마스크/언마스크
```

#### 1.3 APIC 타이머 설정

**파일:** `kernel/src/interrupts/apic.rs` 확장

```
- [ ] LVT Timer 레지스터 설정
- [ ] 분주비 설정
- [ ] 주기적 모드 구성
- [ ] 타이머 인터럽트 핸들러 등록
```

#### 1.4 인터럽트 핸들러 확장

**파일:** `kernel/src/interrupts/mod.rs`

```
- [ ] APIC 타이머 핸들러 (벡터 32)
- [ ] 인터럽트 통계 카운터
- [ ] 선점 플래그 관리
```

---

### Stage 2: 스케줄러 완성 (2-3일)

선점형 태스크 스케줄링 구현

#### 2.1 컨텍스트 스위치 어셈블리

**파일:** `kernel/src/scheduler/context.rs` + `switch.s`

```
- [ ] TaskContext 구조체 (callee-saved 레지스터)
- [ ] switch_context 어셈블리 함수
- [ ] global_asm! 매크로로 인라인
- [ ] 커널 스택 스위치
```

#### 2.2 태스크 관리 강화

**파일:** `kernel/src/scheduler/task.rs`

```
- [ ] 커널 스택 할당 (16KB per task)
- [ ] TaskContext 초기화
- [ ] 태스크 종료 처리
- [ ] 태스크 대기/깨우기 메커니즘
```

#### 2.3 스케줄러 통합

**파일:** `kernel/src/scheduler/mod.rs`

```
- [ ] 타이머 인터럽트에서 schedule() 호출
- [ ] 선점 동작 확인
- [ ] yield_now() 구현
- [ ] 유휴 태스크 (idle task) 동작
```

#### 2.4 테스트

```
- [ ] 두 개의 태스크 번갈아 출력 확인
- [ ] 타이머 기반 선점 확인
- [ ] 태스크 종료 및 정리 확인
```

---

### Stage 3: ACPI 파싱 (1-2일)

하드웨어 정보 동적 획득

#### 3.1 ACPI 크레이트 통합

**파일:** `kernel/Cargo.toml`, `kernel/src/acpi/mod.rs`

```
- [ ] acpi 크레이트 의존성 추가 (no_std)
- [ ] AcpiHandler 구현 (물리-가상 주소 변환)
- [ ] RSDP 검색 (bootloader 제공 또는 메모리 스캔)
- [ ] ACPI 테이블 파싱
```

#### 3.2 MADT 파싱

**파일:** `kernel/src/acpi/madt.rs`

```
- [ ] Local APIC 주소 획득
- [ ] I/O APIC 목록 획득
- [ ] Interrupt Source Override 처리
- [ ] APIC 모듈에 정보 전달
```

---

### Stage 4: PCI 열거 (1일)

PCI 디바이스 검색 및 관리

#### 4.1 PCI 설정 공간 접근

**파일:** `kernel/src/driver/pci/mod.rs`

```
- [ ] I/O 포트 기반 설정 공간 읽기/쓰기
- [ ] Vendor ID, Device ID 읽기
- [ ] Class Code, Subclass 읽기
- [ ] BAR (Base Address Register) 파싱
```

#### 4.2 디바이스 열거

**파일:** `kernel/src/driver/pci/enumerate.rs`

```
- [ ] 모든 버스/디바이스/함수 스캔
- [ ] VirtIO 디바이스 필터링 (Vendor 0x1AF4)
- [ ] 디바이스 목록 저장
- [ ] 디바이스 정보 로깅
```

---

### Stage 5: VirtIO 드라이버 (2-3일)

VirtIO 블록 디바이스 드라이버

#### 5.1 VirtIO 공통 레이어

**파일:** `kernel/src/driver/virtio/mod.rs`

```
- [ ] VirtIO 디바이스 초기화 시퀀스
- [ ] 피처 협상
- [ ] 설정 공간 접근
- [ ] 디바이스 상태 관리
```

#### 5.2 VirtQueue 구현

**파일:** `kernel/src/driver/virtio/queue.rs`

```
- [ ] VirtqDesc, VirtqAvail, VirtqUsed 구조체
- [ ] 메모리 레이아웃 계산
- [ ] 디스크립터 체인 추가
- [ ] 완료 폴링
- [ ] 인터럽트 기반 완료 (선택)
```

#### 5.3 VirtIO-Blk 드라이버

**파일:** `kernel/src/driver/virtio/blk.rs`

```
- [ ] 블록 디바이스 설정 파싱
- [ ] 읽기 요청 (IN)
- [ ] 쓰기 요청 (OUT)
- [ ] 플러시 요청
- [ ] 에러 처리
```

#### 5.4 테스트

```
- [ ] QEMU에 virtio-blk 디바이스 추가
- [ ] 첫 섹터 읽기 성공
- [ ] 섹터 쓰기 및 재읽기 확인
```

---

### Stage 6: WASM 런타임 (3-4일)

no_std 환경에서 WASM 실행

#### 6.1 wasmi 크레이트 통합

**파일:** `runtime/Cargo.toml`, `runtime/src/engine.rs`

```
- [ ] wasmi 의존성 (no_std, alloc)
- [ ] Engine 초기화
- [ ] 모듈 컴파일
- [ ] 인스턴스 생성
```

#### 6.2 WASI 기본 구현

**파일:** `runtime/src/wasi/mod.rs`

```
- [ ] WasiCtx 구조체
- [ ] 파일 디스크립터 테이블 (stdin, stdout, stderr)
- [ ] 환경 변수 / 명령줄 인자
```

#### 6.3 fd_write 구현

**파일:** `runtime/src/wasi/io.rs`

```
- [ ] iovec 파싱
- [ ] stdout/stderr -> 시리얼 출력
- [ ] 바이트 수 반환
```

#### 6.4 clock_time_get 구현

**파일:** `runtime/src/wasi/clock.rs`

```
- [ ] CLOCK_REALTIME (TSC 기반)
- [ ] CLOCK_MONOTONIC
- [ ] 시간 반환
```

#### 6.5 WASI 링커

**파일:** `runtime/src/wasi/linker.rs`

```
- [ ] wasi_snapshot_preview1 함수 등록
- [ ] proc_exit 스텁
- [ ] args_* 스텁
- [ ] environ_* 스텁
```

#### 6.6 "Hello World" 테스트

```
- [ ] Rust로 간단한 WASM 작성 (println!("Hello, WASM!"))
- [ ] WASM 바이너리를 커널에 포함
- [ ] 실행 및 시리얼 출력 확인
```

---

## 의존성 그래프

```
Stage 1 (APIC) ─────────────────┐
                                ▼
Stage 2 (Scheduler) ◄───────── Stage 1
                                │
Stage 3 (ACPI) ─────────────────┤
                                │
Stage 4 (PCI) ──────────────────┤
        │                       │
        ▼                       │
Stage 5 (VirtIO) ◄──────────────┘
                                
Stage 6 (WASM) ◄──── Stage 1, 2 (타이머, 스케줄러 필요)
```

---

## 검증 체크리스트

### Stage 1 완료 기준
- [ ] APIC 타이머 인터럽트 1초당 100회 발생
- [ ] 타이머 핸들러에서 카운터 증가 확인
- [ ] EOI 전송 후 다음 인터럽트 정상 발생

### Stage 2 완료 기준
- [ ] 두 태스크 번갈아 "Task A" / "Task B" 출력
- [ ] 타이머 기반 선점 동작 (한 태스크가 busy loop 중에도 전환)
- [ ] 태스크 종료 후 다른 태스크 정상 실행

### Stage 3 완료 기준
- [ ] ACPI 테이블에서 Local APIC 주소 정상 파싱
- [ ] I/O APIC 정보 로깅
- [ ] 기존 하드코딩 주소 대체

### Stage 4 완료 기준
- [ ] PCI 디바이스 목록 출력
- [ ] VirtIO 디바이스 감지 (Vendor 0x1AF4)

### Stage 5 완료 기준
- [ ] VirtIO-Blk 디바이스 초기화 성공
- [ ] 섹터 0 읽기 성공 (512바이트)
- [ ] 섹터 쓰기 및 검증 성공

### Stage 6 완료 기준
- [ ] wasmi 엔진 초기화 성공
- [ ] WASM 모듈 로드 성공
- [ ] "Hello, WASM!" 시리얼 출력 확인
- [ ] clock_time_get으로 시간 값 획득 확인

---

## 위험 요소 및 대응

| 위험 | 영향 | 대응 |
|------|------|------|
| wasmi no_std 호환 문제 | WASM 실행 불가 | 최신 wasmi 버전 확인, 필요시 포크 |
| ACPI 파싱 실패 | 하드코딩 폴백 사용 | QEMU 기본값 사용 (LAPIC: 0xFEE00000) |
| VirtIO MMIO vs PCI | 드라이버 구조 변경 | QEMU 설정으로 PCI 모드 강제 |
| 컨텍스트 스위치 버그 | 커널 패닉 | 단계별 디버깅, GDB 활용 |

---

## QEMU 테스트 설정

### Stage 5 (VirtIO-Blk) 테스트용

```powershell
# 빈 디스크 이미지 생성
qemu-img create -f raw test-disk.img 64M

# QEMU 실행 (VirtIO-Blk 추가)
qemu-system-x86_64 `
    -bios OVMF.fd `
    -drive format=raw,file=kpio-uefi.img `
    -drive format=raw,file=test-disk.img,if=virtio `
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 `
    -nographic
```

---

## 다음 단계

Phase 1 완료 후:
- Phase 2: 시스템 서비스 (파일시스템, 네트워크)
- Phase 3: 사용자 인터페이스 (터미널, GUI)

---

## 변경 이력

| 날짜 | 변경 사항 |
|------|-----------|
| 2026-01-22 | 초기 실행 계획 작성 |
