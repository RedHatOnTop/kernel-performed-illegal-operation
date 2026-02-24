# Kernel Performed Illegal Operation (KPIO)

**Version:** 2.5.0  
**Status:** Phase 9 In Progress (9-1, 9-2 Complete, 9-3 In Progress) ✅  
**License:** MIT / Apache-2.0 (Dual Licensed)

---

## Overview

KPIO is a next-generation, general-purpose operating system designed to eliminate the fragility of legacy systems. It functions as a high-performance **"Rescue & Utility"** platform that provides a stable host for modern web-native workloads.

The OS adopts a **WASM-Native** architecture, enforcing strict isolation by using WebAssembly for all user-space applications. It leverages a **Vulkan-exclusive** graphics stack to achieve native performance without legacy overhead.

### Unique Value Proposition

**OS-Level Browser Integration:** KPIO includes a Servo-based browser engine deeply integrated with the kernel, achieving unprecedented efficiency:
- **4x faster** tab cold start (0.5s vs 2-3s)
- **3x less memory** per tab (30-80MB vs 100-300MB)  
- **Native-level** WASM execution (shared runtime with OS)
- **Kernel-level** ad blocking (DNS + network layer)

---

## Core Philosophy

1. **"No Illegal Operations"** - The kernel guarantees stability. Applications are sandboxed in WebAssembly; a crash in an app never panics the kernel.

2. **Web-Native Performance** - Applications run as WASM binaries but bypass the heavy browser DOM, interfacing directly with the GPU via minimal abstractions.

3. **The "Vulkan Mandate"** - The OS exclusively supports Vulkan. Legacy APIs (OpenGL) are handled via translation layers (Mesa/Zink).

4. **Immutable Foundation** - The system root is read-only. Updates are atomic. System corruption is architecturally impossible.

---

## Architecture Summary

| Layer | Technology | Purpose |
|-------|------------|---------|
| Kernel | Rust (`no_std`) | Hardware abstraction, memory management, scheduling |
| Runtime | Custom interpreter + JIT | WebAssembly execution environment (no_std compatible) |
| Browser | Servo (Stylo + WebRender) | Full web standards support with OS integration |
| Graphics | Mesa 3D + Vulkan | GPU acceleration via RADV/ANV/NVK |
| Compositor | wgpu + Vello | Window management and vector rendering |
| Network | Custom TCP/IP + VirtIO PIO | Standalone TCP/IP stack with real NIC I/O; DHCP-acquired addressing |
| Storage | Custom VFS | Immutable root with FUSE-like WASM modules |

---

## Documentation

All project documentation is located in the `docs/` directory:

- [Architecture Overview](docs/architecture/README.md) - System architecture and design decisions
- [Kernel Design](docs/architecture/kernel.md) - Ring 0 kernel implementation details
- [Graphics Subsystem](docs/architecture/graphics.md) - Vulkan-exclusive graphics stack
- [WebAssembly Runtime](docs/architecture/wasm-runtime.md) - WASM execution and sandboxing
- [Networking](docs/architecture/networking.md) - TCP/IP stack and driver support
- [Storage](docs/architecture/storage.md) - File system and rescue capabilities
- [Linux Compatibility](docs/architecture/linux-compat.md) - Linux binary compatibility layer
- [Development Roadmap](docs/roadmap.md) - Phase-by-phase development plan
- [WASM App Guide (Rust)](docs/guides/WASM_APP_RUST.md) - Build WASM apps with Rust
- [WASM App Guide (C/C++)](docs/guides/WASM_APP_C_CPP.md) - Build WASM apps with C/C++
- [API Reference](docs/guides/KPIO_APP_API_REFERENCE.md) - KPIO App API reference
- [Contributing Guide](docs/CONTRIBUTING.md) - How to contribute to the project
- [Build Instructions](docs/building.md) - How to build and test the OS

---

## Project Structure

```
kernel-performed-illegal-operation/
    .cargo/                     # Cargo configuration for bare-metal targets
    docs/                       # Comprehensive documentation
        architecture/           # Architecture design documents
        guides/                 # Developer guides (WASM app development)
        phase7/                 # Phase 7 implementation docs
    kernel/                     # Ring 0 kernel implementation
        src/
            arch/               # Architecture-specific code (x86_64)
            boot/               # UEFI bootloader integration
            memory/             # Memory management subsystem
            scheduler/          # Task scheduling
            drivers/            # Hardware drivers
            ipc/                # Inter-process communication
            app/                # App management (lifecycle, registry)
    runtime/                    # WASM runtime (custom interpreter + JIT)
        src/
            component/          # WASM Component Model (canonical ABI)
            jit/                # JIT compiler (IR + x86_64 codegen)
            wasi.rs             # WASI Preview 1
            wasi2/              # WASI Preview 2 (streams, clocks, random, sockets)
            wit/                # WIT parser and type system
            package.rs          # .kpioapp package format
            app_launcher.rs     # App lifecycle management
            registry.rs         # App registry
            posix_shim.rs       # POSIX to WASI P2 shim
    graphics/                   # Graphics subsystem
        src/
            compositor/         # Window compositor
            renderer/           # Vector rendering (Vello)
    network/                    # Network stack
        src/
            stack/              # smoltcp integration
            drivers/            # Network drivers (VirtIO, E1000)
    storage/                    # Storage subsystem
        src/
            vfs/                # Virtual File System
            fs/                 # File system implementations
    examples/                   # Sample apps (.kpioapp examples)
    plans/                      # Phase implementation plans
    tests/                      # Integration tests
    tools/                      # Build and development tools
```

---

## Quick Start

### Prerequisites

- Rust nightly toolchain with `rust-src` component
- QEMU for virtualization testing
- OVMF (UEFI firmware for QEMU)

### Building

```bash
# Install required Rust components
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
rustup target add x86_64-unknown-none --toolchain nightly

# Build the kernel
cargo build --release

# Create bootable image
cargo run --package tools -- build-image

# Run in QEMU
cargo run --package tools -- run-qemu
```

---

## Current Status

**Phase 9: Real I/O — VirtIO Driver Completion & Stack Integration** - 🔄 In Progress (9-1, 9-2 Complete; 9-3 actively integrating)

- ✅ **9-1: VirtIO Net PIO Driver Implementation** — Replaced stub PIO `read8`/`write8`/`read32`/`write32` methods with real `x86_64::instructions::port::Port` I/O. Implemented full PIO init path (reset → ACKNOWLEDGE → DRIVER → feature negotiation → FEATURES_OK → MAC read → virtqueue allocation → DRIVER_OK). Added `pio_reg` constants, `VirtqRings` ring memory tracking, PCI bus mastering, and `probe()` now calls `init_pio()` to fully initialize the NIC on boot. Both MMIO and PIO transport modes are now functional.
- ✅ **9-2: Network Stack Wiring (NIC Registration + DHCP)** — NIC registration in `NETWORK_MANAGER` and DHCP handshake over real VirtIO NIC are completed and verified in QEMU (`10.0.2.15` lease acquisition).
- 🔄 **9-3: VFS ↔ Block Driver Integration** — Kernel-side `KernelBlockAdapter` and storage-side FAT32/VFS read path were implemented; test-disk workflow was added (`scripts/create-test-disk.ps1`, `-TestDisk` in QEMU scripts). Current blocker: VirtIO block sector read timeout on mount path in QEMU (`[VirtIO-Blk] Read timeout (sector 0)`) prevents full QG pass.

**Previous:** Phase 8 — Technical Debt Resolution ✅ (2026-02-23)

**Next:** Finish 9-3 block read reliability and complete 9-4 (WASI2 real network).

See [Development Roadmap](docs/roadmap.md) for detailed progress tracking.

---

## License

This project is dual-licensed under MIT and Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
