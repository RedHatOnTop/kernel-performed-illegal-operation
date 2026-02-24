# KPIO Local Development Environment Guide

This document explains how to set up a local development and testing environment for the KPIO kernel.

## Prerequisites

### 1. Rust toolchain

```powershell
# Install rustup (if not installed)
# Download rustup-init.exe from https://rustup.rs

# Install a nightly toolchain
rustup toolchain install nightly-2026-01-01

# Install required components
rustup component add rust-src --toolchain nightly-2026-01-01
rustup component add llvm-tools --toolchain nightly-2026-01-01
```

### 2. QEMU (x86_64 emulator)

```powershell
# Install via winget (recommended)
winget install SoftwareFreedomConservancy.QEMU

# Or via Chocolatey
choco install qemu

# Or via Scoop
scoop install qemu
```

After installing, verify it is available on PATH:
```powershell
qemu-system-x86_64 --version
```

### 3. OVMF (UEFI firmware)

OVMF is usually installed with QEMU. Check these locations:

- `C:\Program Files\qemu\share\edk2-x86_64-code.fd`
- `C:\Program Files\QEMU\share\OVMF.fd`

If it is missing, download it manually:
```powershell
# Automatic download (attempted by setup-dev-env.ps1)
.\scripts\setup-dev-env.ps1

# Or manual download
# https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF.fd
# -> save to $HOME\.kpio\OVMF.fd
```

## Automatic Setup

To automatically validate and install all tools:

```powershell
.\scripts\setup-dev-env.ps1
```

## Script List

| Script | Description |
|--------|-------------|
| `setup-dev-env.ps1` | Configure the dev environment and install tools |
| `build-image.ps1` | Build the kernel + create a disk image |
| `run-qemu.ps1` | Run the kernel in QEMU |
| `quick-run.ps1` | Build + run immediately (convenience) |
| `run-tests.ps1` | Verify test builds |
| `create-uefi-image.ps1` | Create the ESP directory layout |

## Quick Start

### 1. Set up the development environment

```powershell
.\scripts\setup-dev-env.ps1
```

### 2. Build the kernel

```powershell
cargo build -p kpio-kernel --release
```

### 3. Quick run

```powershell
.\scripts\quick-run.ps1
```

### 4. Full build + run

```powershell
# Build the disk image
.\scripts\build-image.ps1

# Run QEMU
.\scripts\run-qemu.ps1
```

## Debugging

### Connect GDB

```powershell
# Terminal 1: start QEMU in debug mode
.\scripts\run-qemu.ps1 -Debug

# Terminal 2: connect GDB
gdb -ex "target remote :1234" -ex "symbol-file target/x86_64-unknown-none/release/kernel"
```

### Serial output

QEMU can print serial output to the terminal via `-serial stdio`.
Output from the kernel's `serial_println!` macro appears there.

### Run without graphics

```powershell
.\scripts\run-qemu.ps1 -NoGraphic
```

## Tests

### Test exit codes

| Code | Meaning |
|------|---------|
| 33 | Test success (QemuExitCode::Success) |
| 35 | Test failure (QemuExitCode::Failed) |

### Running tests

Kernel tests run via QEMU in `#[cfg(test)]` mode:

```powershell
# Verify the test build
.\scripts\run-tests.ps1

# Run tests in QEMU
.\scripts\quick-run.ps1
```

### Phase 9-3 VFS test workflow (VirtIO block + FAT)

```powershell
# 1) Create a FAT32 test image with HELLO.TXT
.\scripts\create-test-disk.ps1

# 2) Run QEMU test with extra VirtIO block disk attached
.\scripts\qemu-test.ps1 -Mode custom -TestDisk .\tests\e2e\test-disk.img `
    -Expect "[VFS] Mounted FAT filesystem","[VFS] Self-test: read"
```

If the mount path fails with `[VirtIO-Blk] Read timeout (sector 0)`, see `docs/known-issues.md` (Phase 9-3 blocker section).

## Troubleshooting

### QEMU is not found

```powershell
# Add QEMU to PATH
$env:PATH = "C:\Program Files\qemu;$env:PATH"

# Persist the setting (requires administrator privileges)
[Environment]::SetEnvironmentVariable("PATH", "C:\Program Files\qemu;$([Environment]::GetEnvironmentVariable('PATH', 'Machine'))", "Machine")
```

### OVMF cannot be found

```powershell
# Download OVMF manually
Invoke-WebRequest -Uri "https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF.fd" -OutFile "$HOME\.kpio\OVMF.fd"
```

### The kernel does not boot

1. Check serial output (`-serial stdio`)
2. Check the QEMU debug console (Ctrl+Alt+2)
3. Connect GDB and debug step-by-step

### Bootloader build failure

If building `tools/boot` fails, you can use `quick-run.ps1` to test via QEMU's direct kernel loading:

```powershell
.\scripts\quick-run.ps1
```

## Layout

```
scripts/
├── setup-dev-env.ps1      # environment setup
├── build-image.ps1        # disk image build
├── run-qemu.ps1           # run QEMU
├── quick-run.ps1          # build + run (quick path)
├── run-tests.ps1          # run tests
└── create-uefi-image.ps1  # create ESP directory layout

tools/boot/                # bootloader image builder
├── Cargo.toml
└── src/main.rs

target/
├── x86_64-unknown-none/release/
│   ├── kernel             # kernel ELF
│   ├── kpio-uefi.img      # UEFI disk image
│   └── kpio-bios.img      # BIOS disk image
└── esp/                   # ESP directory (virtual FAT)
    ├── EFI/BOOT/BOOTX64.EFI
    └── kernel
```
