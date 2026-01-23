# Phase 1: 코어 (Core) 설계 문서

**상태:** ✅ 완료 (2026-01-23)

## 개요

Phase 1은 KPIO 운영체제의 핵심 런타임을 구축하는 단계입니다. WASM 런타임을 통합하고, 기본적인 디바이스 드라이버와 인터럽트 시스템을 완성했습니다.

---

## 완료된 구현 사항

### 핵심 기능
- ✅ APIC 타이머 (100Hz) - 8259 PIC 비활성화 후 정상 동작
- ✅ PCI 버스 열거 (7개 디바이스 탐지)
- ✅ VirtIO 블록 디바이스 드라이버 (64MB 디스크 인식)
- ✅ WASM 런타임 (wasmi 인터프리터, `add(2,3)=5` 테스트 통과)

### 관련 커밋
- `cbf0c6a` - APIC timer and scheduler infrastructure
- `28b72dd` - PCI bus enumeration with VirtIO detection
- `d4560cf` - VirtIO block device driver (Stage 5)
- `5e0269f` - WASM runtime with wasmi interpreter (Phase 1 Complete)

---

## 중요 설계 결정: wasmi 선택

**Wasmtime 17.0은 `std` 환경을 필요로 합니다.** 따라서 커널 내부에서는 wasmi(no_std 호환 인터프리터)를 사용하기로 결정했습니다.

| 전략 | 설명 | 상태 |
|--------|------|------|
| ~~A. 사용자 공간 WASM 서버~~ | Wasmtime을 사용자 공간에서 실행 | 보류 |
| ~~B. Wasmtime 포크~~ | no_std 환경에 맞게 수정 | 비용 과다 |
| **C. wasmi 사용** | no_std 호환 인터프리터 | ✅ 채택 |
| ~~D. 하이브리드~~ | 초기 wasmi, 이후 Wasmtime | Phase 2에서 재검토 |

**선택 이유:**
- no_std 네이티브 지원
- 커널 내 직접 실행 가능
- 성능은 브라우저의 SpiderMonkey가 보완 (Phase 2)

---

## 선행 조건

- Phase 0 완료 (부팅, 메모리 관리, 시리얼 출력) ✅

## 완료 조건

- ✅ WASM 테스트 애플리케이션 실행 (`add(2,3)=5`)
- ✅ APIC 타이머 동작 (100Hz)
- ✅ VirtIO-Blk 드라이버로 디바이스 인식 (64MB)
- ⏳ WASI 함수 (fd_write, clock_time_get) - Phase 2에서 브라우저와 함께 구현

---

## 1. 인터럽트 및 예외 처리 ✅ 완료

### 1.1 APIC 초기화

```rust
// kernel/src/interrupts/apic.rs

use x86_64::registers::model_specific::Msr;

/// Local APIC 기본 주소
/// 
/// 주의: 이 값은 기본값이며, 실제로는 ACPI MADT 테이블에서
/// Local APIC Address Override 엔트리를 확인해야 합니다.
/// `acpi` 크레이트의 MADT 파서를 사용하여 동적으로 획득:
/// ```rust
/// let madt = acpi_tables.find_table::<Madt>();
/// let lapic_addr = madt.local_apic_address;
/// ```
const LAPIC_BASE_DEFAULT: u64 = 0xFEE0_0000;

/// Local APIC 주소 (ACPI에서 초기화)
static mut LAPIC_BASE: u64 = LAPIC_BASE_DEFAULT;

/// Local APIC 레지스터 오프셋
mod regs {
    pub const ID: u32 = 0x020;
    pub const VERSION: u32 = 0x030;
    pub const TPR: u32 = 0x080;        // Task Priority Register
    pub const EOI: u32 = 0x0B0;        // End of Interrupt
    pub const SVR: u32 = 0x0F0;        // Spurious Interrupt Vector
    pub const ICR_LOW: u32 = 0x300;    // Interrupt Command Register
    pub const ICR_HIGH: u32 = 0x310;
    pub const LVT_TIMER: u32 = 0x320;
    pub const TIMER_INIT: u32 = 0x380;
    pub const TIMER_CURR: u32 = 0x390;
    pub const TIMER_DIV: u32 = 0x3E0;
}

/// Local APIC
pub struct LocalApic {
    base_addr: u64,
}

impl LocalApic {
    /// ACPI MADT에서 LAPIC 주소 설정
    pub fn set_base_from_acpi(addr: u64) {
        unsafe {
            LAPIC_BASE = addr;
        }
        log::info!("Local APIC base set to {:#x}", addr);
    }
    
    /// Local APIC 초기화
    pub unsafe fn init() -> Self {
        // APIC 활성화 (MSR)
        let mut apic_base_msr = Msr::new(0x1B);
        let value = apic_base_msr.read();
        apic_base_msr.write(value | (1 << 11)); // APIC Enable
        
        let lapic = LocalApic { base_addr: LAPIC_BASE };
        
        // Spurious Interrupt Vector 설정 및 APIC 활성화
        lapic.write(regs::SVR, 0x1FF); // Vector 0xFF, APIC 활성화
        
        // Task Priority를 0으로 설정 (모든 인터럽트 허용)
        lapic.write(regs::TPR, 0);
        
        lapic
    }
    
    /// 레지스터 읽기
    fn read(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.base_addr + reg as u64) as *const u32;
            ptr.read_volatile()
        }
    }
    
    /// 레지스터 쓰기
    fn write(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.base_addr + reg as u64) as *mut u32;
            ptr.write_volatile(value);
        }
    }
    
    /// EOI 전송
    pub fn end_of_interrupt(&self) {
        self.write(regs::EOI, 0);
    }
    
    /// 타이머 설정
    pub fn setup_timer(&self, vector: u8, initial_count: u32, divider: u8) {
        // 분주비 설정
        self.write(regs::TIMER_DIV, divider as u32);
        
        // LVT Timer 설정 (주기적 모드)
        self.write(regs::LVT_TIMER, vector as u32 | (1 << 17));
        
        // 초기 카운트 설정
        self.write(regs::TIMER_INIT, initial_count);
    }
}
```

### 1.2 I/O APIC

```rust
// kernel/src/interrupts/ioapic.rs

/// I/O APIC 기본 주소
/// 
/// 주의: 실제 주소는 ACPI MADT의 I/O APIC 엔트리에서 획득해야 합니다.
/// 시스템에 여러 I/O APIC이 있을 수 있으므로 Vector로 관리해야 합니다.
const IOAPIC_BASE_DEFAULT: u64 = 0xFEC0_0000;

/// I/O APIC 레지스터
mod regs {
    pub const IOREGSEL: u32 = 0x00;
    pub const IOWIN: u32 = 0x10;
}

/// I/O APIC 내부 레지스터
mod internal {
    pub const IOAPICID: u8 = 0x00;
    pub const IOAPICVER: u8 = 0x01;
    pub const IOREDTBL_BASE: u8 = 0x10;
}

pub struct IoApic {
    base_addr: u64,
    /// Global System Interrupt 시작 번호
    gsi_base: u32,
}

impl IoApic {
    /// ACPI MADT에서 파싱한 정보로 I/O APIC 생성
    /// 
    /// # Arguments
    /// * `base_addr` - MADT I/O APIC 엔트리의 I/O APIC Address
    /// * `gsi_base` - Global System Interrupt Base
    pub unsafe fn new(base_addr: u64, gsi_base: u32) -> Self {
        IoApic { base_addr, gsi_base }
    }
    
    /// 기본 주소로 생성 (레거시 지원용)
    pub unsafe fn new_default() -> Self {
        IoApic { base_addr: IOAPIC_BASE_DEFAULT, gsi_base: 0 }
    }
    
    fn read(&self, reg: u8) -> u32 {
        unsafe {
            let sel = (self.base_addr + regs::IOREGSEL as u64) as *mut u32;
            let win = (self.base_addr + regs::IOWIN as u64) as *const u32;
            sel.write_volatile(reg as u32);
            win.read_volatile()
        }
    }
    
    fn write(&self, reg: u8, value: u32) {
        unsafe {
            let sel = (self.base_addr + regs::IOREGSEL as u64) as *mut u32;
            let win = (self.base_addr + regs::IOWIN as u64) as *mut u32;
            sel.write_volatile(reg as u32);
            win.write_volatile(value);
        }
    }
    
    /// IRQ 리다이렉션 엔트리 설정
    pub fn set_irq(&self, irq: u8, vector: u8, dest_apic: u8) {
        let redtbl = internal::IOREDTBL_BASE + irq * 2;
        
        // 낮은 32비트: 벡터, 전달 모드, 트리거 모드 등
        let low: u32 = vector as u32;
        
        // 높은 32비트: 대상 APIC ID
        let high: u32 = (dest_apic as u32) << 24;
        
        self.write(redtbl, low);
        self.write(redtbl + 1, high);
    }
    
    /// IRQ 마스크/언마스크
    pub fn mask_irq(&self, irq: u8, mask: bool) {
        let redtbl = internal::IOREDTBL_BASE + irq * 2;
        let mut low = self.read(redtbl);
        
        if mask {
            low |= 1 << 16;
        } else {
            low &= !(1 << 16);
        }
        
        self.write(redtbl, low);
    }
}
```

---

## 2. 스케줄러 (단일 코어)

### 2.1 태스크 구조

```rust
// kernel/src/task/mod.rs

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

/// 태스크 ID 생성기
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// 태스크 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    pub fn new() -> Self {
        TaskId(NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// 태스크 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 생성됨
    Created,
    /// 실행 가능
    Ready,
    /// 실행 중
    Running,
    /// 대기 중 (I/O 등)
    Blocked,
    /// 종료됨
    Terminated,
}

/// 태스크 우선순위
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u8);

impl Priority {
    pub const IDLE: Priority = Priority(0);
    pub const LOW: Priority = Priority(64);
    pub const NORMAL: Priority = Priority(128);
    pub const HIGH: Priority = Priority(192);
    pub const REALTIME: Priority = Priority(255);
}

/// 태스크 컨텍스트 (레지스터 상태)
#[derive(Debug, Default)]
#[repr(C)]
pub struct TaskContext {
    // 범용 레지스터 (callee-saved)
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // 스택 포인터
    pub rsp: u64,
    // 명령 포인터
    pub rip: u64,
    // 플래그
    pub rflags: u64,
}

/// 태스크 (커널 스레드 또는 WASM 인스턴스)
pub struct Task {
    pub id: TaskId,
    pub state: TaskState,
    pub priority: Priority,
    pub context: TaskContext,
    /// 커널 스택
    pub kernel_stack: Box<[u8; 4096 * 4]>,
    /// WASM 인스턴스 (Some이면 WASM 태스크)
    pub wasm_instance: Option<WasmInstance>,
}
```

### 2.2 라운드 로빈 스케줄러

```rust
// kernel/src/task/scheduler.rs

use super::{Task, TaskId, TaskState, Priority};
use alloc::collections::{BTreeMap, VecDeque};
use spin::Mutex;

/// 전역 스케줄러
pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// 라운드 로빈 스케줄러 (우선순위 큐 포함)
pub struct Scheduler {
    /// 준비 큐 (우선순위별)
    ready_queues: [VecDeque<TaskId>; 4],
    /// 모든 태스크
    tasks: BTreeMap<TaskId, Task>,
    /// 현재 실행 중인 태스크
    current: Option<TaskId>,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            ready_queues: [
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
            ],
            tasks: BTreeMap::new(),
            current: None,
        }
    }
    
    /// 태스크 추가
    pub fn spawn(&mut self, task: Task) -> TaskId {
        let id = task.id;
        let priority_idx = self.priority_to_index(task.priority);
        self.tasks.insert(id, task);
        self.ready_queues[priority_idx].push_back(id);
        id
    }
    
    /// 우선순위를 큐 인덱스로 변환
    fn priority_to_index(&self, priority: Priority) -> usize {
        match priority.0 {
            0..=63 => 0,      // IDLE ~ LOW
            64..=127 => 1,    // LOW ~ NORMAL
            128..=191 => 2,   // NORMAL ~ HIGH
            192..=255 => 3,   // HIGH ~ REALTIME
        }
    }
    
    /// 다음 태스크 선택
    pub fn pick_next(&mut self) -> Option<TaskId> {
        // 높은 우선순위부터 검색
        for queue in self.ready_queues.iter_mut().rev() {
            if let Some(id) = queue.pop_front() {
                return Some(id);
            }
        }
        None
    }
    
    /// 컨텍스트 스위치
    pub fn schedule(&mut self) {
        let next_id = match self.pick_next() {
            Some(id) => id,
            None => return, // 실행할 태스크 없음
        };
        
        if let Some(current_id) = self.current {
            if current_id == next_id {
                return; // 같은 태스크
            }
            
            // 현재 태스크를 준비 큐에 복귀
            if let Some(task) = self.tasks.get_mut(&current_id) {
                if task.state == TaskState::Running {
                    task.state = TaskState::Ready;
                    let idx = self.priority_to_index(task.priority);
                    self.ready_queues[idx].push_back(current_id);
                }
            }
        }
        
        // 다음 태스크 실행
        if let Some(task) = self.tasks.get_mut(&next_id) {
            task.state = TaskState::Running;
            self.current = Some(next_id);
            
            // 컨텍스트 스위치 수행 (어셈블리)
            // switch_context(&mut old_context, &new_context);
        }
    }
    
    /// 현재 태스크 블록
    pub fn block_current(&mut self) {
        if let Some(current_id) = self.current {
            if let Some(task) = self.tasks.get_mut(&current_id) {
                task.state = TaskState::Blocked;
            }
            self.current = None;
            self.schedule();
        }
    }
    
    /// 태스크 언블록
    pub fn unblock(&mut self, id: TaskId) {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.state == TaskState::Blocked {
                task.state = TaskState::Ready;
                let idx = self.priority_to_index(task.priority);
                self.ready_queues[idx].push_back(id);
            }
        }
    }
}
```

### 2.3 컨텍스트 스위치 (어셈블리)

```asm
; kernel/src/task/switch.asm

global switch_context
section .text

; void switch_context(TaskContext* old, TaskContext* new)
; rdi = old context, rsi = new context
switch_context:
    ; 현재 컨텍스트 저장
    mov [rdi + 0x00], rbx
    mov [rdi + 0x08], rbp
    mov [rdi + 0x10], r12
    mov [rdi + 0x18], r13
    mov [rdi + 0x20], r14
    mov [rdi + 0x28], r15
    mov [rdi + 0x30], rsp
    
    ; 리턴 주소 저장
    mov rax, [rsp]
    mov [rdi + 0x38], rax
    
    ; 플래그 저장
    pushfq
    pop rax
    mov [rdi + 0x40], rax
    
    ; 새 컨텍스트 복원
    mov rbx, [rsi + 0x00]
    mov rbp, [rsi + 0x08]
    mov r12, [rsi + 0x10]
    mov r13, [rsi + 0x18]
    mov r14, [rsi + 0x20]
    mov r15, [rsi + 0x28]
    mov rsp, [rsi + 0x30]
    
    ; 플래그 복원
    mov rax, [rsi + 0x40]
    push rax
    popfq
    
    ; 새 명령 주소로 점프
    mov rax, [rsi + 0x38]
    push rax
    ret
```

---

## 3. Wasmtime 통합

### 3.1 WASM 런타임 초기화

```rust
// runtime/src/engine.rs

use wasmtime::*;
use alloc::sync::Arc;
use spin::Mutex;

/// WASM 엔진 설정
pub fn create_engine_config() -> Config {
    let mut config = Config::new();
    
    // AOT 컴파일 활성화
    config.cranelift_opt_level(OptLevel::Speed);
    
    // 메모리 설정
    config.static_memory_maximum_size(1 << 32); // 4GB
    config.static_memory_guard_size(1 << 16);   // 64KB
    
    // 보안 설정
    config.wasm_reference_types(true);
    config.wasm_simd(true);
    config.wasm_bulk_memory(true);
    
    // Epoch 기반 인터럽트 (선점)
    config.epoch_interruption(true);
    
    config
}

/// 전역 WASM 엔진
static ENGINE: spin::Once<Engine> = spin::Once::new();

pub fn init_engine() {
    ENGINE.call_once(|| {
        let config = create_engine_config();
        Engine::new(&config).expect("Failed to create Wasmtime engine")
    });
}

pub fn get_engine() -> &'static Engine {
    ENGINE.get().expect("Engine not initialized")
}
```

### 3.2 WASM 인스턴스 관리

```rust
// runtime/src/instance.rs

use wasmtime::*;
use crate::wasi::WasiCtx;
use alloc::string::String;

/// WASM 인스턴스 래퍼
pub struct WasmInstance {
    store: Store<WasiCtx>,
    instance: Instance,
    module_name: String,
}

impl WasmInstance {
    /// WASM 모듈 로드 및 인스턴스화
    pub fn new(engine: &Engine, wasm_bytes: &[u8], name: String) -> Result<Self> {
        // 모듈 컴파일
        let module = Module::new(engine, wasm_bytes)?;
        
        // WASI 컨텍스트 생성
        let wasi_ctx = WasiCtx::new();
        let mut store = Store::new(engine, wasi_ctx);
        
        // Epoch 설정 (선점을 위한)
        store.set_epoch_deadline(1);
        
        // 링커 설정
        let mut linker = Linker::new(engine);
        crate::wasi::add_to_linker(&mut linker)?;
        
        // 인스턴스화
        let instance = linker.instantiate(&mut store, &module)?;
        
        Ok(WasmInstance {
            store,
            instance,
            module_name: name,
        })
    }
    
    /// AOT 캐시에서 로드
    pub fn from_aot_cache(engine: &Engine, aot_bytes: &[u8], name: String) -> Result<Self> {
        // 직렬화된 모듈 역직렬화
        let module = unsafe { Module::deserialize(engine, aot_bytes)? };
        
        let wasi_ctx = WasiCtx::new();
        let mut store = Store::new(engine, wasi_ctx);
        store.set_epoch_deadline(1);
        
        let mut linker = Linker::new(engine);
        crate::wasi::add_to_linker(&mut linker)?;
        
        let instance = linker.instantiate(&mut store, &module)?;
        
        Ok(WasmInstance {
            store,
            instance,
            module_name: name,
        })
    }
    
    /// 엔트리 함수 호출
    pub fn call_start(&mut self) -> Result<()> {
        // _start 함수 찾기 (WASI 표준)
        let start = self.instance
            .get_typed_func::<(), ()>(&mut self.store, "_start")?;
        
        start.call(&mut self.store, ())?;
        Ok(())
    }
    
    /// Epoch 업데이트 (선점용)
    pub fn increment_epoch(&mut self) {
        self.store.engine().increment_epoch();
    }
}
```

---

## 4. WASI 기본 구현

### 4.1 WASI 컨텍스트

```rust
// runtime/src/wasi/mod.rs

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// WASI 컨텍스트
pub struct WasiCtx {
    /// 파일 디스크립터 테이블
    fds: BTreeMap<u32, FileDescriptor>,
    /// 다음 FD 번호
    next_fd: u32,
    /// 환경 변수
    env: Vec<(String, String)>,
    /// 명령줄 인자
    args: Vec<String>,
    /// 단조 시계 오프셋
    clock_offset: u64,
}

/// 파일 디스크립터
pub enum FileDescriptor {
    /// 표준 입력
    Stdin,
    /// 표준 출력
    Stdout,
    /// 표준 에러
    Stderr,
    /// 일반 파일 (Phase 2에서 구현)
    File { /* ... */ },
    /// 디렉토리 (Phase 2에서 구현)
    Directory { /* ... */ },
}

impl WasiCtx {
    pub fn new() -> Self {
        let mut ctx = WasiCtx {
            fds: BTreeMap::new(),
            next_fd: 3,
            env: Vec::new(),
            args: Vec::new(),
            clock_offset: 0,
        };
        
        // 표준 FD 설정
        ctx.fds.insert(0, FileDescriptor::Stdin);
        ctx.fds.insert(1, FileDescriptor::Stdout);
        ctx.fds.insert(2, FileDescriptor::Stderr);
        
        ctx
    }
    
    pub fn set_args(&mut self, args: Vec<String>) {
        self.args = args;
    }
    
    pub fn set_env(&mut self, env: Vec<(String, String)>) {
        self.env = env;
    }
}
```

### 4.2 fd_write 구현

```rust
// runtime/src/wasi/io.rs

use super::WasiCtx;
use wasmtime::*;

/// WASI fd_write 구현
/// 
/// 시그니처: (fd: i32, iovs_ptr: i32, iovs_len: i32, nwritten_ptr: i32) -> errno
pub fn fd_write(
    mut caller: Caller<'_, WasiCtx>,
    fd: i32,
    iovs_ptr: i32,
    iovs_len: i32,
    nwritten_ptr: i32,
) -> i32 {
    let memory = match caller.get_export("memory") {
        Some(Extern::Memory(mem)) => mem,
        _ => return wasi::ERRNO_BADF as i32,
    };
    
    let data = memory.data_mut(&mut caller);
    
    // iovec 구조체 파싱
    let mut total_written = 0u32;
    
    for i in 0..iovs_len {
        let iov_offset = (iovs_ptr + i * 8) as usize;
        
        // iovec: { buf: i32, buf_len: i32 }
        let buf_ptr = u32::from_le_bytes(
            data[iov_offset..iov_offset + 4].try_into().unwrap()
        ) as usize;
        let buf_len = u32::from_le_bytes(
            data[iov_offset + 4..iov_offset + 8].try_into().unwrap()
        ) as usize;
        
        let buf = &data[buf_ptr..buf_ptr + buf_len];
        
        match fd {
            1 | 2 => {
                // stdout/stderr -> 시리얼 출력
                // 주의: runtime 크레이트는 kernel 매크로를 직접 사용할 수 없음
                // 대신 커널이 제공하는 출력 콜백 사용
                let ctx = caller.data();
                for &byte in buf {
                    ctx.write_stdout(byte);
                }
                total_written += buf_len as u32;
            }
            _ => return wasi::ERRNO_BADF as i32,
        }
    }
    
    // 쓴 바이트 수 저장
    let nwritten_offset = nwritten_ptr as usize;
    data[nwritten_offset..nwritten_offset + 4]
        .copy_from_slice(&total_written.to_le_bytes());
    
    wasi::ERRNO_SUCCESS as i32
}
```

### 4.3 clock_time_get 구현

```rust
// runtime/src/wasi/clock.rs

use super::WasiCtx;
use wasmtime::*;

/// WASI clock_time_get 구현
/// 
/// 시그니처: (clock_id: i32, precision: i64, time_ptr: i32) -> errno
pub fn clock_time_get(
    mut caller: Caller<'_, WasiCtx>,
    clock_id: i32,
    _precision: i64,
    time_ptr: i32,
) -> i32 {
    let memory = match caller.get_export("memory") {
        Some(Extern::Memory(mem)) => mem,
        _ => return wasi::ERRNO_BADF as i32,
    };
    
    let time_ns: u64 = match clock_id {
        // CLOCK_REALTIME
        0 => get_realtime_ns(),
        // CLOCK_MONOTONIC
        1 => get_monotonic_ns(),
        // CLOCK_PROCESS_CPUTIME_ID
        2 => get_process_cputime_ns(),
        // CLOCK_THREAD_CPUTIME_ID
        3 => get_thread_cputime_ns(),
        _ => return wasi::ERRNO_INVAL as i32,
    };
    
    let data = memory.data_mut(&mut caller);
    let time_offset = time_ptr as usize;
    data[time_offset..time_offset + 8].copy_from_slice(&time_ns.to_le_bytes());
    
    wasi::ERRNO_SUCCESS as i32
}

/// 실시간 시계 (TSC 기반)
fn get_realtime_ns() -> u64 {
    // Phase 0에서는 TSC 기반 근사값 사용
    // 추후 RTC 및 NTP 동기화 추가
    let tsc = unsafe { core::arch::x86_64::_rdtsc() };
    // 대략 3GHz CPU 가정
    tsc / 3
}

/// 단조 시계
fn get_monotonic_ns() -> u64 {
    let tsc = unsafe { core::arch::x86_64::_rdtsc() };
    tsc / 3
}

fn get_process_cputime_ns() -> u64 {
    // 현재는 단조 시계와 동일
    get_monotonic_ns()
}

fn get_thread_cputime_ns() -> u64 {
    // 현재는 단조 시계와 동일
    get_monotonic_ns()
}
```

### 4.4 WASI 링커 등록

```rust
// runtime/src/wasi/linker.rs

use wasmtime::*;
use super::WasiCtx;

/// WASI 함수를 링커에 등록
pub fn add_to_linker(linker: &mut Linker<WasiCtx>) -> Result<()> {
    // wasi_snapshot_preview1 네임스페이스
    
    linker.func_wrap("wasi_snapshot_preview1", "fd_write", 
        super::io::fd_write)?;
    
    linker.func_wrap("wasi_snapshot_preview1", "clock_time_get",
        super::clock::clock_time_get)?;
    
    // 추가 필수 함수 (스텁)
    linker.func_wrap("wasi_snapshot_preview1", "proc_exit",
        |_: Caller<'_, WasiCtx>, code: i32| {
            crate::serial_println!("WASM process exited with code: {}", code);
        })?;
    
    linker.func_wrap("wasi_snapshot_preview1", "args_sizes_get",
        |mut caller: Caller<'_, WasiCtx>, argc_ptr: i32, argv_buf_size_ptr: i32| -> i32 {
            let ctx = caller.data();
            let argc = ctx.args.len() as u32;
            let argv_buf_size: u32 = ctx.args.iter()
                .map(|s| s.len() as u32 + 1)
                .sum();
            
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .unwrap();
            let data = memory.data_mut(&mut caller);
            
            data[argc_ptr as usize..argc_ptr as usize + 4]
                .copy_from_slice(&argc.to_le_bytes());
            data[argv_buf_size_ptr as usize..argv_buf_size_ptr as usize + 4]
                .copy_from_slice(&argv_buf_size.to_le_bytes());
            
            0
        })?;
    
    linker.func_wrap("wasi_snapshot_preview1", "args_get",
        |_: Caller<'_, WasiCtx>, _argv: i32, _argv_buf: i32| -> i32 {
            // Phase 2에서 완전 구현
            0
        })?;
    
    linker.func_wrap("wasi_snapshot_preview1", "environ_sizes_get",
        |_: Caller<'_, WasiCtx>, _environ_count: i32, _environ_buf_size: i32| -> i32 {
            0
        })?;
    
    linker.func_wrap("wasi_snapshot_preview1", "environ_get",
        |_: Caller<'_, WasiCtx>, _environ: i32, _environ_buf: i32| -> i32 {
            0
        })?;
    
    Ok(())
}
```

---

## 5. VirtIO-Blk 드라이버

### 5.1 VirtIO 디바이스 검색

```rust
// kernel/src/driver/pci.rs

/// PCI 설정 공간 읽기
pub fn pci_config_read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address: u32 = (1 << 31) // Enable bit
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);
    
    unsafe {
        use x86_64::instructions::port::Port;
        let mut addr_port: Port<u32> = Port::new(0xCF8);
        let mut data_port: Port<u32> = Port::new(0xCFC);
        
        addr_port.write(address);
        data_port.read()
    }
}

/// VirtIO 디바이스 검색
pub fn find_virtio_devices() -> Vec<VirtioDevice> {
    let mut devices = Vec::new();
    
    for bus in 0..256 {
        for device in 0..32 {
            let vendor_device = pci_config_read(bus as u8, device, 0, 0);
            let vendor_id = (vendor_device & 0xFFFF) as u16;
            let device_id = ((vendor_device >> 16) & 0xFFFF) as u16;
            
            // VirtIO Vendor ID: 0x1AF4
            if vendor_id == 0x1AF4 {
                let class_code = pci_config_read(bus as u8, device, 0, 8) >> 24;
                
                devices.push(VirtioDevice {
                    bus: bus as u8,
                    device,
                    function: 0,
                    device_id,
                    class_code: class_code as u8,
                });
            }
        }
    }
    
    devices
}
```

### 5.2 VirtQueue 구현

```rust
// kernel/src/driver/virtio/queue.rs

use alloc::vec::Vec;

/// VirtQueue 디스크립터
#[repr(C)]
pub struct VirtqDesc {
    /// 게스트 물리 주소
    pub addr: u64,
    /// 길이
    pub len: u32,
    /// 플래그
    pub flags: u16,
    /// 다음 디스크립터 (체인용)
    pub next: u16,
}

/// 디스크립터 플래그
pub mod desc_flags {
    pub const NEXT: u16 = 1;      // 체인 계속
    pub const WRITE: u16 = 2;     // 디바이스 쓰기 전용
    pub const INDIRECT: u16 = 4;  // 간접 디스크립터
}

/// Available 링
#[repr(C)]
pub struct VirtqAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 256], // 동적 크기
}

/// Used 링
#[repr(C)]
pub struct VirtqUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VirtqUsedElem; 256],
}

#[repr(C)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

/// VirtQueue
pub struct VirtQueue {
    /// 큐 크기
    size: u16,
    /// 디스크립터 테이블
    desc: &'static mut [VirtqDesc],
    /// Available 링
    avail: &'static mut VirtqAvail,
    /// Used 링
    used: &'static mut VirtqUsed,
    /// 다음 사용 가능한 디스크립터
    free_head: u16,
    /// 마지막으로 확인한 used 인덱스
    last_used_idx: u16,
}

impl VirtQueue {
    /// 새 VirtQueue 할당 및 초기화
    pub unsafe fn new(size: u16, phys_addr: u64) -> Self {
        // 메모리 레이아웃 계산 및 초기화
        // ...
        todo!()
    }
    
    /// 디스크립터 체인 추가
    pub fn add(&mut self, inputs: &[&[u8]], outputs: &mut [&mut [u8]]) -> Option<u16> {
        // 디스크립터 체인 구성
        // ...
        todo!()
    }
    
    /// 완료된 요청 폴링
    pub fn poll(&mut self) -> Option<(u16, u32)> {
        if self.last_used_idx == self.used.idx {
            return None;
        }
        
        let idx = self.last_used_idx % self.size;
        let elem = &self.used.ring[idx as usize];
        self.last_used_idx = self.last_used_idx.wrapping_add(1);
        
        Some((elem.id as u16, elem.len))
    }
}
```

### 5.3 VirtIO-Blk 드라이버

```rust
// kernel/src/driver/virtio/blk.rs

use super::queue::VirtQueue;

/// VirtIO Block 요청 헤더
#[repr(C)]
pub struct VirtioBlkReq {
    pub req_type: u32,
    pub reserved: u32,
    pub sector: u64,
}

/// 요청 타입
pub mod req_type {
    pub const IN: u32 = 0;    // 읽기
    pub const OUT: u32 = 1;   // 쓰기
    pub const FLUSH: u32 = 4; // 플러시
}

/// VirtIO Block 디바이스
pub struct VirtioBlkDevice {
    /// 요청 큐
    queue: VirtQueue,
    /// 섹터 수
    capacity: u64,
    /// 섹터 크기
    sector_size: u32,
}

impl VirtioBlkDevice {
    /// 블록 읽기
    pub fn read_block(&mut self, sector: u64, buf: &mut [u8]) -> Result<(), BlkError> {
        assert!(buf.len() >= self.sector_size as usize);
        
        let header = VirtioBlkReq {
            req_type: req_type::IN,
            reserved: 0,
            sector,
        };
        
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<VirtioBlkReq>()
            )
        };
        
        let mut status: u8 = 0xFF;
        
        // 디스크립터 체인: [header] -> [data, write] -> [status, write]
        let inputs = [header_bytes];
        let mut outputs = [buf, core::slice::from_mut(&mut status)];
        
        let token = self.queue.add(&inputs, &mut outputs)
            .ok_or(BlkError::QueueFull)?;
        
        // 디바이스에 알림
        self.notify();
        
        // 완료 대기
        loop {
            if let Some((completed_token, _)) = self.queue.poll() {
                if completed_token == token {
                    break;
                }
            }
            core::hint::spin_loop();
        }
        
        if status == 0 {
            Ok(())
        } else {
            Err(BlkError::IoError)
        }
    }
    
    /// 블록 쓰기
    pub fn write_block(&mut self, sector: u64, buf: &[u8]) -> Result<(), BlkError> {
        assert!(buf.len() >= self.sector_size as usize);
        
        let header = VirtioBlkReq {
            req_type: req_type::OUT,
            reserved: 0,
            sector,
        };
        
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<VirtioBlkReq>()
            )
        };
        
        let mut status: u8 = 0xFF;
        
        let inputs = [header_bytes, buf];
        let mut outputs = [core::slice::from_mut(&mut status)];
        
        let token = self.queue.add(&inputs, &mut outputs)
            .ok_or(BlkError::QueueFull)?;
        
        self.notify();
        
        loop {
            if let Some((completed_token, _)) = self.queue.poll() {
                if completed_token == token {
                    break;
                }
            }
            core::hint::spin_loop();
        }
        
        if status == 0 {
            Ok(())
        } else {
            Err(BlkError::IoError)
        }
    }
    
    fn notify(&self) {
        // MMIO 레지스터에 알림
        // ...
    }
}

#[derive(Debug)]
pub enum BlkError {
    QueueFull,
    IoError,
}
```

---

## 6. 병렬 작업

Phase 1 진행 중 병렬로 수행 가능한 작업:

| 작업 | 의존성 | 비고 |
|------|--------|------|
| VirtIO-Net 드라이버 | 없음 | Phase 2 준비 |
| 추가 WASI 함수 | fd_write 완료 | random_get, path_* 등 |
| AOT 캐시 매니저 | Wasmtime 통합 완료 | 1.2 제안 구현 |

---

## 7. 검증 체크리스트

Phase 1 완료 전 확인 사항:

- [ ] APIC 타이머 인터럽트 동작
- [ ] 컨텍스트 스위치 동작
- [ ] Wasmtime 엔진 초기화 성공
- [ ] WASM "Hello World" 출력
- [ ] fd_write 정상 동작
- [ ] clock_time_get 정상 동작
- [ ] VirtIO-Blk 블록 읽기 성공
- [ ] VirtIO-Blk 블록 쓰기 성공
- [ ] 타이머 기반 선점 동작
