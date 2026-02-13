# Phase 7-2: Tier 2 — WASM/WASI 앱 런타임 (WASM App Runtime)

> **상위 Phase:** Phase 7 — App Execution Layer  
> **목표:** WASM을 KPIO의 범용 앱 바이너리 포맷으로 확립하여, 임의의 프로그래밍 언어(Rust/C/C++/Go/Python)로 작성된 앱을 안전하게 실행한다.  
> **예상 기간:** 6-8주 (8개 서브페이즈)  
> **의존성:** Phase 7-1 (앱 런타임 기반 + 웹 앱 플랫폼)  
> **우선순위:** 🔴 필수

---

## 현재 상태 분석 (As-Is)

Phase 7-1에서 구축된 앱 관리 인프라(레지스트리, 라이프사이클, 권한, VFS 샌드박스, 시스콜 106-111)를 기반으로, WASM 실행 엔진과 WASI 시스템 인터페이스를 **실동작 수준**으로 완성한다.

| 컴포넌트 | 위치 | 상태 | 비고 |
|---------|------|------|------|
| **Linear Memory** | `runtime/src/memory.rs` (243줄) | ✅ 구현 완료 | grow, read/write, bounds check |
| **Sandbox/Security** | `runtime/src/sandbox.rs` (291줄) | ✅ 구현 완료 | 메모리/CPU/FD/파일/네트워크 제한 |
| **JIT IR** | `runtime/src/jit/ir.rs` (827줄) | ✅ 구현 완료 | 포괄적 IR opcode 정의 |
| **JIT 프레임워크** | `runtime/src/jit/mod.rs` (281줄) | 🟡 프레임워크 | 3-tier 아키텍처 설계됨 |
| **JIT Codegen** | `runtime/src/jit/codegen.rs` (567줄) | 🟡 부분 구현 | x86-64 emit 프레임워크 존재 |
| **JIT 코드 캐시** | `runtime/src/jit/cache.rs` (230줄) | ✅ 구현 완료 | LRU 캐시 동작 |
| **JIT 프로파일링** | `runtime/src/jit/profile.rs` (302줄) | ✅ 구현 완료 | 호출 카운트, 분기 통계 |
| **WASI 자료구조** | `runtime/src/wasi.rs` (829줄) | 🟡 부분 구현 | 76개 에러코드, FD 권한 정의. VFS 미연결 |
| **Host 함수** | `runtime/src/host.rs` (143줄) | 🔴 Stub | 모든 함수 0/success 반환 |
| **Engine** | `runtime/src/engine.rs` (78줄) | 🔴 Stub | 실제 WASM 실행 불가 |
| **Module 파싱** | `runtime/src/module.rs` (186줄) | 🔴 Stub | magic number만 검증, 파싱 없음 |
| **Instance** | `runtime/src/instance.rs` (188줄) | 🔴 Stub | `call()` → 빈 Vec 반환 |
| **Component Model** | — | ❌ 없음 | WIT 파서, 컴포넌트 링커 전무 |
| **`.kpioapp` 패키지** | — | ❌ 없음 | 패키지 포맷 처리 코드 없음 |
| **WASM 앱 예제** | — | ❌ 없음 | .wasm 예제 앱 없음 |
| **커널 앱 관리** | `kernel/src/app/` (1,635줄) | ✅ 구현 완료 | Phase 7-1에서 구축 |
| **앱 시스콜 106-111** | `kernel/src/syscall/mod.rs` | ✅ 구현 완료 | Phase 7-1에서 구축 |
| **VFS 샌드박스** | `kernel/src/vfs/sandbox.rs` (297줄) | ✅ 구현 완료 | Phase 7-1에서 구축 |

---

## 서브페이즈 총괄 로드맵

```
주차    1         2         3         4         5         6         7         8
      ├─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤
 A    ██████████                                                                        WASM 모듈 파서
 B    ██████████████████████                                                            인터프리터 엔진 (wasmi 통합)
 C              ██████████████████████                                                  WASI Preview 1 완전 구현
 D                        ██████████████████████                                        Baseline JIT 완성
 E                                  ██████████████████████                              kpio:gui / kpio:system 호스트
 F                                            ██████████████████████                    .kpioapp 패키지 시스템
 G                                                      ██████████████████████          Component Model 기초
 H                                                                ██████████████████████ 종합 검증 & 데모 앱
```

> A+B 병렬 가능, D+E 부분 병렬 가능, F는 A-C 선행 필요

---

## Sub-Phase 7-2.A: WASM 모듈 파서 (WASM Module Parser)

### 목적

`runtime/src/module.rs`의 Stub 파서를 **완전한 WASM 바이너리 포맷 파서**로 교체하여, `.wasm` 파일의 모든 섹션을 구조적으로 해석한다.

### 선행 조건
- `runtime/src/memory.rs` LinearMemory 동작 (이미 완료)

### 작업

#### A-1. WASM 바이너리 포맷 파서 (`runtime/src/parser.rs` 신규)
- [ ] Magic number + Version 검증 (`\0asm` + `0x01`)
- [ ] 섹션 파서 (12개 섹션):
  - [ ] Type Section (1): 함수 시그니처 (`FuncType(params, results)`)
  - [ ] Import Section (2): 외부 함수/메모리/테이블/글로벌 import
  - [ ] Function Section (3): 함수 인덱스 → 타입 인덱스 매핑
  - [ ] Table Section (4): `funcref`/`externref` 테이블 정의
  - [ ] Memory Section (5): 선형 메모리 정의 (`min`, `max`)
  - [ ] Global Section (6): 글로벌 변수 (타입, 뮤터빌리티, 초기값)
  - [ ] Export Section (7): 내보내기 (함수, 메모리, 테이블, 글로벌)
  - [ ] Start Section (8): 시작 함수 인덱스
  - [ ] Element Section (9): 테이블 초기화 데이터
  - [ ] Code Section (10): 함수 바디 (locals + expression)
  - [ ] Data Section (11): 메모리 초기화 데이터
  - [ ] Custom Section (0): 이름 섹션(`name`), 디버그 정보
- [ ] LEB128 가변 길이 정수 디코더 (u32, i32, u64, i64)
- [ ] ValueType 파서 (`i32`, `i64`, `f32`, `f64`, `funcref`, `externref`)
- [ ] Expression(InitExpr) 파서: `i32.const`, `i64.const`, `f32/f64.const`, `global.get`, `ref.null`, `ref.func`

#### A-2. Module 구조체 리팩토링 (`runtime/src/module.rs`)
- [ ] `Module` 구조체 완전 재정의:
  ```rust
  pub struct Module {
      pub types: Vec<FuncType>,
      pub imports: Vec<Import>,
      pub functions: Vec<u32>,          // type_idx
      pub tables: Vec<TableType>,
      pub memories: Vec<MemoryType>,
      pub globals: Vec<Global>,
      pub exports: Vec<Export>,
      pub start: Option<u32>,           // function_idx
      pub elements: Vec<Element>,
      pub code: Vec<FunctionBody>,
      pub data: Vec<DataSegment>,
      pub name: Option<String>,         // custom section
  }
  ```
- [ ] `Module::from_bytes(bytes: &[u8]) → Result<Module, ParseError>` 구현
- [ ] `Module::validate() → Result<(), ValidationError>` — 기본 유효성 검사:
  - 타입 인덱스 범위 검사
  - 함수 시그니처 일치 검사
  - 메모리/테이블 갯수 제한 (각 1개, MVP)
  - Import/Export 이름 중복 검사

#### A-3. WASM 명령어 (Opcode) 정의 (`runtime/src/opcodes.rs` 신규)
- [ ] `Opcode` enum — WASM MVP 명령어 전체 정의 (~200개):
  - 제어 흐름: `unreachable`, `nop`, `block`, `loop`, `if`, `else`, `end`, `br`, `br_if`, `br_table`, `return`, `call`, `call_indirect`
  - 참조: `ref.null`, `ref.is_null`, `ref.func`
  - 매개변수: `drop`, `select`, `select_typed`
  - 변수: `local.get/set/tee`, `global.get/set`
  - 메모리: `i32.load`, `i64.load`, `f32.load`, `f64.load`, 각종 `load8/16/32_s/u`, `store`, `memory.size`, `memory.grow`
  - 상수: `i32.const`, `i64.const`, `f32.const`, `f64.const`
  - 비교: `i32.eqz/eq/ne/lt_s/lt_u/gt_s/gt_u/le_s/le_u/ge_s/ge_u`, `i64` 대응, `f32/f64` 대응
  - 산술: `i32.add/sub/mul/div_s/div_u/rem_s/rem_u/and/or/xor/shl/shr_s/shr_u/rotl/rotr`, `i64` 대응, `f32/f64` 대응
  - 변환: `i32.wrap_i64`, `i32.trunc_f32_s/u`, `i64.extend_i32_s/u`, `f32.convert_i32_s/u`, 등
  - 기타: `i32.clz/ctz/popcnt`, `f32.abs/neg/ceil/floor/trunc/nearest/sqrt`, 등
- [ ] Opcode ↔ `u8` 바이트 변환 (`From<u8>`, `Into<u8>`)
- [ ] 명령어 디코더: 바이트 스트림 → `Instruction` 시퀀스

### 산출물
- `runtime/src/parser.rs` 신규
- `runtime/src/module.rs` 재작성
- `runtime/src/opcodes.rs` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| A-QG1 | 최소 WASM 파싱 | Rust `wasm32-wasi` hello world → `Module::from_bytes()` 성공 | 유닛 테스트 |
| A-QG2 | Export 추출 | 파싱 후 `_start` 함수 export 존재 확인 | 유닛 테스트 |
| A-QG3 | Import 추출 | WASI import (`wasi_snapshot_preview1.fd_write` 등) 추출 | 유닛 테스트 |
| A-QG4 | 메모리 정의 | Memory Section → `LinearMemory` 생성 파라미터 추출 | 유닛 테스트 |
| A-QG5 | 코드 섹션 | 함수 바디 → Opcode 시퀀스 디코딩 성공 | 유닛 테스트 |
| A-QG6 | 검증 | 잘못된 타입 인덱스 → `ValidationError` 반환 | 유닛 테스트 |
| A-QG7 | 빌드 | `cargo build -p runtime` 성공 | CI |

---

## Sub-Phase 7-2.B: 인터프리터 엔진 (Interpreter Engine)

### 목적

파싱된 WASM 모듈을 **스택 기반 인터프리터**로 실행하여, JIT 없이도 모든 WASM 앱을 정확하게(느리더라도) 실행할 수 있게 한다. 이것은 JIT의 "콜드" 티어로도 기능한다.

### 선행 조건
- 7-2.A 퀄리티 게이트 A-QG1~QG7 전체 통과

### 작업

#### B-1. 스택 머신 (`runtime/src/interpreter.rs` 신규)
- [ ] `ValueStack`: `WasmValue` (i32/i64/f32/f64/funcref/externref) 스택
  - `push(value)`, `pop() → WasmValue`
  - `peek()`, `len()`, `is_empty()`
  - 스택 최대 깊이 제한 (64KB, configurable)
- [ ] `CallStack`: 함수 호출 프레임 스택
  - `CallFrame { func_idx, locals, return_arity, pc, block_stack }`
  - 최대 깊이 제한 (1024 프레임)
- [ ] `BlockStack`: 구조적 제어 흐름 (block/loop/if 중첩)
  - `Block { kind: Block|Loop|If, arity, continuation_pc }`
- [ ] `ProgramCounter`: 현재 실행 위치 (`func_idx`, `instr_offset`)

#### B-2. 명령어 실행기 (`runtime/src/executor.rs` 신규)
- [ ] `execute(module, func_idx, args) → Result<Vec<WasmValue>, TrapError>`
- [ ] 제어 흐름 명령어 구현:
  - [ ] `block/loop/if/else/end` — 블록 스택 관리
  - [ ] `br/br_if/br_table` — 분기 (라벨 인덱스 → 블록 탈출)
  - [ ] `call` — 직접 호출 (인자 push, 프레임 생성)
  - [ ] `call_indirect` — 테이블 기반 간접 호출 (시그니처 확인)
  - [ ] `return` — 현재 함수 반환
- [ ] 변수 명령어:
  - [ ] `local.get/set/tee` — 로컬 변수 접근
  - [ ] `global.get/set` — 글로벌 변수 접근
- [ ] 메모리 명령어:
  - [ ] `i32/i64/f32/f64.load` + 변형 (8/16/32 signed/unsigned)
  - [ ] `i32/i64/f32/f64.store` + 변형
  - [ ] `memory.size` → 현재 페이지 수
  - [ ] `memory.grow` → LinearMemory.grow()
  - Bounds check: 메모리 범위 초과 → `TrapError::MemoryOutOfBounds`
- [ ] 정수 산술 (i32/i64):
  - [ ] `add/sub/mul` — 래핑 산술
  - [ ] `div_s/div_u/rem_s/rem_u` — 0 나누기 → `TrapError::DivisionByZero`
  - [ ] `and/or/xor/shl/shr_s/shr_u/rotl/rotr`
  - [ ] `clz/ctz/popcnt`
  - [ ] `eqz/eq/ne/lt_s/lt_u/gt_s/gt_u/le_s/le_u/ge_s/ge_u` — 비교 → 0/1
- [ ] 부동소수점 산술 (f32/f64):
  - [ ] `add/sub/mul/div` — IEEE 754
  - [ ] `abs/neg/ceil/floor/trunc/nearest/sqrt`
  - [ ] `min/max/copysign`
  - [ ] 비교: `eq/ne/lt/gt/le/ge`
- [ ] 변환 명령어:
  - [ ] `i32.wrap_i64`, `i64.extend_i32_s/u`
  - [ ] `i32/i64.trunc_f32/f64_s/u` — NaN/Inf → Trap
  - [ ] `f32/f64.convert_i32/i64_s/u`
  - [ ] `i32/i64.reinterpret_f32/f64`, `f32/f64.reinterpret_i32/i64`
- [ ] 참조 명령어:
  - [ ] `ref.null/ref.is_null/ref.func`
- [ ] 기타:
  - [ ] `unreachable` → `TrapError::Unreachable`
  - [ ] `nop` — 아무 동작 없음
  - [ ] `drop` — 스택 최상위 제거
  - [ ] `select` — 조건부 선택

#### B-3. Instance 및 Store 리팩토링 (`runtime/src/instance.rs`)
- [ ] `Store` 재구현:
  ```rust
  pub struct Store {
      pub memories: Vec<LinearMemory>,
      pub tables: Vec<Table>,
      pub globals: Vec<GlobalValue>,
      pub functions: Vec<FunctionInstance>,
      pub host_functions: BTreeMap<String, HostFunction>,
  }
  ```
- [ ] `Instance` 재구현:
  - `instantiate(module, imports) → Result<Instance, InstantiationError>`
  - `call(export_name, args) → Result<Vec<WasmValue>, TrapError>`
  - Data 세그먼트 → 메모리 초기화
  - Element 세그먼트 → 테이블 초기화
  - Start 함수 자동 실행
- [ ] `Table` 구현: `funcref`/`externref` 엔트리, `get/set/grow`
- [ ] `GlobalValue`: `{value: WasmValue, mutable: bool}`
- [ ] Import 해석: `(module, name)` → Store 내 함수/메모리/글로벌 바인딩

#### B-4. Engine 통합 (`runtime/src/engine.rs`)
- [ ] `Engine` 재구현:
  - `load_module(bytes) → Result<ModuleId, EngineError>`
  - `instantiate(module_id, imports) → Result<InstanceId, EngineError>`
  - `call(instance_id, func, args) → Result<Vec<WasmValue>, TrapError>`
  - `drop_instance(instance_id)`
- [ ] 모듈 캐시 (파싱 결과 캐싱)
- [ ] 인스턴스 풀 관리

### 산출물
- `runtime/src/interpreter.rs` 신규
- `runtime/src/executor.rs` 신규
- `runtime/src/instance.rs` 재작성
- `runtime/src/engine.rs` 재작성

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| B-QG1 | Hello World | `(func (export "_start") (call $fd_write ...))` → stdout에 "Hello" 출력 | 유닛 테스트 |
| B-QG2 | 산술 정확성 | i32/i64 기본 연산 (add/sub/mul/div) 100+개 테스트 케이스 통과 | 유닛 테스트 |
| B-QG3 | 부동소수점 | f32/f64 IEEE 754 준수 (NaN 전파, Inf 처리) | 유닛 테스트 |
| B-QG4 | 제어 흐름 | 피보나치(재귀), 팩토리얼(반복) 정확한 결과 | 유닛 테스트 |
| B-QG5 | 메모리 | memory.grow + load/store → 올바른 읽기/쓰기 | 유닛 테스트 |
| B-QG6 | Trap | division by zero, memory OOB, unreachable → TrapError | 유닛 테스트 |
| B-QG7 | 간접 호출 | call_indirect + 테이블 → 올바른 함수 디스패치 | 유닛 테스트 |
| B-QG8 | Rust WASM | `cargo build --target wasm32-wasi` hello world .wasm → 실행 성공 | 통합 테스트 |
| B-QG9 | 빌드 | `cargo build -p runtime` 성공 | CI |

---

## Sub-Phase 7-2.C: WASI Preview 1 완전 구현 (WASI Complete)

### 목적

기존 `wasi.rs`의 Stub 함수들을 **VFS 및 커널 연동 실제 구현**으로 교체하여, WASI 앱이 파일 I/O, 시계, 난수, 프로세스 종료를 수행할 수 있게 한다.

### 선행 조건
- 7-2.B 퀄리티 게이트 B-QG1 통과 (인터프리터로 WASM 실행 가능)

### 작업

#### C-1. WASI ↔ VFS 연결 (`runtime/src/wasi.rs` 재구현)
- [ ] `WasiCtx` → VFS 연동:
  - `preopened_dirs: Vec<(u32, String)>` → VFS 경로 매핑
  - 기본 FD:
    - 0 = stdin (읽기 → 빈 바이트)
    - 1 = stdout (쓰기 → 커널 콘솔 / 캡처 버퍼)
    - 2 = stderr (쓰기 → 커널 콘솔)
    - 3+ = preopened dirs
- [ ] `fd_read(fd, iovs) → Result<usize>`:
  - FD 유효성 + 권한(`FD_READ`) 검사
  - VFS `read_all()` 호출 → iovs 버퍼에 복사
- [ ] `fd_write(fd, iovs) → Result<usize>`:
  - stdout/stderr → 콘솔 출력 버퍼에 기록
  - 일반 파일 FD → VFS `write_all()` 호출
- [ ] `fd_seek(fd, offset, whence) → Result<u64>`:
  - `Set/Cur/End` whence 처리
  - 현재 오프셋 + 파일 크기 기반 계산
- [ ] `fd_close(fd) → Result<()>`:
  - FD 테이블에서 제거
  - VFS 리소스 해제
- [ ] `fd_fdstat_get(fd) → Result<FdStat>`:
  - 파일 유형, 플래그, 권한 반환
- [ ] `fd_prestat_get(fd) → Result<Prestat>`:
  - preopened dir 이름 길이 반환
- [ ] `fd_prestat_dir_name(fd, buf, len) → Result<()>`:
  - preopened dir 이름 버퍼에 복사

#### C-2. 경로 기반 파일 작업
- [ ] `path_open(dirfd, flags, path, oflags, rights, inheriting, fdflags) → Result<u32>`:
  - 경로 해석 (dirfd 기준 상대 경로)
  - VFS 샌드박스 내 경로 검증
  - `O_CREAT/O_EXCL/O_TRUNC/O_DIRECTORY` 플래그 처리
  - 새 FD 할당 및 반환
- [ ] `path_create_directory(dirfd, path) → Result<()>`:
  - VFS `mkdir()` 호출
- [ ] `path_remove_directory(dirfd, path) → Result<()>`:
  - VFS `rmdir()` 호출
- [ ] `path_unlink_file(dirfd, path) → Result<()>`:
  - VFS `unlink()` 호출
- [ ] `path_rename(old_dirfd, old_path, new_dirfd, new_path) → Result<()>`:
  - VFS `rename()` 호출
- [ ] `path_filestat_get(dirfd, flags, path) → Result<FileStat>`:
  - VFS `stat()` 호출 → FileStat 구조체 변환
- [ ] `fd_readdir(fd, buf, cookie) → Result<usize>`:
  - VFS `readdir()` 호출 → dirent 직렬화

#### C-3. 시계 및 난수
- [ ] `clock_time_get(id, precision) → Result<u64>`:
  - `REALTIME` → 커널 시스템 시계 (나노초)
  - `MONOTONIC` → 커널 모노토닉 카운터 (나노초)
  - `PROCESS_CPUTIME` → 프로세스 CPU 시간 (근사치)
- [ ] `random_get(buf, len) → Result<()>`:
  - 커널 CSPRNG (`kernel::random`) 연동
  - 또는 RDRAND 명령어 기반 난수

#### C-4. 프로세스 및 환경
- [ ] `args_get(argv, argv_buf) → Result<()>`:
  - `WasiCtx.args` → 선형 메모리에 기록
- [ ] `args_sizes_get() → Result<(usize, usize)>`:
  - (인자 수, 총 바이트)
- [ ] `environ_get(environ, environ_buf) → Result<()>`:
  - `WasiCtx.env_vars` → 선형 메모리에 기록
- [ ] `environ_sizes_get() → Result<(usize, usize)>`:
  - (환경 변수 수, 총 바이트)
- [ ] `proc_exit(code)`:
  - 인터프리터 실행 중단
  - `ExitCode(code)` 반환
  - 앱 라이프사이클 → `Terminated` 상태 전이

#### C-5. 호스트 함수 등록 (`runtime/src/host.rs` 재구현)
- [ ] `register_wasi_functions(store, wasi_ctx)`:
  - `wasi_snapshot_preview1.*` 네임스페이스 전체 등록
  - 각 호스트 함수 → 클로저에서 `WasiCtx` 메서드 호출
- [ ] 호스트 함수 호출 프로토콜:
  - 인터프리터 → import 함수 발견 → 호스트 함수 디스패치
  - 인자: 선형 메모리 포인터/길이 → Rust 슬라이스 변환
  - 반환: WASI errno (success=0)

### 산출물
- `runtime/src/wasi.rs` 재작성
- `runtime/src/host.rs` 재작성

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| C-QG1 | stdout 출력 | WASM `fd_write(1, ...)` → 커널 콘솔에 "Hello, WASI!" | 통합 테스트 |
| C-QG2 | 파일 읽기 | preopened dir 내 파일 `path_open` → `fd_read` → 올바른 내용 | 유닛 테스트 |
| C-QG3 | 파일 쓰기 | `path_open(O_CREAT)` → `fd_write` → VFS에 파일 존재 | 유닛 테스트 |
| C-QG4 | 디렉토리 | `path_create_directory` → `fd_readdir` → 항목 존재 | 유닛 테스트 |
| C-QG5 | 시계 | `clock_time_get(MONOTONIC)` → 0 아닌 나노초 값 | 유닛 테스트 |
| C-QG6 | 난수 | `random_get(buf, 32)` → 32바이트 비제로 (확률적) | 유닛 테스트 |
| C-QG7 | 인자 전달 | `WasiCtx.args = ["app", "--flag"]` → `args_get/sizes_get` 올바른 반환 | 유닛 테스트 |
| C-QG8 | proc_exit | `proc_exit(42)` 호출 → `ExitCode(42)` 반환, 실행 중단 | 유닛 테스트 |
| C-QG9 | 샌드박스 | preopened dir 밖 경로 접근 → `EACCES` 에러 | 유닛 테스트 |
| C-QG10 | Rust WASI 앱 | `wasm32-wasi` Rust 앱 (파일 읽기/쓰기) → 정상 종료 | 통합 테스트 |

---

## Sub-Phase 7-2.D: Baseline JIT 완성 (Baseline JIT Completion)

### 목적

기존 JIT 프레임워크(`runtime/src/jit/`)의 Partial 구현을 완성하여, **인터프리터 대비 5-10x 성능 향상**을 달성한다. "웜" 함수(호출 100회 이상)를 자동으로 JIT 컴파일한다.

### 선행 조건
- 7-2.B 퀄리티 게이트 전체 통과 (인터프리터 정확성 보장)

### 작업

#### D-1. WASM → IR 변환기 완성 (`runtime/src/jit/ir.rs` 확장)
- [ ] `WasmToIr::translate_function(code_body) → Result<IrFunction>`:
  - WASM 명령어 → IR opcode 1:1 매핑
  - 블록 구조 → IR 라벨/분기로 변환
  - 스택 기반 → SSA-like IR (가상 레지스터)으로 변환
- [ ] 모든 WASM MVP 명령어 → IR 변환 커버:
  - 정수 연산 (i32/i64): 기존 IR opcode 활용
  - 부동소수점 (f32/f64): 기존 IR opcode 활용
  - 메모리 접근: bounds check IR 주입
  - 제어 흐름: `block/loop/if` → IR `Label` + `Branch`
  - 함수 호출: `call` → IR `Call`, `call_indirect` → IR `IndirectCall`
- [ ] BasicBlock 분할: 분기 타겟에서 블록 경계 생성

#### D-2. IR → x86-64 코드 생성 완성 (`runtime/src/jit/codegen.rs` 확장)
- [ ] 레지스터 알로케이터:
  - 간단한 선형 스캔 알로케이터 (Baseline)
  - 가용 레지스터: RAX, RCX, RDX, R8-R11 (caller-saved)
  - 스필/리로드: 스택 프레임에 공간 확보
- [ ] 코드 생성 규칙:
  - [ ] `IrAdd/Sub/Mul` → x86 `add/sub/imul`
  - [ ] `IrDiv/Rem` → x86 `div/idiv` (RAX:RDX 관례)
  - [ ] `IrAnd/Or/Xor/Shl/Shr` → x86 대응 명령어
  - [ ] `IrLoad/Store` → x86 `mov [base + offset]` + bounds check
  - [ ] `IrBranch/BranchIf` → x86 `jmp/jcc`
  - [ ] `IrCall` → x86 `call` (System V ABI: RDI, RSI, RDX, RCX, R8, R9)
  - [ ] `IrReturn` → x86 `ret`
  - [ ] `IrConst` → x86 `mov reg, imm`
  - [ ] f32/f64 → XMM 레지스터(XMM0-XMM7) + SSE/SSE2 명령어
- [ ] Prologue/Epilogue:
  - `push rbp; mov rbp, rsp; sub rsp, frame_size`
  - `add rsp, frame_size; pop rbp; ret`
- [ ] 메모리 bounds check:
  - `cmp offset, memory_size; ja trap_handler`
  - Trap handler: 실행 중단, `TrapError` 반환

#### D-3. 실행 가능 메모리 관리
- [ ] W^X (Write XOR Execute) 강제:
  1. `mmap` (또는 커널 동적 메모리)으로 RW 페이지 할당
  2. 코드 생성 → RW 버퍼에 기록
  3. `mprotect` → RW → RX 전환
  4. 함수 포인터로 코드 실행
- [ ] 코드 영역 최대 크기 제한 (기본 16MB)

#### D-4. Tiered Compilation 연동
- [ ] 프로파일링 카운터:
  - 함수 진입 시 카운터 증가 (인터프리터에 삽입)
  - `count >= 100` → Baseline JIT 트리거
  - `count >= 10,000` → 향후 Optimizing JIT용 (Phase 7 이후)
- [ ] 비동기 컴파일:
  - 핫 함수 감지 → 컴파일 큐 추가
  - 컴파일 완료 → 코드 캐시에 등록
  - 다음 호출 시 JIT 코드 사용
- [ ] 폴백: JIT 컴파일 실패 시 인터프리터로 계속 실행

### 산출물
- `runtime/src/jit/ir.rs` 확장
- `runtime/src/jit/codegen.rs` 확장
- `runtime/src/jit/mod.rs` 확장

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| D-QG1 | JIT 정확성 | 피보나치(40) — 인터프리터와 JIT 결과 동일 | 유닛 테스트 |
| D-QG2 | 산술 정확성 | i32/i64 전체 연산 — JIT vs 인터프리터 일치 (1000+ 케이스) | 유닛 테스트 |
| D-QG3 | 메모리 접근 | load/store — JIT에서 bounds check 동작, OOB → Trap | 유닛 테스트 |
| D-QG4 | 함수 호출 | 재귀/간접 호출 — JIT 코드 간 + JIT↔호스트 호전환 | 유닛 테스트 |
| D-QG5 | W^X 준수 | JIT 코드 영역 — 쓰기 가능 상태에서 실행 불가 (또는 역) | 보안 테스트 |
| D-QG6 | 성능 향상 | 피보나치(30) 벤치마크 — JIT ≥ 5x 인터프리터 | 벤치마크 |
| D-QG7 | Tiered | 함수 호출 100회 → 자동 JIT 컴파일 발동 확인 | 유닛 테스트 |
| D-QG8 | 폴백 | JIT 불가 함수 (미지원 opcode) → 인터프리터 폴백 | 유닛 테스트 |
| D-QG9 | 코드 캐시 | JIT 컴파일 결과 캐시 → 재호출 시 재컴파일 없음 | 유닛 테스트 |

---

## Sub-Phase 7-2.E: kpio:gui / kpio:system 호스트 API

### 목적

WASM 앱이 KPIO 고유 기능(GUI 윈도우, 시스템 정보, IPC)에 접근할 수 있는 **커스텀 호스트 함수**를 구현한다. 기존 `host.rs`의 Stub을 실제 동작으로 교체한다.

### 선행 조건
- 7-2.C 퀄리티 게이트 C-QG1 통과 (WASI 호스트 함수 호출 동작)

### 작업

#### E-1. kpio:gui API (`runtime/src/host_gui.rs` 신규)
- [ ] `create_window(title_ptr, title_len, width, height) → window_id`:
  - 선형 메모리에서 제목 문자열 읽기
  - `AppLifecycle::launch()` 연동 → WASM 전용 윈도우 생성
  - `WindowContent::WasmApp` variant 추가
- [ ] `set_window_title(window_id, title_ptr, title_len)`:
  - 윈도우 제목 변경
- [ ] `draw_rect(window_id, x, y, w, h, color)`:
  - ARGB 색상 → 프레임버퍼 직접 렌더링
  - 커널 GUI `draw_filled_rect()` 호출
- [ ] `draw_text(window_id, x, y, text_ptr, text_len, size, color)`:
  - 선형 메모리에서 UTF-8 문자열 읽기
  - 커널 GUI `draw_string()` 호출
- [ ] `draw_line(window_id, x1, y1, x2, y2, color)`:
  - Bresenham 알고리즘 기반 라인 렌더링
- [ ] `draw_image(window_id, x, y, w, h, data_ptr, data_len)`:
  - RGBA 비트맵 데이터 → 프레임버퍼 복사
- [ ] `clear_window(window_id, color)`:
  - 전체 윈도우 영역 단색 채움
- [ ] `request_frame(window_id) → bool`:
  - VSync / 프레임 콜백 등록
  - 반환: 다음 프레임 준비 여부
- [ ] `poll_event(window_id, event_buf_ptr, buf_len) → event_type`:
  - 이벤트 큐에서 다음 이벤트 디큐
  - 이벤트 타입: `None(0)`, `KeyDown(1)`, `KeyUp(2)`, `MouseMove(3)`, `MouseDown(4)`, `MouseUp(5)`, `Close(6)`, `Resize(7)`
  - 이벤트 데이터를 선형 메모리 버퍼에 기록
- [ ] `close_window(window_id)`:
  - 윈도우 파괴 + 이벤트 큐 정리

#### E-2. kpio:system API (`runtime/src/host_system.rs` 신규)
- [ ] `get_time() → u64`:
  - 커널 시스템 시계 → 밀리초 timestamp
- [ ] `get_monotonic() → u64`:
  - 모노토닉 카운터 → 나노초
- [ ] `get_hostname(buf_ptr, buf_len) → usize`:
  - 호스트 이름 반환 ("kpio")
- [ ] `notify(title_ptr, title_len, body_ptr, body_len)`:
  - `NotificationCenter::show()` 연동
  - 알림 app_id → 현재 WASM 앱 ID
- [ ] `clipboard_read(buf_ptr, buf_len) → usize`:
  - 시스템 클립보드 → 선형 메모리 복사
  - 권한 검사: `clipboard: true` 필요
- [ ] `clipboard_write(data_ptr, data_len)`:
  - 선형 메모리 → 시스템 클립보드 기록
- [ ] `get_locale(buf_ptr, buf_len) → usize`:
  - 현재 시스템 로케일 코드 반환 (예: "ko-KR")
- [ ] `log(level, msg_ptr, msg_len)`:
  - 디버그 로그 출력 (level: 0=debug, 1=info, 2=warn, 3=error)

#### E-3. kpio:net API (`runtime/src/host_net.rs` 신규)
- [ ] `socket_create(domain, sock_type) → socket_id`:
  - TCP/UDP 소켓 생성
  - 권한 검사: `network != None` 필요
- [ ] `socket_connect(socket_id, addr_ptr, addr_len, port) → Result`:
  - TCP connect / UDP 대상 설정
- [ ] `socket_send(socket_id, data_ptr, data_len) → bytes_sent`:
  - 데이터 전송
- [ ] `socket_recv(socket_id, buf_ptr, buf_len) → bytes_received`:
  - 데이터 수신
- [ ] `socket_close(socket_id)`:
  - 소켓 해제

#### E-4. 호스트 함수 등록 통합
- [ ] `register_kpio_functions(store)`:
  - `kpio:gui.*`, `kpio:system.*`, `kpio:net.*` 전체 등록
- [ ] Import 해석 시 `kpio` 네임스페이스 자동 매핑

### 산출물
- `runtime/src/host_gui.rs` 신규
- `runtime/src/host_system.rs` 신규
- `runtime/src/host_net.rs` 신규
- `runtime/src/host.rs` 수정
- `kernel/src/gui/window.rs` 수정 (`WindowContent::WasmApp`)

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| E-QG1 | 윈도우 생성 | `create_window("Test", 400, 300)` → QEMU에 빈 윈도우 출현 | QEMU 시각 검증 |
| E-QG2 | 사각형 렌더링 | `draw_rect(id, 10, 10, 100, 100, 0xFFFF0000)` → 빨간 사각형 | QEMU 시각 검증 |
| E-QG3 | 텍스트 렌더링 | `draw_text(id, 10, 10, "Hello", 16, 0xFFFFFFFF)` → 텍스트 표시 | QEMU 시각 검증 |
| E-QG4 | 이벤트 수신 | 키보드 입력 → `poll_event` → `KeyDown` + keycode 반환 | 기능 테스트 |
| E-QG5 | 알림 | `notify("Alert", "Test")` → 토스트 알림 표시 | QEMU 시각 검증 |
| E-QG6 | 시계 | `get_time()` 연속 2회 → 두 번째 > 첫 번째 | 유닛 테스트 |
| E-QG7 | 권한 제한 | `clipboard: false` 앱의 `clipboard_read` → 에러 | 유닛 테스트 |
| E-QG8 | 소켓 | `socket_create → connect → send → recv → close` 체인 성공 | 통합 테스트 |

---

## Sub-Phase 7-2.F: .kpioapp 패키지 시스템

### 목적

WASM 앱의 **패키징, 설치, 실행** 파이프라인을 구축하여, `.kpioapp` 파일 하나로 앱 배포가 가능하게 한다.

### 선행 조건
- 7-2.C 퀄리티 게이트 C-QG10 통과 (WASI 앱 실행 가능)
- 7-2.E 퀄리티 게이트 E-QG1 통과 (GUI 앱 동작)

### 작업

#### F-1. 패키지 포맷 정의 (`runtime/src/package.rs` 신규)
- [ ] `.kpioapp` 패키지 구조 (ZIP 기반):
  ```
  my-app.kpioapp (ZIP)
  ├── manifest.toml        # 앱 메타데이터
  ├── app.wasm              # 메인 WASM 모듈
  ├── resources/            # 에셋
  │   ├── icon-192.png
  │   └── icon-512.png
  └── data/                 # 초기 데이터 (선택)
  ```
- [ ] `AppManifest` 구조체 (TOML 파싱):
  ```rust
  pub struct AppManifest {
      pub name: String,
      pub version: String,
      pub description: Option<String>,
      pub author: Option<String>,
      pub icon: Option<String>,         // "resources/icon-192.png"
      pub entry: String,                // "app.wasm"
      pub permissions: ManifestPermissions,
      pub min_kpio_version: Option<String>,
  }
  ```
- [ ] 패키지 검증:
  - ZIP 구조 유효성
  - `manifest.toml` 존재 + 파싱
  - `entry` 지정 WASM 파일 존재
  - WASM magic number 확인
  - 총 크기 제한 (기본 50MB)

#### F-2. 패키지 설치 (`runtime/src/package_installer.rs` 신규)
- [ ] `install_kpioapp(path: &str) → Result<KernelAppId>`:
  1. ZIP 해제 → 임시 디렉토리
  2. `manifest.toml` 파싱 → `AppManifest`
  3. 권한 검토 → 사용자 승인 다이얼로그 (권한 목록 표시)
  4. `AppInstall` 시스콜 → `KernelAppId` 획득
  5. 파일 복사 → `/apps/data/{app_id}/`:
     - `app.wasm` → `/apps/data/{id}/app.wasm`
     - `resources/*` → `/apps/data/{id}/resources/`
     - `manifest.toml` → `/apps/data/{id}/manifest.toml`
  6. 데스크톱 아이콘 등록 (아이콘 데이터 → AppRegistry)
- [ ] 업데이트 설치:
  - 기존 설치 감지 (동일 앱 이름)
  - 버전 비교 (SemVer)
  - 앱 데이터 보존, WASM/리소스만 교체
- [ ] 설치 취소/롤백: 실패 시 생성된 파일/디렉토리 정리

#### F-3. WASM 앱 런처 (`runtime/src/app_launcher.rs` 신규)
- [ ] `launch_wasm_app(app_id: KernelAppId) → Result<AppInstanceId>`:
  1. `/apps/data/{id}/manifest.toml` 로드
  2. `/apps/data/{id}/app.wasm` 로드
  3. `Module::from_bytes()` → 모듈 파싱
  4. `WasiCtx` 생성:
     - args: `[manifest.name]`
     - env: `[KPIO_APP_ID={id}]`
     - preopened: `/apps/data/{id}/data/` (앱 전용 데이터 디렉토리)
  5. `Store` 생성 + WASI/kpio 호스트 함수 등록
  6. `Instance::instantiate(module, imports)`
  7. CLI 앱: `instance.call("_start", [])` 실행
  8. GUI 앱: `instance.call("_start", [])` (이벤트 루프는 앱 내부)
  9. `AppLifecycle::launch()` → `Running` 상태
- [ ] 종료 처리:
  - `proc_exit()` 또는 `_start` 반환 → `Terminated`
  - Trap → `Failed` → 재시작 정책 적용

#### F-4. 패키지 제거
- [ ] `uninstall_kpioapp(app_id: KernelAppId) → Result<()>`:
  1. 실행 중이면 `AppTerminate` 시스콜
  2. `/apps/data/{id}/` 전체 삭제
  3. `AppUninstall` 시스콜 → 레지스트리 제거
  4. 데스크톱 아이콘 제거

### 산출물
- `runtime/src/package.rs` 신규
- `runtime/src/package_installer.rs` 신규
- `runtime/src/app_launcher.rs` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| F-QG1 | 패키지 파싱 | `hello.kpioapp` → manifest + app.wasm 추출 성공 | 유닛 테스트 |
| F-QG2 | 설치 | 설치 → `/apps/data/{id}/app.wasm` 존재 + 레지스트리 등록 | 통합 테스트 |
| F-QG3 | 실행 | 설치된 WASM 앱 → `launch()` → stdout 출력 확인 | 통합 테스트 |
| F-QG4 | GUI 앱 | GUI WASM 앱 설치 → 아이콘 클릭 → 윈도우 출현 | QEMU 시각 검증 |
| F-QG5 | 제거 | 제거 → `/apps/data/{id}/` 삭제 + 아이콘 소멸 | 통합 테스트 |
| F-QG6 | 잘못된 패키지 | manifest 없는 ZIP → 명확한 에러 메시지 | 유닛 테스트 |
| F-QG7 | 크기 제한 | 51MB 패키지 → TooLarge 에러 | 유닛 테스트 |
| F-QG8 | 업데이트 | 동일 앱 재설치 → 데이터 보존, WASM 교체 | 통합 테스트 |

---

## Sub-Phase 7-2.G: Component Model 기초 (Component Model Foundation)

### 목적

WASM Component Model의 **기초 인프라**를 구축하여, 모듈 간 인터페이스 정의(WIT)와 타입 변환을 지원한다. 전체 Component Model은 범위가 매우 크므로, 이 단계에서는 **단일 컴포넌트 인스턴스화 + WIT 인터페이스 정의**에 집중한다.

### 선행 조건
- 7-2.B 퀄리티 게이트 전체 통과
- 7-2.F 퀄리티 게이트 F-QG3 통과

### 작업

#### G-1. WIT 파서 (`runtime/src/wit/parser.rs` 신규)
- [ ] WIT 파일 토크나이저:
  - 키워드: `package`, `world`, `interface`, `import`, `export`, `use`, `func`, `type`, `record`, `enum`, `variant`, `flags`, `resource`
  - 기본 타입: `bool`, `u8`, `u16`, `u32`, `u64`, `s8`, `s16`, `s32`, `s64`, `f32`, `f64`, `char`, `string`
  - 복합 타입: `list<T>`, `option<T>`, `result<T, E>`, `tuple<T...>`
- [ ] WIT AST:
  ```rust
  pub struct WitDocument {
      pub package: Package,
      pub interfaces: Vec<Interface>,
      pub worlds: Vec<World>,
  }
  pub struct Interface {
      pub name: String,
      pub functions: Vec<Function>,
      pub types: Vec<TypeDef>,
  }
  pub struct World {
      pub name: String,
      pub imports: Vec<WorldItem>,
      pub exports: Vec<WorldItem>,
  }
  ```
- [ ] 기본 WIT 파일 파싱 테스트

#### G-2. kpio:gui WIT 정의 (`runtime/wit/kpio-gui.wit` 신규)
- [ ] `kpio:gui` world 정의:
  ```wit
  package kpio:gui@0.1.0;

  interface window {
      create-window: func(title: string, width: u32, height: u32) -> u32;
      close-window: func(id: u32);
      set-title: func(id: u32, title: string);
  }

  interface canvas {
      draw-rect: func(id: u32, x: s32, y: s32, w: u32, h: u32, color: u32);
      draw-text: func(id: u32, x: s32, y: s32, text: string, size: u32, color: u32);
      draw-line: func(id: u32, x1: s32, y1: s32, x2: s32, y2: s32, color: u32);
      clear: func(id: u32, color: u32);
      request-frame: func(id: u32) -> bool;
  }

  interface events {
      enum event-type {
          none, key-down, key-up, mouse-move, mouse-down, mouse-up, close, resize
      }
      record event {
          kind: event-type,
          key-code: u32,
          mouse-x: s32,
          mouse-y: s32,
          width: u32,
          height: u32,
      }
      poll-event: func(id: u32) -> event;
  }

  world gui-app {
      import window;
      import canvas;
      import events;
      export run: func();
  }
  ```

#### G-3. kpio:system WIT 정의 (`runtime/wit/kpio-system.wit` 신규)
- [ ] `kpio:system` world 정의:
  ```wit
  package kpio:system@0.1.0;

  interface clock {
      get-time: func() -> u64;
      get-monotonic: func() -> u64;
  }

  interface notification {
      notify: func(title: string, body: string);
  }

  interface clipboard {
      read: func() -> option<string>;
      write: func(text: string);
  }

  interface info {
      get-hostname: func() -> string;
      get-locale: func() -> string;
  }

  world system-app {
      import clock;
      import notification;
      import clipboard;
      import info;
  }
  ```

#### G-4. 인터페이스 타입 변환 (Canonical ABI 기초)
- [ ] `string` → 선형 메모리 (ptr, len) 쌍 변환
- [ ] `list<T>` → 연속 메모리 (ptr, len) 변환
- [ ] `record` → 필드별 순차 인코딩
- [ ] `enum` → u32 태그
- [ ] `option<T>` → (discriminant: u32, value?)
- [ ] `result<T, E>` → (discriminant: u32, ok_or_err?)

### 산출물
- `runtime/src/wit/mod.rs` 신규
- `runtime/src/wit/parser.rs` 신규
- `runtime/src/wit/types.rs` 신규
- `runtime/wit/kpio-gui.wit` 신규
- `runtime/wit/kpio-system.wit` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| G-QG1 | WIT 파싱 | `kpio-gui.wit` 파싱 → 3개 interface, 10개 함수 추출 | 유닛 테스트 |
| G-QG2 | 타입 파싱 | `string`, `u32`, `option<string>`, `list<u8>` 등 기본 타입 | 유닛 테스트 |
| G-QG3 | World 파싱 | `gui-app` world → imports 3개, exports 1개 | 유닛 테스트 |
| G-QG4 | string 변환 | "Hello" → 선형 메모리 (ptr=X, len=5) → "Hello" 복원 | 유닛 테스트 |
| G-QG5 | record 변환 | `event { kind, key_code, ... }` → 바이트 시퀀스 → 복원 | 유닛 테스트 |
| G-QG6 | 빌드 | `cargo build -p runtime` 성공 | CI |

---

## Sub-Phase 7-2.H: 종합 검증 & 데모 앱 (E2E Validation & Demo Apps)

### 목적

Phase 7-2 전체 WASM 런타임 파이프라인을 **엔드투엔드로 검증**하고, **.kpioapp 데모 앱 3개**를 제작하여 완성도를 증명한다.

### 선행 조건
- 7-2.A ~ 7-2.G 전체 퀄리티 게이트 통과

### 작업

#### H-1. 데모 앱 #1: hello-world.kpioapp (CLI)
- [ ] Rust로 작성, `wasm32-wasi` 타겟:
  - `fn main() { println!("Hello from KPIO!"); }`
  - 명령행 인자 출력
  - 환경 변수 출력
- [ ] `manifest.toml`:
  ```toml
  [app]
  name = "Hello World"
  version = "1.0.0"
  entry = "app.wasm"
  [permissions]
  ```
- [ ] 검증: 설치 → 실행 → stdout에 "Hello from KPIO!" → 종료코드 0

#### H-2. 데모 앱 #2: calculator.kpioapp (GUI)
- [ ] Rust + `kpio:gui` API:
  - 계산기 UI 렌더링 (숫자 버튼 0-9, +, -, ×, ÷, =, C)
  - 이벤트 루프: 마우스 클릭 → 버튼 감지 → 계산 → 결과 표시
  - 윈도우 크기: 300×400
- [ ] `manifest.toml`:
  ```toml
  [app]
  name = "Calculator"
  version = "1.0.0"
  entry = "app.wasm"
  icon = "resources/icon.png"
  [permissions]
  ```
- [ ] 검증: 설치 → 데스크톱 아이콘 → 클릭 → 계산기 윈도우 → 1+2=3

#### H-3. 데모 앱 #3: text-viewer.kpioapp (WASI 파일 I/O)
- [ ] Rust + WASI 파일 API:
  - `.txt` 파일 읽기 → 내용 표시 (GUI 윈도우)
  - 줄 번호 표시
  - PageUp/PageDown 스크롤
- [ ] `manifest.toml`:
  ```toml
  [app]
  name = "Text Viewer"
  version = "1.0.0"
  entry = "app.wasm"
  icon = "resources/icon.png"
  [permissions]
  filesystem = "read-only"
  ```
- [ ] 검증: 설치 → `/apps/data/{id}/data/sample.txt` 생성 → 실행 → 파일 내용 표시

#### H-4. E2E 테스트 스위트 (`tests/e2e/wasm/`)
- [ ] `test_wasm_install_uninstall.rs`:
  - `.kpioapp` 설치 → 레지스트리 확인 → 아이콘 → 제거 → 정리
- [ ] `test_wasm_cli_execution.rs`:
  - CLI WASM 앱 실행 → stdout 캡처 → 기대 출력 확인
- [ ] `test_wasm_gui_window.rs`:
  - GUI WASM 앱 → 윈도우 생성 → draw → 이벤트 → 닫기
- [ ] `test_wasm_file_io.rs`:
  - WASI 파일 읽기/쓰기 → VFS 확인 → 샌드박스 외부 접근 차단
- [ ] `test_wasm_jit_tiering.rs`:
  - 함수 100회 호출 → JIT 발동 → 성능 향상 확인
- [ ] `test_wasm_crash_restart.rs`:
  - WASM 앱 trap → Failed → 자동 재시작 (≤3회) → 4회차 실패 고정

#### H-5. 성능 벤치마크
- [ ] WASM 파싱 시간: **목표 < 100ms** (500KB .wasm 파일)
- [ ] 인터프리터 피보나치(30): **기준선 측정** (ms)
- [ ] JIT 피보나치(30): **목표 ≤ 기준선의 20%** (5x 향상)
- [ ] `.kpioapp` 설치 시간: **목표 < 3초** (1MB 패키지)
- [ ] GUI 앱 콜드 스타트: **목표 < 2초** (파싱 + 인스턴스화 + 윈도우 생성)
- [ ] `kpio:gui` draw_rect 지연: **목표 < 1ms** (단일 호출)

#### H-6. 개발자 문서
- [ ] `docs/phase7/WASM_APP_DEVELOPER_GUIDE.md`:
  - KPIO에서 WASM 앱 개발하기
  - Rust → `.kpioapp` 빌드 가이드
  - C/C++ → WASM → `.kpioapp` 가이드
  - `kpio:gui` API 레퍼런스
  - `kpio:system` API 레퍼런스
  - 권한 모델 설명
  - 디버깅/로깅 방법
- [ ] `docs/phase7/WASM_RUNTIME_ARCHITECTURE.md`:
  - 내부 아키텍처 (파서 → 인터프리터/JIT → WASI/Host)
  - 메모리 모델
  - 보안 모델
  - 성능 특성

### 산출물
- `examples/wasm-hello-world/` — CLI 데모 앱
- `examples/wasm-calculator/` — GUI 데모 앱
- `examples/wasm-text-viewer/` — WASI 파일 I/O 데모 앱
- `tests/e2e/wasm/` — 6개 E2E 테스트
- `docs/phase7/WASM_APP_DEVELOPER_GUIDE.md`
- `docs/phase7/WASM_RUNTIME_ARCHITECTURE.md`

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| H-QG1 | Hello World | 설치 → 실행 → "Hello from KPIO!" 출력 → 종료코드 0 | QEMU E2E |
| H-QG2 | Calculator | 설치 → 아이콘 → 계산기 UI → 1+2=3 정확 | QEMU E2E |
| H-QG3 | Text Viewer | 설치 → 파일 읽기 → 내용 표시 → 스크롤 | QEMU E2E |
| H-QG4 | E2E 테스트 | 6개 테스트 전부 통과 (0 failures) | `cargo test --test e2e` |
| H-QG5 | 파싱 성능 | 500KB WASM < 100ms | 벤치마크 |
| H-QG6 | JIT 성능 | JIT ≥ 5x 인터프리터 (피보나치 벤치) | 벤치마크 |
| H-QG7 | 콜드 스타트 | GUI 앱 실행 < 2초 | 벤치마크 |
| H-QG8 | 개발자 문서 | 가이드 기반으로 신규 WASM 앱 작성 가능 | 문서 리뷰 |
| H-QG9 | 0 panic | QEMU 30분 연속 → 커널 패닉 없음 | 안정성 테스트 |

---

## Phase 7-2 전체 완료 기준 (Exit Criteria)

### 필수 (Must Pass)
1. ✅ 서브페이즈 A~H의 모든 퀄리티 게이트 통과 (**62개 항목**)
2. ✅ 데모 앱 3개 (Hello World CLI, Calculator GUI, Text Viewer WASI) 전체 동작
3. ✅ `.kpioapp` 패키지 설치 → 실행 → 제거 전체 라이프사이클
4. ✅ `cargo build` 전체 빌드 성공
5. ✅ `cargo test` (호스트) 전체 통과
6. ✅ QEMU 30분 연속 사용 시 커널 패닉 없음

### 바람직 (Should Pass)
7. 🔶 JIT 컴파일러 5x 성능 향상 달성
8. 🔶 WIT 파서로 `kpio:gui` / `kpio:system` 인터페이스 파싱 성공
9. 🔶 개발자 가이드 문서 완성
10. 🔶 RELEASE_NOTES.md에 Phase 7-2 변경 사항 기록

### 선택 (Nice to Have)
11. ⬜ C/C++ (wasi-sdk) 크로스 컴파일 WASM 앱 `.kpioapp` 실행 성공
12. ⬜ Component Model로 다중 모듈 합성 (2개 .wasm → 1개 실행 단위)
13. ⬜ AOT 컴파일 캐시 (디스크 영속화)

---

## 아키텍처 다이어그램: WASM 앱 실행 흐름

```
.kpioapp (ZIP)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│  runtime/src/package.rs                                       │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────────┐     │
│  │ Unpack   │→│ manifest.toml │→│ Validate & Install  │     │
│  │  (ZIP)   │  │  Parse        │  │ → /apps/data/{id}/ │     │
│  └──────────┘  └──────────────┘  └─────────┬──────────┘     │
└────────────────────────────────────────────┼─────────────────┘
                                              │ Launch
┌─────────────────────────────────────────────┼─────────────────┐
│  runtime/src/                                ▼                 │
│  ┌──────────┐     ┌──────────────┐     ┌──────────────┐      │
│  │ parser.rs│────→│ Module       │────→│ Instance     │      │
│  │ (WASM   │     │ (types,      │     │ (memories,   │      │
│  │  binary) │     │  code,       │     │  tables,     │      │
│  └──────────┘     │  imports,    │     │  globals)    │      │
│                   │  exports)    │     └──────┬───────┘      │
│                   └──────────────┘            │               │
│                                               ▼               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │              Execution Engine                         │    │
│  │  ┌──────────────┐   ┌───────────┐   ┌────────────┐  │    │
│  │  │ Interpreter  │   │ Baseline  │   │ Profile    │  │    │
│  │  │ (콜드 함수)   │→→│ JIT       │←──│ Counter    │  │    │
│  │  │              │   │ (웜 함수)  │   │ (호출 횟수) │  │    │
│  │  └──────┬───────┘   └─────┬─────┘   └────────────┘  │    │
│  └─────────┼─────────────────┼──────────────────────────┘    │
│            │ import call      │ import call                    │
│  ┌─────────┴─────────────────┴──────────────────────────┐    │
│  │              Host Functions                            │    │
│  │  ┌──────────────┐ ┌──────────┐ ┌─────────────────┐   │    │
│  │  │ wasi_snapshot │ │ kpio:gui │ │ kpio:system     │   │    │
│  │  │ _preview1     │ │ (window, │ │ (clock, notify, │   │    │
│  │  │ (fd_write,   │ │  canvas, │ │  clipboard,     │   │    │
│  │  │  path_open,  │ │  events) │ │  info, log)     │   │    │
│  │  │  clock_get)  │ │          │ │                  │   │    │
│  │  └──────┬───────┘ └────┬─────┘ └────────┬────────┘   │    │
│  └─────────┼──────────────┼────────────────┼─────────────┘    │
└────────────┼──────────────┼────────────────┼──────────────────┘
             │              │                │
             ▼              ▼                ▼
┌────────────────────────────────────────────────────────────────┐
│  kernel/                                                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │
│  │ VFS      │  │ GUI      │  │ Clock    │  │ Notification │  │
│  │ (파일I/O) │  │ (윈도우)  │  │ (시계)   │  │ Center       │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

---

## 신규/수정 파일 총목록

### 신규 파일 (16개)
| 파일 | 서브페이즈 |
|------|-----------|
| `runtime/src/parser.rs` | A |
| `runtime/src/opcodes.rs` | A |
| `runtime/src/interpreter.rs` | B |
| `runtime/src/executor.rs` | B |
| `runtime/src/host_gui.rs` | E |
| `runtime/src/host_system.rs` | E |
| `runtime/src/host_net.rs` | E |
| `runtime/src/package.rs` | F |
| `runtime/src/package_installer.rs` | F |
| `runtime/src/app_launcher.rs` | F |
| `runtime/src/wit/mod.rs` | G |
| `runtime/src/wit/parser.rs` | G |
| `runtime/src/wit/types.rs` | G |
| `runtime/wit/kpio-gui.wit` | G |
| `runtime/wit/kpio-system.wit` | G |
| `docs/phase7/WASM_APP_DEVELOPER_GUIDE.md` | H |

### 수정 파일 (8개)
| 파일 | 서브페이즈 | 변경 내용 |
|------|-----------|----------|
| `runtime/src/module.rs` | A | Module 구조체 재정의, from_bytes() 실 구현 |
| `runtime/src/engine.rs` | B | Engine 재구현 (load/instantiate/call) |
| `runtime/src/instance.rs` | B | Store, Instance, Table 재구현 |
| `runtime/src/wasi.rs` | C | VFS 연동 실 구현 |
| `runtime/src/host.rs` | C, E | WASI + kpio 호스트 함수 실제 등록 |
| `runtime/src/jit/ir.rs` | D | WasmToIr 변환기 완성 |
| `runtime/src/jit/codegen.rs` | D | x86-64 코드 생성 완성 |
| `kernel/src/gui/window.rs` | E | WindowContent::WasmApp variant 추가 |

---

## 기술 위험 및 완화

| 위험 | 영향 | 확률 | 완화 |
|------|------|------|------|
| JIT 코드 생성 버그 (보안 취약점) | 🔴 높음 | 중간 | W^X 강제, 인터프리터 폴백, 퍼징 테스트 |
| WASM 파서 에지 케이스 | 🟡 중간 | 높음 | WASM spec 테스트 스위트 활용, 점진적 커버리지 |
| WASI VFS 연동 복잡도 | 🟡 중간 | 중간 | path_open만 먼저 → 나머지 점진 추가 |
| float 연산 정밀도 | 🟡 중간 | 낮음 | IEEE 754 strictly 준수, f32/f64 testvector |
| Component Model 범위 폭발 | 🔴 높음 | 높음 | MVP: 단일 컴포넌트 + WIT 파싱만, 합성은 후순위 |
| no_std 환경에서 ZIP 해제 | 🟡 중간 | 중간 | miniz_oxide(no_std) 또는 직접 deflate 구현 |

---

*Phase 7-2 완료 시 KPIO OS는 Rust/C/C++/Go로 작성된 WASM 앱을 `.kpioapp` 패키지로 배포·설치·실행할 수 있는 범용 앱 플랫폼을 갖추게 된다. 인터프리터 + Baseline JIT 2-tier 실행, WASI 파일 I/O, 커스텀 GUI API를 통해, 웹 앱(Phase 7-1)과 함께 두 가지 주요 앱 유형을 지원하게 된다.*
