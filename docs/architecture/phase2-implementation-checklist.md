# Phase 2 Implementation Checklist

**Version:** 1.0  
**Start Date:** TBD  
**Estimated Duration:** 20 weeks

---

## Phase 2.1: Userspace Infrastructure (4 weeks)

### Week 1-2: Process Management

#### 1.1 ELF Loader
- [ ] Parse ELF64 headers
- [ ] Process program headers
- [ ] Load sections (.text, .data, .bss, .rodata)
- [ ] Handle relocations (R_X86_64_*)
- [ ] Support dynamic linking (optional)

```rust
// kernel/src/loader/elf.rs
pub struct Elf64Loader {
    pub fn load(binary: &[u8]) -> Result<LoadedProgram, ElfError>;
    pub fn relocate(program: &mut LoadedProgram) -> Result<(), ElfError>;
}
```

#### 1.2 Userspace memory
- [ ] Define a userspace virtual address layout
  - `0x0000_0000_0040_0000` - start of text
  - `0x0000_7FFF_FFFF_F000` - start of stack
- [ ] Separate page tables (kernel/userspace)
- [ ] Copy-on-write fork

#### 1.3 System call interface
- [ ] Syscall entrypoint (SYSCALL instruction)
- [ ] System call table
- [ ] Argument validation
- [ ] Userspace pointer validation

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
// ... 50+ syscalls expected
```

#### 1.4 Basic process management
- [ ] Process creation (fork/exec or spawn)
- [ ] Process exit (exit, wait)
- [ ] Process ID management
- [ ] Process state machine

---

### Week 3-4: IPC System

#### 2.1 Shared memory
- [ ] `mmap` system call (MAP_SHARED)
- [ ] Create/open shared memory objects
- [ ] Share physical pages
- [ ] Reference counting

```rust
// kernel/src/ipc/shm.rs
pub struct SharedMemory {
    pub fn create(name: &str, size: usize) -> Result<ShmId, Error>;
    pub fn open(name: &str) -> Result<ShmId, Error>;
    pub fn map(id: ShmId, offset: usize, size: usize) -> Result<*mut u8, Error>;
    pub fn unmap(addr: *mut u8, size: usize) -> Result<(), Error>;
}
```

#### 2.2 Ring buffer IPC
- [ ] Implement a lock-free SPSC queue
- [ ] Implement a lock-free MPSC queue
- [ ] Event notification (futex-based)
- [ ] Flow control (backpressure)

#### 2.3 Capability system
- [ ] Capability ID generation
- [ ] Permission checks
- [ ] Delegation with authority reduction (diminish)
- [ ] Capability table

#### 2.4 Browser ↔ kernel channel
- [ ] Define `KernelToBrowser` messages
- [ ] Define `BrowserToKernel` messages
- [ ] Channel initialization
- [ ] Bidirectional communication test

---

## Phase 2.2: Servo Porting (8 weeks)

### Week 5-8: Minimal Servo Build

#### 3.1 Cross-compilation environment
- [ ] Define the x86_64-unknown-kpio target
- [ ] Build script for rust-std
- [ ] Cargo configuration (.cargo/config.toml)

#### 3.2 libkpio (libc replacement)
- [ ] Define basic types (size_t, ssize_t, ...)
- [ ] String functions (strlen, memcpy, ...)
- [ ] Memory allocation (malloc, free, realloc)
- [ ] Environment variable stubs

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

#### 3.3 Implement `std` features (libkpio-std)

**Required modules:**
- [ ] `std::alloc` - GlobalAlloc implementation
- [ ] `std::thread` - thread create/join
- [ ] `std::sync` - Mutex, Condvar, RwLock
- [ ] `std::fs` - File, OpenOptions
- [ ] `std::io` - Read, Write, Seek
- [ ] `std::net` - TcpStream, UdpSocket
- [ ] `std::time` - Instant, Duration
- [ ] `std::env` - environment variables

**Optional modules (later):**
- [ ] `std::process` - process creation
- [ ] `std::os::unix` - Unix extensions

#### 3.4 Servo compilation tests
- [ ] Build html5ever
- [ ] Build cssparser
- [ ] Minimal Servo build (console logging only)

---

### Week 9-12: Rendering Pipeline

#### 4.1 Vulkan driver (userspace)
- [ ] Investigate VirtIO-GPU Vulkan extensions
- [ ] Vulkan loader (libvulkan)
- [ ] Create VkInstance
- [ ] Create VkDevice
- [ ] Acquire VkQueue

#### 4.2 Port surfman
- [ ] Add platform backend (KPIO)
- [ ] Create surfaces
- [ ] Context management

#### 4.3 Integrate WebRender
- [ ] Build WebRender
- [ ] Basic rendering test (solid rectangle)
- [ ] Text rendering
- [ ] Image rendering

#### 4.4 Render a basic page
- [ ] Render about:blank
- [ ] Render a simple HTML page
- [ ] Apply CSS styling
- [ ] Handle basic input events (click)

---

## Phase 2.3: OS Integration Optimizations (8 weeks)

### Week 13-16: GPU Scheduler

#### 5.1 GPU priority queues
- [ ] Define priority levels (High, Medium, Low)
- [ ] Separate per-tab queues
- [ ] Preemptive scheduling

```rust
// kernel/src/gpu/scheduler.rs
pub struct GpuScheduler {
    pub fn submit(&mut self, tab_id: TabId, cmds: GpuCommands, priority: Priority);
    pub fn set_tab_priority(&mut self, tab_id: TabId, priority: Priority);
    pub fn process_queue(&mut self);
}
```

#### 5.2 Foreground tab boosting
- [ ] Detect the active tab
- [ ] Adjust GPU time allocation
- [ ] Prioritize frame deadlines

#### 5.3 Background tab throttling
- [ ] Throttle requestAnimationFrame
- [ ] Delay GPU work
- [ ] Power-saving mode

#### 5.4 VSync synchronization
- [ ] Detect display refresh rate
- [ ] Frame submission timing
- [ ] Prevent tearing

---

### Week 17-20: Memory Optimizations

#### 6.1 Per-tab memory tracking
- [ ] Memory usage per process
- [ ] Track JS heap size
- [ ] Track image cache usage

#### 6.2 Background tab compression
- [ ] LZ4 compression
- [ ] Compression trigger conditions
- [ ] Restore performance optimization

```rust
// kernel/src/memory/tab_manager.rs
pub struct TabMemoryManager {
    pub fn compress_tab(&mut self, tab_id: TabId) -> Result<CompressedTab, Error>;
    pub fn restore_tab(&mut self, compressed: CompressedTab) -> Result<(), Error>;
    pub fn get_memory_usage(&self, tab_id: TabId) -> usize;
}
```

#### 6.3 Hibernated tab disk swap
- [ ] Swap file management
- [ ] Tab state serialization
- [ ] Optimize restore time (<1s)

#### 6.4 WASM AOT cache
- [ ] Compute module hashes
- [ ] Store/load AOT code
- [ ] Cache invalidation policy
- [ ] Disk cache

---

## Milestones

| Milestone | Week | Verification |
|----------|------|--------------|
| **M1: Hello Userspace** | Week 2 | Execute an ELF binary and print \"Hello\" |
| **M2: IPC Works** | Week 4 | Message passing between two processes |
| **M3: html5ever Runs** | Week 6 | Parse HTML and dump the DOM |
| **M4: Vulkan Triangle** | Week 10 | Render a triangle |
| **M5: Basic Page** | Week 12 | Render a simple web page |
| **M6: Google Loads** | Week 16 | Load google.com (slow is fine) |
| **M7: Performance** | Week 20 | Reach 50% of performance targets |
