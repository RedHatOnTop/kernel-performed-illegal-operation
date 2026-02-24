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

## 2. ~~VirtIO Network — PIO Mode Not Fully Implemented~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~Low~~ → Resolved                                                    |
| Component   | `kernel/src/drivers/net/virtio_net.rs`                                 |
| Status      | **Fixed** in Phase 9-1 (2026-02-23)                                   |

### Resolution

Phase 9-1 implemented real PIO register access using `x86_64::instructions::port::Port`.
The VirtIO net driver now supports the full legacy PCI init sequence: device reset,
feature negotiation, MAC address read, virtqueue allocation (descriptor table, available
ring, used ring), and DRIVER_OK. `probe()` enables PCI bus mastering and calls
`init_pio()` for each discovered NIC.

---

## 3. ~~DHCP Timeout Delay on Boot~~ ✅ RESOLVED

| Field       | Detail                                                         |
|-------------|----------------------------------------------------------------|
| Severity    | ~~Low~~ → Resolved                                             |
| Component   | `kernel/src/net/dhcp.rs`                                       |
| Status      | **Fixed** in Phase 9-2 (2026-02-24)                            |

### Resolution

Phase 9-2 wired the VirtIO NIC into `NETWORK_MANAGER`, implemented `virt_to_phys()` for
correct DMA address translation, fixed the virtqueue size to match the device's read-only
`QUEUE_SIZE` register (256 entries), and negotiated the `MRG_RXBUF` feature for correct
12-byte VirtIO net headers. DHCP now completes on the first attempt in under 1 second:

```
[DHCP] Lease acquired: 10.0.2.15 (gw 10.0.2.2, dns 10.0.2.3, mask 255.255.255.0, lease=86400s)
[VirtIO Net] TX: 2 packets (684 bytes), RX: 2 packets (1180 bytes)
```

---

## 4. ~~Phase 9-3 Blocker — VirtIO-Blk Read Timeout During FAT Mount~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~High~~ → Resolved                                                     |
| Component   | `kernel/src/driver/virtio/block.rs` + storage mount path              |
| Status      | **Fixed** in Phase 9-3 DMA fix (2026-02-24)                            |

### Resolution

The VirtIO block driver was using **virtual** addresses where **physical** DMA
addresses are required. Three fixes applied (mirroring the 9-2 net driver fix):

1. Queue memory allocated with page-aligned `Layout` (`alloc_zeroed` with 4096 alignment)
2. Queue PFN written to `QUEUE_ADDRESS` register using `virt_to_phys()` translation
3. All descriptor buffer addresses (header, data, status) translated via `virt_to_phys()`

Boot now shows:
```
[VirtIO-Blk] Queue mem virt=0x444444447000 phys=0xa000 pfn=0xa
[VFS] Mounted FAT filesystem on virtio-blk0 at /mnt/test
[VFS] Self-test: read 36 bytes from HELLO.TXT
[VFS] readdir /mnt/test/: 1 entries
  - HELLO.TXT (Regular)
[VFS] Self-test: NOFILE.TXT correctly not found
```

---

## 5. ACPI Misaligned Pointer Panic

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | Low (non-blocking — occurs after all critical init completes)          |
| Component   | `kernel/src/hw/acpi.rs:242`                                            |
| Status      | **Open** — workaround in place (ACPI init moved after network stack)   |

### Symptom

During ACPI initialization, the kernel panics with a misaligned pointer dereference:

```
KERNEL PANIC
Location: kernel\src\hw\acpi.rs:242:39
Message: misaligned pointer dereference: address must be a multiple of 0x8 but is 0x2801f77d10c
```

### Root Cause

The ACPI RSDP table in QEMU's UEFI firmware is at a physical address (`0x1f77e014`) whose
virtual mapping through the physical memory window is not 8-byte aligned. Rust's strict
alignment checks (enabled in debug builds) trigger a panic when casting the raw pointer.

### Workaround

As of Phase 9-2, the kernel init order has been rearranged so that PCI, VirtIO, and
the network stack are initialized **before** ACPI. This ensures all I/O is functional
before the ACPI panic occurs. The ACPI subsystem is not currently required for core
operation.

