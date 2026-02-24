# Building KPIO

This document provides comprehensive instructions for building the KPIO operating system from source.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Quick Start](#quick-start)
3. [Detailed Build Instructions](#detailed-build-instructions)
4. [Build Targets](#build-targets)
5. [Configuration Options](#configuration-options)
6. [Running in QEMU](#running-in-qemu)
7. [Debugging](#debugging)
8. [Creating Bootable Media](#creating-bootable-media)
9. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Software

| Software | Version | Purpose |
|----------|---------|---------|
| Rust | nightly-2026-01-01+ | Compiler |
| QEMU | 7.0+ | Testing |
| Python | 3.8+ | Build scripts |
| NASM | 2.15+ | Assembly (optional) |
| mtools | 4.0+ | FAT image creation |

### Platform-Specific Requirements

#### Windows

```powershell
# Install Rust
winget install Rustlang.Rust.MSVC

# Install QEMU
winget install QEMU.QEMU

# Install Python
winget install Python.Python.3.11

# Install build tools
# Visual Studio Build Tools with C++ workload
winget install Microsoft.VisualStudio.2022.BuildTools
```

#### Linux (Ubuntu/Debian)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install QEMU and tools
sudo apt update
sudo apt install qemu-system-x86 python3 python3-pip mtools nasm

# Install additional build dependencies
sudo apt install build-essential pkg-config libssl-dev
```

#### macOS

```bash
# Install Homebrew if not present
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install Rust
brew install rustup
rustup-init

# Install QEMU and tools
brew install qemu python mtools nasm
```

### Rust Toolchain Setup

```bash
# Clone the repository first (contains rust-toolchain.toml)
git clone https://github.com/kpio/kpio.git
cd kpio

# Rust toolchain will be installed automatically via rustup
rustup show

# Manually install if needed
rustup toolchain install nightly-2026-01-01
rustup target add x86_64-unknown-none
rustup component add rust-src llvm-tools-preview

# Install cargo tools
cargo install bootimage
cargo install cargo-xbuild
```

---

## Quick Start

```bash
# Clone and enter repository
git clone https://github.com/kpio/kpio.git
cd kpio

# Build the kernel
cargo build --release

# Create bootable image
cargo bootimage --release

# Run in QEMU
cargo run --release
```

### Phase 9-3 storage integration check

```powershell
# Create FAT32 test disk image with HELLO.TXT
.\scripts\create-test-disk.ps1

# Run QEMU with additional test disk attached
.\scripts\qemu-test.ps1 -Mode custom -TestDisk .\tests\e2e\test-disk.img `
    -Expect "[VFS] Mounted FAT filesystem","[VFS] Self-test: read"
```

### Phase 9-5 full I/O integration test

```powershell
# Run the end-to-end I/O integration test (auto-attaches test disk)
.\scripts\qemu-test.ps1 -Mode io

# With verbose serial output
.\scripts\qemu-test.ps1 -Mode io -Verbose
```

This validates VirtIO NIC init, DHCP, packet TX/RX, VFS mount, and disk read in a single automated run.

---

## Detailed Build Instructions

### Project Structure

```
kpio/
    Cargo.toml              # Workspace configuration
    .cargo/
        config.toml         # Cargo configuration
    kernel/
        Cargo.toml          # Kernel crate
        src/
            main.rs         # Kernel entry point
    runtime/
        Cargo.toml          # WASM runtime crate
    graphics/
        Cargo.toml          # Graphics subsystem
    network/
        Cargo.toml          # Network stack
    storage/
        Cargo.toml          # Storage subsystem
    userspace/
        Cargo.toml          # User-space applications
```

### Build Commands

#### Debug Build

```bash
# Full debug build (slower, more diagnostics)
cargo build

# Build specific crate
cargo build -p kernel

# Build with verbose output
cargo build -v
```

#### Release Build

```bash
# Optimized build
cargo build --release

# With specific optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

#### Feature Flags

```bash
# Enable specific features
cargo build --features "debug-console,memory-stats"

# Disable default features
cargo build --no-default-features --features "minimal"
```

### Build Artifacts

| Artifact | Location | Description |
|----------|----------|-------------|
| Kernel ELF | `target/x86_64-unknown-none/release/kernel` | Raw kernel binary |
| Bootimage | `target/x86_64-unknown-none/release/bootimage-kernel.bin` | Bootable image |
| ISO | `target/kpio.iso` | Bootable ISO image |

---

## Build Targets

### Target Specification

The custom target is defined in `x86_64-kpio.json`:

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128",
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

### Cross-Compilation

```bash
# Build for the custom target
cargo build --target x86_64-kpio.json

# Or using .cargo/config.toml (automatic)
cargo build
```

---

## Configuration Options

### Cargo Configuration

`.cargo/config.toml`:

```toml
[build]
target = "x86_64-kpio.json"

[target.x86_64-kpio]
runner = "bootimage runner"

[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
build-std-features = ["compiler-builtins-mem"]

[env]
KERNEL_LOG_LEVEL = "debug"
```

### Kernel Configuration

`kernel/src/config.rs`:

```rust
/// Kernel configuration constants
pub mod config {
    /// Maximum number of CPUs supported
    pub const MAX_CPUS: usize = 64;
    
    /// Kernel heap size in bytes
    pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MB
    
    /// Maximum number of processes
    pub const MAX_PROCESSES: usize = 4096;
    
    /// Stack size per kernel task
    pub const KERNEL_STACK_SIZE: usize = 64 * 1024; // 64 KB
    
    /// Enable kernel debugging features
    pub const DEBUG_ENABLED: bool = cfg!(debug_assertions);
    
    /// Serial port for debug output
    pub const DEBUG_SERIAL_PORT: u16 = 0x3F8;
}
```

### Feature Flags

`kernel/Cargo.toml`:

```toml
[features]
default = ["serial-console", "memory-stats"]

# Debug features
serial-console = []
memory-stats = []
debug-interrupts = []
trace-syscalls = []

# Hardware support
acpi = []
smp = []
nvme = []

# Minimal build for testing
minimal = []
```

---

## Running in QEMU

### Basic Execution

```bash
# Run with default settings
cargo run --release

# Equivalent to:
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-kpio/release/bootimage-kernel.bin \
    -serial stdio
```

### Advanced QEMU Options

```bash
# With more memory and CPUs
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-kpio/release/bootimage-kernel.bin \
    -m 2G \
    -smp 4 \
    -serial stdio \
    -enable-kvm

# With VirtIO devices
qemu-system-x86_64 \
    -drive format=raw,file=bootimage-kernel.bin \
    -m 2G \
    -device virtio-net-pci,netdev=net0 \
    -netdev user,id=net0 \
    -device virtio-gpu-pci \
    -device virtio-blk-pci,drive=hd0 \
    -drive file=disk.img,id=hd0,format=raw \
    -serial stdio

# With networking (port forwarding)
qemu-system-x86_64 \
    -drive format=raw,file=bootimage-kernel.bin \
    -m 2G \
    -device virtio-net-pci,netdev=net0 \
    -netdev user,id=net0,hostfwd=tcp::8080-:80 \
    -serial stdio
```

### QEMU Monitor

Press `Ctrl+A, C` to access QEMU monitor:

```
(qemu) info registers     # Show CPU registers
(qemu) info mem           # Show memory mappings
(qemu) x /10i $pc         # Disassemble at PC
(qemu) quit               # Exit QEMU
```

---

## Debugging

### GDB Debugging

```bash
# Terminal 1: Start QEMU with GDB server
qemu-system-x86_64 \
    -drive format=raw,file=bootimage-kernel.bin \
    -s -S \
    -serial stdio

# Terminal 2: Connect GDB
gdb target/x86_64-kpio/release/kernel

(gdb) target remote :1234
(gdb) break kernel_main
(gdb) continue
```

### GDB Commands

```gdb
# Useful commands
break *0xFFFF8000DEADBEEF  # Break at address
watch *0xFFFF8000DEADBEEF  # Watch memory location
info registers             # Show registers
x/10i $rip                 # Disassemble at RIP
x/10gx $rsp                # Show stack
bt                         # Backtrace
```

### VS Code Debugging

`.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "name": "KPIO Debug",
            "type": "cppdbg",
            "request": "launch",
            "program": "${workspaceFolder}/target/x86_64-kpio/debug/kernel",
            "miDebuggerServerAddress": "localhost:1234",
            "miDebuggerPath": "gdb",
            "cwd": "${workspaceFolder}",
            "preLaunchTask": "qemu-debug"
        }
    ]
}
```

### Serial Console Logging

```rust
// In kernel code
use crate::serial::serial_println;

serial_println!("[DEBUG] Value: {:#x}", value);
```

---

## Creating Bootable Media

### USB Drive (Linux)

```bash
# Build the image
cargo bootimage --release

# Find USB device (CAREFUL!)
lsblk

# Write to USB (replace sdX)
sudo dd if=target/x86_64-kpio/release/bootimage-kernel.bin of=/dev/sdX bs=4M status=progress
sync
```

### USB Drive (Windows)

```powershell
# Build the image
cargo bootimage --release

# Use Rufus or similar tool to write the image
# Select the bootimage-kernel.bin file
# Choose DD mode
```

### ISO Image

```bash
# Create ISO for CD/DVD or legacy BIOS boot
mkdir -p isodir/boot/grub
cp target/x86_64-kpio/release/kernel isodir/boot/
cp grub.cfg isodir/boot/grub/

grub-mkrescue -o kpio.iso isodir
```

`grub.cfg`:

```
set timeout=0
set default=0

menuentry "KPIO" {
    multiboot2 /boot/kernel
    boot
}
```

---

## Troubleshooting

### Common Build Errors

#### "error: no matching package found"

```bash
# Update cargo index
cargo update

# Clean and rebuild
cargo clean
cargo build
```

#### "error: linker 'rust-lld' not found"

```bash
# Install LLVM tools
rustup component add llvm-tools-preview
```

#### "error[E0463]: can't find crate for 'core'"

```bash
# Install rust-src
rustup component add rust-src

# Verify build-std is enabled in .cargo/config.toml
```

#### "QEMU: No bootable device"

```bash
# Ensure bootimage is built
cargo bootimage --release

# Check file exists
ls -la target/x86_64-kpio/release/bootimage-kernel.bin
```

### QEMU Issues

#### Black Screen / No Output

```bash
# Add serial output
qemu-system-x86_64 ... -serial stdio

# Check VGA mode
qemu-system-x86_64 ... -vga std
```

#### KVM Errors

```bash
# Check KVM is available
ls -la /dev/kvm

# Add current user to kvm group
sudo usermod -aG kvm $USER
# Then log out and back in

# Run without KVM if unavailable
# (Remove -enable-kvm flag)
```

### Debug Techniques

#### Early Boot Issues

Add serial output at the earliest point:

```rust
// In boot code
unsafe {
    // Direct port I/O
    core::arch::asm!(
        "mov dx, 0x3F8",
        "mov al, 'X'",
        "out dx, al",
    );
}
```

#### Memory Issues

Enable memory debugging:

```bash
cargo build --features "memory-stats,debug-allocator"
```

#### Triple Fault

Common causes:
1. Stack overflow
2. Invalid page table
3. GDT/IDT issues

Debug steps:
```bash
# Run with interrupt logging
qemu-system-x86_64 ... -d int,cpu_reset

# Check log
cat qemu.log
```

---

## Build Optimization

### Profile-Guided Optimization

```bash
# Generate profile data
cargo build --release
./run_benchmarks.sh  # Generates profile data

# Build with PGO
RUSTFLAGS="-Cprofile-use=profile.profdata" cargo build --release
```

### Link-Time Optimization

`Cargo.toml`:

```toml
[profile.release]
lto = true
codegen-units = 1
panic = "abort"
```

### Size Optimization

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

---

## CI/CD Integration

### GitHub Actions

`.github/workflows/build.yml`:

```yaml
name: Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-action@stable
      with:
        toolchain: nightly-2026-01-01
        components: rust-src, llvm-tools-preview
        targets: x86_64-unknown-none
    
    - name: Install bootimage
      run: cargo install bootimage
    
    - name: Build
      run: cargo build --release
    
    - name: Create bootimage
      run: cargo bootimage --release
    
    - name: Run tests
      run: cargo test
    
    - name: Upload artifact
      uses: actions/upload-artifact@v4
      with:
        name: bootimage
        path: target/x86_64-kpio/release/bootimage-kernel.bin
```

---

## Questions?

- Check the FAQ in the wiki
- Open a discussion on GitHub
- Join our community chat
