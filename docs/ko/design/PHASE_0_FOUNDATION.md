# Phase 0: 기초 (Foundation) 설계 문서

## 개요

Phase 0는 KPIO 운영체제의 기반을 구축하는 단계입니다. 이 단계에서는 부팅 가능한 커널 스켈레톤을 만들고, 기본적인 하드웨어 초기화와 메모리 관리를 구현합니다.

---

## 선행 조건

- 없음 (첫 번째 단계)

## 완료 조건

- QEMU에서 "Hello, Kernel" 시리얼 출력
- 기본 페이지 할당자 동작
- 단위 테스트 프레임워크 실행

---

## 1. 부트 프로세스

### 1.1 UEFI 부트로더

```
[부트 시퀀스]

1. UEFI 펌웨어 초기화
       ↓
2. UEFI 부트로더 로드 (bootloader 크레이트)
       ↓
3. 커널 ELF 로드 및 파싱
       ↓
4. 프레임버퍼 설정 (GOP)
       ↓
5. 메모리 맵 획득
       ↓
6. 커널 엔트리 포인트 점프
```

### 1.2 커널 엔트리 포인트

```rust
// kernel/src/main.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader_api::{entry_point, BootInfo, BootloaderConfig};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // 1. 시리얼 초기화
    serial::init();
    serial_println!("Hello, Kernel");
    
    // 2. GDT/IDT 초기화
    gdt::init();
    interrupts::init();
    
    // 3. 메모리 관리 초기화
    let phys_mem_offset = boot_info.physical_memory_offset.into_option()
        .expect("Physical memory offset not provided by bootloader. \
                 Ensure BOOTLOADER_CONFIG.mappings.physical_memory is set.");
    
    // 물리 메모리 오프셋 검증
    memory::validate_physical_memory_offset(phys_mem_offset);
    
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::new(boot_info.memory_regions.into_iter())
    };
    
    // 4. 힙 초기화
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");
    
    serial_println!("Kernel initialized successfully");
    
    #[cfg(test)]
    test_main();
    
    interrupts::hlt_loop();
}
```

---

## 2. 하드웨어 초기화

### 2.1 GDT (Global Descriptor Table)

```rust
// kernel/src/gdt.rs

use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use lazy_static::lazy_static;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, data_selector, tss_selector })
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        DS::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }
}
```

### 2.2 IDT (Interrupt Descriptor Table)

```rust
// kernel/src/interrupts.rs

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use lazy_static::lazy_static;
use crate::gdt;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        // CPU 예외 핸들러
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_handler);
        
        // 하드웨어 인터럽트 (Phase 1에서 구현)
        // idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_handler);
        // idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_handler);
        
        idt
    };
}

pub fn init() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    
    serial_println!("EXCEPTION: PAGE FAULT");
    serial_println!("Accessed Address: {:?}", Cr2::read());
    serial_println!("Error Code: {:?}", error_code);
    serial_println!("{:#?}", stack_frame);
    
    hlt_loop();
}

extern "x86-interrupt" fn general_protection_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION: GENERAL PROTECTION FAULT\nError Code: {}\n{:#?}", 
           error_code, stack_frame);
}

/// HLT 루프 - CPU를 저전력 상태로 유지하며 인터럽트 대기
/// 
/// 이 함수는 커널 전역에서 사용되므로 interrupts 모듈에서 정의
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
```

---

## 3. 메모리 관리

### 3.1 물리 메모리 프레임 할당자

```rust
// kernel/src/memory/frame_allocator.rs

use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

/// 부트 정보 기반 프레임 할당자
pub struct BootInfoFrameAllocator {
    memory_regions: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// 새 프레임 할당자 생성
    /// 
    /// # Safety
    /// 호출자는 메모리 맵이 유효하고 사용 가능한 영역이 실제로 미사용임을 보장해야 함
    pub unsafe fn new(memory_regions: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_regions,
            next: 0,
        }
    }
    
    /// 사용 가능한 프레임 반복자
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        self.memory_regions
            .into_iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| r.start..r.end)
            .flat_map(|r| r.step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
```

### 3.2 물리 메모리 오프셋 검증

물리 메모리 오프셋은 커널 메모리 관리의 핵심이며, 잘못된 값은 심각한 메모리 손상을 초래합니다.

```rust
// kernel/src/memory/validation.rs

use x86_64::VirtAddr;

/// 물리 메모리 오프셋 검증
/// 
/// # Panics
/// - 오프셋이 canonical 주소가 아닌 경우
/// - 오프셋이 커널 주소 공간에 없는 경우
/// - 오프셋이 페이지 정렬되지 않은 경우
pub fn validate_physical_memory_offset(offset: VirtAddr) {
    // 1. 페이지 정렬 확인 (4KB)
    assert!(
        offset.is_aligned(4096u64),
        "Physical memory offset {:#x} is not page-aligned",
        offset.as_u64()
    );
    
    // 2. Canonical 주소 확인 (x86_64에서 상위 16비트는 47번째 비트의 확장)
    let addr = offset.as_u64();
    let is_canonical = {
        let upper_bits = addr >> 47;
        upper_bits == 0 || upper_bits == 0x1FFFF
    };
    assert!(
        is_canonical,
        "Physical memory offset {:#x} is not a canonical address",
        addr
    );
    
    // 3. 커널 주소 공간 확인 (경고만 - Dynamic 매핑에서는 lower half도 유효)
    // bootloader 0.11의 Dynamic 매핑은 higher half를 보장하지 않음
    if addr < 0xFFFF_8000_0000_0000 {
        serial_println!(
            "Warning: Physical memory offset {:#x} is in lower half (< 0xFFFF_8000_0000_0000)",
            addr
        );
        serial_println!("This is valid with bootloader Dynamic mapping");
    }
    
    // 4. 기본 검증: 오프셋 + 0 접근 가능 확인 (첫 페이지 읽기 시도)
    // 부트로더가 이미 매핑했으므로 읽기는 안전해야 함
    let test_ptr = offset.as_u64() as *const u64;
    let _test_read = unsafe { core::ptr::read_volatile(test_ptr) };
    
    serial_println!("Physical memory offset validated: {:#x}", addr);
}

/// 물리 주소를 가상 주소로 변환 (검증 포함)
pub fn phys_to_virt(phys: PhysAddr, offset: VirtAddr) -> VirtAddr {
    let virt = offset + phys.as_u64();
    
    // 오버플로우 검사
    assert!(
        virt.as_u64() >= offset.as_u64(),
        "Physical to virtual address conversion overflow: phys={:#x}, offset={:#x}",
        phys.as_u64(),
        offset.as_u64()
    );
    
    virt
}
```

### 3.3 페이지 테이블 매핑

```rust
// kernel/src/memory/paging.rs

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, 
        PageTable, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

/// 오프셋 페이지 테이블 초기화
///
/// # Safety
/// 호출자는 물리 메모리가 지정된 오프셋에 매핑되어 있음을 보장해야 함
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// 활성 레벨 4 페이지 테이블 참조 획득
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

/// 주어진 가상 주소에 물리 프레임 매핑
pub fn create_mapping(
    page: Page,
    frame: PhysFrame,
    flags: PageTableFlags,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)?.flush();
    }
    Ok(())
}
```

### 3.4 힙 할당자

```rust
// kernel/src/allocator.rs

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// 힙 시작 주소
pub const HEAP_START: usize = 0x_4444_4444_0000;
/// 힙 크기 (16 MiB)
pub const HEAP_SIZE: usize = 16 * 1024 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// 힙 초기화
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    Ok(())
}
```

---

## 4. 시리얼 출력

### 4.1 UART 드라이버

```rust
// kernel/src/serial.rs

use spin::Mutex;
use uart_16550::SerialPort;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
    });
}

/// 시리얼 포트로 출력
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// 시리얼 포트로 줄바꿈 포함 출력
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

pub fn init() {
    // lazy_static은 첫 접근 시 초기화되므로 명시적으로 강제 접근
    let _ = &*SERIAL1;
    serial_println!("Serial port initialized");
}
```

---

## 5. 테스트 프레임워크

### 5.1 커널 테스트 러너

```rust
// kernel/src/test.rs

use crate::serial_println;

pub trait Testable {
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &core::panic::PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    crate::interrupts::hlt_loop();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

### 5.2 테스트 매크로 및 모듈 구조

```rust
// kernel/src/lib.rs

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

pub mod allocator;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod test;

#[cfg(test)]
mod tests;

/// 커널 테스트 매크로
#[macro_export]
macro_rules! kernel_test {
    ($name:ident, $body:block) => {
        #[test_case]
        fn $name() {
            $body
        }
    };
}
```

```rust
// kernel/src/tests/mod.rs

mod basic;
```

### 5.3 예제 테스트

```rust
// kernel/src/tests/basic.rs

use crate::kernel_test;

kernel_test!(test_breakpoint_exception, {
    // 브레이크포인트 예외가 패닉을 일으키지 않아야 함
    x86_64::instructions::interrupts::int3();
});

kernel_test!(test_heap_allocation, {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    
    let heap_value = Box::new(42);
    assert_eq!(*heap_value, 42);
    
    let mut vec = Vec::new();
    for i in 0..100 {
        vec.push(i);
    }
    assert_eq!(vec.len(), 100);
});

kernel_test!(test_large_allocation, {
    use alloc::vec::Vec;
    
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
});
```

---

## 6. 빌드 설정

### 6.1 Cargo.toml

```toml
[package]
name = "kernel"
version = "0.1.0"
edition = "2021"

[dependencies]
bootloader_api = "0.11"
x86_64 = "0.15"
uart_16550 = "0.3"
spin = "0.9"
lazy_static = { version = "1.4", features = ["spin_no_std"] }
linked_list_allocator = "0.10"
log = "0.4"

# bootloader는 별도 빌드 크레이트에서 사용
# kernel 크레이트는 bootloader_api만 의존

[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
lto = true
```

### 6.2 boot/Cargo.toml (부트로더 빌드용)

```toml
[package]
name = "boot"
version = "0.1.0"
edition = "2021"

[dependencies]
bootloader = "0.11"

[build-dependencies]
bootloader = "0.11"
```

### 6.3 boot/src/main.rs

```rust
use bootloader::{BootConfig, DiskImageBuilder};
use std::path::PathBuf;

fn main() {
    let kernel_path = PathBuf::from(std::env::var("CARGO_BIN_FILE_KERNEL").unwrap());
    
    let mut builder = DiskImageBuilder::new(kernel_path);
    
    // UEFI 이미지 생성
    let uefi_path = PathBuf::from("target/uefi.img");
    builder.create_uefi_image(&uefi_path).unwrap();
    
    // BIOS 이미지 생성 (선택적)
    let bios_path = PathBuf::from("target/bios.img");
    builder.create_bios_image(&bios_path).unwrap();
    
    println!("Disk images created:");
    println!("  UEFI: {}", uefi_path.display());
    println!("  BIOS: {}", bios_path.display());
}
```

### 6.4 .cargo/config.toml

```toml
[build]
target = "x86_64-kpio.json"

[unstable]
build-std = ["core", "alloc", "compiler_builtins"]
build-std-features = ["compiler-builtins-mem"]

[alias]
kbuild = "build --release -p kernel"
krun = "run --release -p boot"
```

### 6.5 x86_64-kpio.json (커스텀 타겟)

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true,
    "linker-flavor": "ld.lld",
    "linker": "rust-lld",
    "panic-strategy": "abort",
    "disable-redzone": true,
    "features": "-mmx,-sse,+soft-float"
}
```

**타겟 설정 설명:**

| 옵션 | 값 | 설명 |
|------|-----|------|
| `disable-redzone` | true | 인터럽트 안전성을 위해 red zone 비활성화 |
| `features` | `-mmx,-sse,+soft-float` | 커널에서 SIMD 비활성화 (FPU 상태 관리 복잡성 회피) |
| `panic-strategy` | abort | 패닉 시 unwinding 대신 즉시 중단 |

---

## 7. 병렬 작업

Phase 0 진행 중 병렬로 수행 가능한 작업:

| 작업 | 담당 | 의존성 |
|------|------|--------|
| 문서화 (mdBook) | 별도 | 없음 |
| CI/CD 파이프라인 | 별도 | 없음 |
| 로고/브랜딩 | 별도 | 없음 |
| 기획서 상세화 | 별도 | 없음 |

---

## 8. QEMU 실행 (로컬)

QEMU 테스트는 **로컬 환경에서 실행**하는 것을 권장합니다:

- 빠른 피드백 루프
- 디버거(GDB) 연동 용이
- 그래픽 출력 확인 가능
- CI 리소스 절약

### 8.1 QEMU 실행 스크립트

```powershell
# scripts/run-qemu.ps1

param(
    [switch]$Debug,
    [switch]$NoGraphic
)

$QemuArgs = @(
    "-machine", "q35",
    "-cpu", "qemu64",
    "-m", "512M",
    "-serial", "stdio",
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"
)

if ($NoGraphic) {
    $QemuArgs += "-nographic"
}

if ($Debug) {
    $QemuArgs += @("-s", "-S")  # GDB 서버 시작, 시작 시 일시정지
    Write-Host "Waiting for GDB connection on port 1234..."
}

# UEFI 부팅
$QemuArgs += @(
    "-bios", "OVMF.fd",
    "-drive", "format=raw,file=target/uefi.img"
)

qemu-system-x86_64 @QemuArgs
```

### 8.2 GDB 디버깅

```bash
# 별도 터미널에서
gdb -ex "target remote :1234" -ex "symbol-file target/x86_64-kpio/release/kernel"
```

---

## 9. 검증 체크리스트

Phase 0 완료 전 확인 사항:

### 로컬 테스트 (필수)

- [ ] `cargo kbuild` 성공
- [ ] `cargo run -p boot` 디스크 이미지 생성
- [ ] QEMU에서 "Hello, Kernel" 시리얼 출력
- [ ] 브레이크포인트 예외 처리 정상
- [ ] 힙 할당 테스트 통과
- [ ] 대용량 할당 테스트 통과
- [ ] 페이지 폴트 핸들러 동작
- [ ] 더블 폴트 핸들러 동작
- [ ] 물리 메모리 오프셋 검증 통과
- [ ] 시리얼 출력 안정적

### CI 검증 (빌드만)

- [ ] `cargo build --release` 성공
- [ ] `cargo clippy` 경고 없음
- [ ] `cargo fmt --check` 통과
