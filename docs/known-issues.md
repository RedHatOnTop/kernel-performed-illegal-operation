# Known Issues

This document tracks known issues, limitations, and workarounds for KPIO OS.

---

## 1. BIOS Boot — FAT Parser Integer Overflow

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | Medium                                                                 |
| Component   | `bootloader` crate v0.11.14 (external dependency)                     |
| Affects     | BIOS boot path only; UEFI boot is **not** affected                    |
| Status      | **Won't fix** — external crate bug; UEFI pflash is the recommended workaround |
| Discovered  | Phase 7-4 QEMU boot verification (commit `1efe2d8`)                  |

### Symptom

When booting via BIOS in **debug** mode, the bootloader panics during FAT filesystem parsing:

```
panicked at 'attempt to multiply with overflow'
bootloader-x86_64-bios-0.11.14\...\fat.rs
```

### Root Cause

The `bootloader` crate v0.11.14 contains an arithmetic overflow in its internal FAT
filesystem parser. In debug builds, Rust's overflow checks detect the bug and panic.
In release builds, overflow checks are disabled so the multiplication silently wraps;
this may allow booting to proceed but can cause subtle data corruption.

Because this code lives inside the external `bootloader` crate, it cannot be patched in
the KPIO kernel source tree.

### Upstream Reference

- Crate: [`bootloader` on crates.io](https://crates.io/crates/bootloader) (v0.11.14)
- The `bootloader` 0.11.x series is in maintenance mode. The successor
  [`bootloader` v0.12+](https://github.com/rust-osdev/bootloader) has a rewritten boot
  flow that may resolve this issue.

### Workaround: Use UEFI pflash Boot

UEFI pflash boot bypasses the BIOS FAT parser entirely and is the **recommended** boot
method for KPIO OS. All QEMU run scripts default to UEFI mode.

```powershell
# Default (UEFI pflash) — recommended
.\scripts\run-qemu.ps1

# Explicitly avoid BIOS mode
.\scripts\run-qemu.ps1          # do NOT pass -Bios

# Automated testing — always uses UEFI pflash
.\scripts\qemu-test.ps1 -Mode boot
```

If you must test BIOS boot, use a **release** build to skip Rust's overflow checks:

```powershell
.\scripts\run-qemu.ps1 -Bios    # will likely fail in debug mode
```

> **Note:** Even with a release build, BIOS boot may exhibit other issues due to the
> wrapped arithmetic. UEFI pflash is strongly preferred.

---

## 2. VirtIO Network — PIO Mode Not Fully Implemented

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | Low (detection works; full driver is a future phase)                   |
| Component   | `kernel/src/drivers/net/virtio_net.rs`                                 |
| Status      | **Planned** — full PIO-based VirtIO net driver deferred to a future phase |

### Symptom

VirtIO network devices are **detected** during PCI enumeration and logged, but the
driver cannot send or receive packets because the PIO read/write functions are stubs
(return 0 / no-op).

### Workaround

Network functionality currently uses a static IP fallback (`10.0.2.15`) after DHCP
timeout. Full VirtIO NIC driver implementation is tracked for a future development
phase.

---

## 3. DHCP Timeout Delay on Boot

| Field       | Detail                                                         |
|-------------|----------------------------------------------------------------|
| Severity    | Low (cosmetic — adds ~3 s to boot)                             |
| Component   | `kernel/src/net/dhcp.rs`                                       |
| Status      | **Expected behavior** until VirtIO net driver is complete      |

### Symptom

During boot, the DHCP client sends a DISCOVER packet and waits up to ~3 seconds for a
reply. If no functional NIC driver is available, the timeout elapses and the kernel
falls back to a static configuration.

### Workaround

No action needed. The kernel proceeds normally after the timeout.
