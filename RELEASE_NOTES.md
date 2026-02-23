# KPIO OS Release Notes

## Version 1.0.0

**Release Date**: 2026-XX-XX

---

## 🎉 Highlights

This is the first stable release of KPIO OS, a modern browser-based operating system.

- 🚀 **Custom Kernel** - Built from scratch in Rust for safety and performance
- 🌐 **Full Web Browser** - HTML5, CSS3, and JavaScript support
- 💻 **Desktop Environment** - Modern window management and taskbar
- 🔒 **Security-First** - Capability-based security and sandboxing
- 🌍 **Internationalization** - Multi-language support

---

## ✨ New Features

### Kernel
- Memory management with virtual memory, paging, and ASLR
- Process management with multi-tasking and scheduling
- Inter-process communication (channels, shared memory, message queues)
- Device driver framework (keyboard, mouse, storage, network)
- System call interface for applications
- **Full TCP/IP network stack** for online browsing (see below)

### Network Stack (NEW)
- **VirtIO-net driver** — real packet TX/RX over both MMIO virtqueues and PIO (legacy PCI transport)
- **Ethernet** — IEEE 802.3 frame parse/build
- **ARP** — address resolution with cache (RFC 826)
- **IPv4** — packet construction, internet checksum, ICMP echo, subnet routing
- **UDP** — datagram socket table for DNS queries
- **TCP** — full 3-way handshake, sequence/ack, retransmission, FIN teardown
- **DNS** — RFC 1035 wire-format queries over UDP, host table + TTL cache
- **TLS 1.2** — SHA-256, HMAC, AES-128-CBC, RSA key exchange (demo cert validation)
- **HTTP/1.1** — client with chunked decoding; serves built-in kpio:// pages
- **HTTPS** — TLS-wrapped HTTP for port-443 connections
- Works out-of-the-box with QEMU `-netdev user` (IP 10.0.2.15, GW 10.0.2.2, DNS 10.0.2.3)

### Browser
- HTML5 parsing and rendering
- CSS3 styling with cascade and inheritance
- JavaScript execution environment
- Content Security Policy (CSP) support
- Cookie management with SameSite support
- Private browsing mode
- Tab management
- Extension support framework

### Desktop
- Window management with drag, resize, minimize, maximize
- Taskbar with app launcher and system tray
- Start menu with application search
- File manager with tree navigation
- Settings application

### System Applications
- Terminal emulator
- Text editor with syntax highlighting
- Calculator
- Settings panel

### Security
- Capability-based security model
- Process sandboxing
- Syscall filtering
- Security audit logging

### Internationalization
- English (en)
- Korean (ko)
- Japanese (ja)
- Chinese Simplified (zh-CN)
- Spanish (es)
- German (de)

---

## 📋 System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | x86_64 | 2+ cores, 64-bit |
| RAM | 256 MB | 1 GB |
| Storage | 2 GB | 8 GB |
| Display | 1024x768 | 1920x1080 |
| Graphics | VGA | VESA compatible |

---

## 🐛 Known Issues

- [ ] **#001** - High DPI scaling may have visual artifacts
- [ ] **#002** - Some complex CSS layouts may not render correctly
- [ ] **#003** - JavaScript performance on complex pages needs optimization
- [ ] **#004** - Network driver may require manual configuration on some hardware
- [x] **#005** - ~~ACPI page fault crash when parsing RSDP/XSDT/MADT (physical address used without virtual translation)~~ — Fixed in Phase 8-1
- [x] **#006** - ~~ACPI `tables()` returns dangling `&'static` reference via unsound `MutexGuard` → raw pointer cast~~ — Fixed in Phase 8-2
- [x] **#007** - ~~Network stack initialized before PCI/VirtIO enumeration, causing DHCP to fail due to missing NIC~~ — Fixed in Phase 8-3
- [x] **#008** - ~~VirtIO network probe was a no-op stub; QEMU launched without NIC device, so no NIC was ever discovered~~ — Fixed in Phase 8-4
- [x] **#009** - ~~`free_frame()` was a no-op; physical frames were never returned to the allocator, causing memory leak under sustained allocation~~ — Fixed in Phase 8-5
- [x] **#010** - ~~`acpi` and `aml` crates declared in `kernel/Cargo.toml` but never imported; `[features] acpi` name collided with crate name~~ — Fixed in Phase 8-6
- [x] **#011** - ~~~869 build warnings (dead_code, unused_imports, unused_variables, etc.) obscuring real issues; no workspace-level lint policy~~ — Fixed in Phase 8-7
- [x] **#012** - ~~VirtIO Net PIO `read8`/`write8`/`read32`/`write32` returned 0 / no-op — NIC initialization impossible via PIO transport~~ — Fixed in Phase 9-1
- [x] **#013** - ~~`probe()` discovered VirtIO NIC but did not call `init_pio()` — NIC was never initialized~~ — Fixed in Phase 9-1

---

## 📥 Installation

### From ISO

1. Download `kpio-os-1.0.0.iso`
2. Verify checksum: `sha256sum -c kpio-os-1.0.0.iso.sha256`
3. Create bootable USB: `dd if=kpio-os-1.0.0.iso of=/dev/sdX bs=4M`
4. Boot from USB

### On Virtual Machine

**QEMU:**
```bash
qemu-system-x86_64 -m 1G -cdrom kpio-os-1.0.0.iso -enable-kvm
```

**VirtualBox:**
1. Create new VM (Type: Other, Version: Other/Unknown 64-bit)
2. Allocate 1GB RAM
3. Attach ISO as optical drive
4. Start VM

---

## 🔧 Upgrade Instructions

This is the initial release. Future upgrade instructions will be provided here.

---

## 👥 Contributors

Thank you to all contributors who made this release possible!

- Core kernel development
- Browser engine implementation
- Desktop environment design
- Testing and quality assurance
- Documentation and translations

---

## 📜 License

KPIO OS is released under the MIT License.

```
MIT License

Copyright (c) 2026 KPIO Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## 📮 Feedback

We'd love to hear from you!

- **Bug Reports**: [GitHub Issues](https://github.com/kpio/kpio/issues)
- **Feature Requests**: [GitHub Discussions](https://github.com/kpio/kpio/discussions)
- **General Questions**: [Community Forum](https://forum.kpio.local)

---

*KPIO OS - The browser is the operating system.*
