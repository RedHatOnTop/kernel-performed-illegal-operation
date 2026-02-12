# Phase 7-1: Tier 1 — 웹 앱 실행 플랫폼 (Web App Platform)

> **상위 Phase:** Phase 7 — App Execution Layer  
> **목표:** KPIO OS 위에서 PWA(Progressive Web App)를 네이티브 앱과 동등한 수준으로 설치·실행·관리할 수 있는 완전한 웹 앱 플랫폼을 구축한다.  
> **예상 기간:** 5-6주 (8개 서브페이즈)  
> **의존성:** Phase 5.1 (커널-브라우저 통합), Phase 5.5 (보안), Phase 6.1-6.2 (네트워크/TLS)  

---

## 현재 상태 분석 (As-Is)

Phase 7-1 설계에 앞서 **이미 구현된 인프라**를 정확히 파악한다. 각 서브페이즈는 이 기반 위에 "빠진 연결고리"를 추가하는 방식으로 설계한다.

| 컴포넌트 | 위치 | 상태 | 비고 |
|---------|------|------|------|
| **PWA Manifest 파서** | `kpio-browser/src/pwa/manifest.rs` (540줄) | ✅ 구현 완료 | W3C 전체 필드 지원 |
| **PWA 설치 매니저** | `kpio-browser/src/pwa/install.rs` (516줄) | ✅ 구현 완료 | `BeforeInstallPromptEvent`, 설치 흐름 |
| **PWA 윈도우 모델** | `kpio-browser/src/pwa/window.rs` (452줄) | ✅ 구현 완료 | `PwaWindow`, `DisplayMode`, `TitleBarStyle` |
| **PWA 매니저** | `kpio-browser/src/pwa/mod.rs` (246줄) | ✅ 구현 완료 | `InstalledApp`, install/uninstall/launch |
| **Push 알림** | `kpio-browser/src/pwa/push.rs` (495줄) | ✅ 구현 완료 | `PushManager`, 구독/해제 |
| **앱 모델** | `kpio-browser/src/apps/mod.rs` | ✅ 구현 완료 | `AppId`, `AppInfo`, `AppState`, `AppInstance` |
| **앱 런처** | `kpio-browser/src/apps/app_launcher.rs` | ✅ 구현 완료 | 검색, 핀, 카테고리, 최근 사용 |
| **Service Worker 런타임** | `runtime/src/service_worker/` | ✅ 구현 완료 | 전체 라이프사이클, cache, fetch, sync |
| **VFS** | `kernel/src/vfs/` | ✅ 구현 완료 | read/write/stat/readdir, FD 테이블 |
| **프로세스 매니저** | `kernel/src/process/manager.rs` | ✅ 구현 완료 | ELF spawn, kill, exit (앱 전용 아님) |
| **커널 앱 관리 모듈** | `kernel/src/app/` | ❌ 없음 | **Phase 7-1 핵심 대상** |
| **WindowContent::WebApp** | `kernel/src/gui/window.rs` | ❌ 없음 | Browser variant만 존재 |
| **동적 데스크톱 아이콘** | `kernel/src/gui/desktop.rs` | ❌ 없음 | 5개 하드코딩 (IconType enum) |
| **앱 전용 시스콜** | `kernel/src/syscall/mod.rs` | ❌ 없음 | 106-109 미사용 슬롯 가용 |
| **앱별 VFS 샌드박스** | `kernel/src/vfs/` | ❌ 없음 | 전역 경로 접근 |
| **localStorage/IndexedDB** | — | ❌ 없음 | 웹 스토리지 API 미구현 |
| **알림 토스트 렌더링** | `kernel/src/gui/` | ❌ 없음 | 커널 GUI에 알림 시스템 없음 |

---

## 서브페이즈 총괄 로드맵

```
주차    1         2         3         4         5         6
      ├─────────┤─────────┤─────────┤─────────┤─────────┤─────────┤
 A    ██████████                                                    커널 앱 관리 모듈
 B    ██████████                                                    앱 시스콜 & VFS 샌드박스
 C              ██████████                                          PWA ↔ 커널 통합
 D              ██████████                                          윈도우 & 데스크톱 통합
 E                        ██████████                                Service Worker 연동
 F                        ██████████                                웹 스토리지 엔진
 G                                  ██████████                      알림 & 백그라운드 동기화
 H                                  ██████████████████████          종합 검증 & 데모 앱
```

> A-B는 병렬 가능, C-D는 병렬 가능, E-F는 병렬 가능

---

## Sub-Phase 7-1.A: 커널 앱 관리 모듈 (Kernel App Manager)

### 목적

커널 내부에 `kernel/src/app/` 모듈을 신규 생성하여, 모든 앱 유형(웹 앱, WASM 앱, 네이티브 앱)에 공통되는 **앱 등록·라이프사이클·리소스 관리** 기반을 구축한다. `kpio-browser/src/apps/`의 앱 모델을 커널 수준으로 격상시킨다.

### 선행 조건
- Phase 5.1 커널-브라우저 브릿지 동작
- `kernel/src/process/manager.rs` ProcessManager 정상 동작

### 작업

#### A-1. 앱 레지스트리 (`kernel/src/app/registry.rs`)
- [ ] `KernelAppId(u64)` 타입 정의 (자동 증가 ID)
- [ ] `KernelAppDescriptor` 구조체:
  ```rust
  pub struct KernelAppDescriptor {
      pub id: KernelAppId,
      pub app_type: KernelAppType,  // WebApp | WasmApp | NativeApp
      pub name: String,
      pub icon_data: Option<Vec<u8>>,  // PNG 바이트
      pub install_path: String,        // VFS 내 앱 데이터 경로
      pub permissions: AppPermissions,
      pub installed_at: u64,           // 타임스탬프
      pub last_launched: u64,
  }
  ```
- [ ] `AppRegistry` 구조체:
  - `register(descriptor) → Result<KernelAppId, AppError>`
  - `unregister(id) → Result<(), AppError>`
  - `get(id) → Option<&KernelAppDescriptor>`
  - `list() → Vec<&KernelAppDescriptor>`
  - `find_by_name(name) → Option<&KernelAppDescriptor>`
  - `find_by_type(app_type) → Vec<&KernelAppDescriptor>`
- [ ] 글로벌 정적 인스턴스 `APP_REGISTRY: Mutex<AppRegistry>`
- [ ] VFS 영속화: `/system/apps/registry.json` 에 직렬화/역직렬화
- [ ] 부팅 시 레지스트리 로드, 종료 시 저장

#### A-2. 앱 라이프사이클 매니저 (`kernel/src/app/lifecycle.rs`)
- [ ] `AppRunState` 상태 머신:
  ```
  Registered → Launching → Running → Suspended → Terminated
                   ↓                      ↑
                 Failed ──── (자동 재시작 ≤3회)
  ```
- [ ] `AppLifecycle` 구조체:
  - `launch(id) → Result<AppInstanceId, AppError>`
  - `suspend(instance_id) → Result<(), AppError>`
  - `resume(instance_id) → Result<(), AppError>`
  - `terminate(instance_id) → Result<(), AppError>`
  - `get_state(instance_id) → AppRunState`
  - `list_running() → Vec<AppInstanceInfo>`
- [ ] `AppInstanceId(u64)` — 동일 앱의 다중 인스턴스 구별
- [ ] 앱-프로세스 매핑 테이블 (`HashMap<AppInstanceId, ProcessId>`)
- [ ] 크래시 감지: 프로세스 종료 코드 비정상(≠0) 시 `Failed` 상태 전이
- [ ] 자동 재시작 정책: `restart_count` ≤ 3, 10초 백오프
- [ ] 리소스 해제 보장: terminate 시 VFS FD, SHM, IPC 채널 정리

#### A-3. 앱 권한 프레임워크 (`kernel/src/app/permissions.rs`)
- [ ] `AppPermissions` 구조체:
  ```rust
  pub struct AppPermissions {
      pub filesystem: FsScope,     // 접근 가능 VFS 경로 목록
      pub network: NetScope,       // None | LocalOnly | AllowList(domains) | Full
      pub notifications: bool,
      pub clipboard: bool,
      pub background: bool,        // 백그라운드 실행 허용
      pub max_memory_kb: u32,      // 메모리 상한 (기본 64MB)
  }
  ```
- [ ] `PermissionChecker` 트레이트:
  - `check_fs(app_id, path, op) → Result<(), PermissionDenied>`
  - `check_net(app_id, domain) → Result<(), PermissionDenied>`
  - `check_notification(app_id) → bool`
- [ ] 권한 부여/거부 영속화: `/system/apps/permissions/{app_id}.json`
- [ ] 기본 권한 프로파일: WebApp → `{fs: app_data_only, net: scope_only, notifications: ask}`

#### A-4. 모듈 구조 및 통합
- [ ] `kernel/src/app/mod.rs` — 서브모듈 export, 에러 타입 정의
- [ ] `kernel/src/app/error.rs` — `AppError` enum (NotFound, AlreadyRegistered, PermissionDenied, LaunchFailed, ResourceExhausted)
- [ ] `kernel/src/lib.rs` 또는 `main.rs`에 `mod app;` 추가
- [ ] 부팅 시 `AppRegistry::load_from_vfs()` 호출

### 산출물
- `kernel/src/app/mod.rs`
- `kernel/src/app/registry.rs`
- `kernel/src/app/lifecycle.rs`
- `kernel/src/app/permissions.rs`
- `kernel/src/app/error.rs`

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| A-QG1 | 앱 등록 | `register()` 호출 후 `get()` 으로 동일 데이터 반환 | 유닛 테스트 |
| A-QG2 | 앱 해제 | `unregister()` 후 `get()` → `None`, `list()` 에서 제거됨 | 유닛 테스트 |
| A-QG3 | 상태 전이 | `launch → running → suspend → resume → running → terminate` 전이 성공 | 유닛 테스트 (상태 머신) |
| A-QG4 | 크래시 재시작 | 프로세스 비정상 종료 3회 후 `Failed` 상태 고정, 4회째 재시작 안 됨 | 유닛 테스트 |
| A-QG5 | 영속화 | `register()` → VFS에 `registry.json` 기록됨. 앱 재부팅 후 로드 성공 | 통합 테스트 |
| A-QG6 | 권한 검사 | WebApp이 `/system/` 경로 접근 시 `PermissionDenied` 반환 | 유닛 테스트 |
| A-QG7 | 빌드 | `cargo build --target x86_64-kpio` 에러 없이 통과 | CI |
| A-QG8 | 테스트 커버리지 | `registry.rs`, `lifecycle.rs`, `permissions.rs` 각각 최소 5개 테스트 | `cargo test` |

---

## Sub-Phase 7-1.B: 앱 시스콜 & VFS 샌드박스 (App Syscalls & VFS Sandbox)

### 목적

커널 앱 관리 모듈(7-1.A)을 유저스페이스/브라우저 크레이트에서 호출할 수 있도록 **전용 시스콜 인터페이스**를 추가하고, 앱별 **VFS 경로 격리**를 구현한다.

### 선행 조건
- 7-1.A 커널 앱 관리 모듈 퀄리티 게이트 A-QG1~QG8 전체 통과

### 작업

#### B-1. 앱 관리 시스콜 정의 (`kernel/src/syscall/mod.rs` 확장)
- [ ] 시스콜 106: `AppInstall` — 앱 등록 (app_type, name, entry_point → app_id)
- [ ] 시스콜 107: `AppLaunch` — 앱 실행 (app_id → instance_id)
- [ ] 시스콜 108: `AppTerminate` — 앱 종료 (instance_id → exit_code)
- [ ] 시스콜 109: `AppGetInfo` — 앱 정보 조회 (app_id → AppDescriptor)
- [ ] 시스콜 110: `AppList` — 설치된 앱 목록 (→ app_ids[])
- [ ] 시스콜 111: `AppUninstall` — 앱 제거 (app_id)
- [ ] `dispatch()` 함수에 106-111 라우팅 추가
- [ ] `SyscallNumber` enum에 6개 항목 추가
- [ ] 각 시스콜의 인자 레이아웃 문서화 (레지스터 매핑)

#### B-2. userlib 래퍼 (`userlib/src/` 확장)
- [ ] `userlib/src/app.rs` 모듈 추가:
  ```rust
  pub fn app_install(app_type: u64, name_ptr: *const u8, name_len: u64, entry_ptr: *const u8, entry_len: u64) -> Result<u64, SyscallError>
  pub fn app_launch(app_id: u64) -> Result<u64, SyscallError>
  pub fn app_terminate(instance_id: u64) -> Result<(), SyscallError>
  pub fn app_info(app_id: u64, buf: &mut [u8]) -> Result<usize, SyscallError>
  pub fn app_list(buf: &mut [u64]) -> Result<usize, SyscallError>
  pub fn app_uninstall(app_id: u64) -> Result<(), SyscallError>
  ```
- [ ] `userlib/src/lib.rs`에 `pub mod app;` 추가

#### B-3. VFS 앱 샌드박스 (`kernel/src/vfs/sandbox.rs`)
- [ ] `AppSandbox` 구조체:
  - `app_id: KernelAppId`
  - `home_dir: String` — 앱 전용 경로 (예: `/apps/data/{app_id}/`)
  - `allowed_paths: Vec<String>` — 추가 허용 경로 (읽기 전용)
- [ ] `resolve_path(app_id, requested_path) → Result<String, VfsError>`:
  - 상대 경로 → `home_dir + requested_path`
  - 절대 경로 → `allowed_paths` 내 포함 여부 검사
  - 경로 트래버설 공격 방어 (`../` 탈출 차단)
- [ ] VFS API에 앱 컨텍스트 통합:
  - `read_all_sandboxed(app_id, path)` → 경로 검증 후 `read_all(resolved)` 호출
  - `write_all_sandboxed(app_id, path, data)` → home_dir 내에서만 기록 허용
- [ ] 앱 설치 시 자동 디렉토리 생성: `/apps/data/{app_id}/`
- [ ] 앱 제거 시 디렉토리 삭제: `/apps/data/{app_id}/` 전체 제거
- [ ] 글로벌 읽기 전용 경로 허용: `/system/fonts/`, `/system/locale/`

### 산출물
- `kernel/src/syscall/mod.rs` 수정 (106-111 시스콜)
- `userlib/src/app.rs` 신규
- `kernel/src/vfs/sandbox.rs` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| B-QG1 | 시스콜 호출 | `AppInstall(106)` → `AppGetInfo(109)` 체인으로 앱 정보 왕복 확인 | 통합 테스트 |
| B-QG2 | 시스콜 에러 | 미등록 app_id로 `AppLaunch(107)` → `ENOENT` 에러 반환 | 유닛 테스트 |
| B-QG3 | VFS 격리 | app_id=1 이 `/apps/data/2/secret.txt` 접근 시 `PermissionDenied` | 유닛 테스트 |
| B-QG4 | 경로 탈출 차단 | `resolve_path(app, "../../etc/passwd")` → 에러 | 유닛 테스트 |
| B-QG5 | 디렉토리 생명주기 | 설치 시 `/apps/data/{id}/` 생성, 제거 시 삭제 확인 | 통합 테스트 |
| B-QG6 | userlib 래퍼 | `userlib::app::app_list()` 호출 → 등록된 앱 ID 목록 반환 | 유닛 테스트 |
| B-QG7 | 빌드 | kernel + userlib 전체 빌드 성공 | CI |

---

## Sub-Phase 7-1.C: PWA ↔ 커널 통합 (PWA-Kernel Bridge)

### 목적

`kpio-browser/src/pwa/`에 이미 구현된 PWA 매니페스트 파서·설치 매니저·앱 모델을 **커널 앱 관리 모듈(7-1.A)**과 연결하여, PWA 설치가 커널 레벨 앱 등록으로 이어지도록 한다.

### 선행 조건
- 7-1.A 퀄리티 게이트 전체 통과
- 7-1.B 퀄리티 게이트 B-QG1, B-QG5 통과

### 작업

#### C-1. PWA 설치 브릿지 (`kpio-browser/src/pwa/kernel_bridge.rs`)
- [ ] `pwa_install_to_kernel(manifest: &WebAppManifest) → Result<KernelAppId>`:
  1. `manifest.name` + `manifest.start_url` 추출
  2. 아이콘 데이터 다운로드 (manifest.icons[0].src)
  3. `AppInstall` 시스콜 호출 → `KernelAppId` 수신
  4. 매니페스트 전체를 `/apps/data/{id}/manifest.json`에 저장
  5. 아이콘 바이트를 `/apps/data/{id}/icon.png`에 저장
- [ ] `pwa_uninstall_from_kernel(app_id: KernelAppId) → Result<()>`:
  1. `AppUninstall` 시스콜 호출
  2. `PwaManager::uninstall()` 호출
- [ ] `PwaManager::install()` 수정: 기존 로직 끝에 `pwa_install_to_kernel()` 추가
- [ ] `PwaManager::uninstall()` 수정: `pwa_uninstall_from_kernel()` 추가

#### C-2. PWA 실행 브릿지
- [ ] `pwa_launch_from_kernel(app_id: KernelAppId) → Result<String>`:
  1. `/apps/data/{id}/manifest.json` 로드
  2. `start_url` 추출
  3. `AppLaunch` 시스콜 호출 → `instance_id`
  4. `start_url` 반환 (윈도우 생성은 7-1.D에서)
- [ ] PWA 종료 시 `AppTerminate` 시스콜 호출

#### C-3. 기존 `InstalledApp` ↔ `KernelAppDescriptor` 동기화
- [ ] `InstalledApp`에 `kernel_app_id: Option<KernelAppId>` 필드 추가
- [ ] 부팅 시 `PwaManager`와 `AppRegistry` 동기화:
  - 커널에 등록된 WebApp 타입 → `PwaManager`에 로드
  - 불일치 시 커널 레지스트리 기준으로 복구
- [ ] 동기화 함수: `sync_pwa_registry() → Result<SyncReport>`

#### C-4. 설치 가능성 판별 강화
- [ ] 브라우저 네비게이션 시 `<link rel="manifest">` 감지
- [ ] 설치 조건 검증:
  - HTTPS (또는 `kpio://`) 오리진
  - 유효한 `manifest.json` (name + start_url 필수)
  - Service Worker 등록 여부 (존재하면 offline_capable = true)
- [ ] 설치 가능 시 커널 GUI에 "설치" 버튼 활성화 신호 전달

### 산출물
- `kpio-browser/src/pwa/kernel_bridge.rs` 신규
- `kpio-browser/src/pwa/mod.rs` 수정
- `kpio-browser/src/pwa/install.rs` 수정

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| C-QG1 | PWA 설치 → 커널 등록 | `PwaManager::install(manifest)` → `AppRegistry::get(id)` 성공 | 통합 테스트 |
| C-QG2 | PWA 제거 → 커널 해제 | 제거 후 `AppRegistry::get(id)` → `None` | 통합 테스트 |
| C-QG3 | 매니페스트 영속화 | 설치 후 `/apps/data/{id}/manifest.json` 존재, JSON 파싱 성공 | 통합 테스트 |
| C-QG4 | 부팅 시 동기화 | 레지스트리에 WebApp 2개 등록 → 부팅 후 `PwaManager.installed_apps().len() == 2` | 통합 테스트 |
| C-QG5 | 이중 설치 방지 | 동일 `scope`로 재설치 시도 → `AlreadyRegistered` 에러 | 유닛 테스트 |
| C-QG6 | 매니페스트 감지 | `<link rel="manifest" href="/app.json">` 포함 HTML → 설치 가능 판별 | 유닛 테스트 |

---

## Sub-Phase 7-1.D: 윈도우 & 데스크톱 통합 (Window & Desktop Integration)

### 목적

커널 GUI에 **PWA 전용 윈도우 모드**를 추가하고, 데스크톱에 **동적 앱 아이콘**을 렌더링하여, 설치된 웹 앱이 시각적으로 네이티브 앱과 구별 불가능하게 만든다.

### 선행 조건
- 7-1.C 퀄리티 게이트 C-QG1, C-QG3 통과

### 작업

#### D-1. `WindowContent::WebApp` 변형 추가 (`kernel/src/gui/window.rs`)
- [ ] `WindowContent` enum 확장:
  ```rust
  WebApp {
      app_id: KernelAppId,
      url: String,
      content: String,
      rendered: Option<RenderedPage>,
      display_mode: DisplayMode,  // Standalone | MinimalUi | Fullscreen
      theme_color: Option<u32>,   // ARGB
      scope: String,              // 네비게이션 제한 범위
  }
  ```
- [ ] `Window::new_webapp(id, app_id, manifest, x, y)` 팩토리 메서드:
  - `display_mode` 에 따른 윈도우 크롬 결정:
    - `Standalone`: 타이틀바만 (주소바 없음), `theme_color` 적용
    - `MinimalUi`: 타이틀바 + 뒤로/앞으로/새로고침 미니 버튼
    - `Fullscreen`: 크롬 없음, 전체 화면
  - 시작 크기: 800×600 (이전 세션 기억 크기 우선)
- [ ] 윈도우 타이틀 → `manifest.short_name || manifest.name`
- [ ] 타이틀바 배경색 → `manifest.theme_color`
- [ ] 스코프 밖 URL 네비게이션 시 → 별도 브라우저 윈도우에서 열기

#### D-2. PWA 스플래시 스크린 (`kernel/src/gui/splash.rs`)
- [ ] `render_splash(window, manifest)`:
  - `background_color`로 전체 윈도우 채움
  - 중앙에 앱 아이콘 렌더링 (비트맵 스케일링)
  - 아이콘 아래 `manifest.name` 텍스트
  - `start_url` 로딩 완료 시 자동 닫힘 (최대 3초 타임아웃)

#### D-3. 동적 데스크톱 아이콘 (`kernel/src/gui/desktop.rs`)
- [ ] `IconType` enum 확장:
  ```rust
  pub enum IconType {
      Files, Browser, Terminal, Settings, Trash,
      InstalledApp { app_id: KernelAppId, icon_data: Option<Vec<u8>> },
  }
  ```
- [ ] `Desktop::refresh_app_icons()`:
  - `AppRegistry::list()` 에서 WebApp 타입 조회
  - 시스템 아이콘(5개) 뒤에 설치된 앱 아이콘 동적 배치
  - 아이콘 그리드 계산 (가로 5열 기준)
- [ ] 신규 앱 아이콘 렌더링:
  - `icon_data` 있으면 → PNG 비트맵 디코딩 및 32×32 스케일 렌더링
  - `icon_data` 없으면 → 이름 첫 글자 + 색상 원형 기본 아이콘
- [ ] 아이콘 클릭 → `AppLifecycle::launch(app_id)` → PWA 윈도우 생성
- [ ] 아이콘 우클릭 → 컨텍스트 메뉴 (실행, 제거)

#### D-4. 태스크바 연동 (`kernel/src/gui/taskbar.rs`)
- [ ] 실행 중인 WebApp → 태스크바에 아이콘 + 이름 표시
- [ ] `theme_color` 기반 액센트 색상 태스크바 항목
- [ ] 태스크바 항목 클릭 → 윈도우 포커스/최소화 토글
- [ ] 태스크바에 "모든 앱" 버튼 → 앱 런처 열기

#### D-5. 창 크기/위치 영속 (`kernel/src/app/window_state.rs`)
- [ ] `WindowStateStore`:
  - `save(app_id, x, y, width, height)`
  - `load(app_id) → Option<(i32, i32, u32, u32)>`
- [ ] VFS 영속: `/apps/data/{app_id}/window_state.json`
- [ ] 윈도우 이동/리사이즈 종료 시 자동 저장

### 산출물
- `kernel/src/gui/window.rs` 수정
- `kernel/src/gui/desktop.rs` 수정
- `kernel/src/gui/taskbar.rs` 수정
- `kernel/src/gui/splash.rs` 신규
- `kernel/src/app/window_state.rs` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| D-QG1 | WebApp 윈도우 | `Window::new_webapp()` 생성 → 주소바 없는 독립 윈도우 렌더링 | QEMU 시각 검증 |
| D-QG2 | theme_color | `#2196F3` 테마 → 타이틀바 파란색 렌더링 | QEMU 시각 검증 |
| D-QG3 | 동적 아이콘 | 앱 2개 설치 → 데스크톱에 7개 아이콘(시스템5 + 앱2) | QEMU 시각 검증 |
| D-QG4 | 아이콘 클릭 실행 | 데스크톱 앱 아이콘 클릭 → WebApp 윈도우 생성 | QEMU 기능 검증 |
| D-QG5 | 앱 제거 후 아이콘 | 앱 제거 → 데스크톱 아이콘 즉시 사라짐 | QEMU 시각 검증 |
| D-QG6 | 스코프 제한 | WebApp 내 외부 URL 클릭 → 브라우저 윈도우에서 열림 | 기능 테스트 |
| D-QG7 | 태스크바 표시 | WebApp 실행 → 태스크바에 앱 이름 + 테마 색상 표시 | QEMU 시각 검증 |
| D-QG8 | 스플래시 | WebApp 실행 즉시 → `background_color` + 아이콘 스플래시 표시 (≤3초 후 전환) | QEMU 시각 검증 |
| D-QG9 | 윈도우 크기 기억 | 윈도우 리사이즈 → 닫기 → 재실행 → 이전 크기로 복원 | 기능 테스트 |

---

## Sub-Phase 7-1.E: Service Worker 연동 (Service Worker Integration)

### 목적

`runtime/src/service_worker/` 모듈을 실제 웹 앱의 **fetch 인터셉트**, **캐시 관리**, **오프라인 동작** 파이프라인에 연결한다.

### 선행 조건
- 7-1.C 퀄리티 게이트 C-QG1 통과 (PWA가 커널에 등록 가능)
- Phase 6.3 JS 엔진 기본 동작 (또는 캐시 전용 모드로 우회)

### 작업

#### E-1. SW ↔ 브라우저 이벤트 파이프라인 (`kpio-browser/src/pwa/sw_bridge.rs`)
- [ ] `ServiceWorkerBridge` 구조체:
  - `register(scope: &str, script_url: &str) → Result<ServiceWorkerId>`
  - `unregister(scope: &str) → Result<()>`
  - `get_registration(scope: &str) → Option<ServiceWorkerRegistration>`
- [ ] `navigator.serviceWorker.register()` JS API → `ServiceWorkerBridge::register()` 매핑
- [ ] SW 스크립트 다운로드 → VFS `/apps/cache/{app_id}/sw.js`에 저장
- [ ] SW 상태 변경 이벤트 → 브라우저로 전파 (statechange)

#### E-2. Fetch 인터셉트 파이프라인
- [ ] `FetchInterceptor`:
  1. 웹 앱의 네트워크 요청 발생
  2. 매칭되는 active SW 검색 (`scope` 기준)
  3. SW에 `FetchEvent` 디스패치
  4. SW 응답 대기 (타임아웃 5초):
     - `event.respondWith(response)` → 캐시/커스텀 응답 사용
     - 타임아웃/미처리 → 네트워크로 직접 요청 (fallback)
- [ ] **캐시 전용 모드** (JS 엔진 미완 시 폴백):
  - SW JS 실행 불가 시 → URL 패턴 매칭 기반 캐시 전략
  - `sw_cache_config.json`: `{ patterns: [{ url: "/**/*.css", strategy: "cache-first" }] }`
  - 매칭 시 캐시 히트 → 캐시 응답, 미스 → 네트워크 요청
- [ ] fetch 이벤트 로깅 (디버그용): 요청 URL, 캐시 히트/미스, SW 응답 여부

#### E-3. Cache Storage API 구현
- [ ] `CacheStorage` (글로벌 `caches` 객체):
  - `open(cache_name) → Cache`
  - `has(cache_name) → bool`
  - `delete(cache_name) → bool`
  - `keys() → Vec<String>`
- [ ] `Cache`:
  - `put(request, response)` — URL + 응답 바디 저장
  - `match(request) → Option<Response>` — URL 매칭 조회
  - `delete(request) → bool`
  - `keys() → Vec<Request>`
- [ ] VFS 기반 영속화:
  - 저장 경로: `/apps/cache/{app_id}/{cache_name}/`
  - 메타데이터: `_meta.json` (URL → 파일명 매핑)
  - 응답 바디: `{hash}.body` (바이너리)
  - 응답 헤더: `{hash}.headers` (JSON)
- [ ] 쿼터 관리: 앱당 캐시 용량 상한 25MB (초과 시 LRU 축출)

#### E-4. SW 라이프사이클 이벤트 구현
- [ ] `install` 이벤트:
  - `event.waitUntil(promise)` — 프리캐시 완료 대기
  - 프리캐시 목록 다운로드 → Cache Storage에 저장
- [ ] `activate` 이벤트:
  - `event.waitUntil(promise)` — 이전 캐시 정리
  - `caches.delete(old_cache_name)` 처리
- [ ] SW 업데이트:
  - 바이트 비교: 기존 SW ≠ 새 SW → 업데이트 발동
  - `waiting` 상태 → `skipWaiting()` 호출 시 즉시 활성화
  - `clients.claim()` → 기존 클라이언트 즉시 제어

### 산출물
- `kpio-browser/src/pwa/sw_bridge.rs` 신규
- `kpio-browser/src/pwa/cache_storage.rs` 신규
- `kpio-browser/src/pwa/fetch_interceptor.rs` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| E-QG1 | SW 등록 | `register("/", "/sw.js")` → `ServiceWorkerId` 반환. 상태 `Activated` 도달 | 유닛 테스트 |
| E-QG2 | 캐시 저장/조회 | `cache.put("/style.css", body)` → `cache.match("/style.css")` → 동일 바디 | 유닛 테스트 |
| E-QG3 | 캐시 VFS 영속 | 캐시 put → VFS에 파일 생성 확인 → 재부팅 후 `cache.match()` 성공 | 통합 테스트 |
| E-QG4 | 캐시 전용 모드 | 캐시에 `/index.html` 저장 → 네트워크 차단 → 캐시 응답 반환 | 통합 테스트 |
| E-QG5 | 쿼터 강제 | 25MB 초과 캐시 put → LRU 축출 후 총량 ≤ 25MB | 유닛 테스트 |
| E-QG6 | SW 업데이트 | SW 스크립트 변경 → 새 SW installed → skipWaiting → active 전이 | 유닛 테스트 |
| E-QG7 | Fetch 폴백 | SW 응답 없음(타임아웃) → 네트워크 직접 요청으로 폴백 | 통합 테스트 |

---

## Sub-Phase 7-1.F: 웹 스토리지 엔진 (Web Storage Engine)

### 목적

PWA의 데이터 영속화를 위한 **Web Storage API** (localStorage, sessionStorage) 및 **IndexedDB** 기본 구현을 제공한다.

### 선행 조건
- 7-1.B 퀄리티 게이트 B-QG3 통과 (VFS 샌드박스 동작)

### 작업

#### F-1. Web Storage API (`kpio-browser/src/pwa/web_storage.rs`)
- [ ] `WebStorage` 구조체:
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
- [ ] `localStorage`: VFS 영속 (`/apps/storage/{app_id}/local_storage.json`)
- [ ] `sessionStorage`: 메모리 전용, 앱 종료 시 소멸
- [ ] 용량 제한: 키+값 합계 5MB 초과 시 `QuotaExceededError`
- [ ] `storage` 이벤트: 다른 탭/윈도우에 변경 알림 (`StorageEvent`)

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
  - `oncomplete`, `onerror`, `onabort` 이벤트
- [ ] `IDBIndex`:
  - `get(key) → IDBRequest`
  - `get_all(query?, count?) → IDBRequest`
  - `count() → IDBRequest`
- [ ] `IDBCursor`:
  - `continue()`, `advance(count)`
  - `update(value)`, `delete()`
  - `direction`: Next | Prev | NextUnique | PrevUnique

#### F-3. IndexedDB 저장소 엔진 (`kpio-browser/src/pwa/idb_engine.rs`)
- [ ] B-Tree 기반 키-값 저장소:
  - 키: 정렬 가능한 `IDBKey` (Number | String | Date | Binary | Array)
  - 값: JSON 직렬화된 JS 값
  - 인덱스: key_path 기반 보조 B-Tree
- [ ] VFS 영속화:
  - 데이터베이스 경로: `/apps/storage/{app_id}/idb/{db_name}/`
  - 오브젝트 스토어: `{store_name}.kvidb` (커스텀 바이너리 포맷)
  - 메타데이터: `_schema.json` (스토어 목록, 인덱스 정보, 버전)
- [ ] 트랜잭션 격리:
  - Readwrite: 스토어 단위 `Mutex` 잠금
  - Readonly: 동시 다중 접근 허용
  - Versionchange: 전체 DB 배타 잠금
- [ ] 앱당 쿼터: 50MB (localStorage 5MB + IndexedDB 45MB)

### 산출물
- `kpio-browser/src/pwa/web_storage.rs` 신규
- `kpio-browser/src/pwa/indexed_db.rs` 신규
- `kpio-browser/src/pwa/idb_engine.rs` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| F-QG1 | localStorage CRUD | `setItem("k","v")` → `getItem("k")` = `"v"` → `removeItem("k")` → `getItem("k")` = None | 유닛 테스트 |
| F-QG2 | localStorage 영속 | `setItem` → 앱 종료 → 앱 재실행 → `getItem` 동일 값 | 통합 테스트 |
| F-QG3 | sessionStorage 비영속 | `setItem` → 앱 종료 → 앱 재실행 → `getItem` = None | 통합 테스트 |
| F-QG4 | 용량 제한 | 5MB 초과 `setItem` → `QuotaExceededError` | 유닛 테스트 |
| F-QG5 | IDB 기본 CRUD | `objectStore.put({name:"test"}, 1)` → `get(1)` → `{name:"test"}` | 유닛 테스트 |
| F-QG6 | IDB 트랜잭션 | readwrite 트랜잭션 내 `put` 2회 → `commit` → `getAll` = 2건 | 유닛 테스트 |
| F-QG7 | IDB abort | 트랜잭션 내 `put` → `abort` → `get` = None (롤백됨) | 유닛 테스트 |
| F-QG8 | IDB 인덱스 | `createIndex("byName", "name")` → 인덱스 기반 `get("test")` 성공 | 유닛 테스트 |
| F-QG9 | IDB 영속 | `put` → 앱 종료 → 재실행 → `get` 동일 데이터 | 통합 테스트 |
| F-QG10 | 쿼터 통합 | localStorage 3MB + IDB 47MB → 다음 write 시 QuotaExceeded | 통합 테스트 |

---

## Sub-Phase 7-1.G: 알림 & 백그라운드 동기화 (Notifications & Background Sync)

### 목적

PWA의 사용자 참여(engagement) 기능인 **푸시 알림**과 **백그라운드 동기화**를 커널 GUI에 통합한다.

### 선행 조건
- 7-1.D 퀄리티 게이트 D-QG1 통과 (WebApp 윈도우 존재)
- 7-1.B 퀄리티 게이트 B-QG1 통과 (앱 시스콜 동작)

### 작업

#### G-1. 커널 알림 센터 (`kernel/src/gui/notification.rs`)
- [ ] `Notification` 구조체:
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
      pub action_url: Option<String>,  // 클릭 시 이동할 URL
  }
  ```
- [ ] `NotificationCenter`:
  - `show(notification) → NotificationId` — 토스트 표시 큐
  - `dismiss(id)` — 토스트 닫기
  - `list_unread() → Vec<&Notification>`
  - `mark_read(id)`
  - `clear_all()`
- [ ] 글로벌 인스턴스: `NOTIFICATION_CENTER: Mutex<NotificationCenter>`
- [ ] 알림 이력: 최근 50건 보관, FIFO 축출

#### G-2. 토스트 렌더링 (`kernel/src/gui/toast.rs`)
- [ ] 토스트 위치: 화면 우상단, 위에서 아래로 큐잉 (최대 3개 동시)
- [ ] 토스트 레이아웃:
  ```
  ┌────────────────────────────────┐
  │ [앱 아이콘] 앱 이름    ✕ (닫기) │
  │ 알림 제목 (볼드)                │
  │ 본문 텍스트 (최대 2줄)          │
  └────────────────────────────────┘
  ```
- [ ] 자동 사라짐: 5초 후 페이드 아웃 (또는 닫기 클릭)
- [ ] 클릭 시 동작:
  - `action_url` 있으면 → 해당 앱 윈도우 포커스 + URL 네비게이션
  - 없으면 → 앱 윈도우 단순 포커스
- [ ] 렌더링 z-order: 모든 윈도우 위 (항상 최상위)

#### G-3. Notification API 연동 (`kpio-browser/src/pwa/notification_bridge.rs`)
- [ ] `Notification.requestPermission()` → 사용자 승인 다이얼로그:
  - "앱이름이(가) 알림을 보내려고 합니다" + [허용] [차단] 버튼
  - 결과 영속화 → `/system/apps/permissions/{app_id}.json`
- [ ] `new Notification(title, { body, icon })` → `NotificationCenter::show()` 호출
- [ ] `notification.onclick` → `action_url` 기반 이벤트 디스패치
- [ ] `notification.close()` → `NotificationCenter::dismiss()` 호출

#### G-4. 백그라운드 동기화 (`kpio-browser/src/pwa/background_sync.rs`)
- [ ] `SyncManager`:
  - `register(tag) → Result<()>` — 동기화 태스크 등록
  - `get_tags() → Vec<String>` — 등록된 태그 목록
- [ ] 네트워크 상태 감시:
  - `kernel/src/net/` 네트워크 연결 상태 폴링 (5초 간격)
  - 오프라인 → 온라인 전환 감지
- [ ] 온라인 복귀 시:
  - 등록된 `sync` 태스크 → SW에 `sync` 이벤트 디스패치
  - 이벤트 처리 실패 시 → 백오프 재시도 (30초, 60초, 300초)
  - 최대 3회 재시도 후 폐기
- [ ] 태스크 영속화: `/apps/data/{app_id}/sync_tasks.json`

#### G-5. 알림 관리 UI (`kernel/src/gui/notification_panel.rs`)
- [ ] 태스크바 알림 아이콘 (벨 모양):
  - 미읽은 알림 있으면 → 빨간 뱃지 (숫자)
  - 클릭 → 알림 패널 토글
- [ ] 알림 패널:
  - 최근 알림 목록 (앱별 그룹핑)
  - 각 알림: 제목 + 본문 + 시간 + 앱 이름
  - "모두 읽음" 버튼
  - "앱별 알림 설정" 링크 → 설정 앱

### 산출물
- `kernel/src/gui/notification.rs` 신규
- `kernel/src/gui/toast.rs` 신규
- `kernel/src/gui/notification_panel.rs` 신규
- `kpio-browser/src/pwa/notification_bridge.rs` 신규
- `kpio-browser/src/pwa/background_sync.rs` 신규

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| G-QG1 | 토스트 렌더링 | `NotificationCenter::show()` → 우상단에 토스트 표시 (제목+본문) | QEMU 시각 검증 |
| G-QG2 | 자동 사라짐 | 토스트 표시 → 5초 후 자동 제거 | QEMU 시각 검증 (타이머) |
| G-QG3 | 동시 큐잉 | 3건 연속 show → 3개 토스트 수직 배치 | QEMU 시각 검증 |
| G-QG4 | 토스트 클릭 | 토스트 클릭 → 해당 앱 윈도우 포커스 | QEMU 기능 검증 |
| G-QG5 | 권한 요청 | 미허용 앱의 알림 → 승인 다이얼로그 표시 | QEMU 시각 검증 |
| G-QG6 | 권한 차단 | 차단된 앱의 `Notification()` → 무시 (토스트 없음) | 기능 테스트 |
| G-QG7 | 알림 이력 | 10건 show → `list_unread().len() == 10` | 유닛 테스트 |
| G-QG8 | 알림 패널 | 벨 아이콘 클릭 → 알림 패널에 알림 목록 표시 | QEMU 시각 검증 |
| G-QG9 | 백그라운드 동기화 | sync 등록 → 네트워크 차단 → 복구 → `sync` 이벤트 발생 | 통합 테스트 |
| G-QG10 | 재시도 제한 | sync 이벤트 처리 실패 3회 → 태스크 폐기 | 유닛 테스트 |

---

## Sub-Phase 7-1.H: 종합 검증 & 데모 앱 (E2E Validation & Demo Apps)

### 목적

Phase 7-1 전체 파이프라인을 **엔드투엔드로 검증**하고, 실제 동작하는 **데모 PWA 2개**를 제작하여 웹 앱 플랫폼의 완성도를 증명한다.

### 선행 조건
- 7-1.A ~ 7-1.G 전체 퀄리티 게이트 통과

### 작업

#### H-1. 데모 PWA #1: KPIO Notes (메모 앱)
- [ ] 단일 페이지 웹 앱:
  - HTML: 텍스트 에어리어 + 저장 버튼 + 메모 목록
  - CSS: 미니멀 디자인, `theme_color: #4CAF50` (녹색)
  - JS: localStorage 기반 메모 CRUD
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
- [ ] Service Worker: Cache First 전략 (HTML/CSS/JS 오프라인 가용)
- [ ] 검증 시나리오:
  1. 브라우저에서 `/notes/` 접속
  2. "설치" 버튼 → 설치 완료
  3. 데스크톱에 녹색 Notes 아이콘 출현
  4. 아이콘 클릭 → standalone 윈도우 열림 (주소바 없음, 녹색 타이틀바)
  5. 메모 작성 → localStorage 저장
  6. 앱 종료 → 재실행 → 메모 유지
  7. 네트워크 차단 → 앱 정상 동작 (오프라인)

#### H-2. 데모 PWA #2: KPIO Weather (날씨 앱)
- [ ] 단일 페이지 웹 앱:
  - HTML: 날씨 카드 + 도시 선택
  - CSS: 그라데이션 배경, `theme_color: #2196F3` (파란색)
  - JS: fetch API로 날씨 데이터 요청 (모의 API)
- [ ] Service Worker: Network First 전략 (최신 데이터 우선, 오프라인 시 캐시)
- [ ] 알림: "기온 변화 알림" 데모 (Notification API)
- [ ] Background Sync: 오프라인 중 "도시 추가" → 온라인 복귀 시 자동 동기화
- [ ] 검증 시나리오:
  1. 설치 → 파란색 Weather 아이콘 출현
  2. 날씨 데이터 로드 → 캐시
  3. "기온 알림" 허용 → 토스트 알림 수신
  4. 네트워크 차단 → 캐시된 날씨 표시
  5. 네트워크 복귀 → 자동 갱신 + sync 이벤트 처리

#### H-3. E2E 테스트 스위트 (`tests/e2e/pwa/`)
- [ ] `test_pwa_install_uninstall.rs`:
  - PWA 설치 → 커널 레지스트리 확인 → 데스크톱 아이콘 확인 → 제거 → 정리 확인
- [ ] `test_pwa_offline.rs`:
  - SW 등록 → 리소스 캐시 → 네트워크 차단 → 페이지 로드 성공
- [ ] `test_pwa_storage.rs`:
  - localStorage CRUD → 앱 재시작 → 데이터 영속 확인
  - IndexedDB CRUD → 트랜잭션 커밋/롤백 → 영속 확인
- [ ] `test_pwa_notification.rs`:
  - 권한 요청 → 허용 → 알림 표시 → 클릭 → 앱 포커스
- [ ] `test_pwa_lifecycle.rs`:
  - launch → running → suspend → resume → terminate 전체 주기
- [ ] `test_pwa_multi_instance.rs`:
  - 동일 앱 2회 실행 → 별도 윈도우 → 별도 instance_id → 1개 종료 시 다른 것 유지

#### H-4. 성능 벤치마크
- [ ] PWA 설치 소요 시간: **목표 < 2초** (매니페스트 파싱 + 아이콘 저장 + 레지스트리 기록)
- [ ] PWA 실행 소요 시간 (콜드 스타트): **목표 < 1초** (윈도우 생성 + 스플래시 + start_url 로드)
- [ ] localStorage `setItem` 지연: **목표 < 5ms** (키 100바이트 + 값 1KB)
- [ ] Cache API `match` 지연: **목표 < 10ms** (1MB 캐시 엔트리)
- [ ] 알림 토스트 렌더링 지연: **목표 < 16ms** (1프레임 이내)

#### H-5. 문서화
- [ ] `docs/phase7/WEB_APP_DEVELOPER_GUIDE.md`:
  - KPIO에서 PWA 개발하기 (매니페스트 작성, SW 등록, 오프라인 전략)
  - 지원/미지원 Web API 목록
  - 제한 사항 (쿼터, 권한)
- [ ] `docs/phase7/WEB_APP_ARCHITECTURE.md`:
  - 내부 아키텍처 다이어그램
  - 컴포넌트 간 데이터 흐름
  - VFS 디렉토리 레이아웃

### 산출물
- `examples/pwa-notes/` — Notes 데모 앱 (HTML/CSS/JS/manifest/SW)
- `examples/pwa-weather/` — Weather 데모 앱
- `tests/e2e/pwa/` — 6개 E2E 테스트
- `docs/phase7/WEB_APP_DEVELOPER_GUIDE.md`
- `docs/phase7/WEB_APP_ARCHITECTURE.md`

### 퀄리티 게이트

| # | 검증 항목 | 통과 기준 | 검증 방법 |
|---|----------|----------|----------|
| H-QG1 | Notes 전체 흐름 | 설치 → 데스크톱 아이콘 → standalone 실행 → 메모 저장 → 오프라인 동작 | QEMU E2E 수동 검증 |
| H-QG2 | Weather 전체 흐름 | 설치 → 날씨 로드 → 알림 → 오프라인 캐시 → 온라인 복귀 동기화 | QEMU E2E 수동 검증 |
| H-QG3 | E2E 테스트 통과 | 6개 E2E 테스트 전부 통과 (0 failures) | `cargo test --test e2e` |
| H-QG4 | 설치 성능 | PWA 설치 < 2초 (3회 평균) | 벤치마크 |
| H-QG5 | 콜드 스타트 성능 | PWA 실행 < 1초 (3회 평균) | 벤치마크 |
| H-QG6 | localStorage 성능 | setItem (1KB) < 5ms (100회 평균) | 벤치마크 |
| H-QG7 | 다중 인스턴스 | 동일 PWA 2개 윈도우 독립 동작 | E2E 테스트 |
| H-QG8 | 개발자 문서 | 가이드 문서 기반으로 신규 데모 PWA 작성 가능 (self-contained) | 문서 리뷰 |
| H-QG9 | 0 panic | QEMU에서 30분 연속 사용 시 커널 패닉 없음 | 안정성 테스트 |

---

## Phase 7-1 전체 완료 기준 (Exit Criteria)

Phase 7-1이 완료되었다고 선언하려면 아래 **모든 조건**이 충족되어야 한다:

### 필수 (Must Pass)
1. ✅ 서브페이즈 A~H의 모든 퀄리티 게이트 통과 (**56개 항목**)
2. ✅ 데모 PWA 2개 (Notes, Weather) 설치→실행→오프라인→알림 전체 동작
3. ✅ `cargo build --target x86_64-kpio` 경고 0건으로 빌드 성공
4. ✅ `cargo test` (호스트) 전체 통과
5. ✅ E2E 테스트 6건 전체 통과
6. ✅ QEMU에서 30분 연속 사용 시 커널 패닉 없음

### 바람직 (Should Pass)
7. 🔶 성능 벤치마크 5개 항목 중 4개 이상 목표치 충족
8. 🔶 개발자 가이드 문서 작성 완료
9. 🔶 RELEASE_NOTES.md에 Phase 7-1 변경 사항 기록

### 선택 (Nice to Have)
10. ⬜ 서드파티 PWA (예: 간단한 Todo MVC 앱) KPIO에서 설치·실행 성공
11. ⬜ IndexedDB 커서(cursor) 순방향/역방향 이터레이션 완전 동작

---

## 아키텍처 다이어그램: 전체 데이터 흐름

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

## 신규/수정 파일 총목록

### 신규 파일 (20개)
| 파일 | 서브페이즈 |
|------|-----------|
| `kernel/src/app/mod.rs` | A |
| `kernel/src/app/registry.rs` | A |
| `kernel/src/app/lifecycle.rs` | A |
| `kernel/src/app/permissions.rs` | A |
| `kernel/src/app/error.rs` | A |
| `kernel/src/app/window_state.rs` | D |
| `kernel/src/vfs/sandbox.rs` | B |
| `kernel/src/gui/splash.rs` | D |
| `kernel/src/gui/notification.rs` | G |
| `kernel/src/gui/toast.rs` | G |
| `kernel/src/gui/notification_panel.rs` | G |
| `userlib/src/app.rs` | B |
| `kpio-browser/src/pwa/kernel_bridge.rs` | C |
| `kpio-browser/src/pwa/sw_bridge.rs` | E |
| `kpio-browser/src/pwa/cache_storage.rs` | E |
| `kpio-browser/src/pwa/fetch_interceptor.rs` | E |
| `kpio-browser/src/pwa/web_storage.rs` | F |
| `kpio-browser/src/pwa/indexed_db.rs` | F |
| `kpio-browser/src/pwa/idb_engine.rs` | F |
| `kpio-browser/src/pwa/notification_bridge.rs` | G |
| `kpio-browser/src/pwa/background_sync.rs` | G |

### 수정 파일 (10개)
| 파일 | 서브페이즈 | 변경 내용 |
|------|-----------|----------|
| `kernel/src/lib.rs` (또는 main.rs) | A | `mod app;` 추가 |
| `kernel/src/syscall/mod.rs` | B | 시스콜 106-111 추가 |
| `kernel/src/gui/window.rs` | D | `WindowContent::WebApp` variant |
| `kernel/src/gui/desktop.rs` | D | `IconType::InstalledApp`, 동적 아이콘 |
| `kernel/src/gui/taskbar.rs` | D | WebApp 태스크바 항목 |
| `kernel/src/gui/mod.rs` | D, G | 신규 서브모듈 export |
| `userlib/src/lib.rs` | B | `pub mod app;` 추가 |
| `kpio-browser/src/pwa/mod.rs` | C | kernel_bridge 연동 |
| `kpio-browser/src/pwa/install.rs` | C | kernel 등록 호출 추가 |
| `kernel/src/vfs/mod.rs` | B | sandbox 모듈 export |

---

*Phase 7-1 완료 시 KPIO OS는 PWA를 설치·실행·오프라인 캐시·알림·백그라운드 동기화까지 지원하는 완전한 웹 앱 플랫폼을 갖추게 된다.*
