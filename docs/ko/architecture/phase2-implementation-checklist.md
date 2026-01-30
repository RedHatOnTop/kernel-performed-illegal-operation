# Phase 2 구현 체크리스트

**버전:** 1.0  
**시작일:** 미정  
**예상 기간:** 20주

---

## Phase 2.1: Userspace 기반 인프라 (4주)

### Week 1-2: 프로세스 관리

#### 1.1 ELF 로더
- [ ] ELF64 헤더 파싱
- [ ] 프로그램 헤더 처리
- [ ] 섹션 로딩 (.text, .data, .bss, .rodata)
- [ ] 재배치 처리 (R_X86_64_*)
- [ ] 동적 링킹 지원 (선택적)

```rust
// kernel/src/loader/elf.rs
pub struct Elf64Loader {
    pub fn load(binary: &[u8]) -> Result<LoadedProgram, ElfError>;
    pub fn relocate(program: &mut LoadedProgram) -> Result<(), ElfError>;
}
```

#### 1.2 사용자 공간 메모리
- [ ] 사용자 공간 가상 주소 레이아웃
  - `0x0000_0000_0040_0000` - Text 시작
  - `0x0000_7FFF_FFFF_F000` - 스택 시작
- [ ] 페이지 테이블 분리 (커널/사용자)
- [ ] Copy-on-Write 포크

#### 1.3 시스템 콜 인터페이스
- [ ] syscall 진입점 (SYSCALL 명령어)
- [ ] 시스템 콜 테이블
- [ ] 인자 유효성 검사
- [ ] 사용자 공간 포인터 검증

```rust
// kernel/src/syscall/mod.rs
pub const SYS_EXIT: usize = 0;
pub const SYS_READ: usize = 1;
pub const SYS_WRITE: usize = 2;
pub const SYS_OPEN: usize = 3;
pub const SYS_CLOSE: usize = 4;
pub const SYS_MMAP: usize = 5;
pub const SYS_MUNMAP: usize = 6;
pub const SYS_THREAD_CREATE: usize = 7;
pub const SYS_THREAD_EXIT: usize = 8;
pub const SYS_FUTEX: usize = 9;
// ... 50+ syscalls 예상
```

#### 1.4 기본 프로세스 관리
- [ ] 프로세스 생성 (fork/exec 또는 spawn)
- [ ] 프로세스 종료 (exit, wait)
- [ ] 프로세스 ID 관리
- [ ] 프로세스 상태 머신

---

### Week 3-4: IPC 시스템

#### 2.1 공유 메모리
- [ ] `mmap` 시스템 콜 (MAP_SHARED)
- [ ] 공유 메모리 객체 생성/열기
- [ ] 물리 페이지 공유
- [ ] 참조 카운팅

```rust
// kernel/src/ipc/shm.rs
pub struct SharedMemory {
    pub fn create(name: &str, size: usize) -> Result<ShmId, Error>;
    pub fn open(name: &str) -> Result<ShmId, Error>;
    pub fn map(id: ShmId, offset: usize, size: usize) -> Result<*mut u8, Error>;
    pub fn unmap(addr: *mut u8, size: usize) -> Result<(), Error>;
}
```

#### 2.2 Ring Buffer IPC
- [ ] Lock-free SPSC 큐 구현
- [ ] Lock-free MPSC 큐 구현
- [ ] 이벤트 알림 (futex 기반)
- [ ] 흐름 제어 (백프레셔)

#### 2.3 Capability 시스템
- [ ] Capability ID 생성
- [ ] 권한 검사
- [ ] 권한 위임 (diminish)
- [ ] Capability 테이블

#### 2.4 브라우저-커널 채널
- [ ] `KernelToBrowser` 메시지 정의
- [ ] `BrowserToKernel` 메시지 정의
- [ ] 채널 초기화
- [ ] 양방향 통신 테스트

---

## Phase 2.2: Servo 포팅 (8주)

### Week 5-8: 최소 Servo 빌드

#### 3.1 크로스 컴파일 환경
- [ ] x86_64-unknown-kpio 타겟 정의
- [ ] rust-std 빌드 스크립트
- [ ] Cargo 설정 (.cargo/config.toml)

#### 3.2 libkpio (libc 대체)
- [ ] 기본 타입 정의 (size_t, ssize_t, ...)
- [ ] 문자열 함수 (strlen, memcpy, ...)
- [ ] 메모리 할당 (malloc, free, realloc)
- [ ] 환경 변수 스텁

```rust
// userspace/libkpio/src/lib.rs
#![no_std]
#![no_main]

pub mod alloc;
pub mod string;
pub mod syscall;
pub mod thread;
pub mod io;
pub mod net;
```

#### 3.3 std 기능 구현 (libkpio-std)

**필수 모듈:**
- [ ] `std::alloc` - GlobalAlloc 구현
- [ ] `std::thread` - 스레드 생성/조인
- [ ] `std::sync` - Mutex, Condvar, RwLock
- [ ] `std::fs` - File, OpenOptions
- [ ] `std::io` - Read, Write, Seek
- [ ] `std::net` - TcpStream, UdpSocket
- [ ] `std::time` - Instant, Duration
- [ ] `std::env` - 환경 변수

**선택 모듈 (나중에):**
- [ ] `std::process` - 프로세스 생성
- [ ] `std::os::unix` - Unix 확장

#### 3.4 Servo 컴파일 테스트
- [ ] html5ever 컴파일
- [ ] cssparser 컴파일
- [ ] 최소 servo 빌드 (console 로깅만)

---

### Week 9-12: 렌더링 파이프라인

#### 4.1 Vulkan 드라이버 (Userspace)
- [ ] VirtIO-GPU Vulkan 확장 조사
- [ ] Vulkan 로더 (libvulkan)
- [ ] VkInstance 생성
- [ ] VkDevice 생성
- [ ] VkQueue 획득

#### 4.2 surfman 포팅
- [ ] 플랫폼 백엔드 추가 (KPIO)
- [ ] Surface 생성
- [ ] 컨텍스트 관리

#### 4.3 WebRender 통합
- [ ] WebRender 빌드
- [ ] 기본 렌더링 테스트 (단색 사각형)
- [ ] 텍스트 렌더링
- [ ] 이미지 렌더링

#### 4.4 기본 페이지 렌더링
- [ ] about:blank 렌더링
- [ ] 간단한 HTML 페이지
- [ ] CSS 스타일 적용
- [ ] 이벤트 처리 (클릭)

---

## Phase 2.3: OS 통합 최적화 (8주)

### Week 13-16: GPU 스케줄러

#### 5.1 GPU 우선순위 큐
- [ ] 우선순위 레벨 정의 (High, Medium, Low)
- [ ] 탭별 큐 분리
- [ ] 선점형 스케줄링

```rust
// kernel/src/gpu/scheduler.rs
pub struct GpuScheduler {
    pub fn submit(&mut self, tab_id: TabId, cmds: GpuCommands, priority: Priority);
    pub fn set_tab_priority(&mut self, tab_id: TabId, priority: Priority);
    pub fn process_queue(&mut self);
}
```

#### 5.2 포그라운드 탭 부스팅
- [ ] 활성 탭 감지
- [ ] GPU 시간 할당 조정
- [ ] 프레임 데드라인 우선순위

#### 5.3 백그라운드 탭 스로틀링
- [ ] requestAnimationFrame 스로틀
- [ ] GPU 작업 지연
- [ ] 전력 절약 모드

#### 5.4 VSync 동기화
- [ ] 디스플레이 새로고침 속도 감지
- [ ] 프레임 제출 타이밍
- [ ] 티어링 방지

---

### Week 17-20: 메모리 최적화

#### 6.1 탭별 메모리 추적
- [ ] 프로세스별 메모리 사용량
- [ ] JS 힙 크기 추적
- [ ] 이미지 캐시 추적

#### 6.2 백그라운드 탭 압축
- [ ] LZ4 압축
- [ ] 압축 트리거 조건
- [ ] 복원 성능 최적화

```rust
// kernel/src/memory/tab_manager.rs
pub struct TabMemoryManager {
    pub fn compress_tab(&mut self, tab_id: TabId) -> Result<CompressedTab, Error>;
    pub fn restore_tab(&mut self, compressed: CompressedTab) -> Result<(), Error>;
    pub fn get_memory_usage(&self, tab_id: TabId) -> usize;
}
```

#### 6.3 휴면 탭 디스크 스왑
- [ ] 스왑 파일 관리
- [ ] 탭 상태 직렬화
- [ ] 복원 시간 최적화 (<1초)

#### 6.4 WASM AOT 캐시
- [ ] 모듈 해시 계산
- [ ] AOT 코드 저장/로드
- [ ] 캐시 무효화 정책
- [ ] 디스크 캐시

---

## 마일스톤

| 마일스톤 | 주차 | 검증 방법 |
|----------|------|----------|
| **M1: Hello Userspace** | 2주차 | ELF 바이너리 실행, "Hello" 출력 |
| **M2: IPC Works** | 4주차 | 두 프로세스 간 메시지 전달 |
| **M3: html5ever Runs** | 6주차 | HTML 파싱, DOM 출력 |
| **M4: Vulkan Triangle** | 10주차 | 삼각형 렌더링 |
| **M5: Basic Page** | 12주차 | 간단한 웹페이지 렌더링 |
| **M6: Google Loads** | 16주차 | google.com 로딩 (느려도 OK) |
| **M7: Performance** | 20주차 | 성능 목표 50% 달성 |

---

## 테스트 계획

### 단위 테스트
```rust
// kernel/src/loader/elf_tests.rs
#[test]
fn test_elf_header_parsing() { ... }

#[test]
fn test_section_loading() { ... }

#[test]
fn test_relocation() { ... }
```

### 통합 테스트
```bash
# tests/userspace/hello_world.sh
cargo build --target x86_64-unknown-kpio
./scripts/run-qemu.sh tests/hello_world.elf
# 예상 출력: "Hello from userspace!"
```

### 성능 테스트
```rust
// tests/benchmark/page_load.rs
#[bench]
fn bench_about_blank_load() { ... }

#[bench]
fn bench_google_load() { ... }

#[bench]
fn bench_memory_per_tab() { ... }
```

---

## 파일 구조 (예상)

```
kernel/
├── src/
│   ├── loader/
│   │   ├── mod.rs
│   │   └── elf.rs           # ELF 로더
│   ├── process/
│   │   ├── mod.rs
│   │   ├── manager.rs       # 프로세스 관리자
│   │   └── context.rs       # 컨텍스트 스위칭
│   ├── syscall/
│   │   ├── mod.rs
│   │   ├── table.rs         # 시스템 콜 테이블
│   │   ├── memory.rs        # mmap, munmap
│   │   ├── io.rs            # read, write
│   │   └── thread.rs        # 스레드 syscalls
│   ├── ipc/
│   │   ├── mod.rs
│   │   ├── shm.rs           # 공유 메모리
│   │   ├── channel.rs       # Ring buffer IPC
│   │   ├── capability.rs    # Capability 시스템
│   │   └── browser.rs       # 브라우저 채널
│   └── gpu/
│       ├── mod.rs
│       └── scheduler.rs     # GPU 스케줄러

userspace/
├── libkpio/                  # libc 대체
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── alloc.rs
│       ├── syscall.rs
│       └── ...
├── libkpio-std/              # std 포팅
│   ├── Cargo.toml
│   └── src/
│       └── ...
└── servo_browser/            # Servo 포팅
    ├── Cargo.toml
    └── src/
        ├── main.rs
        └── kpio_platform/    # KPIO 플랫폼 레이어
```

---

## 의존성 그래프

```
                    ┌──────────────┐
                    │ servo_browser│
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ Servo    │ │libkpio-  │ │ Vulkan   │
        │Components│ │std       │ │ Driver   │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │            │            │
             │            ▼            │
             │      ┌──────────┐       │
             │      │ libkpio  │       │
             │      └────┬─────┘       │
             │           │             │
             └───────────┼─────────────┘
                         │
                         ▼
              ┌────────────────────┐
              │   KPIO Kernel      │
              │  (syscall layer)   │
              └────────────────────┘
```

---

## 위험 완화 체크포인트

### 4주차 체크포인트
- [ ] ELF 로더 동작 확인?
- [ ] IPC 기본 동작 확인?
- **실패 시:** 기반 인프라 재설계

### 8주차 체크포인트  
- [ ] html5ever 컴파일 성공?
- [ ] 기본 파싱 동작?
- **실패 시:** Servo 대신 경량 HTML 파서 고려

### 12주차 체크포인트
- [ ] Vulkan 삼각형 렌더링 성공?
- [ ] 기본 페이지 표시?
- **실패 시:** 소프트웨어 렌더링 fallback

### 20주차 체크포인트
- [ ] 성능 목표 50% 달성?
- **실패 시:** 최적화 기간 연장 또는 목표 조정
