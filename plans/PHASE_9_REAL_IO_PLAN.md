# Phase 9: Real I/O — VirtIO Driver Completion & Stack Integration

> **Status**: In Progress (9-1, 9-2 Complete)  
> **Predecessor**: Phase 8 (Technical Debt Resolution) — completed 2026-02-23  
> **Boot environment**: QEMU 10.2.0, UEFI pflash, bootloader 0.11.14, nightly-2026-01-01  
> **Goal**: Make VirtIO network and storage actually functional — DHCP succeeds, packets transmit/receive, VFS reads/writes reach disk  
> **Milestone**: *A WASM app running in QEMU downloads data via HTTP and persists it to a VirtIO block device*

---

## Context

Phase 8 resolved critical boot issues (ACPI page fault, boot ordering, NIC detection).
The kernel now discovers VirtIO NIC (`1af4:1000`) via PCI and logs it, but the driver
cannot send or receive packets because the PIO read/write functions are stubs. Similarly,
the VirtIO block driver in the kernel crate is fully implemented, but the storage crate's
VFS layer is disconnected — `read()`/`write()` calls never reach an actual filesystem.

The existing VirtIO **block** driver (`kernel/src/driver/virtio/block.rs`) already
demonstrates a complete PIO-mode VirtIO implementation using `x86_64::instructions::port::Port`.
Phase 9 replicates this proven pattern for the network driver and wires the full I/O path
end-to-end.

### Current Gaps

| # | Gap | Location | Impact |
|---|-----|----------|--------|
| 1 | VirtIO Net PIO `read8/write8/read32/write32` return 0 / no-op | `kernel/src/drivers/net/virtio_net.rs` L257-291 | No register access → device init impossible |
| 2 | `probe()` discovers NIC but does not call `init_pio()` | `kernel/src/drivers/net/virtio_net.rs` L684-700 | NIC never initialized |
| 3 | `init()` skips entire init sequence when `mmio_base.is_none()` | `kernel/src/drivers/net/virtio_net.rs` L293-377 | PIO path is dead code |
| 4 | VirtqAvail/VirtqUsed rings not allocated in memory | `kernel/src/drivers/net/virtio_net.rs` L379-430 | Virtqueue inoperative |
| 5 | NIC not registered in `NETWORK_MANAGER` → DHCP packets silently dropped | `kernel/src/net/mod.rs` L259-269 | No network TX/RX |
| 6 | Storage VFS `read()`/`write()` are TODO stubs | `storage/src/vfs.rs` L413, L425 | File I/O non-functional |
| 7 | Storage crate VirtIO driver is entirely stubbed | `storage/src/driver/virtio.rs` L188-251 | Block I/O disconnected |
| 8 | WASI2 HTTP returns mock responses | `runtime/src/wasi2/http.rs` L296-327 | WASM apps get fake data |
| 9 | WASI2 Sockets use in-memory loopback only | `runtime/src/wasi2/sockets.rs` L123-410 | No real TCP/UDP |

---

## Sub-phase 9-1: VirtIO Net PIO Driver Implementation

### Goal

Implement real PIO-mode register access for the VirtIO network device so that device
initialization, feature negotiation, virtqueue setup, and packet TX/RX all function
correctly when running on QEMU with `-device virtio-net-pci`.

### Scope

This sub-phase focuses **exclusively** on the kernel-side VirtIO net driver. The network
stack (`net::init()`, DHCP, etc.) is wired up in sub-phase 9-2.

### Root Cause

The four PIO accessor methods in `VirtioNetDevice` (`read8`, `write8`, `read32`,
`write32` at L257-291) have no implementation in the `else` (PIO) branch:

```rust
fn read8(&self, offset: u32) -> u8 {
    if let Some(mmio) = self.mmio_base {
        unsafe { ptr::read_volatile((mmio + offset as usize) as *const u8) }
    } else {
        0  // ← always returns 0
    }
}
```

The VirtIO **block** driver (`kernel/src/driver/virtio/block.rs` L161-290) already solves
this exact problem using `x86_64::instructions::port::Port`. Phase 9-1 applies the same
pattern.

### Tasks

1. **Add PIO register offset constants** — Define legacy PIO register offsets for VirtIO
   net (matching VirtIO 1.0 spec §4.1.4.8, legacy interface):
   ```rust
   mod pio_reg {
       pub const DEVICE_FEATURES: u16 = 0x00;   // 4 bytes
       pub const GUEST_FEATURES: u16  = 0x04;   // 4 bytes
       pub const QUEUE_ADDRESS: u16   = 0x08;   // 4 bytes
       pub const QUEUE_SIZE: u16      = 0x0C;   // 2 bytes
       pub const QUEUE_SELECT: u16    = 0x0E;   // 2 bytes
       pub const QUEUE_NOTIFY: u16    = 0x10;   // 2 bytes
       pub const DEVICE_STATUS: u16   = 0x12;   // 1 byte
       pub const ISR_STATUS: u16      = 0x13;   // 1 byte
       pub const MAC0: u16            = 0x14;   // 6 bytes (MAC)
       pub const NET_STATUS: u16      = 0x1A;   // 2 bytes
   }
   ```
2. **Implement PIO read/write** — Replace the 0/no-op stubs with real port I/O:
   ```rust
   fn read8(&self, offset: u32) -> u8 {
       if let Some(mmio) = self.mmio_base {
           unsafe { ptr::read_volatile((mmio + offset as usize) as *const u8) }
       } else {
           let port = self.io_base + offset as u16;
           unsafe { x86_64::instructions::port::Port::new(port).read() }
       }
   }
   // Same pattern for write8, read32, write32
   ```
3. **Implement PIO init path in `init()`** — Remove the `if self.mmio_base.is_some()`
   guard. The PIO register offsets differ from MMIO offsets, so add an abstraction:
   - Use `pio_reg::DEVICE_STATUS` instead of `mmio_reg::STATUS` when in PIO mode
   - Device init sequence: reset → ACKNOWLEDGE → DRIVER → read features → write features
     → FEATURES_OK → read MAC → init virtqueues → DRIVER_OK
4. **Fix virtqueue memory allocation** — Allocate VirtqAvail and VirtqUsed ring buffers
   using `alloc::alloc::alloc()` with proper alignment, then write their physical
   addresses to `QUEUE_ADDRESS` register (legacy: `pfn = phys_addr >> 12`)
5. **Implement `notify_tx()` and `notify_rx()`** — Write to `QUEUE_NOTIFY` register
   with the correct queue index (0=RX, 1=TX)
6. **Enable PCI bus mastering** for the NIC — Set bit 2 of PCI command register
   (offset 0x04), same as block driver does at `block.rs` L184-188
7. **Update `probe()` to call `init_pio(bar0)`** — After discovering the NIC, extract
   the I/O base from BAR0 (`bar0 & 0xFFFC`) and initialize the device:
   ```rust
   pub fn probe() {
       let network_devs = crate::driver::pci::find_virtio_network();
       for dev in &network_devs {
           let io_base = (dev.bars[0] & 0xFFFC) as u16;
           serial_println!("[VirtIO Net] Found NIC at {} (IO base={:#x})", dev.address, io_base);
           match init_pio(io_base) {
               Ok(()) => serial_println!("[VirtIO Net] NIC initialized successfully"),
               Err(e) => serial_println!("[VirtIO Net] NIC init failed: {:?}", e),
           }
       }
   }
   ```

### Reference Implementation

The VirtIO block driver (`kernel/src/driver/virtio/block.rs`) provides a complete
reference for every pattern needed:
- PCI bus mastering: L184-188
- BAR0 I/O base extraction: L192-193
- VirtIO legacy PIO init sequence: L196-260
- Virtqueue descriptor chain setup: L318-370
- Port-based notify: L372-374
- Polling for completion: L376-394

### QG (Quality Gate)

- [x] `cargo build --target x86_64-kpio.json` succeeds
- [x] PIO `read8`/`write8`/`read32`/`write32` use `x86_64::instructions::port::Port` — no more 0/no-op stubs
- [x] `probe()` calls `init_pio()` and logs `[VirtIO Net] NIC initialized successfully` in QEMU serial
- [x] VirtIO status register reads `DRIVER_OK` (0x04) after init — logged to serial
- [x] MAC address read from device and logged: `[VirtIO Net] MAC: xx:xx:xx:xx:xx:xx`

### Changes After Completion

**Completed:** 2026-02-23

Files modified:
- `kernel/src/drivers/net/virtio_net.rs` — Major rewrite:
  - Added `use x86_64::instructions::port::Port` import
  - Added `pio_reg` module with legacy PCI register offsets (DEVICE_FEATURES through NET_STATUS)
  - Added `device_status` module with VirtIO status constants (ACKNOWLEDGE, DRIVER, DRIVER_OK, FEATURES_OK, FAILED)
  - Added `VirtqRings` struct to track descriptor table, available ring, and used ring physical addresses
  - Added `rx_rings`/`tx_rings` fields to `VirtioNetDevice`
  - Replaced stub PIO branches in `read8`/`write8`/`read32`/`write32` with real `Port::new(port).read()`/`.write()`
  - Added `read16`/`write16` methods for 16-bit register access
  - Split `init()` into `init_pio_device()` and `init_mmio_device()`
  - `init_pio_device()`: full VirtIO legacy PCI init sequence (reset → ACK → DRIVER → features → FEATURES_OK → MAC → virtqueues → DRIVER_OK)
  - Added `init_virtqueue_pio()`: allocates descriptor table + available ring + used ring, writes PFN to QUEUE_ADDRESS register
  - `put_avail_rx()`: writes to available ring and notifies device via PIO
  - `notify_tx()`: writes queue index 1 to QUEUE_NOTIFY via PIO
  - `ack_interrupt()`: reads ISR_STATUS register (auto-clears on read) via PIO
  - `transmit()`: writes descriptors to device-visible memory and updates available ring for PIO mode
  - `receive()`: checks used ring for completed RX descriptors in PIO mode
  - `rx_available()`: checks used ring index in PIO mode
  - `poll()`: reads NET_STATUS register for link state in PIO mode
  - `down()`: resets device via DEVICE_STATUS register in PIO mode
  - `refill_rx()`: writes updated descriptors to device-visible memory in PIO mode
  - `probe()`: now enables PCI bus mastering + I/O space, extracts BAR0 I/O base, and calls `init_pio()` for each discovered NIC

---

## Sub-phase 9-2: Network Stack Wiring — NIC Registration & DHCP Success

### Goal

Connect the initialized VirtIO NIC to the kernel's network stack so that `transmit_frame()`
actually sends packets through the device, and DHCP successfully obtains an IP lease from
QEMU's built-in DHCP server (`10.0.2.2`).

### Root Cause

After 9-1, the NIC is initialized but not registered in `NETWORK_MANAGER`. The existing
`transmit_frame()` function (L259-269 of `kernel/src/net/mod.rs`) iterates over
`NETWORK_MANAGER`'s devices, finds none, and silently drops all packets. DHCP therefore
always times out and falls back to the static IP `10.0.2.15`.

### Tasks

1. **Register NIC in `NETWORK_MANAGER` after successful init** — In `init_pio()` or
   `probe()`, after `device.init()` succeeds, register the device:
   ```rust
   let device = VirtioNetDevice::new_pio("virtio-net0", io_base);
   device.init()?;
   crate::drivers::net::NETWORK_MANAGER.lock().register(device);
   ```
2. **Implement `NetworkDevice` trait for `VirtioNetDevice`** — Ensure `transmit()` and
   `receive()` methods match the trait expected by `NETWORK_MANAGER`:
   - `transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError>`
   - `receive(&mut self) -> Result<Vec<u8>, NetworkError>`
   - `mac_address(&self) -> MacAddress`
   - `is_link_up(&self) -> bool`
3. **Fix interrupt handling** — Register an IRQ handler for the NIC's PCI interrupt line.
   On interrupt, check ISR status register and process completed RX/TX descriptors.
   Alternatively, use polling mode for initial implementation (simpler, sufficient for
   QEMU).
4. **Verify DHCP flow** — With the NIC registered:
   - `dhcp::discover_and_apply()` → `build_dhcp_frame()` → `transmit_frame()` → NIC `transmit()` → actual packet out
   - QEMU's SLIRP replies with DHCP OFFER → NIC `receive()` picks it up
   - DHCP REQUEST/ACK completes → kernel obtains `10.0.2.15/24` via DHCP
5. **Add a `receive_poll()` loop** — DHCP currently calls `wait_for_dhcp_reply()` which
   spins but never actually reads from the NIC. Add NIC receive polling in the DHCP wait
   loop:
   ```rust
   fn wait_for_dhcp_reply() -> Option<DhcpPacket> {
       for _ in 0..MAX_RETRIES {
           if let Ok(frame) = net::receive_frame() {
               if let Some(dhcp) = parse_dhcp_response(&frame) {
                   return Some(dhcp);
               }
           }
           spin_loop_hint(POLL_INTERVAL);
       }
       None
   }
   ```
6. **Add `net::receive_frame()` function** — Symmetric to `transmit_frame()`:
   ```rust
   pub fn receive_frame() -> Result<Vec<u8>, NetworkError> {
       let mut mgr = NETWORK_MANAGER.lock();
       for name in mgr.device_names() {
           if let Some(dev) = mgr.device_mut(&name) {
               if let Ok(frame) = dev.receive() {
                   return Ok(frame);
               }
           }
       }
       Err(NetworkError::NoData)
   }
   ```

### QG (Quality Gate)

- [x] `cargo build` succeeds
- [x] QEMU serial shows `[DHCP] Lease acquired: 10.0.2.15` (or similar) instead of `DHCP timeout`
- [x] `[VirtIO Net] TX: N packets` counter increments during DHCP (at least DISCOVER + REQUEST = 2)
- [x] `[VirtIO Net] RX: N packets` counter increments (at least OFFER + ACK = 2)
- [x] No page faults or panics during network init

### Changes After Completion

**Completed:** 2026-02-24

Files modified:
- `kernel/src/drivers/net/virtio_net.rs` — Multiple critical fixes:
  - Changed `QUEUE_SIZE` from 128 to 256 to match the device's read-only queue size register
    (legacy VirtIO PCI). The mismatch caused avail/used ring offsets to diverge from what
    the device expected, so the device never consumed TX descriptors.
  - Added `MRG_RXBUF` to negotiated features (required because `VirtioNetHdr::SIZE` is 12
    bytes, which includes the `num_buffers` field present only with `MRG_RXBUF`).
  - Queue memory now allocated with `alloc::alloc::alloc_zeroed` + 4096-byte aligned `Layout`
    instead of `alloc::vec!` (legacy VirtIO `QUEUE_ADDRESS` register stores a PFN, so the
    queue must start at a page boundary).
  - All DMA buffer addresses translated to physical via `memory::virt_to_phys()` (descriptors,
    TX buffers, RX buffers, refill).
  - `VirtqRings` struct now tracks both physical (for device DMA) and virtual (for CPU
    access) addresses of descriptor table, available ring, and used ring.
- `kernel/src/net/mod.rs` — Network stack wiring:
  - `init()` now syncs the NIC's real MAC address to the IP config via `ipv4::set_mac()`.
  - Added `receive_frame()` function (symmetric to `transmit_frame()`).
  - Added TX/RX packet counter logging after DHCP completes.
- `kernel/src/net/dhcp.rs` — DHCP reliability:
  - `discover_and_apply()` retries the full DISCOVER→OFFER→REQUEST→ACK cycle up to 3 times.
  - `wait_for_dhcp_reply()` polls 800 iterations with 500k-cycle spin delay per iteration.
- `kernel/src/main.rs` — Boot order fix:
  - Moved PCI / VirtIO / Network stack initialization BEFORE ACPI init, because ACPI has
    a known misaligned-pointer panic that would otherwise block all networking.
  - Fixed frame allocator `pool_start` page alignment.
- `kernel/src/memory/mod.rs` — DMA address translation:
  - Added `virt_to_phys(virt_addr: u64) -> Option<u64>` that performs a full 4-level page
    table walk (PML4 → PDPT → PD → PT), handling 1 GiB and 2 MiB huge pages.
  - `init()` now calls `user_page_table::init(physical_memory_offset)` to store the phys
    offset for page table walks.

---

## Sub-phase 9-3: VFS ↔ Block Driver Integration

> **Status**: **Complete** (all QG items pass — 2026-02-24)

### Goal

Connect the storage crate's VFS layer to the kernel's existing VirtIO block driver so
that `vfs::read()` and `vfs::write()` perform actual disk I/O through the VirtIO block
device in QEMU.

### Root Cause

The kernel crate has a **fully functional** VirtIO block driver (`kernel/src/driver/virtio/block.rs`)
with working `read_sector()` and `write_sector()` methods. However, the storage crate's
VFS layer (`storage/src/vfs.rs`) has TODO stubs for every file operation, and its own
VirtIO driver (`storage/src/driver/virtio.rs`) is entirely stubbed.

The architecture mismatch: the kernel crate owns the working driver, but the storage
crate owns the VFS interface. These need to be connected.

### Design Decision

**Option A** (Recommended): Bridge the storage crate's `BlockDevice` trait to the kernel
crate's existing `VirtioBlock` implementation via a thin adapter. The storage crate calls
into the kernel driver through a function pointer or global accessor.

**Option B**: Move the kernel's VirtIO block driver code into the storage crate. This
would require the storage crate to depend on `x86_64` for port I/O, breaking its
platform-agnostic design.

→ **Option A selected** — maintains separation of concerns.

### Tasks

1. **Create a `KernelBlockAdapter`** in the kernel crate that implements the storage
   crate's `BlockDevice` trait by forwarding to `VIRTIO_BLOCK_DEVICES`:
   ```rust
   // kernel/src/driver/virtio/block_adapter.rs
   pub struct KernelBlockAdapter {
       device_index: usize,
   }
   impl storage::BlockDevice for KernelBlockAdapter {
       fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<(), storage::StorageError> {
           let devices = VIRTIO_BLOCK_DEVICES.lock();
           devices[self.device_index].read_sector(lba, buf)
       }
       fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<(), storage::StorageError> {
           let devices = VIRTIO_BLOCK_DEVICES.lock();
           devices[self.device_index].write_sector(lba, buf)
       }
   }
   ```
2. **Implement a minimal FAT32 (or FAT16) reader** in the storage crate that implements
   the `Filesystem` trait:
   - Parse the BIOS Parameter Block (BPB) from sector 0
   - Navigate the FAT chain for directory traversal
   - Read file contents by following cluster chains
   - Write support can be deferred to a later phase (read-only FS is valuable on its own)
3. **Wire VFS `open()`/`read()`/`readdir()`** to the `Filesystem` implementation:
   - `open()`: look up file in FAT directory → return FS handle
   - `read()`: follow clusters → copy data to buffer
   - `readdir()`: iterate FAT directory entries
4. **Register the block device during boot** — After `driver::virtio::block::init()`
   succeeds, create a `KernelBlockAdapter` and register it with the storage crate:
   ```rust
   storage::register_block_device("virtio-blk0", KernelBlockAdapter { device_index: 0 });
   ```
5. **Add a QEMU test disk** — Create a small FAT32 disk image with test files and
   attach it to QEMU via `-drive file=test.img,format=raw,if=none,id=disk1 -device virtio-blk-pci,drive=disk1`:
   ```powershell
   # scripts/create-test-disk.ps1
   # Creates a 16MB FAT32 disk image with test files
   ```
6. **Add a boot-time self-test** — After VFS mount, read a known file from the test disk
   and verify its contents:
   ```rust
   if let Ok(data) = vfs::read("/mnt/test/hello.txt") {
       serial_println!("[VFS] Self-test: read {} bytes from hello.txt", data.len());
   }
   ```

### QG (Quality Gate)

- [x] `cargo build` succeeds
- [x] QEMU serial shows `[VFS] Mounted FAT filesystem on virtio-blk0 at /mnt/test`
- [x] `[VFS] Self-test: read 36 bytes from HELLO.TXT` — end-to-end read path confirmed
- [x] `vfs::readdir("/mnt/test/")` returns `1 entries: HELLO.TXT (Regular)`
- [x] No panics during VFS operations (NOFILE.TXT gracefully returns FileNotFound)

### Changes After Completion

- `kernel/src/driver/virtio/block_adapter.rs` added (`KernelBlockAdapter`) to bridge kernel VirtIO block device to `storage::driver::BlockDevice`.
- `kernel/src/driver/virtio/block.rs`:
  - Exported helper functions to read/write sectors by device index.
  - **Fixed DMA address translation**: page-aligned queue memory via `alloc_zeroed(Layout)`, `virt_to_phys()` for queue PFN and all descriptor buffer addresses. Previously virtual addresses were passed as physical, causing the device to read from wrong memory and time out.
  - Stores both virtual (CPU access) and physical (DMA) addresses for descriptor table, available ring, and used ring.
- `storage/src/vfs.rs` now routes `mount/open/read/write/readdir/stat/statfs/...` through mounted `Filesystem` instances instead of TODO stubs.
- `storage/src/fs/fat32.rs` replaced with a minimal read-focused FAT32 implementation (BPB parse, FAT chain traversal, directory iteration, `open/read/readdir/lookup`).
- Boot path integration added in `kernel/src/main.rs` to register adapter, mount FAT, and run self-tests (read, readdir, invalid-path).
- Test workflow scripts added/updated:
    - `scripts/create-test-disk.ps1`
    - `scripts/run-qemu.ps1` (`-TestDisk`)
    - `scripts/qemu-test.ps1` (`-TestDisk`)

---

## Sub-phase 9-4: WASI2 Real Network Integration

### Goal

Replace the mock/loopback implementations in `runtime/src/wasi2/http.rs` and
`runtime/src/wasi2/sockets.rs` with calls to the kernel's actual network stack, so that
WASM apps can perform real HTTP requests and TCP/UDP communication.

### Root Cause

The WASI2 HTTP handler (`http.rs` L296-327) returns hardcoded mock responses:
```rust
pub fn handle(request: &OutgoingRequest, _options: Option<&RequestOptions>)
    -> Result<IncomingResponse, HttpError>
{
    let status = StatusCode::ok();  // always 200
    let body_text = format!("KPIO Mock Response for {} {}", ...);
    // ...
}
```

The WASI2 sockets (`sockets.rs`) use in-memory `Vec<u8>` buffers with no connection to
the kernel TCP/UDP stack.

After 9-1 and 9-2 complete, the kernel has a working network path. This sub-phase
connects the WASI2 layer to it.

### Tasks

1. **Create a kernel network bridge module** — `kernel/src/net/wasi_bridge.rs` that
   exposes synchronous network operations callable from the WASM runtime:
   ```rust
   pub fn http_request(method: &str, url: &str, headers: &[(String, String)], body: &[u8])
       -> Result<HttpResponse, NetworkError>
   {
       // Parse URL → DNS resolve → TCP connect → send HTTP request → read response
       let addr = dns::resolve(host)?;
       let conn = tcp::connect(addr, port)?;
       conn.send(&build_http_request(method, path, headers, body))?;
       let response = conn.recv_all()?;
       parse_http_response(&response)
   }
   ```
2. **Update `runtime/src/wasi2/http.rs`** — Replace mock `handle()` with a call to the
   kernel bridge:
   ```rust
   pub fn handle(request: &OutgoingRequest, _options: Option<&RequestOptions>)
       -> Result<IncomingResponse, HttpError>
   {
       #[cfg(feature = "kernel")]
       {
           let response = kernel::net::wasi_bridge::http_request(
               &request.method.to_string(),
               &request.url(),
               &request.headers,
               &request.body,
           )?;
           Ok(IncomingResponse::from_kernel(response))
       }
       #[cfg(not(feature = "kernel"))]
       {
           // Keep mock for testing outside kernel
           Ok(mock_response(request))
       }
   }
   ```
3. **Update `runtime/src/wasi2/sockets.rs`** — Replace `TcpSocket` in-memory buffers
   with kernel TCP connections:
   - `connect()` → `kernel::net::tcp::connect()`
   - `send()` → `kernel::net::tcp::send()`
   - `receive()` → `kernel::net::tcp::recv()`
   - `bind()`/`listen()`/`accept()` → deferred (server-side sockets are lower priority)
4. **Update `resolve_addresses()`** — Replace hardcoded results with actual DNS queries:
   ```rust
   pub fn resolve_addresses(name: &str) -> Result<Vec<IpAddress>, SocketError> {
       let ip = kernel::net::dns::resolve(name)?;
       Ok(vec![IpAddress::Ipv4(ip)])
   }
   ```
5. **Add a WASM integration test** — Create a small WASM app that performs an HTTP GET
   to QEMU's host-forwarded port and verifies the response:
   ```rust
   // examples/http-test-kpioapp/
   #[kpio_app::main]
   fn main() {
       let response = http::get("http://10.0.2.2:8080/test");
       assert_eq!(response.status(), 200);
   }
   ```

### QG (Quality Gate)

- [ ] `cargo build` succeeds
- [ ] WASI2 `http::handle()` returns a real HTTP response (not mock) when kernel feature is enabled
- [ ] WASI2 `TcpSocket::connect()` establishes a real TCP connection through the kernel stack
- [ ] `resolve_addresses("example.com")` returns a real IP from DNS (not hardcoded `10.0.0.1`)
- [ ] Mock fallback still works when `kernel` feature is disabled (for unit testing)

### Changes After Completion

_(To be filled after sub-phase is implemented)_

---

## Sub-phase 9-5: End-to-End Integration Test

### Goal

Create an automated QEMU-based test that validates the entire I/O path: boot → NIC init
→ DHCP → HTTP request → disk write → disk read → verify. This serves as the Phase 9
integration milestone.

### Tasks

1. **Extend `qemu-test.ps1`** with a new test mode `io`:
   ```powershell
   $IoChecks = $SmokeChecks + @(
       @{ Pattern = "NIC initialized successfully";  Label = "VirtIO NIC init" },
       @{ Pattern = "Lease acquired";                Label = "DHCP success" },
       @{ Pattern = "VirtIO Net.*TX:";               Label = "Packet TX" },
       @{ Pattern = "VFS.*Mounted";                  Label = "VFS mount" },
       @{ Pattern = "Self-test.*read.*bytes";         Label = "VFS read" },
   )
   ```
2. **Add a boot-time E2E self-test sequence** in `kernel/src/main.rs` (gated behind
   a `#[cfg(test)]` or auto-detected QEMU environment):
   - After all init: attempt DNS resolve → TCP connect → HTTP GET → write response to VFS → read back → verify → log result
   - Log `[E2E] Integration test PASSED` or `[E2E] Integration test FAILED: <reason>`
3. **Create test artifacts**:
   - `tests/e2e/test-disk.img` — Pre-built FAT32 image with known files
   - `tests/e2e/http-server.py` — Simple HTTP server for QEMU host-side testing
4. **Update QEMU scripts** to optionally attach the test disk:
   ```powershell
   if ($Mode -eq "io") {
       $argParts += "-drive `"file=$TestDisk,format=raw,if=none,id=testdisk`""
       $argParts += "-device `"virtio-blk-pci,drive=testdisk`""
   }
   ```
5. **Document test procedure** in `docs/guides/LOCAL_DEVELOPMENT.md`:
   ```bash
   # Run full I/O integration test
   .\scripts\qemu-test.ps1 -Mode io -Verbose
   ```

### QG (Quality Gate)

- [ ] `.\scripts\qemu-test.ps1 -Mode io` runs without manual intervention
- [ ] All `$IoChecks` patterns found in serial log
- [ ] Test completes in under 60 seconds
- [ ] Exit code 0 (success) from test script

### Changes After Completion

_(To be filled after sub-phase is implemented)_

---

## Execution Order & Dependencies

```
9-1 (VirtIO Net PIO) ──→ 9-2 (NIC Registration + DHCP)
                                    │
                                    ▼
                          [QEMU Network Verification]
                                    │
                    ┌───────────────┤
                    ▼               ▼
           9-3 (VFS ↔ Block)    9-4 (WASI2 Real Network)
                    │               │
                    └───────┬───────┘
                            ▼
                    9-5 (E2E Integration Test)
```

- **9-1 → 9-2**: NIC must be initialized before it can be registered in the network manager
- **9-2 → 9-4**: Kernel network stack must work before WASI2 can forward to it
- **9-3**: Independent of network — can run in parallel with 9-2, but ordered here for clarity
- **9-5**: Requires both storage and network paths to be functional

---

## Commit Plan

| Sub-phase | Commit message |
|-----------|---------------|
| 9-1 | `feat(virtio-net): implement PIO register access with x86_64 Port I/O` |
| 9-2 | `feat(net): register VirtIO NIC in network manager, enable DHCP` |
| 9-3 | `feat(storage): connect VFS to kernel VirtIO block driver via adapter` |
| 9-4 | `feat(wasi2): replace mock HTTP/sockets with real kernel network calls` |
| 9-5 | `test(e2e): add QEMU I/O integration test mode` |

---

## Expected Outcomes

After Phase 9 completion:

1. **VirtIO NIC fully operational** — PIO register access, virtqueue setup, packet TX/RX
2. **DHCP succeeds** — kernel obtains IP lease from QEMU's SLIRP network (`10.0.2.15`)
3. **DNS resolution works** — WASM apps can resolve hostnames
4. **HTTP requests work** — end-to-end from WASM app → WASI2 → kernel TCP → NIC → QEMU → internet
5. **VFS reads from disk** — FAT filesystem mounted, files readable from VirtIO block device
6. **Automated E2E test** — `qemu-test.ps1 -Mode io` validates the full I/O path

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| VirtIO virtqueue memory alignment issues | Medium | High — device rejects queue | Use the exact allocation pattern from the working block driver |
| QEMU SLIRP timing — DHCP reply arrives before receive poll starts | Medium | Medium — DHCP timeout | Add retry logic; buffer early-arriving frames in the NIC driver |
| FAT32 parser bugs on edge cases | Low | Medium — read failures | Start with read-only FAT16 (simpler); upgrade to FAT32 later |
| `x86_64::Port` I/O causes triple fault | Low | Critical — kernel crash | Copy the proven pattern from block driver exactly; test incrementally |
| WASI2 integration breaks existing WASM apps | Low | Medium — regression | Keep mock fallback behind `#[cfg(not(feature = "kernel"))]` |

---

## Reference Materials

- [VirtIO 1.0 Specification](https://docs.oasis-open.org/virtio/virtio/v1.0/virtio-v1.0.html) — §2 (virtqueues), §4.1 (PCI transport), §5.1 (network device)
- [OSDev Wiki — VirtIO](https://wiki.osdev.org/Virtio) — practical implementation guide
- Existing implementation reference: [kernel/src/driver/virtio/block.rs](../kernel/src/driver/virtio/block.rs) — complete PIO VirtIO driver
- QEMU networking: [QEMU User Networking (SLIRP)](https://wiki.qemu.org/Documentation/Networking) — default `10.0.2.0/24` subnet
