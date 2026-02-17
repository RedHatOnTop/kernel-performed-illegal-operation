# Phase 7: App Execution Layer

> **Goal:** Build a unified app runtime on top of KPIO OS that can execute web apps, WASM apps, and a subset of Linux binaries.  
> **Dependencies:** Phase 5 (System Integration), Phase 6.1-6.2 (Network/TLS)  
> **Estimated Duration:** 10-14 weeks  

---

## Strategy Summary

| Tier | App Type | Priority | Approach |
|------|----------|----------|----------|
| **Tier 1** | Web Apps (PWA) | 🔴 Required | Service Worker + Web App Manifest + offline storage |
| **Tier 2** | WASM/WASI Apps | 🔴 Required | WASI Preview 2 + Component Model + native GUI bindings |
| **Tier 3** | Linux ELF Binaries | 🟡 Optional | Static-linked ELF loader + Linux syscall translation layer (subset) |
| **Tier 4** | Other OS Apps (Win/Android) | ⚪ Not adopted | Provide WASM cross-compilation path (no native emulation) |

### Tier 4 Rejection Rationale

**Windows App Compatibility (Wine-style):**
- Win32 API surface: ~10,000+ functions → infeasible to implement
- Requires PE loader, Registry, COM, GDI/DirectX emulation → millions of LOC
- Wine project: 30 years, thousands of contributors → not realistic

**Android Apps:**
- Requires full Dalvik/ART JVM runtime implementation
- Android Framework dependencies (Activity, Service, ContentProvider...) → massive surface area

**Alternative:** Adopt WASM as the universal binary format. Apps from other OSes can be cross-compiled to WASM and run on KPIO. This effectively supports apps written in any language (C/C++/Rust/Go/C#/Swift) while keeping kernel complexity manageable.

---

## Phase 7.1: App Runtime Foundation ✅ COMPLETE

> **Goal:** Build the shared app model, lifecycle management, and sandboxing framework for all app types.  
> **Estimated Duration:** 2 weeks  
> **Dependencies:** Phase 5.1 (Kernel-Browser Integration), Phase 5.5 (Security Hardening)

### 7.1.1 Unified App Model

```
┌──────────────────────────────────────────────────────┐
│                    App Manager                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │ Web App  │  │ WASM App │  │ Native (ELF) App │   │
│  │ Runtime  │  │ Runtime  │  │    Runtime        │   │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘   │
│       │              │                 │              │
│  ┌────┴──────────────┴─────────────────┴──────────┐  │
│  │          Capability-based Sandbox               │  │
│  ├─────────────────────────────────────────────────┤  │
│  │          Unified IPC Bus (Channel + SHM)        │  │
│  ├─────────────────────────────────────────────────┤  │
│  │          Resource Manager (CPU / Mem / GPU)      │  │
│  └─────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

**AppDescriptor** (App Metadata):
```rust
pub struct AppDescriptor {
    pub id: AppId,                    // Unique identifier (reverse-domain: com.example.app)
    pub name: String,                 // Display name
    pub version: SemVer,              // Version
    pub app_type: AppType,            // WebApp | WasmApp | NativeApp
    pub entry_point: EntryPoint,      // URL | WASM byte path | ELF path
    pub permissions: PermissionSet,   // Requested permissions
    pub icon: Option<IconData>,       // App icon (PNG/SVG)
    pub categories: Vec<Category>,    // Categories (productivity, games, utilities...)
    pub min_kpio_version: SemVer,     // Minimum KPIO version
}

pub enum AppType {
    WebApp { manifest_url: String, offline_capable: bool },
    WasmApp { wasi_version: WasiVersion, component: bool },
    NativeApp { arch: Arch, abi: Abi },
}
```

**Checklist:**
- [x] `AppDescriptor` struct and serialization (JSON/TOML)
- [x] `AppId` type (reverse-domain format validation)
- [x] `AppType` enum (WebApp, WasmApp, NativeApp)
- [x] `AppState` state machine: `Installing → Installed → Launching → Running → Suspended → Terminated`
- [x] `AppRegistry`: Persistent installed app list (VFS `/apps/registry.json`)
- [x] Per-app data directory isolation (`/apps/data/{app_id}/`)

### 7.1.2 App Lifecycle Manager

```
Install → Launch → Running ⇄ Suspended → Terminate → Uninstall
                     ↓
                  Crashed → Recovery/Restart
```

**Checklist:**
- [x] `AppLifecycleManager` trait:
  - `install(descriptor) → Result<AppId>`
  - `launch(app_id) → Result<ProcessHandle>`
  - `suspend(app_id) → Result<()>`
  - `resume(app_id) → Result<()>`
  - `terminate(app_id) → Result<()>`
  - `uninstall(app_id) → Result<()>`
- [x] Process-to-app mapping table
- [x] Crash detection and auto-restart policy (max 3 retries)
- [x] Resource release guarantees (file handles, SHM, GPU buffers)
- [x] App state persistence (serialization on suspend)

### 7.1.3 Capability-Based Sandbox

Extends the Phase 5.5 capability model to the app level:

```rust
pub struct PermissionSet {
    pub filesystem: FsPermission,     // None | ReadOnly(paths) | ReadWrite(paths)
    pub network: NetPermission,       // None | LocalOnly | FullAccess | AllowList(domains)
    pub gpu: GpuPermission,           // None | Canvas2D | WebGL | FullGPU
    pub camera: bool,
    pub microphone: bool,
    pub clipboard: ClipboardPermission, // None | Read | ReadWrite
    pub notifications: bool,
    pub background_execution: bool,
    pub ipc: IpcPermission,           // None | AllowList(app_ids)
    pub max_memory_mb: u32,           // Memory cap
    pub max_cpu_percent: u8,          // CPU cap
}
```

**Checklist:**
- [x] `PermissionSet` struct definition
- [x] Runtime permission check layer (syscall proxy)
- [x] Permission request UI (user approval dialog)
- [x] Permission persistence (per-app granted/denied records)
- [x] Resource quota enforcement (OOM killer integration)

### 7.1.4 Inter-App IPC Framework

Wraps existing syscalls `ChannelCreate/Send/Recv` + `ShmCreate/Map` as app-level API:

**Checklist:**
- [x] `AppIpcBus`: App ID-based message routing
- [x] Structured message format (header + payload)
- [x] Intent system: `OpenFile(path)`, `ShareText(text)`, `ViewUrl(url)` standard intents
- [x] File sharing protocol (fd passing or SHM-based)
- [x] App discovery: Query apps that can handle a given intent

---

## Phase 7.2: Web App Platform — 🔴 Required ✅ COMPLETE

> **Goal:** Install and run PWAs at the same level as native apps, with offline support and push notifications.  
> **Estimated Duration:** 3 weeks  
> **Dependencies:** Phase 7.1, Phase 6.3 (JS Engine), Phase 6.5 (Web Platform API)

### 7.2.1 Web App Manifest Processing

W3C Web App Manifest spec parsing and application:

**Checklist:**
- [x] `WebAppManifest` parser (JSON)
- [x] Field handling: `name`, `short_name`, `start_url`, `scope`, `display`, `orientation`
- [x] `display` modes: `fullscreen`, `standalone`, `minimal-ui`, `browser`
- [x] Icon download and multi-resolution handling (192px, 512px)
- [x] `theme_color` → window titlebar/taskbar color
- [x] `background_color` → splash screen
- [x] `scope`-based navigation restriction
- [x] Installability determination: HTTPS + manifest + Service Worker registration

### 7.2.2 Service Worker Runtime

**Checklist:**
- [x] Service Worker registration (`navigator.serviceWorker.register()`)
- [x] SW lifecycle: `installing → waiting → active → redundant`
- [x] `install` event: static resource precaching
- [x] `activate` event: previous cache cleanup
- [x] `fetch` event interception
- [x] Caching strategies: Cache First, Network First, Stale While Revalidate
- [x] Cache Storage API (`caches.open()`, `cache.put()`, `cache.match()`)
- [x] VFS-based cache persistence
- [x] SW update detection and refresh

### 7.2.3 Offline Storage

**Checklist:**
- [x] `localStorage` / `sessionStorage` (key-value store, 5MB limit)
- [x] IndexedDB basic implementation
- [x] Quota management (max 50MB per app, user-expandable)
- [x] Data persistence (VFS `/apps/storage/{app_id}/`)

### 7.2.4-7.2.6 Web App Window Integration, Notifications, Install UX

**Checklist:**
- [x] `standalone` / `minimal-ui` display modes
- [x] Taskbar icon integration
- [x] Notification API integration
- [x] Install banner/prompt
- [x] App update detection

---

## Phase 7.3: WASM/WASI App Runtime — 🔴 Required ✅ COMPLETE

> **Goal:** Establish WASM as KPIO's universal app binary format, enabling apps written in any language to run safely.  
> **Estimated Duration:** 3 weeks  
> **Dependencies:** Phase 7.1, existing runtime/ crate

### 7.3.1 WASI Preview 2 Full Implementation

**Checklist:**
- [x] `wasi:io` — Stream read/write (stdin, stdout, stderr)
- [x] `wasi:filesystem` — File/directory CRUD, stat, readdir
- [x] `wasi:sockets` — TCP/UDP sockets (connect, bind, listen, accept)
- [x] `wasi:clocks` — Monotonic/system clocks
- [x] `wasi:random` — CSPRNG-based random
- [x] `wasi:cli` — Command-line args, env vars, exit code
- [x] `wasi:http` — HTTP outgoing handler
- [x] Preopened file descriptors — sandbox boundary
- [x] Capability-based filesystem access

### 7.3.2 WASM Component Model

**Checklist:**
- [x] WIT (WebAssembly Interface Type) parser
- [x] Component linker: import/export resolution and binding
- [x] Interface type conversions (string, list, record, variant, enum, flags)
- [x] Canonical ABI (lower/lift functions)
- [x] `kpio:gui` custom world: KPIO GUI API bindings
- [x] `kpio:system` custom world: System info API

### 7.3.3 JIT Compiler Activation

**Checklist:**
- [x] Baseline JIT: WASM bytecode → x86_64 machine code 1:1 translation
  - Integer/floating-point operations (full i32/i64/f32/f64)
  - Control flow (br, br_if, block, loop, if)
  - Memory access (load/store + bounds check)
  - Function calls (direct + indirect)
- [x] W^X-compliant executable memory allocation
- [x] Tiered compilation framework (interpreter → baseline JIT)
- [x] Code cache: disk-persistent compiled machine code (AOT cache)
- [x] Benchmarks: 120 tests, 7 benchmark scenarios

### 7.3.4 WASM App Packaging and Execution

**Checklist:**
- [x] `.kpioapp` package format (ZIP with manifest.toml + app.wasm)
- [x] Package validation (signature check, manifest validity)
- [x] WASM app launcher: `.kpioapp` → unpack → instantiate → run
- [x] WASM app update: version comparison with semver
- [x] Sample apps: hello-world, calculator, counter `.kpioapp` examples

### 7.3.5 Cross-Compile Support

**Checklist:**
- [x] Guide: C/C++ → WASM (wasi-sdk/Emscripten)
- [x] Guide: Rust → WASM (cargo build --target wasm32-wasip2)
- [x] POSIX compatibility shim library (22 POSIX → WASI P2 mappings)
- [x] KPIO App API Reference documentation

---

## Phase 7.4: Linux Binary Compatibility Layer — 🟡 Optional

> **Goal:** Run statically-linked Linux x86_64 ELF binaries directly on KPIO.  
> **Estimated Duration:** 2-3 weeks  
> **Dependencies:** Phase 7.1  
> **Scope Limitation:** Dynamic linking (glibc/ld-linux.so) and GUI apps (X11/Wayland) are NOT supported.

### Design Principle: "Lightweight" Compatibility

Implement only a **practical subset**, not full Linux ABI emulation:

```
┌────────────────────────────────────┐
│        Linux ELF Binary            │
│    (statically linked, musl)       │
└──────────────┬─────────────────────┘
               │ syscall (int 0x80 / syscall)
┌──────────────┴─────────────────────┐
│    Linux Syscall Translation Layer  │
│    ┌─────────────────────────────┐  │
│    │ Linux syscall # → KPIO     │  │
│    │ sys_write → SYS_WRITE      │  │
│    │ sys_read  → SYS_READ       │  │
│    │ sys_open  → SYS_OPEN       │  │
│    │ ...                         │  │
│    └─────────────────────────────┘  │
└──────────────┬─────────────────────┘
               │ KPIO syscall
┌──────────────┴─────────────────────┐
│          KPIO Kernel                │
└────────────────────────────────────┘
```

### 7.4.1 ELF Loader

**Checklist:**
- [ ] ELF64 header parsing (magic number, endianness, ABI validation)
- [ ] Program Header processing:
  - `PT_LOAD`: Map segments into user-space memory
  - `PT_INTERP`: Reject dynamic linker requests (static only)
  - `PT_GNU_STACK`: Set NX bit
- [ ] Entry point (`e_entry`) extraction and jump
- [ ] User-space stack setup: `argc`, `argv[]`, `envp[]`, auxv[]
- [ ] Auxiliary Vector (AT_PAGESZ, AT_CLKTCK, AT_RANDOM, etc.)
- [ ] PIE (Position Independent Executable) support: ASLR-applied base address
- [ ] BSS segment zero initialization

### 7.4.2 Linux Syscall Translation Layer

Implement ~40 core syscalls to cover most CLI tools:

```
Required Syscalls (Tier A — covers most CLI tools):
───────────────────────────────────────────────
read(0), write(1), open(2), close(3), stat(4), fstat(5),
lseek(8), mmap(9), mprotect(10), munmap(11), brk(12),
ioctl(16)†, access(21), pipe(22), dup(32), dup2(33),
getpid(39), fork(57)†, execve(59)†, exit(60), wait4(61),
kill(62)†, uname(63), fcntl(72)†, getcwd(79), chdir(80),
mkdir(83), unlink(87), readlink(89), getuid(102), getgid(104),
gettimeofday(96), nanosleep(35), clock_gettime(228),
exit_group(231), openat(257), readlinkat(267),
arch_prctl(158), set_tid_address(218), set_robust_list(273)
```
> † = Partial implementation (stub or subset)

**Checklist:**
- [ ] `syscall` instruction intercept (user-space → kernel transition)
- [ ] Linux syscall number → KPIO syscall routing table
- [ ] Tier A 40 syscalls:
  - [ ] File I/O: `read`, `write`, `open`, `close`, `stat`, `fstat`, `lseek`, `access`, `openat`
  - [ ] Memory: `mmap`, `mprotect`, `munmap`, `brk`
  - [ ] Process: `getpid`, `exit`, `exit_group`, `uname`, `arch_prctl`
  - [ ] Directory: `getcwd`, `chdir`, `mkdir`, `unlink`, `readlink`
  - [ ] Pipe/FD: `pipe`, `dup`, `dup2`, `fcntl`
  - [ ] Time: `gettimeofday`, `nanosleep`, `clock_gettime`
  - [ ] Identity: `getuid`, `getgid` (single user → always 0)
  - [ ] Threading helpers: `set_tid_address`, `set_robust_list` (stubs)
- [ ] Unsupported syscall handling: return `ENOSYS` + log
- [ ] errno mapping table (Linux errno → KPIO errno)
- [ ] Basic signal handling: `SIGTERM` → process termination, `SIGKILL` → immediate kill

### 7.4.3 Compatibility Testing

**Checklist:**
- [ ] BusyBox (static musl build) execution:
  - `busybox ls`, `busybox cat`, `busybox grep`, `busybox wc`
  - `busybox echo`, `busybox head`, `busybox tail`
  - `busybox sort`, `busybox uniq`, `busybox tr`
- [ ] Statically-linked Rust binary (hello world, file I/O)
- [ ] Statically-linked Go binary (hello world, HTTP server)
- [ ] Statically-linked C binary (musl-gcc, basic system programming)
- [ ] Compatibility matrix documentation (supported/unsupported syscall list)

### 7.4.4 Limitations and Alternative Paths

Unsupported areas and recommended alternatives:

| Unsupported | Reason | Alternative |
|-------------|--------|-------------|
| Dynamically-linked ELF | Requires ld-linux.so + full glibc | Recommend static linking (musl) |
| X11/Wayland GUI | Display server protocol too large | Use WASM + `kpio:gui` bindings |
| Linux kernel modules | Kernel ABI incompatible | Write KPIO-native drivers |
| ptrace/seccomp | Complex kernel features | Use WASM sandbox |
| Systemd/Init | Service manager compat unnecessary | Use KPIO App Manager |

---

## Phase 7.5: App Distribution & Management

> **Goal:** Build an integrated app management system for searching, installing, updating, and removing apps.  
> **Estimated Duration:** 1-2 weeks  
> **Dependencies:** Phase 7.1, Phase 6.1 (Network)

### 7.5.1 App Store Client

**Checklist:**
- [ ] App catalog UI (grid/list view)
- [ ] Category browsing (Productivity, Games, Utilities, Dev Tools, Media)
- [ ] Search functionality (name, description, keywords)
- [ ] App detail page (screenshots, description, permissions, version history)
- [ ] Install/uninstall buttons + progress indicator
- [ ] Catalog source configuration:
  - Local repository (USB/VFS)
  - HTTP-based remote repository (URL registration)
  - Built-in system app list

### 7.5.2 Package Management

**Checklist:**
- [ ] Package format validation (`.kpioapp`, `.wasm`, web URL)
- [ ] Digital signature verification (Ed25519):
  - Developer signature → public key verification
  - Optional store signature (additional trust layer)
- [ ] Dependency resolution (between WASM components)
- [ ] Automatic updates:
  - Background update checking (periodic)
  - User confirmation before applying updates
  - Rollback support (previous version retention)
- [ ] Storage management: app size display, cache cleanup, large app warnings

### 7.5.3 Developer Tools

**Checklist:**
- [ ] `kpio-sdk` CLI (runs on host OS):
  - `kpio build` — Build WASM app
  - `kpio package` — Package into `.kpioapp`
  - `kpio sign` — Sign package
  - `kpio validate` — Validate manifest/WASM
  - `kpio run --emulate` — Emulation run on host
- [ ] Templates: Rust WASM, C/C++ WASI, Web App (PWA)
- [ ] Developer documentation: API reference, tutorials, sample code

---

## Phase 7.6: Cross-Platform Integration

> **Goal:** Provide a consistent system integration experience across all app types.  
> **Estimated Duration:** 1-2 weeks  
> **Dependencies:** Phase 7.1-7.3

### 7.6.1 Unified Clipboard

**Checklist:**
- [ ] System clipboard service (text, rich text, images)
- [ ] Cross-app copy/paste (WASM ↔ Web App ↔ Native)
- [ ] MIME type-based data negotiation
- [ ] Clipboard history (last 10 items)

### 7.6.2 File Association

**Checklist:**
- [ ] MIME type → default app mapping table
- [ ] File extension → MIME type mapping
- [ ] "Open With" selection dialog
- [ ] `file_handlers` declaration in app manifest
- [ ] File explorer double-click → launch associated app

### 7.6.3 Drag and Drop

**Checklist:**
- [ ] Inter-window drag and drop protocol
- [ ] File drag: source app → system → target app
- [ ] Text/image drag
- [ ] Drag preview (ghost image)
- [ ] Drop target highlighting

### 7.6.4 System Tray and Status Bar Integration

**Checklist:**
- [ ] Per-app system tray icon registration API
- [ ] Tray icon context menu
- [ ] Badge/notification count display
- [ ] Background app status indicator

---

## Implementation Roadmap

```
Week     1    2    3    4    5    6    7    8    9   10   11   12
        ├────┤────┤────┤────┤────┤────┤────┤────┤────┤────┤────┤────┤
 7.1    ████████████████                                            App Runtime Foundation
 7.2                    ██████████████████████                       Web App Platform
 7.3                    ██████████████████████                       WASM/WASI Runtime
 7.4                                          ████████████████      Linux Compat Layer
 7.5                                          ██████████            App Distribution
 7.6                                                    ████████   Cross-Platform Integration
```

> 7.2 and 7.3 can proceed in parallel (only 7.1 must be completed first)  
> 7.4 is optional and can run in parallel with 7.5/7.6  

---

## Success Criteria

### Must Have
- [x] PWA installation and offline execution (at least 1 demo app)
- [x] WASM/WASI app `.kpioapp` package execution (including GUI apps)
- [x] Inter-app isolation (no crash propagation)
- [x] App install/uninstall lifecycle fully working
- [x] Baseline JIT achieves 5x performance improvement for WASM apps

### Should Have
- [ ] BusyBox basic commands (10+) running via Linux ELF compatibility
- [ ] App store UI (local repository-based)
- [ ] Inter-app clipboard integration
- [ ] File association (double-click → launch app)

### Nice to Have
- [ ] Remote app repository support
- [ ] Automatic updates
- [ ] SDL2 → `kpio:gui` translation layer
- [ ] WASM Component Model composition

---

## Technical Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| JIT compiler bugs (security vulnerabilities) | 🔴 High | Medium | W^X enforcement, fuzz testing, interpreter fallback |
| Service Worker JS execution complexity | 🟡 Medium | High | Depends on Phase 6.3 JS engine → implement simple cache strategies first |
| ELF loader memory safety | 🔴 High | Medium | User-space isolation, ASLR, syscall filtering |
| WASM Component Model complexity | 🟡 Medium | Medium | MVP (single module) first → component composition later |
| Lack of app ecosystem (nobody writes apps) | 🟡 Medium | High | Provide many sample apps + existing WASI app compat + cross-compile guides |

---

## New Crate/Module Structure

```
kernel/src/
  app/                    # Added in Phase 7.1
    mod.rs                # AppManager, AppRegistry
    lifecycle.rs          # Lifecycle management
    sandbox.rs            # Capability-based sandbox
    ipc_bus.rs            # Inter-app IPC routing
    manifest.rs           # AppDescriptor parsing
    store.rs              # App store client
    file_assoc.rs         # File association
  elf/                    # Added in Phase 7.4 (planned)
    loader.rs             # ELF64 parser/loader
    linux_abi.rs          # Linux syscall translation layer

kpio-browser/src/
  pwa/                    # Extended
    manifest.rs           # Web App Manifest parser
    service_worker.rs     # SW runtime
    cache.rs              # Cache Storage API
    install.rs            # Install UX
    storage.rs            # localStorage/IndexedDB

runtime/src/
  wasi2/                  # Added in Phase 7.3 (WASI Preview 2 full implementation)
  component/              # Added in Phase 7.3 (Component Model)
  jit/                    # Extended in Phase 7.3 (Baseline JIT actual codegen)
  package.rs              # Added in Phase 7.2 (.kpioapp package handling)
  app_launcher.rs         # Added in Phase 7.2 (app lifecycle management)
  registry.rs             # Added in Phase 7.3 (app registry)
  posix_shim.rs           # Added in Phase 7.3 (POSIX → WASI P2 mapping)
```

---

## Relationship with Phase 6

Phase 7 is a **consumer** of Phase 6:

- **Phase 6.3** (JS Engine) → Required for 7.2 Service Worker execution
- **Phase 6.5** (Web Platform API) → Required for 7.2 Cache API, Notification API
- **Phase 6.8** (PWA Foundation) → Overlaps with 7.2 → **7.2 absorbs/replaces it**
- **Phase 6.9** (Framework Compatibility) → Contributes to web app quality

Until Phase 6.3 is complete, 7.2's Service Worker operates in **cache-only mode** (URL pattern matching without JS).

---

*This document is the design specification for KPIO OS's App Execution Layer. Detailed implementation follows each sub-phase's checklist.*
