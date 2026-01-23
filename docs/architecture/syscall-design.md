# KPIO 시스템 콜 설계

**버전:** 1.0  
**상태:** 설계 중

---

## 개요

KPIO의 시스템 콜 인터페이스는 Servo 브라우저 실행에 필요한 기능을 제공합니다.
POSIX 호환성을 부분적으로 유지하면서 OS-레벨 최적화를 위한 확장을 포함합니다.

---

## 시스템 콜 호출 규약

### x86_64 ABI

```
레지스터 사용:
- RAX: 시스템 콜 번호
- RDI: 인자 1
- RSI: 인자 2
- RDX: 인자 3
- R10: 인자 4
- R8:  인자 5
- R9:  인자 6

반환값:
- RAX: 결과 또는 에러 코드 (음수)

호출 방법:
- SYSCALL 명령어 사용
```

---

## 시스템 콜 테이블

### 1. 프로세스 관리 (0-19)

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 0 | `sys_exit` | code: i32 | 프로세스 종료 |
| 1 | `sys_fork` | - | 프로세스 복제 |
| 2 | `sys_exec` | path, argv, envp | 새 프로그램 실행 |
| 3 | `sys_wait` | pid, status, options | 자식 대기 |
| 4 | `sys_getpid` | - | PID 반환 |
| 5 | `sys_getppid` | - | 부모 PID 반환 |
| 6 | `sys_kill` | pid, signal | 시그널 전송 |
| 7 | `sys_yield` | - | CPU 양보 |

```rust
/// 프로세스 종료
pub fn sys_exit(code: i32) -> ! {
    // 절대 반환하지 않음
}

/// 프로세스 복제 (Copy-on-Write)
pub fn sys_fork() -> Result<Pid, SyscallError> {
    // 자식: 0 반환
    // 부모: 자식 PID 반환
}
```

### 2. 메모리 관리 (20-39)

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 20 | `sys_mmap` | addr, len, prot, flags, fd, offset | 메모리 매핑 |
| 21 | `sys_munmap` | addr, len | 매핑 해제 |
| 22 | `sys_mprotect` | addr, len, prot | 보호 속성 변경 |
| 23 | `sys_brk` | addr | 힙 확장 |
| 24 | `sys_madvise` | addr, len, advice | 메모리 힌트 |

```rust
/// 메모리 매핑
pub fn sys_mmap(
    addr: Option<*mut u8>,  // 힌트 주소 (NULL이면 커널이 선택)
    len: usize,
    prot: ProtFlags,        // PROT_READ | PROT_WRITE | PROT_EXEC
    flags: MapFlags,        // MAP_PRIVATE | MAP_SHARED | MAP_ANON
    fd: Option<Fd>,         // 파일 매핑인 경우
    offset: u64,
) -> Result<*mut u8, SyscallError>;

bitflags! {
    pub struct ProtFlags: u32 {
        const READ  = 1 << 0;
        const WRITE = 1 << 1;
        const EXEC  = 1 << 2;
    }
    
    pub struct MapFlags: u32 {
        const SHARED    = 1 << 0;
        const PRIVATE   = 1 << 1;
        const ANONYMOUS = 1 << 2;
        const FIXED     = 1 << 3;
    }
}
```

### 3. 파일 I/O (40-69)

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 40 | `sys_open` | path, flags, mode | 파일 열기 |
| 41 | `sys_close` | fd | 파일 닫기 |
| 42 | `sys_read` | fd, buf, count | 읽기 |
| 43 | `sys_write` | fd, buf, count | 쓰기 |
| 44 | `sys_lseek` | fd, offset, whence | 위치 이동 |
| 45 | `sys_fstat` | fd, stat | 파일 정보 |
| 46 | `sys_stat` | path, stat | 경로로 파일 정보 |
| 47 | `sys_dup` | oldfd | 파일 디스크립터 복제 |
| 48 | `sys_dup2` | oldfd, newfd | 특정 번호로 복제 |
| 49 | `sys_pipe` | fds[2] | 파이프 생성 |
| 50 | `sys_fcntl` | fd, cmd, arg | 파일 제어 |
| 51 | `sys_ioctl` | fd, request, arg | 장치 제어 |
| 52 | `sys_readdir` | fd, dirent | 디렉토리 읽기 |
| 53 | `sys_mkdir` | path, mode | 디렉토리 생성 |
| 54 | `sys_rmdir` | path | 디렉토리 삭제 |
| 55 | `sys_unlink` | path | 파일 삭제 |
| 56 | `sys_rename` | oldpath, newpath | 이름 변경 |

```rust
/// 파일 열기
pub fn sys_open(
    path: *const u8,
    path_len: usize,
    flags: OpenFlags,
    mode: FileMode,
) -> Result<Fd, SyscallError>;

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY    = 0;
        const WRONLY    = 1 << 0;
        const RDWR      = 1 << 1;
        const CREAT     = 1 << 2;
        const EXCL      = 1 << 3;
        const TRUNC     = 1 << 4;
        const APPEND    = 1 << 5;
        const NONBLOCK  = 1 << 6;
        const CLOEXEC   = 1 << 7;
    }
}
```

### 4. 스레딩 (70-89)

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 70 | `sys_thread_create` | entry, arg, stack, stack_size | 스레드 생성 |
| 71 | `sys_thread_exit` | retval | 스레드 종료 |
| 72 | `sys_thread_join` | tid, retval | 스레드 대기 |
| 73 | `sys_thread_detach` | tid | 스레드 분리 |
| 74 | `sys_thread_self` | - | 현재 TID 반환 |
| 75 | `sys_futex` | addr, op, val, timeout | Futex 연산 |
| 76 | `sys_thread_setname` | tid, name | 스레드 이름 설정 |

```rust
/// 스레드 생성
pub fn sys_thread_create(
    entry: fn(*mut u8) -> *mut u8,
    arg: *mut u8,
    stack: *mut u8,        // 사용자 제공 스택
    stack_size: usize,
) -> Result<Tid, SyscallError>;

/// Futex 연산 (동기화 기본 요소)
pub fn sys_futex(
    addr: *mut u32,
    op: FutexOp,
    val: u32,
    timeout: Option<Duration>,
) -> Result<i32, SyscallError>;

pub enum FutexOp {
    Wait,           // *addr == val이면 대기
    Wake,           // val개 스레드 깨우기
    WakeOp,         // 조건부 깨우기
    Requeue,        // 다른 futex로 이동
}
```

### 5. 네트워크 (90-109)

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 90 | `sys_socket` | domain, type, protocol | 소켓 생성 |
| 91 | `sys_bind` | fd, addr, addrlen | 주소 바인드 |
| 92 | `sys_listen` | fd, backlog | 리슨 모드 |
| 93 | `sys_accept` | fd, addr, addrlen | 연결 수락 |
| 94 | `sys_connect` | fd, addr, addrlen | 연결 |
| 95 | `sys_send` | fd, buf, len, flags | 전송 |
| 96 | `sys_recv` | fd, buf, len, flags | 수신 |
| 97 | `sys_sendto` | fd, buf, len, flags, addr | UDP 전송 |
| 98 | `sys_recvfrom` | fd, buf, len, flags, addr | UDP 수신 |
| 99 | `sys_shutdown` | fd, how | 소켓 종료 |
| 100 | `sys_getsockopt` | fd, level, optname, ... | 옵션 읽기 |
| 101 | `sys_setsockopt` | fd, level, optname, ... | 옵션 설정 |
| 102 | `sys_getpeername` | fd, addr, addrlen | 피어 주소 |
| 103 | `sys_getsockname` | fd, addr, addrlen | 로컬 주소 |

```rust
/// 소켓 생성
pub fn sys_socket(
    domain: SocketDomain,   // AF_INET, AF_INET6
    sock_type: SocketType,  // SOCK_STREAM, SOCK_DGRAM
    protocol: u32,
) -> Result<Fd, SyscallError>;

/// TCP 연결
pub fn sys_connect(
    fd: Fd,
    addr: *const SocketAddr,
    addrlen: u32,
) -> Result<(), SyscallError>;
```

### 6. 시간 (110-119)

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 110 | `sys_clock_gettime` | clockid, timespec | 현재 시간 |
| 111 | `sys_clock_nanosleep` | clockid, flags, req, rem | 정밀 대기 |
| 112 | `sys_gettimeofday` | tv, tz | 시스템 시간 |
| 113 | `sys_timer_create` | clockid, evp, timerid | 타이머 생성 |
| 114 | `sys_timer_settime` | timerid, flags, new, old | 타이머 설정 |
| 115 | `sys_timer_delete` | timerid | 타이머 삭제 |

### 7. 이벤트 폴링 (120-129)

| 번호 | 이름 | 인자 | 설명 |
|------|------|------|------|
| 120 | `sys_epoll_create` | flags | epoll 생성 |
| 121 | `sys_epoll_ctl` | epfd, op, fd, event | epoll 제어 |
| 122 | `sys_epoll_wait` | epfd, events, maxevents, timeout | 이벤트 대기 |
| 123 | `sys_eventfd` | initval, flags | eventfd 생성 |

```rust
/// epoll 이벤트 대기
pub fn sys_epoll_wait(
    epfd: Fd,
    events: *mut EpollEvent,
    maxevents: i32,
    timeout: i32,  // ms, -1 = 무한 대기
) -> Result<i32, SyscallError>;
```

---

## KPIO 확장 시스템 콜 (200+)

### 8. IPC 채널 (200-219)

| 번호 | 이름 | 설명 |
|------|------|------|
| 200 | `sys_shm_create` | 공유 메모리 생성 |
| 201 | `sys_shm_open` | 공유 메모리 열기 |
| 202 | `sys_shm_map` | 공유 메모리 매핑 |
| 203 | `sys_shm_unlink` | 공유 메모리 삭제 |
| 210 | `sys_channel_create` | IPC 채널 생성 |
| 211 | `sys_channel_send` | 채널에 메시지 전송 |
| 212 | `sys_channel_recv` | 채널에서 메시지 수신 |

```rust
/// IPC 채널 생성
pub fn sys_channel_create(
    name: *const u8,
    name_len: usize,
    buffer_size: usize,
) -> Result<ChannelId, SyscallError>;

/// 채널 전송 (Zero-copy)
pub fn sys_channel_send(
    channel: ChannelId,
    msg: *const u8,
    len: usize,
    caps: *const CapabilityId,
    cap_count: usize,
) -> Result<(), SyscallError>;
```

### 9. Capability (220-229)

| 번호 | 이름 | 설명 |
|------|------|------|
| 220 | `sys_cap_create` | Capability 생성 |
| 221 | `sys_cap_diminish` | 권한 축소 |
| 222 | `sys_cap_transfer` | Capability 전달 |
| 223 | `sys_cap_revoke` | Capability 취소 |
| 224 | `sys_cap_check` | 권한 확인 |

### 10. GPU (230-249)

| 번호 | 이름 | 설명 |
|------|------|------|
| 230 | `sys_gpu_alloc` | GPU 메모리 할당 |
| 231 | `sys_gpu_free` | GPU 메모리 해제 |
| 232 | `sys_gpu_map` | GPU 메모리 매핑 |
| 233 | `sys_gpu_submit` | GPU 명령 제출 |
| 234 | `sys_gpu_wait` | GPU 작업 완료 대기 |
| 235 | `sys_gpu_set_priority` | 탭 GPU 우선순위 설정 |

```rust
/// GPU 명령 제출 (브라우저 최적화)
pub fn sys_gpu_submit(
    tab_id: u32,
    command_buffer: *const u8,
    len: usize,
    priority: GpuPriority,
) -> Result<GpuFenceId, SyscallError>;

/// GPU 작업 완료 대기
pub fn sys_gpu_wait(
    fence: GpuFenceId,
    timeout_ns: u64,
) -> Result<(), SyscallError>;
```

### 11. 브라우저 특화 (250-269)

| 번호 | 이름 | 설명 |
|------|------|------|
| 250 | `sys_tab_register` | 탭 등록 |
| 251 | `sys_tab_set_state` | 탭 상태 변경 |
| 252 | `sys_tab_get_memory` | 탭 메모리 사용량 |
| 253 | `sys_wasm_cache_get` | WASM AOT 캐시 조회 |
| 254 | `sys_wasm_cache_put` | WASM AOT 캐시 저장 |
| 255 | `sys_network_zero_copy` | Zero-copy 네트워크 요청 |

```rust
/// 탭 등록 (커널에 브라우저 탭 알림)
pub fn sys_tab_register(
    tab_id: u32,
    initial_state: TabState,
) -> Result<(), SyscallError>;

/// 탭 상태 변경
pub fn sys_tab_set_state(
    tab_id: u32,
    state: TabState,
) -> Result<(), SyscallError>;

/// WASM AOT 캐시 조회
pub fn sys_wasm_cache_get(
    module_hash: *const [u8; 32],
    out_buffer: *mut u8,
    buffer_len: usize,
) -> Result<Option<usize>, SyscallError>;
```

---

## 에러 코드

```rust
#[repr(i32)]
pub enum SyscallError {
    /// 성공
    Success = 0,
    
    /// 일반 에러
    PermissionDenied = -1,
    NoSuchFile = -2,
    IOError = -3,
    NoMemory = -4,
    InvalidArgument = -5,
    BadFileDescriptor = -6,
    
    /// 블로킹 에러
    WouldBlock = -11,
    Interrupted = -12,
    
    /// 프로세스/스레드 에러
    NoSuchProcess = -20,
    NoSuchThread = -21,
    
    /// 네트워크 에러
    ConnectionRefused = -30,
    ConnectionReset = -31,
    NetworkUnreachable = -32,
    
    /// 리소스 에러
    TooManyOpenFiles = -40,
    ResourceBusy = -41,
    
    /// KPIO 확장 에러
    CapabilityDenied = -100,
    ChannelFull = -101,
    ChannelEmpty = -102,
    GpuBusy = -103,
    CacheMiss = -104,
}
```

---

## Servo에 필요한 최소 시스템 콜

Servo 실행을 위한 **최소 구현 우선순위**:

### Priority 1 (html5ever 실행)
1. `sys_mmap` - 메모리 할당
2. `sys_munmap` - 메모리 해제
3. `sys_write` - 콘솔 출력
4. `sys_exit` - 프로세스 종료
5. `sys_thread_create` - 스레드 생성
6. `sys_futex` - 동기화

### Priority 2 (네트워크)
7. `sys_socket` - 소켓 생성
8. `sys_connect` - TCP 연결
9. `sys_send` / `sys_recv` - 데이터 전송
10. `sys_epoll_*` - 비동기 I/O

### Priority 3 (파일시스템)
11. `sys_open` / `sys_close`
12. `sys_read` / `sys_write`
13. `sys_stat` / `sys_fstat`

### Priority 4 (GPU)
14. `sys_gpu_alloc`
15. `sys_gpu_submit`
16. `sys_gpu_wait`

---

## 구현 예시

```rust
// kernel/src/syscall/mod.rs

use crate::process::current_task;
use crate::memory::UserPtr;

/// 시스템 콜 핸들러
#[no_mangle]
pub extern "C" fn syscall_handler(
    num: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    arg6: usize,
) -> isize {
    match num {
        SYS_EXIT => sys_exit(arg1 as i32),
        SYS_READ => sys_read(arg1 as Fd, UserPtr::new(arg2), arg3),
        SYS_WRITE => sys_write(arg1 as Fd, UserPtr::new(arg2), arg3),
        SYS_MMAP => sys_mmap(
            arg1,
            arg2,
            ProtFlags::from_bits_truncate(arg3 as u32),
            MapFlags::from_bits_truncate(arg4 as u32),
            arg5 as i32,
            arg6 as u64,
        ),
        SYS_THREAD_CREATE => sys_thread_create(
            arg1 as fn(*mut u8) -> *mut u8,
            arg2 as *mut u8,
            arg3 as *mut u8,
            arg4,
        ),
        SYS_FUTEX => sys_futex(
            UserPtr::new(arg1),
            FutexOp::from_u32(arg2 as u32),
            arg3 as u32,
            arg4,
        ),
        // ... 더 많은 syscalls
        _ => SyscallError::InvalidArgument as isize,
    }
}
```
