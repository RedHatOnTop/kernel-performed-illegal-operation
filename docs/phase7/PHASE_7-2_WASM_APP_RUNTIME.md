# Phase 7-2: Tier 2 — WASM/WASI App Runtime

> **Parent Phase:** Phase 7 — App Execution Layer  
> **Goal:** Establish WASM as KPIO's universal app binary format, enabling safe execution of apps written in any programming language (Rust/C/C++/Go/Python).  
> **Estimated Duration:** 6-8 weeks (8 sub-phases)  
> **Dependencies:** Phase 7-1 (App Runtime Foundation + Web App Platform)  
> **Priority:** 🔴 Required

---

## 현재 상태 분석 (As-Is)

Phase 7-1에서 구축된 앱 관리 인프라(레지스트리, 라이프사이클, 권한, VFS 샌드박스, 시스콜 106-111)를 기반으로, WASM 실행 엔진과 WASI 시스템 인터페이스를 **실동작 수준**으로 완성한다.

> **정합성 노트(2026-02-15):** 현재 코드베이스에는 `parser/module/instance/executor/interpreter/wasi/host/engine`가 이미 존재하며,
> 본 문서의 체크리스트(서브페이즈 A~H)는 “향후 확장/완성도 향상”을 위한 계획 항목으로 유지됩니다.

| Component | Location | Status | Notes |
|---------|------|------|------|
| **Linear Memory** | `runtime/src/memory.rs` | ✅ Implemented | grow, read/write, bounds check |
| **Sandbox/Security** | `runtime/src/sandbox.rs` | ✅ Implemented | Memory/CPU/FD/file/network limits |
| **JIT IR** | `runtime/src/jit/ir.rs` | ✅ Implemented | Comprehensive IR opcode definitions |
| **JIT Framework** | `runtime/src/jit/mod.rs` | ✅ Implemented | Tiered/JIT 프레임워크 + 5종 최적화 패스(inline, unroll, const-prop, DCE, CSE) |
| **JIT Codegen** | `runtime/src/jit/codegen.rs` | ✅ Implemented | x86-64 emit 프레임워크 + PersistentCache 구현 완료 |
| **JIT Code Cache** | `runtime/src/jit/cache.rs` | ✅ Implemented | LRU cache |
| **JIT Profiling** | `runtime/src/jit/profile.rs` | ✅ Implemented | Call counts/branch statistics |
| **WASI (Preview 1)** | `runtime/src/wasi.rs` | ✅ Implemented | Includes in-memory VFS, preopen directory-based sandbox |
| **Host Functions** | `runtime/src/host.rs` | ✅ Implemented | `wasi_snapshot_preview1.*` + `kpio`(IPC/process/capability) + `kpio_gpu` + `kpio_gui`/`kpio_system`/`kpio_net` 모두 구현 완료 |
| **Engine** | `runtime/src/engine.rs` | ✅ Basic impl | Provides load/instantiate/execute (minimal functionality) |
| **Module Parsing** | `runtime/src/parser.rs`, `runtime/src/module.rs` | ✅ Implemented | Section parsing + structure validation (`validate_structure`) |
| **Instance** | `runtime/src/instance.rs` | ✅ Basic impl | Import binding + `call_typed()` execution. `call()` is legacy encoding API |
| **Component Model** | `runtime/src/wit/` | ✅ Implemented | WIT 파서 + AST 타입 + 인터페이스 정의(gui/system/net) 구현 완료 |
| **`.kpioapp` Package** | `runtime/src/package.rs`, `app_launcher.rs` | ✅ Implemented | ZIP 기반 패키지 포맷 + 매니페스트 해석 + 앱 라이프사이클 관리 구현 완료 |
| **WASM App Examples** | — | ❌ None | No separate `.wasm`/`.kpioapp` example directory (at this time) |
| **Kernel App Management** | `kernel/src/app/` | ✅ Implemented | Built in Phase 7-1 |
| **App Syscalls 106-111** | `kernel/src/syscall/mod.rs` | ✅ Implemented | Built in Phase 7-1 |
| **VFS Sandbox** | `kernel/src/vfs/sandbox.rs` | ✅ Implemented | Built in Phase 7-1 |

---

## Sub-Phase Overall Roadmap

```
Week    1         2         3         4         5         6         7         8
      ├─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤
 A    ██████████                                                                        WASM Module Parser
 B    ██████████████████████                                                            Interpreter Engine (wasmi integration)
 C              ██████████████████████                                                  WASI Preview 1 Complete Implementation
 D                        ██████████████████████                                        Baseline JIT Completion
 E                                  ██████████████████████                              kpio:gui / kpio:system Host
 F                                            ██████████████████████                    .kpioapp Package System
 G                                                      ██████████████████████          Component Model Foundation
 H                                                                ██████████████████████ E2E Validation & Demo Apps
```

> A+B can run in parallel, D+E partially parallelizable, F requires A-C completion first

---

## Sub-Phase 7-2.A: WASM Module Parser

### Purpose

Building on the existing `runtime/src/parser.rs` / `runtime/src/module.rs` implementation, extend parser coverage (sections/opcodes) and validation to structurally parse all sections of `.wasm` files.

### Prerequisites
- `runtime/src/memory.rs` LinearMemory operational (already completed)

### Tasks

#### A-1. WASM Binary Format Parser (`runtime/src/parser.rs` extension)
- [ ] Magic number + Version verification (`\0asm` + `0x01`)
- [ ] Section parser (12 sections):
  - [ ] Type Section (1): Function signatures (`FuncType(params, results)`)
  - [ ] Import Section (2): External function/memory/table/global imports
  - [ ] Function Section (3): Function index → type index mapping
  - [ ] Table Section (4): `funcref`/`externref` table definitions
  - [ ] Memory Section (5): Linear memory definitions (`min`, `max`)
  - [ ] Global Section (6): Global variables (type, mutability, initial value)
  - [ ] Export Section (7): Exports (functions, memory, tables, globals)
  - [ ] Start Section (8): Start function index
  - [ ] Element Section (9): Table initialization data
  - [ ] Code Section (10): Function bodies (locals + expression)
  - [ ] Data Section (11): Memory initialization data
  - [ ] Custom Section (0): Name section (`name`), debug info
- [ ] LEB128 variable-length integer decoder (u32, i32, u64, i64)
- [ ] ValueType parser (`i32`, `i64`, `f32`, `f64`, `funcref`, `externref`)
- [ ] Expression (InitExpr) parser: `i32.const`, `i64.const`, `f32/f64.const`, `global.get`, `ref.null`, `ref.func`

#### A-2. Module Struct Refactoring (`runtime/src/module.rs`)
- [ ] Complete `Module` struct redefinition:
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
- [ ] `Module::from_bytes(bytes: &[u8]) → Result<Module, ParseError>` implementation
- [ ] `Module::validate() → Result<(), ValidationError>` — basic validity checks:
  - Type index range checking
  - Function signature match checking
  - Memory/table count limit (1 each, MVP)
  - Import/Export name duplication checking

#### A-3. WASM Instruction (Opcode) Definitions (`runtime/src/opcodes.rs` extension)
- [ ] `Opcode` enum — Full WASM MVP instruction definitions (~200):
  - Control flow: `unreachable`, `nop`, `block`, `loop`, `if`, `else`, `end`, `br`, `br_if`, `br_table`, `return`, `call`, `call_indirect`
  - References: `ref.null`, `ref.is_null`, `ref.func`
  - Parametric: `drop`, `select`, `select_typed`
  - Variables: `local.get/set/tee`, `global.get/set`
  - Memory: `i32.load`, `i64.load`, `f32.load`, `f64.load`, various `load8/16/32_s/u`, `store`, `memory.size`, `memory.grow`
  - Constants: `i32.const`, `i64.const`, `f32.const`, `f64.const`
  - Comparison: `i32.eqz/eq/ne/lt_s/lt_u/gt_s/gt_u/le_s/le_u/ge_s/ge_u`, `i64` counterparts, `f32/f64` counterparts
  - Arithmetic: `i32.add/sub/mul/div_s/div_u/rem_s/rem_u/and/or/xor/shl/shr_s/shr_u/rotl/rotr`, `i64` counterparts, `f32/f64` counterparts
  - Conversion: `i32.wrap_i64`, `i32.trunc_f32_s/u`, `i64.extend_i32_s/u`, `f32.convert_i32_s/u`, etc.
  - Others: `i32.clz/ctz/popcnt`, `f32.abs/neg/ceil/floor/trunc/nearest/sqrt`, etc.
- [ ] Opcode ↔ `u8` byte conversion (`From<u8>`, `Into<u8>`)
- [ ] Instruction decoder: byte stream → `Instruction` sequence

### 산출물
- `runtime/src/parser.rs` 보강
- `runtime/src/module.rs` 보강
- `runtime/src/opcodes.rs` 보강

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

#### B-1. Stack Machine (`runtime/src/interpreter.rs` enhancement)
- [ ] `ValueStack`: `WasmValue` (i32/i64/f32/f64/funcref/externref) stack
  - `push(value)`, `pop() → WasmValue`
  - `peek()`, `len()`, `is_empty()`
  - Stack maximum depth limit (64KB, configurable)
- [ ] `CallStack`: Function call frame stack
  - `CallFrame { func_idx, locals, return_arity, pc, block_stack }`
  - Maximum depth limit (1024 frames)
- [ ] `BlockStack`: Structured control flow (block/loop/if nesting)
  - `Block { kind: Block|Loop|If, arity, continuation_pc }`
- [ ] `ProgramCounter`: Current execution position (`func_idx`, `instr_offset`)

#### B-2. Instruction Executor (`runtime/src/executor.rs` enhancement)
- [ ] `execute(module, func_idx, args) → Result<Vec<WasmValue>, TrapError>`
- [ ] Control flow instruction implementation:
  - [ ] `block/loop/if/else/end` — block stack management
  - [ ] `br/br_if/br_table` — branching (label index → block escape)
  - [ ] `call` — direct call (push args, create frame)
  - [ ] `call_indirect` — table-based indirect call (signature verification)
  - [ ] `return` — return from current function
- [ ] Variable instructions:
  - [ ] `local.get/set/tee` — local variable access
  - [ ] `global.get/set` — global variable access
- [ ] Memory instructions:
  - [ ] `i32/i64/f32/f64.load` + variants (8/16/32 signed/unsigned)
  - [ ] `i32/i64/f32/f64.store` + variants
  - [ ] `memory.size` → current page count
  - [ ] `memory.grow` → LinearMemory.grow()
  - Bounds check: out-of-bounds memory access → `TrapError::MemoryOutOfBounds`
- [ ] Integer arithmetic (i32/i64):
  - [ ] `add/sub/mul` — wrapping arithmetic
  - [ ] `div_s/div_u/rem_s/rem_u` — division by zero → `TrapError::DivisionByZero`
  - [ ] `and/or/xor/shl/shr_s/shr_u/rotl/rotr`
  - [ ] `clz/ctz/popcnt`
  - [ ] `eqz/eq/ne/lt_s/lt_u/gt_s/gt_u/le_s/le_u/ge_s/ge_u` — comparison → 0/1
- [ ] Floating-point arithmetic (f32/f64):
  - [ ] `add/sub/mul/div` — IEEE 754
  - [ ] `abs/neg/ceil/floor/trunc/nearest/sqrt`
  - [ ] `min/max/copysign`
  - [ ] Comparison: `eq/ne/lt/gt/le/ge`
- [ ] Conversion instructions:
  - [ ] `i32.wrap_i64`, `i64.extend_i32_s/u`
  - [ ] `i32/i64.trunc_f32/f64_s/u` — NaN/Inf → Trap
  - [ ] `f32/f64.convert_i32/i64_s/u`
  - [ ] `i32/i64.reinterpret_f32/f64`, `f32/f64.reinterpret_i32/i64`
- [ ] Reference instructions:
  - [ ] `ref.null/ref.is_null/ref.func`
- [ ] Others:
  - [ ] `unreachable` → `TrapError::Unreachable`
  - [ ] `nop` — no operation
  - [ ] `drop` — remove top of stack
  - [ ] `select` — conditional selection

#### B-3. Instance and Store Refactoring (`runtime/src/instance.rs`)
- [ ] `Store` reimplementation:
  ```rust
  pub struct Store {
      pub memories: Vec<LinearMemory>,
      pub tables: Vec<Table>,
      pub globals: Vec<GlobalValue>,
      pub functions: Vec<FunctionInstance>,
      pub host_functions: BTreeMap<String, HostFunction>,
  }
  ```
- [ ] `Instance` reimplementation:
  - `instantiate(module, imports) → Result<Instance, InstantiationError>`
  - `call(export_name, args) → Result<Vec<WasmValue>, TrapError>`
  - Data segments → memory initialization
  - Element segments → table initialization
  - Start function auto-execution
- [ ] `Table` implementation: `funcref`/`externref` entries, `get/set/grow`
- [ ] `GlobalValue`: `{value: WasmValue, mutable: bool}`
- [ ] Import resolution: `(module, name)` → function/memory/global binding in Store

#### B-4. Engine Integration (`runtime/src/engine.rs`)
- [ ] `Engine` reimplementation:
  - `load_module(bytes) → Result<ModuleId, EngineError>`
  - `instantiate(module_id, imports) → Result<InstanceId, EngineError>`
  - `call(instance_id, func, args) → Result<Vec<WasmValue>, TrapError>`
  - `drop_instance(instance_id)`
- [ ] Module cache (parsed result caching)
- [ ] Instance pool management

### Deliverables
- `runtime/src/interpreter.rs` enhanced
- `runtime/src/executor.rs` enhanced
- `runtime/src/instance.rs` enhanced
- `runtime/src/engine.rs` enhanced

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| B-QG1 | Hello World | `(func (export "_start") (call $fd_write ...))` → "Hello" output to stdout | Unit test |
| B-QG2 | Arithmetic accuracy | i32/i64 basic operations (add/sub/mul/div) 100+ test cases pass | Unit test |
| B-QG3 | Floating-point | f32/f64 IEEE 754 compliant (NaN propagation, Inf handling) | Unit test |
| B-QG4 | Control flow | Fibonacci (recursive), factorial (iterative) correct results | Unit test |
| B-QG5 | Memory | memory.grow + load/store → correct read/write | Unit test |
| B-QG6 | Trap | division by zero, memory OOB, unreachable → TrapError | Unit test |
| B-QG7 | Indirect call | call_indirect + table → correct function dispatch | Unit test |
| B-QG8 | Rust WASM | `cargo build --target wasm32-wasi` hello world .wasm → execution success | Integration test |
| B-QG9 | Build | `cargo build -p runtime` success | CI |

---

## Sub-Phase 7-2.C: WASI Preview 1 완전 구현 (WASI Complete)

### 목적

기존 `wasi.rs`의 WASI Preview 1 구현을 **커널 VFS/시계/난수 소스와 연동**하는 방향으로 확장하여, WASI 앱이 파일 I/O, 시계, 난수, 프로세스 종료를 수행할 수 있게 한다.

### 선행 조건
- 7-2.B 퀄리티 게이트 B-QG1 통과 (인터프리터로 WASM 실행 가능)

### 작업

#### C-1. WASI ↔ VFS 연결 (`runtime/src/wasi.rs` 확장)
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

#### C-2. Path-Based File Operations
- [ ] `path_open(dirfd, flags, path, oflags, rights, inheriting, fdflags) → Result<u32>`:
  - Path resolution (relative path based on dirfd)
  - Path validation within VFS sandbox
  - `O_CREAT/O_EXCL/O_TRUNC/O_DIRECTORY` flag handling
  - Allocate and return new FD
- [ ] `path_create_directory(dirfd, path) → Result<()>`:
  - VFS `mkdir()` call
- [ ] `path_remove_directory(dirfd, path) → Result<()>`:
  - VFS `rmdir()` call
- [ ] `path_unlink_file(dirfd, path) → Result<()>`:
  - VFS `unlink()` call
- [ ] `path_rename(old_dirfd, old_path, new_dirfd, new_path) → Result<()>`:
  - VFS `rename()` call
- [ ] `path_filestat_get(dirfd, flags, path) → Result<FileStat>`:
  - VFS `stat()` call → FileStat struct conversion
- [ ] `fd_readdir(fd, buf, cookie) → Result<usize>`:
  - VFS `readdir()` call → dirent serialization

#### C-3. Clock and Random
- [ ] `clock_time_get(id, precision) → Result<u64>`:
  - `REALTIME` → kernel system clock (nanoseconds)
  - `MONOTONIC` → kernel monotonic counter (nanoseconds)
  - `PROCESS_CPUTIME` → process CPU time (approximate)
- [ ] `random_get(buf, len) → Result<()>`:
  - Kernel CSPRNG (`kernel::random`) integration
  - Or RDRAND instruction-based random

#### C-4. Process and Environment
- [ ] `args_get(argv, argv_buf) → Result<()>`:
  - `WasiCtx.args` → write to linear memory
- [ ] `args_sizes_get() → Result<(usize, usize)>`:
  - (argument count, total bytes)
- [ ] `environ_get(environ, environ_buf) → Result<()>`:
  - `WasiCtx.env_vars` → write to linear memory
- [ ] `environ_sizes_get() → Result<(usize, usize)>`:
  - (environment variable count, total bytes)
- [ ] `proc_exit(code)`:
  - Halt interpreter execution
  - Return `ExitCode(code)`
  - App lifecycle → transition to `Terminated` state

#### C-5. Host Function Registration (`runtime/src/host.rs` extension)
- [ ] `register_wasi_functions(store, wasi_ctx)`:
  - Register entire `wasi_snapshot_preview1.*` namespace
  - Each host function → closure calls `WasiCtx` methods
- [ ] Host function call protocol:
  - Interpreter → discovers import function → host function dispatch
  - Arguments: linear memory pointer/length → Rust slice conversion
  - Return: WASI errno (success=0)

### Deliverables
- `runtime/src/wasi.rs` rewritten
- `runtime/src/host.rs` rewritten

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| C-QG1 | stdout output | WASM `fd_write(1, ...)` → "Hello, WASI!" on kernel console | Integration test |
| C-QG2 | File read | File in preopened dir `path_open` → `fd_read` → correct content | Unit test |
| C-QG3 | File write | `path_open(O_CREAT)` → `fd_write` → file exists in VFS | Unit test |
| C-QG4 | Directory | `path_create_directory` → `fd_readdir` → entry exists | Unit test |
| C-QG5 | Clock | `clock_time_get(MONOTONIC)` → non-zero nanosecond value | Unit test |
| C-QG6 | Random | `random_get(buf, 32)` → 32 bytes non-zero (probabilistic) | Unit test |
| C-QG7 | Argument passing | `WasiCtx.args = ["app", "--flag"]` → `args_get/sizes_get` correct return | Unit test |
| C-QG8 | proc_exit | `proc_exit(42)` call → `ExitCode(42)` returned, execution halted | Unit test |
| C-QG9 | Sandbox | Access to path outside preopened dir → `EACCES` error | Unit test |
| C-QG10 | Rust WASI app | `wasm32-wasi` Rust app (file read/write) → normal termination | Integration test |

---

## Sub-Phase 7-2.D: Baseline JIT Completion

### Purpose

Complete the partial implementation of the existing JIT framework (`runtime/src/jit/`) to achieve **5-10x performance improvement over the interpreter**. Automatically JIT-compile "warm" functions (100+ invocations).

### Prerequisites
- 7-2.B Quality Gates all passed (interpreter correctness guaranteed)

### Tasks

#### D-1. WASM → IR Translator Completion (`runtime/src/jit/ir.rs` extension)
- [ ] `WasmToIr::translate_function(code_body) → Result<IrFunction>`:
  - WASM instructions → IR opcode 1:1 mapping
  - Block structure → IR labels/branches conversion
  - Stack-based → SSA-like IR (virtual registers) conversion
- [ ] All WASM MVP instructions → IR translation coverage:
  - Integer operations (i32/i64): use existing IR opcodes
  - Floating-point (f32/f64): use existing IR opcodes
  - Memory access: bounds check IR injection
  - Control flow: `block/loop/if` → IR `Label` + `Branch`
  - Function calls: `call` → IR `Call`, `call_indirect` → IR `IndirectCall`
- [ ] BasicBlock splitting: generate block boundaries at branch targets

#### D-2. IR → x86-64 Code Generation Completion (`runtime/src/jit/codegen.rs` extension)
- [ ] Register allocator:
  - Simple linear scan allocator (Baseline)
  - Available registers: RAX, RCX, RDX, R8-R11 (caller-saved)
  - Spill/reload: allocate space on stack frame
- [ ] Code generation rules:
  - [ ] `IrAdd/Sub/Mul` → x86 `add/sub/imul`
  - [ ] `IrDiv/Rem` → x86 `div/idiv` (RAX:RDX convention)
  - [ ] `IrAnd/Or/Xor/Shl/Shr` → x86 corresponding instructions
  - [ ] `IrLoad/Store` → x86 `mov [base + offset]` + bounds check
  - [ ] `IrBranch/BranchIf` → x86 `jmp/jcc`
  - [ ] `IrCall` → x86 `call` (System V ABI: RDI, RSI, RDX, RCX, R8, R9)
  - [ ] `IrReturn` → x86 `ret`
  - [ ] `IrConst` → x86 `mov reg, imm`
  - [ ] f32/f64 → XMM registers (XMM0-XMM7) + SSE/SSE2 instructions
- [ ] Prologue/Epilogue:
  - `push rbp; mov rbp, rsp; sub rsp, frame_size`
  - `add rsp, frame_size; pop rbp; ret`
- [ ] Memory bounds check:
  - `cmp offset, memory_size; ja trap_handler`
  - Trap handler: halt execution, return `TrapError`

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

## Sub-Phase 7-2.E: kpio:gui / kpio:system Host API

### Purpose

Implement **custom host functions** that allow WASM apps to access KPIO-specific features (GUI windows, system information, IPC).

> `runtime/src/host.rs`의 `wasi_snapshot_preview1.*`, `kpio`(IPC/process/capability), `kpio_gpu`(surface/buffer/commands/present) 네임스페이스가 모두 구현 완료됨. 추가로 `host_gui.rs`, `host_system.rs`, `host_net.rs`로 kpio:gui/system/net API도 별도 모듈로 구현됨.

### Prerequisites
- 7-2.C Quality Gate C-QG1 passed (WASI host function call working)

### Tasks

#### E-1. kpio:gui API (`runtime/src/host_gui.rs` new)
- [ ] `create_window(title_ptr, title_len, width, height) → window_id`:
  - Read title string from linear memory
  - `AppLifecycle::launch()` integration → create WASM-dedicated window
  - Add `WindowContent::WasmApp` variant
- [ ] `set_window_title(window_id, title_ptr, title_len)`:
  - Change window title
- [ ] `draw_rect(window_id, x, y, w, h, color)`:
  - ARGB color → direct framebuffer rendering
  - Kernel GUI `draw_filled_rect()` call
- [ ] `draw_text(window_id, x, y, text_ptr, text_len, size, color)`:
  - Read UTF-8 string from linear memory
  - Kernel GUI `draw_string()` call
- [ ] `draw_line(window_id, x1, y1, x2, y2, color)`:
  - Bresenham algorithm-based line rendering
- [ ] `draw_image(window_id, x, y, w, h, data_ptr, data_len)`:
  - RGBA bitmap data → framebuffer copy
- [ ] `clear_window(window_id, color)`:
  - Fill entire window area with solid color
- [ ] `request_frame(window_id) → bool`:
  - VSync / frame callback registration
  - Return: whether next frame is ready
- [ ] `poll_event(window_id, event_buf_ptr, buf_len) → event_type`:
  - Dequeue next event from event queue
  - Event types: `None(0)`, `KeyDown(1)`, `KeyUp(2)`, `MouseMove(3)`, `MouseDown(4)`, `MouseUp(5)`, `Close(6)`, `Resize(7)`
  - Write event data to linear memory buffer
- [ ] `close_window(window_id)`:
  - Destroy window + clean up event queue

#### E-2. kpio:system API (`runtime/src/host_system.rs` new)
- [ ] `get_time() → u64`:
  - Kernel system clock → millisecond timestamp
- [ ] `get_monotonic() → u64`:
  - Monotonic counter → nanoseconds
- [ ] `get_hostname(buf_ptr, buf_len) → usize`:
  - Return hostname ("kpio")
- [ ] `notify(title_ptr, title_len, body_ptr, body_len)`:
  - `NotificationCenter::show()` integration
  - Notification app_id → current WASM app ID
- [ ] `clipboard_read(buf_ptr, buf_len) → usize`:
  - System clipboard → copy to linear memory
  - Permission check: requires `clipboard: true`
- [ ] `clipboard_write(data_ptr, data_len)`:
  - Linear memory → write to system clipboard
- [ ] `get_locale(buf_ptr, buf_len) → usize`:
  - Return current system locale code (e.g., "ko-KR")
- [ ] `log(level, msg_ptr, msg_len)`:
  - Debug log output (level: 0=debug, 1=info, 2=warn, 3=error)

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
  ├── manifest.toml        # App metadata
  ├── app.wasm              # Main WASM module
  ├── resources/            # Assets
  │   ├── icon-192.png
  │   └── icon-512.png
  └── data/                 # Initial data (optional)
  ```
- [ ] `AppManifest` struct (TOML parsing):
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
- [ ] Package validation:
  - ZIP structure validity
  - `manifest.toml` existence + parsing
  - `entry` specified WASM file existence
  - WASM magic number verification
  - Total size limit (default 50MB)

#### F-2. Package Installation (`runtime/src/package_installer.rs` new)
- [ ] `install_kpioapp(path: &str) → Result<KernelAppId>`:
  1. ZIP extraction → temporary directory
  2. `manifest.toml` parsing → `AppManifest`
  3. Permission review → user approval dialog (display permission list)
  4. `AppInstall` syscall → obtain `KernelAppId`
  5. File copy → `/apps/data/{app_id}/`:
     - `app.wasm` → `/apps/data/{id}/app.wasm`
     - `resources/*` → `/apps/data/{id}/resources/`
     - `manifest.toml` → `/apps/data/{id}/manifest.toml`
  6. Desktop icon registration (icon data → AppRegistry)
- [ ] Update installation:
  - Detect existing installation (same app name)
  - Version comparison (SemVer)
  - Preserve app data, replace only WASM/resources
- [ ] Installation cancel/rollback: clean up created files/directories on failure

#### F-3. WASM App Launcher (`runtime/src/app_launcher.rs` new)
- [ ] `launch_wasm_app(app_id: KernelAppId) → Result<AppInstanceId>`:
  1. Load `/apps/data/{id}/manifest.toml`
  2. Load `/apps/data/{id}/app.wasm`
  3. `Module::from_bytes()` → module parsing
  4. Create `WasiCtx`:
     - args: `[manifest.name]`
     - env: `[KPIO_APP_ID={id}]`
     - preopened: `/apps/data/{id}/data/` (app-specific data directory)
  5. Create `Store` + register WASI/kpio host functions
  6. `Instance::instantiate(module, imports)`
  7. CLI app: execute `instance.call("_start", [])`
  8. GUI app: `instance.call("_start", [])` (event loop inside the app)
  9. `AppLifecycle::launch()` → `Running` state
- [ ] Termination handling:
  - `proc_exit()` or `_start` return → `Terminated`
  - Trap → `Failed` → apply restart policy

#### F-4. 패키지 제거
- [ ] `uninstall_kpioapp(app_id: KernelAppId) → Result<()>`:
  1. 실행 중이면 `AppTerminate` 시스콜
  2. `/apps/data/{id}/` 전체 삭제
  3. `AppUninstall` syscall → remove from registry
  4. Remove desktop icon

### Deliverables
- `runtime/src/package.rs` new
- `runtime/src/package_installer.rs` new
- `runtime/src/app_launcher.rs` new

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| F-QG1 | Package parsing | `hello.kpioapp` → manifest + app.wasm extraction success | Unit test |
| F-QG2 | Installation | Install → `/apps/data/{id}/app.wasm` exists + registry registered | Integration test |
| F-QG3 | Execution | Installed WASM app → `launch()` → stdout output confirmed | Integration test |
| F-QG4 | GUI app | GUI WASM app installed → icon click → window appears | QEMU visual verification |
| F-QG5 | Removal | Remove → `/apps/data/{id}/` deleted + icon disappeared | Integration test |
| F-QG6 | Invalid package | ZIP without manifest → clear error message | Unit test |
| F-QG7 | Size limit | 51MB package → TooLarge error | Unit test |
| F-QG8 | Update | Same app reinstall → data preserved, WASM replaced | Integration test |

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
- [ ] Basic WIT file parsing test

#### G-2. kpio:gui WIT Definition (`runtime/wit/kpio-gui.wit` new)
- [ ] `kpio:gui` world definition:
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

#### G-3. kpio:system WIT Definition (`runtime/wit/kpio-system.wit` new)
- [ ] `kpio:system` world definition:
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
- `runtime/src/wit/types.rs` new
- `runtime/wit/kpio-gui.wit` new
- `runtime/wit/kpio-system.wit` new

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| G-QG1 | WIT parsing | `kpio-gui.wit` parsing → 3 interfaces, 10 functions extracted | Unit test |
| G-QG2 | Type parsing | `string`, `u32`, `option<string>`, `list<u8>` etc. basic types | Unit test |
| G-QG3 | World parsing | `gui-app` world → 3 imports, 1 export | Unit test |
| G-QG4 | string conversion | "Hello" → linear memory (ptr=X, len=5) → "Hello" restored | Unit test |
| G-QG5 | record conversion | `event { kind, key_code, ... }` → byte sequence → restored | Unit test |
| G-QG6 | Build | `cargo build -p runtime` success | CI |

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
- [ ] Verification: Install → Execute → "Hello from KPIO!" on stdout → exit code 0

#### H-2. Demo App #2: calculator.kpioapp (GUI)
- [ ] Rust + `kpio:gui` API:
  - Calculator UI rendering (number buttons 0-9, +, -, ×, ÷, =, C)
  - Event loop: mouse click → button detection → calculation → result display
  - Window size: 300×400
- [ ] `manifest.toml`:
  ```toml
  [app]
  name = "Calculator"
  version = "1.0.0"
  entry = "app.wasm"
  icon = "resources/icon.png"
  [permissions]
  ```
- [ ] Verification: Install → desktop icon → click → calculator window → 1+2=3

#### H-3. Demo App #3: text-viewer.kpioapp (WASI File I/O)
- [ ] Rust + WASI File API:
  - Read `.txt` file → display content (GUI window)
  - Display line numbers
  - PageUp/PageDown scrolling
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

#### H-5. Performance Benchmarks
- [ ] WASM parsing time: **target < 100ms** (500KB .wasm file)
- [ ] Interpreter Fibonacci(30): **baseline measurement** (ms)
- [ ] JIT Fibonacci(30): **target ≤ 20% of baseline** (5x improvement)
- [ ] `.kpioapp` installation time: **target < 3 seconds** (1MB package)
- [ ] GUI app cold start: **target < 2 seconds** (parsing + instantiation + window creation)
- [ ] `kpio:gui` draw_rect latency: **target < 1ms** (single call)

#### H-6. Developer Documentation
- [ ] `docs/phase7/WASM_APP_DEVELOPER_GUIDE.md`:
  - Developing WASM apps on KPIO
  - Rust → `.kpioapp` build guide
  - C/C++ → WASM → `.kpioapp` guide
  - `kpio:gui` API reference
  - `kpio:system` API reference
  - Permission model explanation
  - Debugging/logging methods
- [ ] `docs/phase7/WASM_RUNTIME_ARCHITECTURE.md`:
  - Internal architecture (parser → interpreter/JIT → WASI/Host)
  - Memory model
  - Security model
  - Performance characteristics

### Deliverables
- `examples/wasm-hello-world/` — CLI demo app
- `examples/wasm-calculator/` — GUI demo app
- `examples/wasm-text-viewer/` — WASI file I/O demo app
- `tests/e2e/wasm/` — 6 E2E tests
- `docs/phase7/WASM_APP_DEVELOPER_GUIDE.md`
- `docs/phase7/WASM_RUNTIME_ARCHITECTURE.md`

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| H-QG1 | Hello World | Install → Execute → "Hello from KPIO!" output → exit code 0 | QEMU E2E |
| H-QG2 | Calculator | Install → icon → calculator UI → 1+2=3 correct | QEMU E2E |
| H-QG3 | Text Viewer | Install → file read → content displayed → scroll | QEMU E2E |
| H-QG4 | E2E tests | All 6 tests pass (0 failures) | `cargo test --test e2e` |
| H-QG5 | Parsing performance | 500KB WASM < 100ms | Benchmark |
| H-QG6 | JIT performance | JIT ≥ 5x interpreter (Fibonacci bench) | Benchmark |
| H-QG7 | Cold start | GUI app launch < 2 seconds | Benchmark |
| H-QG8 | Developer docs | New WASM app can be written based on the guide | Document review |
| H-QG9 | 0 panic | QEMU 30 minutes continuous → no kernel panic | Stability test |

---

## Phase 7-2 Overall Exit Criteria

### Required (Must Pass)
1. ✅ All Quality Gates across Sub-Phases A~H passed (**62 items**)
2. ✅ All 3 demo apps (Hello World CLI, Calculator GUI, Text Viewer WASI) fully operational
3. ✅ `.kpioapp` package install → execute → remove full lifecycle
4. ✅ `cargo build` full build success
5. ✅ `cargo test` (host) all pass
6. ✅ No kernel panic during 30 minutes continuous QEMU usage

### Desirable (Should Pass)
7. 🔶 JIT compiler 5x performance improvement achieved
8. 🔶 WIT parser successfully parses `kpio:gui` / `kpio:system` interfaces
9. 🔶 Developer guide documentation complete
10. 🔶 Phase 7-2 changes recorded in RELEASE_NOTES.md

### Optional (Nice to Have)
11. ⬜ C/C++ (wasi-sdk) cross-compiled WASM app `.kpioapp` execution success
12. ⬜ Multi-module composition via Component Model (2 .wasm → 1 execution unit)
13. ⬜ AOT compilation cache (disk persistence)

---

## Architecture Diagram: WASM App Execution Flow

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
│  │ (file I/O)│  │ (window) │  │ (clock)  │  │ Center       │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

---

## 파일 현황 (현재 코드베이스 기준)

이 섹션은 “계획 산출물”이 아니라 **현재 레포지토리에 실제로 존재하는 파일**을 기준으로 정리합니다.

### 이미 존재/구현됨 (Phase 7-2 런타임 핵심)

| 파일 | 상태 | 비고 |
|------|------|------|
| `runtime/src/parser.rs` | ✅ 존재 | WASM 섹션 파싱 + instruction 디코딩 |
| `runtime/src/opcodes.rs` | ✅ 존재 | `Instruction` 정의 |
| `runtime/src/module.rs` | ✅ 존재 | `Module::from_bytes()` + 구조 검증 |
| `runtime/src/instance.rs` | ✅ 존재 | import 해석 + `call_typed()` 실행 |
| `runtime/src/interpreter.rs` | ✅ 존재 | 스택/프레임/트랩 타입 |
| `runtime/src/executor.rs` | ✅ 존재 | 인터프리터 실행기 (`execute_export` 등) |
| `runtime/src/engine.rs` | ✅ 존재 | 최소 엔진 (load/instantiate/execute) |
| `runtime/src/wasi.rs` | ✅ 존재 | WASI Preview 1 + in-memory VFS + 샌드박스 |
| `runtime/src/host.rs` | ✅ 구현 | WASI + kpio IPC/process/capability/GPU 모두 구현 완료 (~1,600줄) |
| `runtime/src/host_gui.rs` | ✅ 구현 | kpio:gui 윈도우/캔버스/이벤트 API (~526줄) |
| `runtime/src/host_system.rs` | ✅ 구현 | kpio:system 시계/클립보드/로깅 API (~298줄) |
| `runtime/src/host_net.rs` | ✅ 구현 | kpio:net 소켓 TCP/UDP API (~460줄) |
| `runtime/src/package.rs` | ✅ 구현 | .kpioapp ZIP 패키지 파싱/매니페스트 해석 (~542줄) |
| `runtime/src/app_launcher.rs` | ✅ 구현 | WASM 앱 라이프사이클 관리 (~248줄) |
| `runtime/src/wit/` | ✅ 구현 | WIT 파서 + AST 타입 + 인터페이스 정의 (3 .wit) |
| `runtime/src/service_worker/` | ✅ 구현 | SW 라이프사이클, 캐시, Fetch, Sync (7 모듈, ~2,717줄) |
| `runtime/src/memory.rs` | ✅ 구현 | LinearMemory + fill/copy_within |
| `runtime/src/sandbox.rs` | ✅ 구현 | 자원/권한 제한 |
| `runtime/src/jit/` | ✅ 구현 | IR + 프로파일링 + 최적화 패스 5종 + PersistentCache (~4,500줄) |

### 문서 내 “향후 구현 예정” 항목 (현재는 미존재/미구현)

| 항목 | 상태 |
|------|------|
| `runtime/src/host_gui.rs`, `host_system.rs`, `host_net.rs` | ✅ 구현 |
| `.kpioapp` 패키지: `package.rs`, `app_launcher.rs` | ✅ 구현 |
| Component Model/WIT: `wit/mod.rs`, `wit/types.rs`, `wit/parser.rs`, `*.wit` | ✅ 구현 |
| 문서: `WASM_APP_DEVELOPER_GUIDE.md`, `WASM_RUNTIME_ARCHITECTURE.md` | ❌ 미작성 |

> Sub-Phase E~G 구현 완료. JIT codegen(D)과 E2E 데모(H)만 잔여.

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
