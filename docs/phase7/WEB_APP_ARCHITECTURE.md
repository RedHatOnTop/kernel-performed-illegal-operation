# KPIO Web App Platform — Internal Architecture

Describes the internal architecture of the web app platform implemented in Phase 7-1.

## Overall Structure

```
User (click/keyboard)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│  kernel/src/gui/                                             │
│  ┌──────────┐  ┌──────────┐  ┌─────────────┐  ┌──────────┐ │
│  │ Desktop  │  │ Taskbar  │  │ Notification │  │  Toast   │ │
│  │ (icons)  │  │(run apps)│  │   Panel     │  │ (alerts) │ │
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
│  │(reg/query)│  │(launch/exit) │  │(access check)  │         │
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
│  ┌──────────────────────────────────────────────────────┐    │
│  │ Background Sync (SyncManager + SyncRegistry)         │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. Kernel App Manager (`kernel/src/app/`)

| File | Role |
|------|------|
| `registry.rs` | App registration/query/deletion. `APP_REGISTRY: Mutex<AppRegistry>` global |
| `lifecycle.rs` | App instance creation/termination/state management |
| `permissions.rs` | Filesystem/network access permission checks |
| `error.rs` | `AppError` error type |
| `window_state.rs` | Window position/size persistence |

### 2. GUI Integration (`kernel/src/gui/`)

| File | Role |
|------|------|
| `window.rs` | `WindowContent::WebApp` variant, `PwaDisplayMode`, `new_webapp()` |
| `desktop.rs` | `IconType::InstalledApp`, `refresh_app_icons()` |
| `taskbar.rs` | `AppType::WebApp`, taskbar entries |
| `splash.rs` | PWA splash screen rendering |
| `notification.rs` | `NotificationCenter` (50-item history, FIFO) |
| `toast.rs` | `ToastManager` (max 3, 5-second auto-dismiss) |
| `notification_panel.rs` | Bell icon + notification panel |

### 3. PWA Engine (`kpio-browser/src/pwa/`)

| File | Role |
|------|------|
| `manifest.rs` | Web App Manifest parsing |
| `install.rs` | Install/uninstall manager |
| `kernel_bridge.rs` | Kernel ↔ Browser function pointer bridge |
| `sw_bridge.rs` | Service Worker lifecycle management |
| `cache_storage.rs` | Cache API (25MB quota, LRU eviction) |
| `fetch_interceptor.rs` | Fetch interception (CacheFirst/NetworkFirst/...) |
| `web_storage.rs` | localStorage / sessionStorage (5MB quota) |
| `indexed_db.rs` | IndexedDB API (IDBFactory/Database/ObjectStore) |
| `idb_engine.rs` | B-Tree KV store (50MB quota) |
| `notification_bridge.rs` | Notification API permissions + dispatch |
| `background_sync.rs` | Background Sync (retry backoff) |

## Data Flow

### PWA Installation

```
User clicks "Install"
  → install.rs: InstallManager::start_install(manifest)
  → kernel_bridge.rs: pwa_install_to_kernel(name, scope, ...)
  → [function pointer callback]
  → kernel/browser/pwa_bridge.rs: bridge_install(...)
  → app/registry.rs: APP_REGISTRY.lock().register(WebApp {...})
  → gui/desktop.rs: Desktop::refresh_app_icons()
  → Icon appears on desktop
```

### PWA Launch

```
Double-click desktop icon
  → gui/mod.rs: launch_app(AppType::WebApp { ... })
  → gui/window.rs: Window::new_webapp(...)
  → splash.rs: render_splash() (displayed briefly)
  → Load start_url → Display app screen
```

### Notifications

```
App JS: new Notification("Title", { body: "Content" })
  → notification_bridge.rs: show_notification(app_id, ...)
  → [kernel callback]
  → notification.rs: NOTIFICATION_CENTER.lock().show(...)
  → toast.rs: ToastManager::push(...)
  → Toast rendered in top-right corner
```

### Offline Cache

```
App fetch("/api/data")
  → fetch_interceptor.rs: intercept(url, scope)
  → sw_bridge.rs: match_scope(url) → Check active SW
  → cache_storage.rs: match_url(url) → Cache hit
  → FetchResult::Response(cached_data)
```

## VFS Directory Structure

```
/
├── apps/
│   ├── data/
│   │   └── {app_id}/
│   │       ├── manifest.json      ← Stored manifest
│   │       ├── window_state.json  ← Window position/size
│   │       └── sync_tasks.json    ← Background Sync tasks
│   ├── cache/
│   │   └── {app_id}/
│   │       └── {cache_name}/      ← Cache API data
│   └── storage/
│       └── {app_id}/
│           ├── local_storage.json ← localStorage
│           └── idb/
│               └── {db_name}/     ← IndexedDB data
├── system/
│   └── apps/
│       ├── registry.json          ← App registry
│       └── permissions/
│           └── {app_id}.json      ← App permission settings
```

## Syscall Interface

| Number | Name | Arguments | Description |
|--------|------|-----------|-------------|
| 106 | `AppInstall` | manifest_ptr, manifest_len | Install PWA |
| 107 | `AppLaunch` | app_id | Launch app |
| 108 | `AppTerminate` | instance_id | Terminate app |
| 109 | `AppGetInfo` | app_id, buf_ptr, buf_len | Query app info |
| 110 | `AppList` | buf_ptr, buf_len | List apps |
| 111 | `AppUninstall` | app_id | Uninstall app |

## Circular Dependency Resolution

Due to the `kpio-browser` → `kpio-graphics` → `kpio-kernel` dependency chain,
direct crate dependency between kernel and browser is not possible.

**Solution**: Function pointer callback bridge

```rust
// kpio-browser side (callback registration)
static INSTALL_CALLBACK: RwLock<Option<fn(...)>> = RwLock::new(None);

pub fn register_kernel_callbacks(install_fn: fn(...)) {
    *INSTALL_CALLBACK.write() = Some(install_fn);
}

// kernel side (callback provider)
fn bridge_install(...) { /* APP_REGISTRY.lock().register(...) */ }

// During initialization
kpio_browser::pwa::kernel_bridge::register_kernel_callbacks(bridge_install);
```
