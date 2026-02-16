# Phase 7-3: WASM/WASI App Runtime — Master Plan

> **Goal:** Establish WASM as KPIO's universal app binary format, enabling apps written in any language to run safely with full system access via WASI Preview 2 and Component Model.  
> **Dependencies:** Phase 7-2 (WASM interpreter + JIT framework + packaging foundation)  
> **Estimated Duration:** 6-8 weeks  
> **Principle:** No sub-phase transition until current sub-phase quality gate is 100% satisfied.

---

## Baseline Assessment (from Phase 7-2)

| Area | Existing Code | Completeness | Key Gap |
|------|---------------|-------------|---------|
| WASI P1 | `wasi.rs` ~1,801 lines, 25 tests | ~25% of P2 | No streams/resources/sockets/http |
| WIT Parser | `wit/` ~1,131 lines, 8 tests | ~20% of CM | No canonical ABI, no linker, no binary parser |
| JIT Compiler | `jit/` ~4,831 lines, 41 tests | ~55% | Missing FP codegen, full i64, benchmarks |
| App Packaging | `package.rs` + `app_launcher.rs` ~805 lines, 9 tests | ~75% | Missing registry, updates, sample apps |
| Cross-Compile | 0 lines | 0% | All docs + POSIX shim + SDL2 adapter |

---

## Sub-Phase Overview

| Sub-Phase | Title | Effort | Key Deliverable |
|-----------|-------|--------|-----------------|
| **S1** | WASI P2 Foundation — Streams & Resources | M | `wasi:io` streams + resource handle model |
| **S2** | WASI P2 Core Interfaces | M | `wasi:filesystem` P2 + `wasi:clocks` + `wasi:random` + `wasi:cli` |
| **S3** | WASI Network Interfaces | M | `wasi:sockets` (TCP/UDP) + `wasi:http` |
| **S4** | JIT Compiler Completion | M | Full i64 + FP codegen + tests + benchmarks |
| **S5** | Component Model Core | L | Canonical ABI + component linker + type conversion |
| **S6** | App Packaging & Registry | S | App registry + updates + 3 sample `.kpioapp` apps |
| **S7** | Cross-Compile & Developer Docs | S | Build guides (Rust/C/C++/Go) + POSIX shim |

---

## Sub-Phase S1: WASI P2 Foundation — Streams & Resources

### Goal

Implement the `wasi:io` stream abstraction and resource handle model that all other WASI P2 interfaces depend on.

### Tasks

#### S1-T1. Resource Handle Table
- [ ] Create `runtime/src/wasi2/mod.rs` — WASI P2 module root
- [ ] Implement `ResourceTable<T>` — generic handle-based resource container
  - `push(resource) → handle: u32`
  - `get(handle) → &T`
  - `get_mut(handle) → &mut T`
  - `delete(handle) → T`
  - Handle generation with free list (O(1) alloc/dealloc)
- [ ] Implement `ResourceType` trait for type-safe handle casting

#### S1-T2. wasi:io Input/Output Streams
- [ ] Create `runtime/src/wasi2/streams.rs`
- [ ] Implement `InputStream` trait:
  - `read(len) → Result<Vec<u8>, StreamError>`
  - `blocking_read(len) → Result<Vec<u8>, StreamError>`
  - `skip(len) → Result<u64, StreamError>`
  - `subscribe() → Pollable`
- [ ] Implement `OutputStream` trait:
  - `check_write() → Result<u64, StreamError>`
  - `write(bytes) → Result<(), StreamError>`
  - `blocking_write_and_flush(bytes) → Result<(), StreamError>`
  - `flush() → Result<(), StreamError>`
  - `subscribe() → Pollable`
- [ ] Implement concrete stream types:
  - `MemoryInputStream` — reads from a byte buffer
  - `MemoryOutputStream` — writes to a growable buffer
  - `StdinStream`, `StdoutStream`, `StderrStream` — terminal I/O

#### S1-T3. wasi:io Poll
- [ ] Create `runtime/src/wasi2/poll.rs`
- [ ] Implement `Pollable` resource:
  - `ready() → bool`
  - `block() → ()` (no-op in single-threaded kernel)
- [ ] Implement `poll(list<pollable>) → list<u32>` — index of ready pollables
- [ ] Initial implementation: all pollables immediately ready (non-blocking)

#### S1-T4. Host Function Registration
- [ ] Create `runtime/src/wasi2/host.rs` — WASI P2 host function registry
- [ ] Register `wasi:io/streams` import functions in host dispatcher
- [ ] Register `wasi:io/poll` import functions
- [ ] Integration test: simple InputStream read + OutputStream write round-trip

### Quality Gate S1-QG

| ID | Criterion | Verification |
|----|-----------|-------------|
| S1-QG1 | `ResourceTable` push/get/delete works with 3+ resource types | Unit tests (≥6 tests) |
| S1-QG2 | `InputStream::read()` and `OutputStream::write()` round-trip produces correct data | Unit test |
| S1-QG3 | `poll([pollable])` returns correct ready indices | Unit test |
| S1-QG4 | Host functions for `wasi:io/streams` registered and callable | Integration test |
| S1-QG5 | `cargo build -p kpio-runtime --target x86_64-unknown-none` passes with 0 errors | Build check |
| S1-QG6 | ≥15 total new tests in `wasi2/` module | Test count |

---

## Sub-Phase S2: WASI P2 Core Interfaces

### Goal

Implement the four core WASI P2 interfaces that cover filesystem, clocks, randomness, and CLI.

### Prerequisites
- S1 Quality Gate passed (streams + resource table available)

### Tasks

#### S2-T1. wasi:filesystem — Descriptor-based API
- [ ] Create `runtime/src/wasi2/filesystem.rs`
- [ ] Map existing `Vfs` to P2 `Descriptor` resource:
  - `open_at(descriptor, path, flags) → Result<descriptor, error-code>`
  - `read_via_stream(descriptor, offset) → Result<input-stream>`
  - `write_via_stream(descriptor, offset) → Result<output-stream>`
  - `stat(descriptor) → Result<descriptor-stat>`
  - `readdir(descriptor) → Result<directory-entry-stream>`
  - `metadata_hash(descriptor) → Result<metadata-hash-value>`
- [ ] Reuse existing `WasiCtx::fd_*` logic where possible (wrap in P2 resource model)
- [ ] Implement `preopens` — expose pre-opened directories as resource handles

#### S2-T2. wasi:clocks
- [ ] Create `runtime/src/wasi2/clocks.rs`
- [ ] `monotonic-clock`:
  - `now() → instant` (wraps existing `clock_time_get(Monotonic)`)
  - `resolution() → duration`
  - `subscribe_instant(instant) → pollable`
  - `subscribe_duration(duration) → pollable`
- [ ] `wall-clock`:
  - `now() → datetime { seconds, nanoseconds }`
  - `resolution() → datetime`

#### S2-T3. wasi:random
- [ ] Create `runtime/src/wasi2/random.rs`
- [ ] `random`:
  - `get_random_bytes(len) → list<u8>` (reuse existing xorshift64)
  - `get_random_u64() → u64`
- [ ] `insecure`:
  - `get_insecure_random_bytes(len) → list<u8>`
  - `get_insecure_random_u64() → u64`
- [ ] `insecure-seed`:
  - `insecure_seed() → (u64, u64)`

#### S2-T4. wasi:cli
- [ ] Create `runtime/src/wasi2/cli.rs`
- [ ] `stdin / stdout / stderr` — return stream handles
- [ ] `environment` — `get_environment() → list<(string, string)>`
- [ ] `terminal-input / terminal-output` — stub (no real terminal in kernel)
- [ ] `exit` — `exit(status: result)` wraps existing `proc_exit`

#### S2-T5. Host Function Registration
- [ ] Register all S2 interfaces in host dispatcher (`wasi:filesystem/*`, `wasi:clocks/*`, `wasi:random/*`, `wasi:cli/*`)
- [ ] Integration test: open file via P2 API → read via stream → verify content

### Quality Gate S2-QG

| ID | Criterion | Verification |
|----|-----------|-------------|
| S2-QG1 | `open_at` → `read_via_stream` → data matches written file | Integration test |
| S2-QG2 | `monotonic-clock::now()` returns monotonically increasing values | Unit test |
| S2-QG3 | `get_random_bytes(N)` returns N bytes, not all zeros | Unit test |
| S2-QG4 | `cli::environment::get_environment()` returns configured env vars | Unit test |
| S2-QG5 | All P2 host functions registered without name conflicts | Build + dispatch test |
| S2-QG6 | `cargo build` passes, ≥20 new tests in S2 modules | Build + test count |

---

## Sub-Phase S3: WASI Network Interfaces

### Goal

Implement TCP/UDP socket and HTTP request APIs for WASM apps.

### Prerequisites
- S1 Quality Gate passed (streams required for socket I/O)

### Tasks

#### S3-T1. wasi:sockets — TCP
- [ ] Create `runtime/src/wasi2/sockets.rs`
- [ ] Implement `tcp-socket` resource:
  - `start_bind(network, address) → Result<()>`
  - `finish_bind() → Result<()>`
  - `start_connect(network, address) → Result<()>`
  - `finish_connect() → Result<(input-stream, output-stream)>`
  - `start_listen() → Result<()>`
  - `finish_listen() → Result<()>`
  - `accept() → Result<(tcp-socket, input-stream, output-stream)>`
  - `shutdown(shutdown-type) → Result<()>`
- [ ] `ip-name-lookup` resource:
  - `resolve_addresses(network, name) → Result<resolve-address-stream>`
- [ ] In-memory socket simulation (loopback) for kernel environment

#### S3-T2. wasi:sockets — UDP
- [ ] Implement `udp-socket` resource:
  - `start_bind(network, address) → Result<()>`
  - `finish_bind() → Result<()>`
  - `stream(remote_address?) → Result<(incoming-datagram-stream, outgoing-datagram-stream)>`
- [ ] `incoming-datagram-stream` / `outgoing-datagram-stream` resources

#### S3-T3. wasi:http — Outgoing Requests
- [ ] Create `runtime/src/wasi2/http.rs`
- [ ] Implement `outgoing-handler`:
  - `handle(request, options?) → Result<future-incoming-response>`
- [ ] `outgoing-request` / `outgoing-body` / `incoming-response` / `incoming-body` resources
- [ ] `fields` (HTTP headers) resource with get/set/append/delete
- [ ] `method`, `scheme`, `status-code` types
- [ ] Stub implementation: returns mock responses (real network integration deferred)

#### S3-T4. Network Resource Registration
- [ ] Register `wasi:sockets/*` and `wasi:http/*` in host dispatcher
- [ ] Integration test: TCP connect → write → read loopback

### Quality Gate S3-QG

| ID | Criterion | Verification |
|----|-----------|-------------|
| S3-QG1 | TCP: bind → listen → accept → send/recv loopback works | Integration test |
| S3-QG2 | UDP: bind → send → recv datagram loopback works | Integration test |
| S3-QG3 | HTTP: `outgoing-handler::handle()` returns a response with status/headers/body | Unit test |
| S3-QG4 | IP name lookup returns at least localhost (127.0.0.1) for "localhost" | Unit test |
| S3-QG5 | `cargo build` passes, ≥15 new tests in S3 modules | Build + test count |

---

## Sub-Phase S4: JIT Compiler Completion

### Goal

Complete the JIT code generator to handle all WASM integer, float, and control flow operations; add tests and benchmarks.

### Prerequisites
- None (independent of S1-S3)

### Tasks

#### S4-T1. Complete i64 Codegen
- [ ] Implement in `jit/codegen.rs`:
  - i64: `div_s`, `div_u`, `rem_s`, `rem_u`
  - i64: `and`, `or`, `xor`, `shl`, `shr_s`, `shr_u`, `rotl`, `rotr`
  - i64: `eq`, `ne`, `lt_s`, `lt_u`, `gt_s`, `gt_u`, `le_s`, `le_u`, `ge_s`, `ge_u`, `eqz`
  - i64: `clz`, `ctz`, `popcnt`

#### S4-T2. Implement f32/f64 Codegen (SSE/SSE2)
- [ ] Implement f32: `add`, `sub`, `mul`, `div`, `neg`, `abs`, `sqrt`, `ceil`, `floor`, `trunc`, `nearest`
- [ ] Implement f64: `add`, `sub`, `mul`, `div`, `neg`, `abs`, `sqrt`, `ceil`, `floor`, `trunc`, `nearest`
- [ ] Implement f32/f64 comparisons: `eq`, `ne`, `lt`, `gt`, `le`, `ge`
- [ ] Implement f32/f64 conversions: `f32.convert_i32_s/u`, `f64.convert_i64_s/u`, `i32.trunc_f32_s/u`, `f32.demote_f64`, `f64.promote_f32`
- [ ] Implement reinterpret: `i32.reinterpret_f32`, `f32.reinterpret_i32`, etc.
- [ ] Use XMM registers (XMM0-XMM15) for FP operations

#### S4-T3. Complete Control Flow Codegen
- [ ] Implement `br_if` — conditional branch with i32 test
- [ ] Implement `br_table` — jump table dispatch
- [ ] Implement `call_indirect` — table-based indirect call
- [ ] Implement `memory.size` and `memory.grow`
- [ ] Implement 8/16-bit load/store variants (i32.load8_s, i32.load8_u, i32.load16_s, etc.)

#### S4-T4. Codegen Tests
- [ ] Add ≥20 unit tests for `codegen.rs`:
  - i64 arithmetic correctness
  - f32/f64 arithmetic correctness (NaN, Inf, -0)
  - Control flow: br_if, br_table
  - Memory load/store variants
  - Full function compilation round-trip
- [ ] Add ≥5 unit tests for `compiler.rs` optimization passes

#### S4-T5. Benchmark Harness
- [ ] Create `runtime/src/jit/bench.rs` — benchmark framework
- [ ] Implement benchmark: fibonacci(35) — interpreter vs JIT speedup
- [ ] Implement benchmark: matrix multiply 4x4 — FP performance
- [ ] Implement benchmark: bubble sort 1000 elements — memory access patterns
- [ ] Record baseline measurements (interpreter timing)

### Quality Gate S4-QG

| ID | Criterion | Verification |
|----|-----------|-------------|
| S4-QG1 | All i64 arithmetic/bitwise/compare codegen produces correct results | ≥10 unit tests |
| S4-QG2 | f32/f64 add/sub/mul/div/sqrt codegen produces correct results | ≥8 unit tests |
| S4-QG3 | `br_if` and `br_table` produce correct control flow | ≥3 unit tests |
| S4-QG4 | `call_indirect` with type checking works or traps correctly | ≥2 unit tests |
| S4-QG5 | Benchmark harness runs and records interpreter baseline | Benchmark output |
| S4-QG6 | `cargo build` passes, ≥25 new tests in `jit/` | Build + test count |

---

## Sub-Phase S5: Component Model Core

### Goal

Implement the canonical ABI that bridges WIT interface types to WASM core module types, and a basic component linker.

### Prerequisites
- S1 Quality Gate passed (resource table used by component resources)
- S2 Quality Gate passed (WASI P2 interfaces are component imports)

### Tasks

#### S5-T1. Canonical ABI — Type Lowering
- [ ] Create `runtime/src/component/mod.rs`
- [ ] Create `runtime/src/component/canonical.rs`
- [ ] Implement `lower` functions (high-level → core WASM types):
  - Primitive types: bool, u8-u64, s8-s64, f32, f64, char → direct mapping
  - `string` → (ptr: i32, len: i32) in linear memory
  - `list<T>` → (ptr: i32, len: i32)
  - `record` → flattened fields
  - `variant` / `enum` → discriminant (i32) + payload
  - `flags` → one or more i32 bitmasks
  - `option<T>` → discriminant (i32) + optional payload
  - `result<T, E>` → discriminant (i32) + ok or error payload

#### S5-T2. Canonical ABI — Type Lifting
- [ ] Implement `lift` functions (core WASM types → high-level):
  - Inverse of all lowering operations
  - String lifting: copy from linear memory to host string
  - List lifting: copy from linear memory to host vec
  - Validation: discriminants in range, pointers in bounds

#### S5-T3. Component Linker
- [ ] Create `runtime/src/component/linker.rs`
- [ ] Implement `ComponentLinker`:
  - `define_instance(name, exports)` — register a host-provided instance
  - `instantiate(component_bytes) → ComponentInstance`
  - Import resolution: match component imports to linker definitions
  - Export extraction: extract callable component exports
- [ ] Single-module component support (MVP — one core module per component)

#### S5-T4. Component Instance
- [ ] Create `runtime/src/component/instance.rs`
- [ ] Implement `ComponentInstance`:
  - `call(name, args: &[ComponentValue]) → Result<Vec<ComponentValue>>`
  - Automatic lowering of args → core WASM values
  - Automatic lifting of core WASM results → component values
- [ ] `ComponentValue` enum: Bool, U8..U64, S8..S64, F32, F64, Char, String, List, Record, Variant, Enum, Flags, Option, Result

#### S5-T5. WASI P2 as Component Imports
- [ ] Wire up WASI P2 interfaces (from S1-S3) as component imports
- [ ] Integration test: load a component with `wasi:cli/command` world → execute `run()` → verify stdout

### Quality Gate S5-QG

| ID | Criterion | Verification |
|----|-----------|-------------|
| S5-QG1 | Lowering/lifting round-trip for string, list, record, variant, option, result | ≥12 unit tests |
| S5-QG2 | `ComponentLinker::instantiate()` successfully loads a single-core-module component | Unit test |
| S5-QG3 | `ComponentInstance::call()` invokes a function and returns correct result | Unit test |
| S5-QG4 | WASI P2 imports resolve correctly during component instantiation | Integration test |
| S5-QG5 | `cargo build` passes, ≥20 new tests in `component/` | Build + test count |

---

## Sub-Phase S6: App Packaging & Registry

### Goal

Complete the app lifecycle with persistent registry, update mechanism, and sample apps.

### Prerequisites
- S1-S2 Quality Gates passed (WASI P2 available for apps)

### Tasks

#### S6-T1. App Registry
- [ ] Create `runtime/src/registry.rs`
- [ ] Implement `AppRegistry`:
  - `register(manifest) → Result<AppId>`
  - `unregister(app_id) → Result<()>`
  - `get(app_id) → Option<&AppManifest>`
  - `list() → Vec<&AppManifest>`
  - `is_installed(app_id) → bool`
- [ ] In-memory registry backed by BTreeMap (VFS persistence deferred to kernel integration)

#### S6-T2. App Update Mechanism
- [ ] Add to `app_launcher.rs`:
  - `check_update(installed: &AppManifest, candidate: &[u8]) → UpdateAction`
  - `UpdateAction`: UpToDate, UpdateAvailable { new_version }, IncompatibleVersion
  - Version comparison using semver rules
- [ ] Migration: preserve app data directory on update

#### S6-T3. Sample `.kpioapp` Packages
- [ ] Create `examples/hello-world-kpioapp/`:
  - `manifest.toml` with proper fields
  - WASM binary (minimal `_start` that writes "Hello, KPIO!" to stdout)
  - Package documentation
- [ ] Create `examples/calculator-kpioapp/`:
  - Basic WASM app with integer arithmetic via stdin/stdout
  - GUI functions stubbed (kpio:gui import declarations but no visual rendering)
- [ ] Create `examples/counter-kpioapp/`:
  - Stateful app: increment/decrement counter, display via stdout

#### S6-T4. Package Validation Enhancement
- [ ] Add Deflate decompression to ZIP reader (or document Store-only limitation)
- [ ] Add manifest schema validation (required fields, version format, valid permissions)
- [ ] Entry point validation: verify `entry` field points to valid WASM export

### Quality Gate S6-QG

| ID | Criterion | Verification |
|----|-----------|-------------|
| S6-QG1 | `AppRegistry` register/unregister/list works correctly | ≥5 unit tests |
| S6-QG2 | `check_update` returns correct action for same, newer, older, incompatible versions | ≥4 unit tests |
| S6-QG3 | `hello-world-kpioapp` example has valid `manifest.toml` and builds | File existence + parse test |
| S6-QG4 | `launch_kpioapp()` for hello-world produces "Hello, KPIO!" on stdout | Integration test |
| S6-QG5 | `cargo build` passes, ≥12 new tests | Build + test count |

---

## Sub-Phase S7: Cross-Compile & Developer Docs

### Goal

Provide developer documentation for building WASM apps targeting KPIO, with a POSIX compatibility shim.

### Prerequisites
- S6 Quality Gate passed (app packaging complete)

### Tasks

#### S7-T1. Rust → WASM Guide
- [ ] Create `docs/guides/WASM_APP_RUST.md`:
  - Toolchain setup: `rustup target add wasm32-wasip2`
  - Hello world example with step-by-step
  - Using `kpio:gui` WIT bindings
  - Packaging as `.kpioapp`
  - Debugging tips

#### S7-T2. C/C++ → WASM Guide
- [ ] Create `docs/guides/WASM_APP_C_CPP.md`:
  - wasi-sdk installation and setup
  - CMake toolchain file for WASI
  - Compiling C hello world
  - Linking with KPIO host functions
  - Known limitations

#### S7-T3. POSIX Shim Library
- [ ] Create `runtime/src/posix_shim.rs`:
  - Map POSIX `open/read/write/close/stat` → WASI P2 filesystem calls
  - Map `socket/connect/bind/listen/accept/send/recv` → WASI P2 sockets
  - Map `malloc/free` → WASM linear memory management
  - Map `clock_gettime` → WASI P2 clocks
  - This is documentation + type definitions (actual linking happens at compile time via wasi-sdk)

#### S7-T4. Developer Reference
- [ ] Create `docs/guides/KPIO_APP_API_REFERENCE.md`:
  - Complete list of available WASI P2 interfaces with function signatures
  - Complete list of `kpio:gui`, `kpio:system`, `kpio:net` WIT interfaces
  - Permission model explanation
  - Resource limits documentation

### Quality Gate S7-QG

| ID | Criterion | Verification |
|----|-----------|-------------|
| S7-QG1 | Rust guide contains complete hello world example with build commands | Doc review |
| S7-QG2 | C/C++ guide contains wasi-sdk setup + hello world example | Doc review |
| S7-QG3 | POSIX shim maps at least 10 POSIX functions to WASI P2 equivalents | Code review |
| S7-QG4 | API reference covers all WASI P2 + kpio WIT interfaces | Doc completeness |
| S7-QG5 | `cargo build` passes, all docs compile (no broken links) | Build check |

---

## Implementation Roadmap

```
Week      1       2       3       4       5       6       7       8
         ├───────┤───────┤───────┤───────┤───────┤───────┤───────┤───────┤
 S1      ████████                                                        WASI P2 Foundation
 S2              ████████████████                                         WASI P2 Core
 S3                      ████████████████                                 WASI Network
 S4      ████████████████████████                                         JIT Completion
 S5                              ████████████████████████                 Component Model
 S6                                              ████████████████         App Packaging
 S7                                                      ████████████████ Dev Docs
```

> S1 and S4 can start in parallel (S4 has no dependencies on S1).  
> S3 depends on S1 (streams). S5 depends on S1+S2.  
> S6 and S7 are sequential, after core infrastructure is ready.

---

## Success Criteria (Phase 7-3 Complete)

### Must Have
- [ ] WASI P2 streams, filesystem, clocks, random, CLI interfaces implemented
- [ ] TCP/UDP socket API (loopback) working
- [ ] JIT codegen handles all i32/i64 integer operations correctly
- [ ] Component Model MVP: single-module component instantiation and execution
- [ ] At least 1 sample `.kpioapp` loads and runs to completion

### Should Have
- [ ] JIT f32/f64 FP codegen with SSE/SSE2 instructions
- [ ] HTTP outgoing request API (stub/mock responses)
- [ ] App registry with install/uninstall lifecycle
- [ ] Developer guide for Rust → WASM → `.kpioapp`

### Nice to Have
- [ ] Component composition (multiple WASM modules linked together)
- [ ] JIT benchmark showing measurable speedup over interpreter
- [ ] POSIX shim library with 10+ mapped functions
- [ ] C/C++ cross-compile guide

---

## Technical Risks and Mitigation

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Canonical ABI complexity (lift/lower) | 🔴 High | High | MVP: support primitive + string + list only, defer complex types |
| SSE codegen bugs (NaN handling) | 🟡 Medium | Medium | IEEE 754 test vectors, interpreter fallback for edge cases |
| WASI P2 spec churn | 🟡 Medium | Low | Pin to wasi-preview2 snapshot |
| Component binary format complexity | 🔴 High | High | MVP: text WIT only, skip binary component parsing |
| Network stack integration for real sockets | 🟡 Medium | Medium | Loopback-only for S3, defer kernel network integration |

---

*This document defines the Phase 7-3 execution plan. Each sub-phase must achieve 100% quality gate compliance before proceeding to the next.*
