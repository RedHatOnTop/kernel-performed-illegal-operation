# KPIO Web App Platform — 내부 아키텍처

Phase 7-1에서 구현된 웹 앱 플랫폼의 내부 아키텍처를 설명합니다.

## 전체 구조

```
사용자 (클릭/키보드)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│  kernel/src/gui/                                             │
│  ┌──────────┐  ┌──────────┐  ┌─────────────┐  ┌──────────┐ │
│  │ Desktop  │  │ Taskbar  │  │ Notification │  │  Toast   │ │
│  │ (아이콘)  │  │ (실행 앱) │  │   Panel     │  │ (알림)   │ │
│  └────┬─────┘  └────┬─────┘  └──────┬──────┘  └────┬─────┘ │
│       ▼              ▼               ▼               ▼       │
│  ┌──────────────────────────────────────────────────────┐    │
│  │              Window (WebApp variant)                  │    │
│  │  display_mode | theme_color | scope | splashscreen   │    │
│  └───────────────────────┬──────────────────────────────┘    │
└──────────────────────────┼───────────────────────────────────┘
                           │ 시스콜 (106-111)
┌──────────────────────────┼───────────────────────────────────┐
│  kernel/src/app/         ▼                                    │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────┐         │
│  │ Registry │  │  Lifecycle   │  │  Permissions   │         │
│  │ (등록/조회)│  │ (실행/종료)  │  │ (권한 검사)    │         │
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

## 컴포넌트 상세

### 1. 커널 앱 매니저 (`kernel/src/app/`)

| 파일 | 역할 |
|------|------|
| `registry.rs` | 앱 등록/조회/삭제. `APP_REGISTRY: Mutex<AppRegistry>` 글로벌 |
| `lifecycle.rs` | 앱 인스턴스 생성/종료/상태 관리 |
| `permissions.rs` | 파일시스템/네트워크 접근 권한 검사 |
| `error.rs` | `AppError` 에러 타입 |
| `window_state.rs` | 윈도우 위치/크기 영속화 |

### 2. GUI 통합 (`kernel/src/gui/`)

| 파일 | 역할 |
|------|------|
| `window.rs` | `WindowContent::WebApp` variant, `PwaDisplayMode`, `new_webapp()` |
| `desktop.rs` | `IconType::InstalledApp`, `refresh_app_icons()` |
| `taskbar.rs` | `AppType::WebApp`, 태스크바 항목 |
| `splash.rs` | PWA 스플래시 스크린 렌더링 |
| `notification.rs` | `NotificationCenter` (50건 이력, FIFO) |
| `toast.rs` | `ToastManager` (최대 3개, 5초 자동 사라짐) |
| `notification_panel.rs` | 벨 아이콘 + 알림 패널 |

### 3. PWA 엔진 (`kpio-browser/src/pwa/`)

| 파일 | 역할 |
|------|------|
| `manifest.rs` | Web App Manifest 파싱 |
| `install.rs` | 설치/제거 매니저 |
| `kernel_bridge.rs` | 커널 ↔ 브라우저 함수 포인터 브릿지 |
| `sw_bridge.rs` | Service Worker 라이프사이클 관리 |
| `cache_storage.rs` | Cache API (25MB 쿼터, LRU 축출) |
| `fetch_interceptor.rs` | Fetch 가로채기 (CacheFirst/NetworkFirst/...) |
| `web_storage.rs` | localStorage / sessionStorage (5MB 쿼터) |
| `indexed_db.rs` | IndexedDB API (IDBFactory/Database/ObjectStore) |
| `idb_engine.rs` | B-Tree KV 스토어 (50MB 쿼터) |
| `notification_bridge.rs` | Notification API 권한 + 디스패치 |
| `background_sync.rs` | Background Sync (재시도 백오프) |

## 데이터 흐름

### PWA 설치

```
사용자 "설치" 클릭
  → install.rs: InstallManager::start_install(manifest)
  → kernel_bridge.rs: pwa_install_to_kernel(name, scope, ...)
  → [function pointer callback]
  → kernel/browser/pwa_bridge.rs: bridge_install(...)
  → app/registry.rs: APP_REGISTRY.lock().register(WebApp {...})
  → gui/desktop.rs: Desktop::refresh_app_icons()
  → 데스크톱에 아이콘 출현
```

### PWA 실행

```
데스크톱 아이콘 더블클릭
  → gui/mod.rs: launch_app(AppType::WebApp { ... })
  → gui/window.rs: Window::new_webapp(...)
  → splash.rs: render_splash() (잠시 표시)
  → start_url 로드 → 앱 화면 표시
```

### 알림

```
앱 JS: new Notification("제목", { body: "내용" })
  → notification_bridge.rs: show_notification(app_id, ...)
  → [kernel callback]
  → notification.rs: NOTIFICATION_CENTER.lock().show(...)
  → toast.rs: ToastManager::push(...)
  → 화면 우상단에 토스트 렌더링
```

### 오프라인 캐시

```
앱 fetch("/api/data")
  → fetch_interceptor.rs: intercept(url, scope)
  → sw_bridge.rs: match_scope(url) → active SW 확인
  → cache_storage.rs: match_url(url) → 캐시 히트
  → FetchResult::Response(cached_data)
```

## VFS 디렉토리 구조

```
/
├── apps/
│   ├── data/
│   │   └── {app_id}/
│   │       ├── manifest.json      ← 저장된 매니페스트
│   │       ├── window_state.json  ← 윈도우 위치/크기
│   │       └── sync_tasks.json    ← Background Sync 태스크
│   ├── cache/
│   │   └── {app_id}/
│   │       └── {cache_name}/      ← Cache API 데이터
│   └── storage/
│       └── {app_id}/
│           ├── local_storage.json ← localStorage
│           └── idb/
│               └── {db_name}/     ← IndexedDB 데이터
├── system/
│   └── apps/
│       ├── registry.json          ← 앱 레지스트리
│       └── permissions/
│           └── {app_id}.json      ← 앱 권한 설정
```

## 시스콜 인터페이스

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 106 | `AppInstall` | manifest_ptr, manifest_len | PWA 설치 |
| 107 | `AppLaunch` | app_id | 앱 실행 |
| 108 | `AppTerminate` | instance_id | 앱 종료 |
| 109 | `AppGetInfo` | app_id, buf_ptr, buf_len | 앱 정보 조회 |
| 110 | `AppList` | buf_ptr, buf_len | 앱 목록 조회 |
| 111 | `AppUninstall` | app_id | 앱 제거 |

## 순환 의존성 해결

`kpio-browser` → `kpio-graphics` → `kpio-kernel` 의존 관계로 인해
커널과 브라우저 사이 직접 크레이트 의존성이 불가합니다.

**해결**: 함수 포인터 콜백 브릿지

```rust
// kpio-browser 측 (콜백 등록)
static INSTALL_CALLBACK: RwLock<Option<fn(...)>> = RwLock::new(None);

pub fn register_kernel_callbacks(install_fn: fn(...)) {
    *INSTALL_CALLBACK.write() = Some(install_fn);
}

// kernel 측 (콜백 제공)
fn bridge_install(...) { /* APP_REGISTRY.lock().register(...) */ }

// 초기화 시
kpio_browser::pwa::kernel_bridge::register_kernel_callbacks(bridge_install);
```
