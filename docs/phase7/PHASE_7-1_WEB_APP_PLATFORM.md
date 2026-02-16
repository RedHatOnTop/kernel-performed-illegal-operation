# Phase 7-1: Tier 1 — Web App Platform

> **Parent Phase:** Phase 7 — App Execution Layer  
> **Goal:** Build a complete web app platform on top of KPIO OS that allows PWAs (Progressive Web Apps) to be installed, launched, and managed at the same level as native apps.  
> **Estimated Duration:** 5-6 weeks (8 sub-phases)  
> **Dependencies:** Phase 5.1 (Kernel-Browser Integration), Phase 5.5 (Security), Phase 6.1-6.2 (Network/TLS)  

---

## Current State Analysis (As-Is)

Before designing Phase 7-1, we identify the **infrastructure already implemented**. Each sub-phase is designed to add the "missing links" on top of this foundation.

> **Consistency Note (2026-02-15):** This document was originally written as a "pre-implementation plan,"
> but the current codebase already has a significant portion of Phase 7-1's core components (kernel app module, WebApp window, storage/notifications, etc.) implemented.
> The table below has been updated to reflect the **current code status**.

| Component | Location | Status | Notes |
|---------|------|------|------|
| **PWA Manifest Parser** | `kpio-browser/src/pwa/manifest.rs` | ✅ Implemented | |
| **PWA Install Manager** | `kpio-browser/src/pwa/install.rs` | ✅ Implemented | |
| **PWA Window Model** | `kpio-browser/src/pwa/window.rs` | ✅ Implemented | |
| **PWA Manager** | `kpio-browser/src/pwa/mod.rs` | ✅ Implemented | |
| **Push Notifications** | `kpio-browser/src/pwa/push.rs` | ✅ Implemented | |
| **App Model** | `kpio-browser/src/apps/mod.rs` | ✅ Implemented | |
| **App Launcher** | `kpio-browser/src/apps/app_launcher.rs` | ✅ Implemented | |
| **Service Worker Runtime** | `runtime/src/service_worker/` | ✅ Implemented | |
| **VFS** | `kernel/src/vfs/` | ✅ Implemented | |
| **Kernel App Management Module** | `kernel/src/app/` | ✅ Implemented | Includes registry/lifecycle/permissions/window_state |
| **App-specific Syscalls (106-111)** | `kernel/src/syscall/mod.rs` | ✅ Implemented | Document's 106-109 notation has been expanded to 106-111 |
| **Per-app VFS Sandbox** | `kernel/src/vfs/sandbox.rs` | ✅ Implemented | Per-app path isolation |
| **WindowContent::WebApp** | `kernel/src/gui/window.rs` | ✅ Implemented | `Window::new_webapp()` + `PwaDisplayMode` |
| **Dynamic Desktop Icons** | `kernel/src/gui/desktop.rs` | ✅ Implemented | `IconType::InstalledApp` |
| **localStorage / sessionStorage** | `kpio-browser/src/pwa/web_storage.rs` | ✅ Implemented | 5MB quota |
| **IndexedDB** | `kpio-browser/src/pwa/indexed_db.rs`, `idb_engine.rs` | ✅ Implemented | 50MB quota |
| **Cache Storage** | `kpio-browser/src/pwa/cache_storage.rs` | ✅ Implemented | 25MB quota |
| **Notification/Toast Rendering** | `kernel/src/gui/notification.rs`, `toast.rs` | ✅ Implemented | NotificationCenter(50), Toast(3, 5s) |
| **Notification Panel** | `kernel/src/gui/notification_panel.rs` | ✅ Implemented | |

---

## Sub-Phase Overall Roadmap

```
Week    1         2         3         4         5         6
      ├─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤
 A    ██████████                                                    Kernel App Manager
 B    ██████████                                                    App Syscalls & VFS Sandbox
 C              ██████████                                          PWA ↔ Kernel Integration
 D              ██████████                                          Window & Desktop Integration
 E                        ██████████                                Service Worker Integration
 F                        ██████████                                Web Storage Engine
 G                                  ██████████                      Notifications & Background Sync
 H                                  ██████████████████████          E2E Validation & Demo Apps
```

> A-B can be parallelized, C-D can be parallelized, E-F can be parallelized

---

## Sub-Phase 7-1.A: Kernel App Manager

### Purpose

Create a new `kernel/src/app/` module inside the kernel to establish the **app registration, lifecycle, and resource management** foundation common to all app types (web apps, WASM apps, native apps). This elevates the app model from `kpio-browser/src/apps/` to the kernel level.

### Prerequisites
- Phase 5.1 Kernel-Browser bridge operational
- `kernel/src/process/manager.rs` ProcessManager operational

### Tasks

#### A-1. App Registry (`kernel/src/app/registry.rs`)
- [ ] Define `KernelAppId(u64)` type (auto-incrementing ID)
- [ ] `KernelAppDescriptor` struct:
  ```rust
  pub struct KernelAppDescriptor {
      pub id: KernelAppId,
      pub app_type: KernelAppType,  // WebApp | WasmApp | NativeApp
      pub name: String,
      pub icon_data: Option<Vec<u8>>,  // PNG bytes
      pub install_path: String,        // App data path within VFS
      pub permissions: AppPermissions,
      pub installed_at: u64,           // Timestamp
      pub last_launched: u64,
  }
  ```
- [ ] `AppRegistry` struct:
  - `register(descriptor) → Result<KernelAppId, AppError>`
  - `unregister(id) → Result<(), AppError>`
  - `get(id) → Option<&KernelAppDescriptor>`
  - `list() → Vec<&KernelAppDescriptor>`
  - `find_by_name(name) → Option<&KernelAppDescriptor>`
  - `find_by_type(app_type) → Vec<&KernelAppDescriptor>`
- [ ] Global static instance `APP_REGISTRY: Mutex<AppRegistry>`
- [ ] VFS persistence: Serialize/deserialize to `/system/apps/registry.json`
- [ ] Load registry on boot, save on shutdown

#### A-2. App Lifecycle Manager (`kernel/src/app/lifecycle.rs`)
- [ ] `AppRunState` state machine:
  ```
  Registered → Launching → Running → Suspended → Terminated
                   ↓                      ↑
                 Failed ──── (auto-restart ≤3 times)
  ```
- [ ] `AppLifecycle` struct:
  - `launch(id) → Result<AppInstanceId, AppError>`
  - `suspend(instance_id) → Result<(), AppError>`
  - `resume(instance_id) → Result<(), AppError>`
  - `terminate(instance_id) → Result<(), AppError>`
  - `get_state(instance_id) → AppRunState`
  - `list_running() → Vec<AppInstanceInfo>`
- [ ] `AppInstanceId(u64)` — Distinguish multiple instances of the same app
- [ ] App-to-process mapping table (`HashMap<AppInstanceId, ProcessId>`)
- [ ] Crash detection: Transition to `Failed` state on abnormal process exit code (≠0)
- [ ] Auto-restart policy: `restart_count` ≤ 3, 10-second backoff
- [ ] Guaranteed resource cleanup: Clean up VFS FDs, SHM, and IPC channels on terminate

#### A-3. App Permission Framework (`kernel/src/app/permissions.rs`)
- [ ] `AppPermissions` struct:
  ```rust
  pub struct AppPermissions {
      pub filesystem: FsScope,     // List of accessible VFS paths
      pub network: NetScope,       // None | LocalOnly | AllowList(domains) | Full
      pub notifications: bool,
      pub clipboard: bool,
      pub background: bool,        // Allow background execution
      pub max_memory_kb: u32,      // Memory limit (default 64MB)
  }
  ```
- [ ] `PermissionChecker` trait:
  - `check_fs(app_id, path, op) → Result<(), PermissionDenied>`
  - `check_net(app_id, domain) → Result<(), PermissionDenied>`
  - `check_notification(app_id) → bool`
- [ ] Permission grant/deny persistence: `/system/apps/permissions/{app_id}.json`
- [ ] Default permission profile: WebApp → `{fs: app_data_only, net: scope_only, notifications: ask}`

#### A-4. Module Structure and Integration
- [ ] `kernel/src/app/mod.rs` — Sub-module exports, error type definitions
- [ ] `kernel/src/app/error.rs` — `AppError` enum (NotFound, AlreadyRegistered, PermissionDenied, LaunchFailed, ResourceExhausted)
- [ ] Add `mod app;` to `kernel/src/lib.rs` or `main.rs`
- [ ] Call `AppRegistry::load_from_vfs()` on boot

### Deliverables
- `kernel/src/app/mod.rs`
- `kernel/src/app/registry.rs`
- `kernel/src/app/lifecycle.rs`
- `kernel/src/app/permissions.rs`
- `kernel/src/app/error.rs`

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| A-QG1 | App registration | After `register()` call, `get()` returns the same data | Unit test |
| A-QG2 | App unregistration | After `unregister()`, `get()` → `None`, removed from `list()` | Unit test |
| A-QG3 | State transitions | `launch → running → suspend → resume → running → terminate` transitions succeed | Unit test (state machine) |
| A-QG4 | Crash restart | After 3 abnormal process terminations, state fixed to `Failed`; no restart on 4th attempt | Unit test |
| A-QG5 | Persistence | `register()` → `registry.json` written to VFS. Load succeeds after reboot | Integration test |
| A-QG6 | Permission check | WebApp accessing `/system/` path returns `PermissionDenied` | Unit test |
| A-QG7 | Build | `cargo build --target x86_64-kpio` passes without errors | CI |
| A-QG8 | Test coverage | At least 5 tests each for `registry.rs`, `lifecycle.rs`, `permissions.rs` | `cargo test` |

---

## Sub-Phase 7-1.B: App Syscalls & VFS Sandbox

### Purpose

Add a **dedicated syscall interface** so that the userspace/browser crate can invoke the kernel app management module (7-1.A), and implement per-app **VFS path isolation**.

### Prerequisites
- 7-1.A Kernel App Manager quality gates A-QG1~QG8 all passed

### Tasks

#### B-1. App Management Syscall Definitions (extending `kernel/src/syscall/mod.rs`)
- [ ] Syscall 106: `AppInstall` — Register app (app_type, name, entry_point → app_id)
- [ ] Syscall 107: `AppLaunch` — Launch app (app_id → instance_id)
- [ ] Syscall 108: `AppTerminate` — Terminate app (instance_id → exit_code)
- [ ] Syscall 109: `AppGetInfo` — Query app info (app_id → AppDescriptor)
- [ ] Syscall 110: `AppList` — List installed apps (→ app_ids[])
- [ ] Syscall 111: `AppUninstall` — Remove app (app_id)
- [ ] Add 106-111 routing to `dispatch()` function
- [ ] Add 6 entries to `SyscallNumber` enum
- [ ] Document argument layout for each syscall (register mapping)

#### B-2. userlib Wrappers (extending `userlib/src/`)
- [ ] Add `userlib/src/app.rs` module:
  ```rust
  pub fn app_install(app_type: u64, name_ptr: *const u8, name_len: u64, entry_ptr: *const u8, entry_len: u64) -> Result<u64, SyscallError>
  pub fn app_launch(app_id: u64) -> Result<u64, SyscallError>
  pub fn app_terminate(instance_id: u64) -> Result<(), SyscallError>
  pub fn app_info(app_id: u64, buf: &mut [u8]) -> Result<usize, SyscallError>
  pub fn app_list(buf: &mut [u64]) -> Result<usize, SyscallError>
  pub fn app_uninstall(app_id: u64) -> Result<(), SyscallError>
  ```
- [ ] Add `pub mod app;` to `userlib/src/lib.rs`

#### B-3. VFS App Sandbox (`kernel/src/vfs/sandbox.rs`)
- [ ] `AppSandbox` struct:
  - `app_id: KernelAppId`
  - `home_dir: String` — App-specific path (e.g., `/apps/data/{app_id}/`)
  - `allowed_paths: Vec<String>` — Additional allowed paths (read-only)
- [ ] `resolve_path(app_id, requested_path) → Result<String, VfsError>`:
  - Relative path → `home_dir + requested_path`
  - Absolute path → Check inclusion in `allowed_paths`
  - Path traversal attack defense (block `../` escape)
- [ ] Integrate app context into VFS API:
  - `read_all_sandboxed(app_id, path)` → Call `read_all(resolved)` after path validation
  - `write_all_sandboxed(app_id, path, data)` → Allow writes only within home_dir
- [ ] Auto-create directory on app install: `/apps/data/{app_id}/`
- [ ] Delete directory on app uninstall: Remove entire `/apps/data/{app_id}/`
- [ ] Allow global read-only paths: `/system/fonts/`, `/system/locale/`

### Deliverables
- `kernel/src/syscall/mod.rs` modified (syscalls 106-111)
- `userlib/src/app.rs` new
- `kernel/src/vfs/sandbox.rs` new

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| B-QG1 | Syscall invocation | `AppInstall(106)` → `AppGetInfo(109)` chain roundtrip confirmed | Integration test |
| B-QG2 | Syscall error | `AppLaunch(107)` with unregistered app_id → `ENOENT` error returned | Unit test |
| B-QG3 | VFS isolation | app_id=1 accessing `/apps/data/2/secret.txt` → `PermissionDenied` | Unit test |
| B-QG4 | Path escape blocked | `resolve_path(app, "../../etc/passwd")` → error | Unit test |
| B-QG5 | Directory lifecycle | `/apps/data/{id}/` created on install, deleted on uninstall | Integration test |
| B-QG6 | userlib wrapper | `userlib::app::app_list()` call → returns list of registered app IDs | Unit test |
| B-QG7 | Build | kernel + userlib full build succeeds | CI |

---

## Sub-Phase 7-1.C: PWA-Kernel Bridge

### Purpose

Connect the PWA manifest parser, install manager, and app model already implemented in `kpio-browser/src/pwa/` with the **kernel app management module (7-1.A)**, so that PWA installation leads to kernel-level app registration.

### Prerequisites
- 7-1.A Quality gates all passed
- 7-1.B Quality gates B-QG1, B-QG5 passed

### Tasks

#### C-1. PWA Install Bridge (`kpio-browser/src/pwa/kernel_bridge.rs`)
- [ ] `pwa_install_to_kernel(manifest: &WebAppManifest) → Result<KernelAppId>`:
  1. Extract `manifest.name` + `manifest.start_url`
  2. Download icon data (manifest.icons[0].src)
  3. Invoke `AppInstall` syscall → receive `KernelAppId`
  4. Save full manifest to `/apps/data/{id}/manifest.json`
  5. Save icon bytes to `/apps/data/{id}/icon.png`
- [ ] `pwa_uninstall_from_kernel(app_id: KernelAppId) → Result<()>`:
  1. Invoke `AppUninstall` syscall
  2. Call `PwaManager::uninstall()`
- [ ] Modify `PwaManager::install()`: Add `pwa_install_to_kernel()` at the end of existing logic
- [ ] Modify `PwaManager::uninstall()`: Add `pwa_uninstall_from_kernel()`

#### C-2. PWA Launch Bridge
- [ ] `pwa_launch_from_kernel(app_id: KernelAppId) → Result<String>`:
  1. Load `/apps/data/{id}/manifest.json`
  2. Extract `start_url`
  3. Invoke `AppLaunch` syscall → `instance_id`
  4. Return `start_url` (window creation handled in 7-1.D)
- [ ] Invoke `AppTerminate` syscall on PWA termination

#### C-3. Synchronize existing `InstalledApp` ↔ `KernelAppDescriptor`
- [ ] Add `kernel_app_id: Option<KernelAppId>` field to `InstalledApp`
- [ ] Synchronize `PwaManager` and `AppRegistry` on boot:
  - WebApp types registered in kernel → load into `PwaManager`
  - On mismatch, recover based on kernel registry
- [ ] Synchronization function: `sync_pwa_registry() → Result<SyncReport>`

#### C-4. Enhanced Installability Detection
- [ ] Detect `<link rel="manifest">` during browser navigation
- [ ] Install condition verification:
  - HTTPS (or `kpio://`) origin
  - Valid `manifest.json` (name + start_url required)
  - Service Worker registration status (if present, offline_capable = true)
- [ ] Send "Install" button activation signal to kernel GUI when installable

### Deliverables
- `kpio-browser/src/pwa/kernel_bridge.rs` new
- `kpio-browser/src/pwa/mod.rs` modified
- `kpio-browser/src/pwa/install.rs` modified

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| C-QG1 | PWA install → kernel registration | `PwaManager::install(manifest)` → `AppRegistry::get(id)` succeeds | Integration test |
| C-QG2 | PWA uninstall → kernel deregistration | After removal, `AppRegistry::get(id)` → `None` | Integration test |
| C-QG3 | Manifest persistence | After install, `/apps/data/{id}/manifest.json` exists and JSON parsing succeeds | Integration test |
| C-QG4 | Boot synchronization | 2 WebApps registered in registry → after boot, `PwaManager.installed_apps().len() == 2` | Integration test |
| C-QG5 | Duplicate install prevention | Re-install attempt with same `scope` → `AlreadyRegistered` error | Unit test |
| C-QG6 | Manifest detection | HTML containing `<link rel="manifest" href="/app.json">` → installability detected | Unit test |

---

## Sub-Phase 7-1.D: Window & Desktop Integration

### Purpose

Add a **PWA-specific window mode** to the kernel GUI and render **dynamic app icons** on the desktop, making installed web apps visually indistinguishable from native apps.

### Prerequisites
- 7-1.C Quality gates C-QG1, C-QG3 passed

### Tasks

#### D-1. Add `WindowContent::WebApp` Variant (`kernel/src/gui/window.rs`)
- [ ] Extend `WindowContent` enum:
  ```rust
  WebApp {
      app_id: KernelAppId,
      url: String,
      content: String,
      rendered: Option<RenderedPage>,
      display_mode: DisplayMode,  // Standalone | MinimalUi | Fullscreen
      theme_color: Option<u32>,   // ARGB
      scope: String,              // Navigation restriction scope
  }
  ```
- [ ] `Window::new_webapp(id, app_id, manifest, x, y)` factory method:
  - Determine window chrome based on `display_mode`:
    - `Standalone`: Title bar only (no address bar), apply `theme_color`
    - `MinimalUi`: Title bar + back/forward/refresh mini buttons
    - `Fullscreen`: No chrome, full screen
  - Initial size: 800×600 (previous session remembered size takes priority)
- [ ] Window title → `manifest.short_name || manifest.name`
- [ ] Title bar background color → `manifest.theme_color`
- [ ] Navigation to URL outside scope → open in separate browser window

#### D-2. PWA Splash Screen (`kernel/src/gui/splash.rs`)
- [ ] `render_splash(window, manifest)`:
  - Fill entire window with `background_color`
  - Render app icon centered (bitmap scaling)
  - `manifest.name` text below icon
  - Auto-dismiss when `start_url` finishes loading (max 3-second timeout)

#### D-3. Dynamic Desktop Icons (`kernel/src/gui/desktop.rs`)
- [ ] Extend `IconType` enum:
  ```rust
  pub enum IconType {
      Files, Browser, Terminal, Settings, Trash,
      InstalledApp { app_id: KernelAppId, icon_data: Option<Vec<u8>> },
  }
  ```
- [ ] `Desktop::refresh_app_icons()`:
  - Query WebApp types from `AppRegistry::list()`
  - Dynamically place installed app icons after system icons (5)
  - Calculate icon grid (based on 5-column layout)
- [ ] New app icon rendering:
  - If `icon_data` exists → decode PNG bitmap and render at 32×32 scale
  - If no `icon_data` → default icon with first letter of name + colored circle
- [ ] Icon click → `AppLifecycle::launch(app_id)` → create PWA window
- [ ] Icon right-click → context menu (Launch, Uninstall)

#### D-4. Taskbar Integration (`kernel/src/gui/taskbar.rs`)
- [ ] Running WebApp → display icon + name on taskbar
- [ ] `theme_color`-based accent color for taskbar items
- [ ] Taskbar item click → toggle window focus/minimize
- [ ] "All Apps" button on taskbar → open app launcher

#### D-5. Window Size/Position Persistence (`kernel/src/app/window_state.rs`)
- [ ] `WindowStateStore`:
  - `save(app_id, x, y, width, height)`
  - `load(app_id) → Option<(i32, i32, u32, u32)>`
- [ ] VFS persistence: `/apps/data/{app_id}/window_state.json`
- [ ] Auto-save on window move/resize completion

### Deliverables
- `kernel/src/gui/window.rs` modified
- `kernel/src/gui/desktop.rs` modified
- `kernel/src/gui/taskbar.rs` modified
- `kernel/src/gui/splash.rs` new
- `kernel/src/app/window_state.rs` new

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| D-QG1 | WebApp window | `Window::new_webapp()` creates → standalone window rendered without address bar | QEMU visual verification |
| D-QG2 | theme_color | `#2196F3` theme → title bar rendered in blue | QEMU visual verification |
| D-QG3 | Dynamic icons | 2 apps installed → 7 icons on desktop (5 system + 2 apps) | QEMU visual verification |
| D-QG4 | Icon click launch | Desktop app icon click → WebApp window created | QEMU functional verification |
| D-QG5 | Icon after uninstall | App uninstalled → desktop icon disappears immediately | QEMU visual verification |
| D-QG6 | Scope restriction | External URL clicked within WebApp → opens in browser window | Functional test |
| D-QG7 | Taskbar display | WebApp running → app name + theme color shown on taskbar | QEMU visual verification |
| D-QG8 | Splash screen | WebApp launch immediately → `background_color` + icon splash shown (transition after ≤3s) | QEMU visual verification |
| D-QG9 | Window size memory | Window resize → close → relaunch → restored to previous size | Functional test |

---

## Sub-Phase 7-1.E: Service Worker Integration

### Purpose

Connect the `runtime/src/service_worker/` module to the actual web app's **fetch interception**, **cache management**, and **offline operation** pipeline.

### Prerequisites
- 7-1.C Quality gate C-QG1 passed (PWA can be registered with kernel)
- Phase 6.3 JS engine basic operation (or bypass with cache-only mode)

### Tasks

#### E-1. SW ↔ Browser Event Pipeline (`kpio-browser/src/pwa/sw_bridge.rs`)
- [ ] `ServiceWorkerBridge` struct:
  - `register(scope: &str, script_url: &str) → Result<ServiceWorkerId>`
  - `unregister(scope: &str) → Result<()>`
  - `get_registration(scope: &str) → Option<ServiceWorkerRegistration>`
- [ ] `navigator.serviceWorker.register()` JS API → map to `ServiceWorkerBridge::register()`
- [ ] Download SW script → save to VFS `/apps/cache/{app_id}/sw.js`
- [ ] SW state change events → propagate to browser (statechange)

#### E-2. Fetch Intercept Pipeline
- [ ] `FetchInterceptor`:
  1. Web app network request occurs
  2. Search for matching active SW (by `scope`)
  3. Dispatch `FetchEvent` to SW
  4. Wait for SW response (5-second timeout):
     - `event.respondWith(response)` → use cache/custom response
     - Timeout/unhandled → direct network request (fallback)
- [ ] **Cache-only mode** (fallback when JS engine is incomplete):
  - If SW JS execution is unavailable → URL pattern matching based cache strategy
  - `sw_cache_config.json`: `{ patterns: [{ url: "/**/*.css", strategy: "cache-first" }] }`
  - On match: cache hit → cache response; miss → network request
- [ ] fetch event logging (for debugging): request URL, cache hit/miss, SW response status

#### E-3. Cache Storage API Implementation
- [ ] `CacheStorage` (global `caches` object):
  - `open(cache_name) → Cache`
  - `has(cache_name) → bool`
  - `delete(cache_name) → bool`
  - `keys() → Vec<String>`
- [ ] `Cache`:
  - `put(request, response)` — Store URL + response body
  - `match(request) → Option<Response>` — URL matching lookup
  - `delete(request) → bool`
  - `keys() → Vec<Request>`
- [ ] VFS-based persistence:
  - Storage path: `/apps/cache/{app_id}/{cache_name}/`
  - Metadata: `_meta.json` (URL → filename mapping)
  - Response body: `{hash}.body` (binary)
  - Response headers: `{hash}.headers` (JSON)
- [ ] Quota management: 25MB per-app cache size limit (LRU eviction on exceed)

#### E-4. SW Lifecycle Event Implementation
- [ ] `install` event:
  - `event.waitUntil(promise)` — Wait for pre-cache completion
  - Download pre-cache list → save to Cache Storage
- [ ] `activate` event:
  - `event.waitUntil(promise)` — Clean up previous caches
  - Process `caches.delete(old_cache_name)`
- [ ] SW update:
  - Byte comparison: existing SW ≠ new SW → trigger update
  - `waiting` state → immediate activation on `skipWaiting()` call
  - `clients.claim()` → immediately control existing clients

### Deliverables
- `kpio-browser/src/pwa/sw_bridge.rs` new
- `kpio-browser/src/pwa/cache_storage.rs` new
- `kpio-browser/src/pwa/fetch_interceptor.rs` new

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| E-QG1 | SW registration | `register("/", "/sw.js")` → `ServiceWorkerId` returned. State reaches `Activated` | Unit test |
| E-QG2 | Cache store/retrieve | `cache.put("/style.css", body)` → `cache.match("/style.css")` → same body | Unit test |
| E-QG3 | Cache VFS persistence | Cache put → file created in VFS → `cache.match()` succeeds after reboot | Integration test |
| E-QG4 | Cache-only mode | `/index.html` saved in cache → network blocked → cache response returned | Integration test |
| E-QG5 | Quota enforcement | Cache put exceeding 25MB → LRU eviction, total ≤ 25MB | Unit test |
| E-QG6 | SW update | SW script changed → new SW installed → skipWaiting → transition to active | Unit test |
| E-QG7 | Fetch fallback | No SW response (timeout) → fallback to direct network request | Integration test |

---

## Sub-Phase 7-1.F: Web Storage Engine

### Purpose

Provide **Web Storage API** (localStorage, sessionStorage) and basic **IndexedDB** implementation for PWA data persistence.

### Prerequisites
- 7-1.B Quality gate B-QG3 passed (VFS sandbox operational)

### Tasks

#### F-1. Web Storage API (`kpio-browser/src/pwa/web_storage.rs`)
- [ ] `WebStorage` struct:
  ```rust
  pub struct WebStorage {
      origin: String,
      data: BTreeMap<String, String>,
      storage_type: StorageType,  // Local | Session
      max_size: usize,            // 5MB (5_242_880 bytes)
      current_size: usize,
  }
  ```
- [ ] API:
  - `get_item(key) → Option<String>`
  - `set_item(key, value) → Result<(), QuotaExceededError>`
  - `remove_item(key)`
  - `clear()`
  - `key(index) → Option<String>`
  - `length() → usize`
- [ ] `localStorage`: VFS persistence (`/apps/storage/{app_id}/local_storage.json`)
- [ ] `sessionStorage`: Memory-only, destroyed on app termination
- [ ] Size limit: `QuotaExceededError` when key+value total exceeds 5MB
- [ ] `storage` event: Notify changes to other tabs/windows (`StorageEvent`)

#### F-2. IndexedDB Core (`kpio-browser/src/pwa/indexed_db.rs`)
- [ ] `IDBFactory`:
  - `open(name, version) → IDBOpenRequest`
  - `delete_database(name) → IDBOpenRequest`
  - `databases() → Vec<IDBDatabaseInfo>`
- [ ] `IDBDatabase`:
  - `create_object_store(name, options) → IDBObjectStore`
  - `delete_object_store(name)`
  - `transaction(store_names, mode) → IDBTransaction`
  - `object_store_names() → Vec<String>`
  - `close()`
- [ ] `IDBObjectStore`:
  - `put(value, key?) → IDBRequest`
  - `get(key) → IDBRequest`
  - `delete(key) → IDBRequest`
  - `clear() → IDBRequest`
  - `count() → IDBRequest`
  - `get_all(query?, count?) → IDBRequest`
  - `create_index(name, key_path, options) → IDBIndex`
- [ ] `IDBTransaction`:
  - `mode`: Readonly | Readwrite | Versionchange
  - `object_store(name) → IDBObjectStore`
  - `commit()` / `abort()`
  - `oncomplete`, `onerror`, `onabort` events
- [ ] `IDBIndex`:
  - `get(key) → IDBRequest`
  - `get_all(query?, count?) → IDBRequest`
  - `count() → IDBRequest`
- [ ] `IDBCursor`:
  - `continue()`, `advance(count)`
  - `update(value)`, `delete()`
  - `direction`: Next | Prev | NextUnique | PrevUnique

#### F-3. IndexedDB Storage Engine (`kpio-browser/src/pwa/idb_engine.rs`)
- [ ] B-Tree based key-value store:
  - Key: Sortable `IDBKey` (Number | String | Date | Binary | Array)
  - Value: JSON-serialized JS values
  - Index: Secondary B-Tree based on key_path
- [ ] VFS persistence:
  - Database path: `/apps/storage/{app_id}/idb/{db_name}/`
  - Object store: `{store_name}.kvidb` (custom binary format)
  - Metadata: `_schema.json` (store list, index info, version)
- [ ] Transaction isolation:
  - Readwrite: Store-level `Mutex` locking
  - Readonly: Multiple concurrent access allowed
  - Versionchange: Exclusive lock on entire DB
- [ ] Per-app quota: 50MB (localStorage 5MB + IndexedDB 45MB)

### Deliverables
- `kpio-browser/src/pwa/web_storage.rs` new
- `kpio-browser/src/pwa/indexed_db.rs` new
- `kpio-browser/src/pwa/idb_engine.rs` new

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| F-QG1 | localStorage CRUD | `setItem("k","v")` → `getItem("k")` = `"v"` → `removeItem("k")` → `getItem("k")` = None | Unit test |
| F-QG2 | localStorage persistence | `setItem` → app exit → app relaunch → `getItem` returns same value | Integration test |
| F-QG3 | sessionStorage non-persistence | `setItem` → app exit → app relaunch → `getItem` = None | Integration test |
| F-QG4 | Size limit | `setItem` exceeding 5MB → `QuotaExceededError` | Unit test |
| F-QG5 | IDB basic CRUD | `objectStore.put({name:"test"}, 1)` → `get(1)` → `{name:"test"}` | Unit test |
| F-QG6 | IDB transaction | 2x `put` within readwrite transaction → `commit` → `getAll` = 2 records | Unit test |
| F-QG7 | IDB abort | `put` within transaction → `abort` → `get` = None (rolled back) | Unit test |
| F-QG8 | IDB index | `createIndex("byName", "name")` → index-based `get("test")` succeeds | Unit test |
| F-QG9 | IDB persistence | `put` → app exit → relaunch → `get` returns same data | Integration test |
| F-QG10 | Combined quota | localStorage 3MB + IDB 47MB → next write triggers QuotaExceeded | Integration test |

---

## Sub-Phase 7-1.G: Notifications & Background Sync

### Purpose

Integrate PWA user engagement features — **push notifications** and **background sync** — into the kernel GUI.

### Prerequisites
- 7-1.D Quality gate D-QG1 passed (WebApp window exists)
- 7-1.B Quality gate B-QG1 passed (app syscalls operational)

### Tasks

#### G-1. Kernel Notification Center (`kernel/src/gui/notification.rs`)
- [ ] `Notification` struct:
  ```rust
  pub struct Notification {
      pub id: u64,
      pub app_id: KernelAppId,
      pub app_name: String,
      pub title: String,
      pub body: String,
      pub icon_data: Option<Vec<u8>>,
      pub timestamp: u64,
      pub read: bool,
      pub action_url: Option<String>,  // URL to navigate to on click
  }
  ```
- [ ] `NotificationCenter`:
  - `show(notification) → NotificationId` — Queue for toast display
  - `dismiss(id)` — Close toast
  - `list_unread() → Vec<&Notification>`
  - `mark_read(id)`
  - `clear_all()`
- [ ] Global instance: `NOTIFICATION_CENTER: Mutex<NotificationCenter>`
- [ ] Notification history: Retain latest 50 entries, FIFO eviction

#### G-2. Toast Rendering (`kernel/src/gui/toast.rs`)
- [ ] Toast position: Top-right of screen, queued top to bottom (max 3 simultaneous)
- [ ] Toast layout:
  ```
  ┌────────────────────────────────┐
  │ [App Icon] App Name    ✕ (close) │
  │ Notification Title (bold)        │
  │ Body text (max 2 lines)          │
  └────────────────────────────────┘
  ```
- [ ] Auto-dismiss: Fade out after 5 seconds (or on close click)
- [ ] Click behavior:
  - If `action_url` exists → focus app window + navigate to URL
  - Otherwise → simply focus app window
- [ ] Rendering z-order: Above all windows (always topmost)

#### G-3. Notification API Bridge (`kpio-browser/src/pwa/notification_bridge.rs`)
- [ ] `Notification.requestPermission()` → User approval dialog:
  - "[App name] wants to send notifications" + [Allow] [Block] buttons
  - Persist result → `/system/apps/permissions/{app_id}.json`
- [ ] `new Notification(title, { body, icon })` → invoke `NotificationCenter::show()`
- [ ] `notification.onclick` → event dispatch based on `action_url`
- [ ] `notification.close()` → invoke `NotificationCenter::dismiss()`

#### G-4. Background Sync (`kpio-browser/src/pwa/background_sync.rs`)
- [ ] `SyncManager`:
  - `register(tag) → Result<()>` — Register sync task
  - `get_tags() → Vec<String>` — List registered tags
- [ ] Network state monitoring:
  - Poll `kernel/src/net/` network connection status (5-second interval)
  - Detect offline → online transition
- [ ] On return to online:
  - Dispatch `sync` event to SW for registered `sync` tasks
  - On event processing failure → backoff retry (30s, 60s, 300s)
  - Discard after max 3 retries
- [ ] Task persistence: `/apps/data/{app_id}/sync_tasks.json`

#### G-5. Notification Management UI (`kernel/src/gui/notification_panel.rs`)
- [ ] Taskbar notification icon (bell shape):
  - If unread notifications exist → red badge (with count)
  - Click → toggle notification panel
- [ ] Notification panel:
  - Recent notification list (grouped by app)
  - Each notification: title + body + time + app name
  - "Mark all read" button
  - "Per-app notification settings" link → settings app

### Deliverables
- `kernel/src/gui/notification.rs` new
- `kernel/src/gui/toast.rs` new
- `kernel/src/gui/notification_panel.rs` new
- `kpio-browser/src/pwa/notification_bridge.rs` new
- `kpio-browser/src/pwa/background_sync.rs` new

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| G-QG1 | Toast rendering | `NotificationCenter::show()` → toast displayed at top-right (title + body) | QEMU visual verification |
| G-QG2 | Auto-dismiss | Toast displayed → auto-removed after 5 seconds | QEMU visual verification (timer) |
| G-QG3 | Simultaneous queuing | 3 consecutive shows → 3 toasts vertically stacked | QEMU visual verification |
| G-QG4 | Toast click | Toast click → corresponding app window focused | QEMU functional verification |
| G-QG5 | Permission request | Notification from unauthorized app → approval dialog displayed | QEMU visual verification |
| G-QG6 | Permission blocked | `Notification()` from blocked app → ignored (no toast) | Functional test |
| G-QG7 | Notification history | 10x show → `list_unread().len() == 10` | Unit test |
| G-QG8 | Notification panel | Bell icon click → notification list displayed in panel | QEMU visual verification |
| G-QG9 | Background sync | Sync registered → network blocked → restored → `sync` event fired | Integration test |
| G-QG10 | Retry limit | Sync event processing fails 3 times → task discarded | Unit test |

---

## Sub-Phase 7-1.H: E2E Validation & Demo Apps

### Purpose

Perform **end-to-end validation** of the entire Phase 7-1 pipeline and build **2 working demo PWAs** to prove the completeness of the web app platform.

### Prerequisites
- 7-1.A through 7-1.G all quality gates passed

### Tasks

#### H-1. Demo PWA #1: KPIO Notes (Memo App)
- [ ] Single-page web app:
  - HTML: Text area + save button + memo list
  - CSS: Minimal design, `theme_color: #4CAF50` (green)
  - JS: localStorage-based memo CRUD
- [ ] `manifest.json`:
  ```json
  {
    "name": "KPIO Notes",
    "short_name": "Notes",
    "start_url": "/notes/",
    "display": "standalone",
    "theme_color": "#4CAF50",
    "background_color": "#FFFFFF",
    "icons": [{ "src": "/notes/icon.png", "sizes": "192x192" }]
  }
  ```
- [ ] Service Worker: Cache First strategy (HTML/CSS/JS available offline)
- [ ] Verification scenario:
  1. Navigate to `/notes/` in browser
  2. "Install" button → installation complete
  3. Green Notes icon appears on desktop
  4. Icon click → standalone window opens (no address bar, green title bar)
  5. Write memo → saved to localStorage
  6. Close app → relaunch → memo persisted
  7. Network blocked → app works normally (offline)

#### H-2. Demo PWA #2: KPIO Weather (Weather App)
- [ ] Single-page web app:
  - HTML: Weather card + city selector
  - CSS: Gradient background, `theme_color: #2196F3` (blue)
  - JS: Fetch API for weather data requests (mock API)
- [ ] Service Worker: Network First strategy (prefer latest data, cache on offline)
- [ ] Notifications: "Temperature change notification" demo (Notification API)
- [ ] Background Sync: "Add city" while offline → auto-sync on return to online
- [ ] Verification scenario:
  1. Install → blue Weather icon appears
  2. Weather data loaded → cached
  3. "Temperature notification" allowed → toast notification received
  4. Network blocked → cached weather displayed
  5. Network restored → auto-refresh + sync event processed

#### H-3. E2E Test Suite (`tests/e2e/pwa/`)
- [ ] `test_pwa_install_uninstall.rs`:
  - PWA install → kernel registry check → desktop icon check → uninstall → cleanup verified
- [ ] `test_pwa_offline.rs`:
  - SW registration → resource caching → network blocked → page loads successfully
- [ ] `test_pwa_storage.rs`:
  - localStorage CRUD → app restart → data persistence verified
  - IndexedDB CRUD → transaction commit/rollback → persistence verified
- [ ] `test_pwa_notification.rs`:
  - Permission request → allowed → notification displayed → click → app focused
- [ ] `test_pwa_lifecycle.rs`:
  - launch → running → suspend → resume → terminate full cycle
- [ ] `test_pwa_multi_instance.rs`:
  - Same app launched twice → separate windows → separate instance_ids → terminating one keeps the other

#### H-4. Performance Benchmarks
- [ ] PWA install time: **Target < 2 seconds** (manifest parsing + icon saving + registry write)
- [ ] PWA launch time (cold start): **Target < 1 second** (window creation + splash + start_url load)
- [ ] localStorage `setItem` latency: **Target < 5ms** (100-byte key + 1KB value)
- [ ] Cache API `match` latency: **Target < 10ms** (1MB cache entry)
- [ ] Notification toast rendering latency: **Target < 16ms** (within 1 frame)

#### H-5. Documentation
- [ ] `docs/phase7/WEB_APP_DEVELOPER_GUIDE.md`:
  - Developing PWAs for KPIO (manifest authoring, SW registration, offline strategies)
  - Supported/unsupported Web API list
  - Limitations (quotas, permissions)
- [ ] `docs/phase7/WEB_APP_ARCHITECTURE.md`:
  - Internal architecture diagram
  - Data flow between components
  - VFS directory layout

### Deliverables
- `examples/pwa-notes/` — Notes demo app (HTML/CSS/JS/manifest/SW)
- `examples/pwa-weather/` — Weather demo app
- `tests/e2e/pwa/` — 6 E2E tests
- `docs/phase7/WEB_APP_DEVELOPER_GUIDE.md`
- `docs/phase7/WEB_APP_ARCHITECTURE.md`

### Quality Gates

| # | Verification Item | Pass Criteria | Verification Method |
|---|----------|----------|----------|
| H-QG1 | Notes full flow | Install → desktop icon → standalone launch → save memo → offline operation | QEMU E2E manual verification |
| H-QG2 | Weather full flow | Install → weather load → notification → offline cache → online restore sync | QEMU E2E manual verification |
| H-QG3 | E2E tests pass | All 6 E2E tests pass (0 failures) | `cargo test --test e2e` |
| H-QG4 | Install performance | PWA install < 2 seconds (3-run average) | Benchmark |
| H-QG5 | Cold start performance | PWA launch < 1 second (3-run average) | Benchmark |
| H-QG6 | localStorage performance | setItem (1KB) < 5ms (100-run average) | Benchmark |
| H-QG7 | Multi-instance | 2 windows of same PWA operate independently | E2E test |
| H-QG8 | Developer docs | Guide document enables writing a new demo PWA from scratch (self-contained) | Document review |
| H-QG9 | 0 panic | No kernel panic during 30 minutes of continuous use in QEMU | Stability test |

---

## Phase 7-1 Overall Exit Criteria

To declare Phase 7-1 complete, **all of the following conditions** must be met:

### Required (Must Pass)
1. ✅ All quality gates for sub-phases A through H passed (**56 items**)
2. ✅ 2 demo PWAs (Notes, Weather) fully operational: install → launch → offline → notifications
3. ✅ `cargo build --target x86_64-kpio` builds successfully with 0 warnings
4. ✅ `cargo test` (host) all pass
5. ✅ All 6 E2E tests pass
6. ✅ No kernel panic during 30 minutes of continuous use in QEMU

### Desirable (Should Pass)
7. 🔶 At least 4 out of 5 performance benchmark items meet targets
8. 🔶 Developer guide documentation complete
9. 🔶 Phase 7-1 changes recorded in RELEASE_NOTES.md

### Optional (Nice to Have)
10. ⬜ Third-party PWA (e.g., simple Todo MVC app) installs and runs successfully on KPIO
11. ⬜ IndexedDB cursor forward/backward iteration fully operational

---

## Architecture Diagram: Overall Data Flow

```
User (click/keyboard)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│  kernel/src/gui/                                             │
│  ┌──────────┐  ┌──────────┐  ┌─────────────┐  ┌──────────┐ │
│  │ Desktop  │  │ Taskbar  │  │ Notification │  │  Toast   │ │
│  │ (icons)  │  │(running  │  │   Panel     │  │(alerts)  │ │
│  │          │  │  apps)   │  │             │  │          │ │
│  └────┬─────┘  └────┬─────┘  └──────┬──────┘  └────┬─────┘ │
│       ▼              ▼               ▼               ▼       │
│  ┌──────────────────────────────────────────────────────┐    │
│  │              Window (WebApp variant)                  │    │
│  │  display_mode | theme_color | scope | splashscreen   │    │
│  └───────────────────────┬──────────────────────────────┘    │
└──────────────────────────┼───────────────────────────────────┘
                           │ Syscalls (106-111)
┌──────────────────────────┼───────────────────────────────────┐
│  kernel/src/app/         ▼                                    │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────┐         │
│  │ Registry │  │  Lifecycle   │  │  Permissions   │         │
│  │(register/│  │(launch/      │  │(permission     │         │
│  │  query)  │  │ terminate)   │  │  checks)       │         │
│  └────┬─────┘  └──────┬───────┘  └───────┬────────┘         │
│       │               │                  │                    │
│  ┌────┴───────────────┴──────────────────┴────────────┐      │
│  │                VFS Sandbox                          │      │
│  │  /apps/data/{id}/   /apps/cache/{id}/              │      │
│  │  /apps/storage/{id}/  /system/apps/registry.json   │      │
│  └────────────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────────────┘
                           │
┌──────────────────────────┼───────────────────────────────────┐
│  kpio-browser/src/pwa/   ▼                                    │
│  ┌──────────┐  ┌───────────────┐  ┌────────────────────┐    │
│  │ Manifest │  │ Install/      │  │ Notification       │    │
│  │ Parser   │  │ KernelBridge  │  │ Bridge             │    │
│  └──────────┘  └───────────────┘  └────────────────────┘    │
│  ┌──────────┐  ┌───────────────┐  ┌────────────────────┐    │
│  │ SW       │  │ Cache         │  │ Fetch              │    │
│  │ Bridge   │  │ Storage       │  │ Interceptor        │    │
│  └──────────┘  └───────────────┘  └────────────────────┘    │
│  ┌──────────────────────┐  ┌───────────────────────────┐    │
│  │ Web Storage          │  │ IndexedDB                  │    │
│  │ (local/session)      │  │ (IDB Engine + B-Tree)      │    │
│  └──────────────────────┘  └───────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
                           │
┌──────────────────────────┼───────────────────────────────────┐
│  runtime/src/            ▼                                    │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Service Worker Runtime                               │    │
│  │  (lifecycle, cache, fetch, sync, events)              │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

---

## File Status (Based on Current Codebase)

This section is organized based on **files that actually exist in the current repository**, not "planned new/modified files at the time of writing."

### Already Existing/Implemented (Phase 7-1 Core)

| File | Status | Notes |
|------|------|------|
| `kernel/src/app/mod.rs` | ✅ Exists | App module entry |
| `kernel/src/app/registry.rs` | ✅ Exists | App registry |
| `kernel/src/app/lifecycle.rs` | ✅ Exists | App lifecycle |
| `kernel/src/app/permissions.rs` | ✅ Exists | Permission model |
| `kernel/src/app/error.rs` | ✅ Exists | `AppError` definition |
| `kernel/src/app/window_state.rs` | ✅ Exists | Window state persistence |
| `kernel/src/vfs/sandbox.rs` | ✅ Exists | Per-app sandbox |
| `kernel/src/gui/window.rs` | ✅ Exists | `WindowContent::WebApp`, `new_webapp()` |
| `kernel/src/gui/desktop.rs` | ✅ Exists | Installed app icon integration |
| `kernel/src/gui/taskbar.rs` | ✅ Exists | WebApp taskbar items |
| `kernel/src/gui/splash.rs` | ✅ Exists | Splash rendering |
| `kernel/src/gui/notification.rs` | ✅ Exists | NotificationCenter (50-entry history) |
| `kernel/src/gui/toast.rs` | ✅ Exists | ToastManager (3 simultaneous, 5s) |
| `kernel/src/gui/notification_panel.rs` | ✅ Exists | Notification panel |
| `kpio-browser/src/pwa/kernel_bridge.rs` | ✅ Exists | Browser↔Kernel bridge |
| `kpio-browser/src/pwa/sw_bridge.rs` | ✅ Exists | SW registration/state management |
| `kpio-browser/src/pwa/cache_storage.rs` | ✅ Exists | Cache Storage |
| `kpio-browser/src/pwa/fetch_interceptor.rs` | ✅ Exists | Fetch interception |
| `kpio-browser/src/pwa/web_storage.rs` | ✅ Exists | local/session storage |
| `kpio-browser/src/pwa/indexed_db.rs` | ✅ Exists | IndexedDB API |
| `kpio-browser/src/pwa/idb_engine.rs` | ✅ Exists | IDB engine |
| `kpio-browser/src/pwa/notification_bridge.rs` | ✅ Exists | Notification bridge |
| `kpio-browser/src/pwa/background_sync.rs` | ✅ Exists | Background Sync |

### Planned Items in Document Currently Not Present

| Item | Status |
|------|------|
| `userlib/src/app.rs` | ❌ Not present (based on current repo) |

---

*Upon completion of Phase 7-1, KPIO OS provides a web app platform that supports PWA installation, execution, offline caching, notifications, and background sync. This document is a planning document and checklists/status will be continuously updated as implementation progresses.*
