# Phase 7-2: Tier 2 — WASM/WASI App Runtime

> **Parent Phase:** Phase 7 — App Execution Layer  
> **Goal:** Establish WASM as KPIO's universal app binary format, enabling safe execution of apps written in any programming language (Rust/C/C++/Go/Python).  
> **Estimated Duration:** 6-8 weeks (8 sub-phases)  
> **Dependencies:** Phase 7-1 (App Runtime Foundation + Web App Platform)  
> **Priority:** 🔴 Required

---

## Current State Analysis (As-Is)

Building on the app management infrastructure established in Phase 7-1 (registry, lifecycle, permissions, VFS sandbox, syscalls 106-111), complete the WASM execution engine and WASI system interface to a **production-ready level**.

> **Consistency Note (2026-02-15):** The current codebase already contains `parser/module/instance/executor/interpreter/wasi/host/engine`,
> and the checklists in this document (Sub-Phases A~H) are maintained as planning items for "future expansion/completeness improvements."

| Component | Location | Status | Notes |
|---------|------|------|------|
| **Linear Memory** | `runtime/src/memory.rs` | ✅ Implemented | grow, read/write, bounds check |
| **Sandbox/Security** | `runtime/src/sandbox.rs` | ✅ Implemented | Memory/CPU/FD/file/network limits |
| **JIT IR** | `runtime/src/jit/ir.rs` | ✅ Implemented | Comprehensive IR opcode definitions |
| **JIT Framework** | `runtime/src/jit/mod.rs` | ✅ Implemented | Tiered/JIT framework + 5 optimization passes (inline, unroll, const-prop, DCE, CSE) |
| **JIT Codegen** | `runtime/src/jit/codegen.rs` | ✅ Implemented | x86-64 emit framework + PersistentCache fully implemented |
| **JIT Code Cache** | `runtime/src/jit/cache.rs` | ✅ Implemented | LRU cache |
| **JIT Profiling** | `runtime/src/jit/profile.rs` | ✅ Implemented | Call counts/branch statistics |
| **WASI (Preview 1)** | `runtime/src/wasi.rs` | ✅ Implemented | Includes in-memory VFS, preopen directory-based sandbox |
| **Host Functions** | `runtime/src/host.rs` | ✅ Implemented | `wasi_snapshot_preview1.*` + `kpio`(IPC/process/capability) + `kpio_gpu` + `kpio_gui`/`kpio_system`/`kpio_net` fully implemented |
| **Engine** | `runtime/src/engine.rs` | ✅ Basic impl | Provides load/instantiate/execute (minimal functionality) |
| **Module Parsing** | `runtime/src/parser.rs`, `runtime/src/module.rs` | ✅ Implemented | Section parsing + structure validation (`validate_structure`) |
| **Instance** | `runtime/src/instance.rs` | ✅ Basic impl | Import binding + `call_typed()` execution. `call()` is legacy encoding API |
| **Component Model** | `runtime/src/wit/` | ✅ Implemented | WIT parser + AST types + interface definitions (gui/system/net) fully implemented |
| **`.kpioapp` Package** | `runtime/src/package.rs`, `app_launcher.rs` | ✅ Implemented | ZIP-based package format + manifest parsing + app lifecycle management |
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

### Deliverables
- `runtime/src/parser.rs` enhanced
- `runtime/src/module.rs` enhanced
- `runtime/src/opcodes.rs` enhanced

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| A-QG1 | Minimal WASM parsing | Rust `wasm32-wasi` hello world → `Module::from_bytes()` success | Unit test |
| A-QG2 | Export extraction | After parsing, `_start` function export exists | Unit test |
| A-QG3 | Import extraction | WASI imports (`wasi_snapshot_preview1.fd_write` etc.) extracted | Unit test |
| A-QG4 | Memory definition | Memory Section → `LinearMemory` creation parameters extracted | Unit test |
| A-QG5 | Code section | Function body → Opcode sequence decoding success | Unit test |
| A-QG6 | Validation | Invalid type index → `ValidationError` returned | Unit test |
| A-QG7 | Build | `cargo build -p runtime` success | CI |

---

## Sub-Phase 7-2.B: Interpreter Engine

### Purpose

Execute parsed WASM modules using a **stack-based interpreter**, enabling correct (if slow) execution of all WASM apps without JIT. This also serves as the "cold" tier for the JIT.

### Prerequisites
- 7-2.A Quality Gates A-QG1~QG7 all passed

### Tasks

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

## Sub-Phase 7-2.C: WASI Preview 1 Complete Implementation (WASI Complete)

### Purpose

Extend the existing WASI Preview 1 implementation in `wasi.rs` to **integrate with the kernel VFS/clock/random sources**, enabling WASI apps to perform file I/O, clock access, random number generation, and process termination.

### Prerequisites
- 7-2.B Quality Gate B-QG1 passed (WASM execution possible via interpreter)

### Tasks

#### C-1. WASI ↔ VFS Integration (`runtime/src/wasi.rs` extension)
- [ ] `WasiCtx` → VFS integration:
  - `preopened_dirs: Vec<(u32, String)>` → VFS path mapping
  - Default FDs:
    - 0 = stdin (read → empty bytes)
    - 1 = stdout (write → kernel console / capture buffer)
    - 2 = stderr (write → kernel console)
    - 3+ = preopened dirs
- [ ] `fd_read(fd, iovs) → Result<usize>`:
  - FD validity + permission (`FD_READ`) check
  - VFS `read_all()` call → copy to iovs buffer
- [ ] `fd_write(fd, iovs) → Result<usize>`:
  - stdout/stderr → write to console output buffer
  - Regular file FD → VFS `write_all()` call
- [ ] `fd_seek(fd, offset, whence) → Result<u64>`:
  - `Set/Cur/End` whence handling
  - Calculation based on current offset + file size
- [ ] `fd_close(fd) → Result<()>`:
  - Remove from FD table
  - Release VFS resources
- [ ] `fd_fdstat_get(fd) → Result<FdStat>`:
  - Return file type, flags, permissions
- [ ] `fd_prestat_get(fd) → Result<Prestat>`:
  - Return preopened dir name length
- [ ] `fd_prestat_dir_name(fd, buf, len) → Result<()>`:
  - Copy preopened dir name to buffer

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

#### D-3. Executable Memory Management
- [ ] W^X (Write XOR Execute) enforcement:
  1. Allocate RW pages via `mmap` (or kernel dynamic memory)
  2. Code generation → write to RW buffer
  3. `mprotect` → RW → RX transition
  4. Execute code via function pointer
- [ ] Code region maximum size limit (default 16MB)

#### D-4. Tiered Compilation Integration
- [ ] Profiling counters:
  - Increment counter on function entry (injected in interpreter)
  - `count >= 100` → trigger Baseline JIT
  - `count >= 10,000` → reserved for future Optimizing JIT (post Phase 7)
- [ ] Asynchronous compilation:
  - Hot function detected → add to compilation queue
  - Compilation complete → register in code cache
  - Next invocation → use JIT code
- [ ] Fallback: on JIT compilation failure, continue execution via interpreter

### Deliverables
- `runtime/src/jit/ir.rs` extended
- `runtime/src/jit/codegen.rs` extended
- `runtime/src/jit/mod.rs` extended

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| D-QG1 | JIT correctness | Fibonacci(40) — interpreter and JIT results identical | Unit test |
| D-QG2 | Arithmetic correctness | i32/i64 all operations — JIT vs interpreter match (1000+ cases) | Unit test |
| D-QG3 | Memory access | load/store — bounds check works in JIT, OOB → Trap | Unit test |
| D-QG4 | Function calls | Recursive/indirect calls — JIT-to-JIT + JIT↔host transitions | Unit test |
| D-QG5 | W^X compliance | JIT code region — not executable while writable (or vice versa) | Security test |
| D-QG6 | Performance improvement | Fibonacci(30) benchmark — JIT ≥ 5x interpreter | Benchmark |
| D-QG7 | Tiered | Function called 100 times → automatic JIT compilation triggered | Unit test |
| D-QG8 | Fallback | JIT-unsupported function (unsupported opcode) → interpreter fallback | Unit test |
| D-QG9 | Code cache | JIT compilation result cached → no recompilation on re-invocation | Unit test |

---

## Sub-Phase 7-2.E: kpio:gui / kpio:system Host API

### Purpose

Implement **custom host functions** that allow WASM apps to access KPIO-specific features (GUI windows, system information, IPC).

> All host function namespaces in `runtime/src/host.rs` are now fully implemented: `wasi_snapshot_preview1.*`, `kpio` (IPC/process/capability), `kpio_gpu` (surface/buffer/commands/present). Additionally, `host_gui.rs`, `host_system.rs`, and `host_net.rs` provide separate kpio:gui/system/net API modules.

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

#### E-3. kpio:net API (`runtime/src/host_net.rs` new)
- [ ] `socket_create(domain, sock_type) → socket_id`:
  - TCP/UDP socket creation
  - Permission check: requires `network != None`
- [ ] `socket_connect(socket_id, addr_ptr, addr_len, port) → Result`:
  - TCP connect / UDP target setup
- [ ] `socket_send(socket_id, data_ptr, data_len) → bytes_sent`:
  - Data transmission
- [ ] `socket_recv(socket_id, buf_ptr, buf_len) → bytes_received`:
  - Data reception
- [ ] `socket_close(socket_id)`:
  - Socket release

#### E-4. Host Function Registration Integration
- [ ] `register_kpio_functions(store)`:
  - Register all `kpio:gui.*`, `kpio:system.*`, `kpio:net.*`
- [ ] Auto-mapping of `kpio` namespace during import resolution

### Deliverables
- `runtime/src/host_gui.rs` new
- `runtime/src/host_system.rs` new
- `runtime/src/host_net.rs` new
- `runtime/src/host.rs` modified
- `kernel/src/gui/window.rs` modified (`WindowContent::WasmApp`)

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| E-QG1 | Window creation | `create_window("Test", 400, 300)` → empty window appears in QEMU | QEMU visual verification |
| E-QG2 | Rectangle rendering | `draw_rect(id, 10, 10, 100, 100, 0xFFFF0000)` → red rectangle | QEMU visual verification |
| E-QG3 | Text rendering | `draw_text(id, 10, 10, "Hello", 16, 0xFFFFFFFF)` → text displayed | QEMU visual verification |
| E-QG4 | Event reception | Keyboard input → `poll_event` → `KeyDown` + keycode returned | Functional test |
| E-QG5 | Notification | `notify("Alert", "Test")` → toast notification displayed | QEMU visual verification |
| E-QG6 | Clock | `get_time()` called twice consecutively → second > first | Unit test |
| E-QG7 | Permission restriction | App with `clipboard: false` calls `clipboard_read` → error | Unit test |
| E-QG8 | Socket | `socket_create → connect → send → recv → close` chain succeeds | Integration test |

---

## Sub-Phase 7-2.F: .kpioapp Package System

### Purpose

Build the **packaging, installation, and execution** pipeline for WASM apps, enabling app distribution as a single `.kpioapp` file.

### Prerequisites
- 7-2.C Quality Gate C-QG10 passed (WASI app execution works)
- 7-2.E Quality Gate E-QG1 passed (GUI app works)

### Tasks

#### F-1. Package Format Definition (`runtime/src/package.rs` new)
- [ ] `.kpioapp` package structure (ZIP-based):
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

#### F-4. Package Removal
- [ ] `uninstall_kpioapp(app_id: KernelAppId) → Result<()>`:
  1. If running, issue `AppTerminate` syscall
  2. Delete entire `/apps/data/{id}/`
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

## Sub-Phase 7-2.G: Component Model Foundation

### Purpose

Build the **foundational infrastructure** of the WASM Component Model, supporting inter-module interface definitions (WIT) and type conversions. Since the full Component Model is extremely large in scope, this phase focuses on **single component instantiation + WIT interface definitions**.

### Prerequisites
- 7-2.B Quality Gates all passed
- 7-2.F Quality Gate F-QG3 passed

### Tasks

#### G-1. WIT Parser (`runtime/src/wit/parser.rs` new)
- [ ] WIT file tokenizer:
  - Keywords: `package`, `world`, `interface`, `import`, `export`, `use`, `func`, `type`, `record`, `enum`, `variant`, `flags`, `resource`
  - Primitive types: `bool`, `u8`, `u16`, `u32`, `u64`, `s8`, `s16`, `s32`, `s64`, `f32`, `f64`, `char`, `string`
  - Compound types: `list<T>`, `option<T>`, `result<T, E>`, `tuple<T...>`
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

#### G-4. Interface Type Conversion (Canonical ABI Foundation)
- [ ] `string` → linear memory (ptr, len) pair conversion
- [ ] `list<T>` → contiguous memory (ptr, len) conversion
- [ ] `record` → sequential per-field encoding
- [ ] `enum` → u32 tag
- [ ] `option<T>` → (discriminant: u32, value?)
- [ ] `result<T, E>` → (discriminant: u32, ok_or_err?)

### Deliverables
- `runtime/src/wit/mod.rs` new
- `runtime/src/wit/parser.rs` new
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

## Sub-Phase 7-2.H: E2E Validation & Demo Apps

### Purpose

Perform **end-to-end validation** of the entire Phase 7-2 WASM runtime pipeline, and build **3 .kpioapp demo apps** to demonstrate completeness.

### Prerequisites
- 7-2.A ~ 7-2.G all Quality Gates passed

### Tasks

#### H-1. Demo App #1: hello-world.kpioapp (CLI)
- [ ] Written in Rust, targeting `wasm32-wasi`:
  - `fn main() { println!("Hello from KPIO!"); }`
  - Print command-line arguments
  - Print environment variables
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
- [ ] Verification: Install → create `/apps/data/{id}/data/sample.txt` → execute → file content displayed

#### H-4. E2E Test Suite (`tests/e2e/wasm/`)
- [ ] `test_wasm_install_uninstall.rs`:
  - Install `.kpioapp` → verify registry → icon → remove → cleanup
- [ ] `test_wasm_cli_execution.rs`:
  - Execute CLI WASM app → capture stdout → verify expected output
- [ ] `test_wasm_gui_window.rs`:
  - GUI WASM app → window creation → draw → events → close
- [ ] `test_wasm_file_io.rs`:
  - WASI file read/write → verify VFS → block access outside sandbox
- [ ] `test_wasm_jit_tiering.rs`:
  - Call function 100 times → JIT triggered → verify performance improvement
- [ ] `test_wasm_crash_restart.rs`:
  - WASM app trap → Failed → auto-restart (≤3 times) → 4th failure stays failed

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
│  │  │ (cold funcs) │→→│ JIT       │←──│ Counter    │  │    │
│  │  │              │   │ (warm     │   │ (call      │  │    │
│  │  │              │   │  funcs)   │   │  count)    │  │    │
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
│  │ (file    │  │ (window) │  │ (clock)  │  │ Center       │  │
│  │  I/O)    │  │          │  │          │  │              │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

---

## File Status (Based on Current Codebase)

This section is organized based on **files that actually exist in the current repository**, not "planned deliverables."

### Already Existing/Implemented (Phase 7-2 Runtime Core)

| File | Status | Notes |
|------|------|------|
| `runtime/src/parser.rs` | ✅ Exists | WASM section parsing + instruction decoding |
| `runtime/src/opcodes.rs` | ✅ Exists | `Instruction` definitions |
| `runtime/src/module.rs` | ✅ Exists | `Module::from_bytes()` + structure validation |
| `runtime/src/instance.rs` | ✅ Exists | Import resolution + `call_typed()` execution |
| `runtime/src/interpreter.rs` | ✅ Exists | Stack/frame/trap types |
| `runtime/src/executor.rs` | ✅ Exists | Interpreter executor (`execute_export` etc.) |
| `runtime/src/engine.rs` | ✅ Exists | Minimal engine (load/instantiate/execute) |
| `runtime/src/wasi.rs` | ✅ Exists | WASI Preview 1 + in-memory VFS + sandbox |
| `runtime/src/host.rs` | ✅ Implemented | WASI + kpio IPC/process/capability/GPU fully implemented (~1,600 lines) |
| `runtime/src/host_gui.rs` | ✅ Implemented | kpio:gui window/canvas/event API (~526 lines) |
| `runtime/src/host_system.rs` | ✅ Implemented | kpio:system clock/clipboard/logging API (~298 lines) |
| `runtime/src/host_net.rs` | ✅ Implemented | kpio:net socket TCP/UDP API (~460 lines) |
| `runtime/src/package.rs` | ✅ Implemented | .kpioapp ZIP package parsing/manifest (~542 lines) |
| `runtime/src/app_launcher.rs` | ✅ Implemented | WASM app lifecycle management (~248 lines) |
| `runtime/src/wit/` | ✅ Implemented | WIT parser + AST types + interface definitions (3 .wit files) |
| `runtime/src/service_worker/` | ✅ Implemented | SW lifecycle, cache, fetch, sync (7 modules, ~2,717 lines) |
| `runtime/src/memory.rs` | ✅ Implemented | LinearMemory + fill/copy_within |
| `runtime/src/sandbox.rs` | ✅ Implemented | Resource/permission limits |
| `runtime/src/jit/` | ✅ Implemented | IR + profiling + 5 optimization passes + PersistentCache (~4,500 lines) |

### Planned for Future Implementation (Currently Non-existent/Unimplemented)

| Item | Status |
|------|------|
| `runtime/src/host_gui.rs`, `runtime/src/host_system.rs`, `runtime/src/host_net.rs` | ✅ Implemented |
| `.kpioapp` package: `runtime/src/package.rs`, `app_launcher.rs` | ✅ Implemented |
| Component Model/WIT: `runtime/src/wit/mod.rs`, `wit/types.rs`, `wit/parser.rs`, `*.wit` | ✅ Implemented |
| Service Worker: `runtime/src/service_worker/` (7 modules) | ✅ Implemented |
| Documentation: `docs/phase7/WASM_APP_DEVELOPER_GUIDE.md`, `docs/phase7/WASM_RUNTIME_ARCHITECTURE.md` | ❌ Not yet created |

> The above unimplemented items are covered in Sub-Phases E~H of this document and this section will be updated as they are implemented.

---

## Technical Risks and Mitigation

| Risk | Impact | Probability | Mitigation |
|------|------|------|------|
| JIT code generation bugs (security vulnerabilities) | 🔴 High | Medium | W^X enforcement, interpreter fallback, fuzz testing |
| WASM parser edge cases | 🟡 Medium | High | Leverage WASM spec test suite, incremental coverage |
| WASI VFS integration complexity | 🟡 Medium | Medium | Implement path_open first → add rest incrementally |
| Float operation precision | 🟡 Medium | Low | Strictly comply with IEEE 754, f32/f64 test vectors |
| Component Model scope explosion | 🔴 High | High | MVP: single component + WIT parsing only, composition deferred |
| ZIP extraction in no_std environment | 🟡 Medium | Medium | miniz_oxide (no_std) or custom deflate implementation |

---

*Upon completion of Phase 7-2, KPIO OS will have a universal app platform capable of distributing, installing, and executing WASM apps written in Rust/C/C++/Go as `.kpioapp` packages. With interpreter + Baseline JIT 2-tier execution, WASI file I/O, and custom GUI APIs, it will support two major app types alongside web apps (Phase 7-1).*
