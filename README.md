# Kernel Performed Illegal Operation (KPIO)

**Version:** 1.0.0  
**Status:** Initial Development  
**License:** MIT / Apache-2.0 (Dual Licensed)

---

## Overview

KPIO is a next-generation, general-purpose operating system designed to eliminate the fragility of legacy systems. It functions as a high-performance **"Rescue & Utility"** platform that provides a stable host for modern web-native workloads.

The OS adopts a **WASM-Native** architecture, enforcing strict isolation by using WebAssembly for all user-space applications. It leverages a **Vulkan-exclusive** graphics stack to achieve native performance without legacy overhead.

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
| Runtime | Wasmtime | WebAssembly execution environment |
| Graphics | Mesa 3D + Vulkan | GPU acceleration via RADV/ANV/NVK |
| Compositor | wgpu + Vello | Window management and vector rendering |
| Network | smoltcp | Standalone TCP/IP stack |
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
- [Development Roadmap](docs/roadmap.md) - Phase-by-phase development plan
- [Contributing Guide](docs/CONTRIBUTING.md) - How to contribute to the project
- [Build Instructions](docs/building.md) - How to build and test the OS

---

## Project Structure

```
kernel-performed-illegal-operation/
    .cargo/                     # Cargo configuration for bare-metal targets
    .github/                    # GitHub Actions CI/CD workflows
    docs/                       # Comprehensive documentation
        architecture/           # Architecture design documents
        specifications/         # Technical specifications
    kernel/                     # Ring 0 kernel implementation
        src/
            arch/               # Architecture-specific code (x86_64)
            boot/               # UEFI bootloader integration
            memory/             # Memory management subsystem
            scheduler/          # Task scheduling
            drivers/            # Hardware drivers
            ipc/                # Inter-process communication
    runtime/                    # WASM runtime integration
        src/
            wasi/               # WASI implementation
            gpu/                # GPU extensions for WASM
    graphics/                   # Graphics subsystem
        src/
            drm/                # Display Resource Management
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
    userspace/                  # User-space utilities and services
        src/
            shell/              # WASM-based shell
            init/               # Init system
    tools/                      # Build and development tools
    tests/                      # Integration and system tests
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

**Phase 01: Boot & Core** - In Development

See [Development Roadmap](docs/roadmap.md) for detailed progress tracking.

---

## License

This project is dual-licensed under MIT and Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
