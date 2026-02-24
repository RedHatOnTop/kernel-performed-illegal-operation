# Documentation

This directory contains design and implementation documentation for KPIO.

## Getting Started

- [Build Instructions](building.md)
- [Local Development Guide](guides/LOCAL_DEVELOPMENT.md)
- [Quick Start](QUICK_START.md)
- [Known Issues](known-issues.md)
- [Contributing](CONTRIBUTING.md)

## Architecture

- [System Architecture Overview](architecture/README.md)
- [Kernel Design](architecture/kernel.md)
- [Graphics Subsystem](architecture/graphics.md)
- [Networking](architecture/networking.md)
- [Storage](architecture/storage.md)
- [WebAssembly Runtime](architecture/wasm-runtime.md)
- [System Call Design](architecture/syscall-design.md)

## Developer Guides

- [WASM App Development with Rust](guides/WASM_APP_RUST.md)
- [WASM App Development with C/C++](guides/WASM_APP_C_CPP.md)
- [KPIO App API Reference](guides/KPIO_APP_API_REFERENCE.md)
- [Local Development Setup](guides/LOCAL_DEVELOPMENT.md)

## Phase 9: Real I/O — VirtIO Driver Completion (Complete ✅)

- [Phase 9 Plan](../plans/PHASE_9_REAL_IO_PLAN.md)
- **9-1**: VirtIO Net PIO Driver — Complete ✅
- **9-2**: Network Stack Wiring (NIC Registration & DHCP) — Complete ✅
- **9-3**: VFS ↔ Block Driver Integration — Complete ✅
- **9-4**: WASI2 Real Network Integration — Complete ✅
- **9-5**: End-to-End Integration Test — Complete ✅
- I/O integration test: `.\scripts\qemu-test.ps1 -Mode io`
- Test disk creation: `.\scripts\create-test-disk.ps1`
- Host HTTP server for testing: `python tests/e2e/http-server.py`

## Phase 8: Technical Debt Resolution (Complete)

- [Phase 8 Plan](../plans/PHASE_8_BUGFIX_PLAN.md)

## Phase 7: App Execution Layer

- [Phase 7-2: WASM App Runtime](phase7/PHASE_7-2_WASM_APP_RUNTIME_EN.md)
- [Phase 7-3: WASI App Runtime Plan](phase7/PHASE_7-3_WASI_APP_RUNTIME_PLAN.md)
- [Phase 7-4: Linux Binary Compatibility Layer](phase7/PHASE_7-4_LINUX_COMPAT_PLAN.md)
- [Web App Architecture](phase7/WEB_APP_ARCHITECTURE.md)
- [Web App Developer Guide](phase7/WEB_APP_DEVELOPER_GUIDE.md)

## Project Docs

- [Roadmap](roadmap.md)
- [Recommendations](RECOMMENDATIONS.md)
- [Design System](design-system.md)
- [Quick Start](QUICK_START.md)
- [User Guide](USER_GUIDE.md)
