//! POSIX Shim — maps POSIX function semantics to WASI P2 equivalents.
//!
//! This module documents and provides type definitions for the mapping
//! between traditional POSIX C library functions and their WASI Preview 2
//! interface equivalents. The actual linking happens at compile time
//! via wasi-sdk's libc, which implements these mappings.
//!
//! # Mapping Table
//!
//! | POSIX Function | WASI P2 Interface | Notes |
//! |---------------|-------------------|-------|
//! | `open()` | `wasi:filesystem/types.open-at` | Relative to preopened dir |
//! | `read()` | `wasi:io/streams.read` | Via input-stream |
//! | `write()` | `wasi:io/streams.write` | Via output-stream |
//! | `close()` | `wasi:filesystem/types.drop` | Resource cleanup |
//! | `stat()` / `fstat()` | `wasi:filesystem/types.stat` | Descriptor stat |
//! | `lseek()` | N/A (stream-based) | Use offset in read/write |
//! | `mkdir()` | `wasi:filesystem/types.create-directory-at` | Relative path |
//! | `unlink()` | `wasi:filesystem/types.unlink-file-at` | Relative path |
//! | `readdir()` | `wasi:filesystem/types.read-directory` | Directory stream |
//! | `rename()` | `wasi:filesystem/types.rename-at` | Relative paths |
//! | `socket()` | `wasi:sockets/tcp.create-tcp-socket` | AF_INET → ipv4 |
//! | `connect()` | `wasi:sockets/tcp.connect` | Returns streams |
//! | `bind()` | `wasi:sockets/tcp.bind` | Address binding |
//! | `listen()` | `wasi:sockets/tcp.listen` | Start accepting |
//! | `accept()` | `wasi:sockets/tcp.accept` | Returns new socket |
//! | `send()` / `sendto()` | `wasi:io/streams.write` | Via output-stream |
//! | `recv()` / `recvfrom()` | `wasi:io/streams.read` | Via input-stream |
//! | `malloc()` / `free()` | WASM linear memory | `memory.grow` + allocator |
//! | `clock_gettime(MONOTONIC)` | `wasi:clocks/monotonic-clock.now` | Nanoseconds |
//! | `clock_gettime(REALTIME)` | `wasi:clocks/wall-clock.now` | Seconds + nanos |
//! | `getenv()` | `wasi:cli/environment.get-environment` | Key-value pairs |
//! | `getrandom()` | `wasi:random/random.get-random-bytes` | Crypto-secure |
//! | `poll()` / `select()` | `wasi:io/poll.poll` | Pollable list |
//! | `exit()` | `proc_exit` | Exit code |

use alloc::string::String;
use alloc::vec::Vec;

/// POSIX-to-WASI function mapping entry.
#[derive(Debug, Clone)]
pub struct PosixMapping {
    /// POSIX function name.
    pub posix_name: &'static str,
    /// WASI P2 interface and function.
    pub wasi_interface: &'static str,
    /// Brief description of the mapping.
    pub description: &'static str,
    /// Whether this mapping is fully implemented.
    pub implemented: bool,
}

/// Get the complete POSIX-to-WASI mapping table.
pub fn posix_mappings() -> Vec<PosixMapping> {
    alloc::vec![
        PosixMapping {
            posix_name: "open",
            wasi_interface: "wasi:filesystem/types.open-at",
            description: "Open file relative to preopened directory descriptor",
            implemented: true,
        },
        PosixMapping {
            posix_name: "read",
            wasi_interface: "wasi:io/streams.read",
            description: "Read bytes from an input stream",
            implemented: true,
        },
        PosixMapping {
            posix_name: "write",
            wasi_interface: "wasi:io/streams.write",
            description: "Write bytes to an output stream",
            implemented: true,
        },
        PosixMapping {
            posix_name: "close",
            wasi_interface: "wasi:filesystem/types.drop",
            description: "Close a file descriptor (drop resource)",
            implemented: true,
        },
        PosixMapping {
            posix_name: "stat",
            wasi_interface: "wasi:filesystem/types.stat",
            description: "Get file metadata (size, timestamps, type)",
            implemented: true,
        },
        PosixMapping {
            posix_name: "mkdir",
            wasi_interface: "wasi:filesystem/types.create-directory-at",
            description: "Create a directory relative to a descriptor",
            implemented: true,
        },
        PosixMapping {
            posix_name: "unlink",
            wasi_interface: "wasi:filesystem/types.unlink-file-at",
            description: "Delete a file relative to a descriptor",
            implemented: true,
        },
        PosixMapping {
            posix_name: "readdir",
            wasi_interface: "wasi:filesystem/types.read-directory",
            description: "Read directory entries via a stream",
            implemented: true,
        },
        PosixMapping {
            posix_name: "socket",
            wasi_interface: "wasi:sockets/tcp.create-tcp-socket",
            description: "Create a TCP socket (AF_INET → ipv4 family)",
            implemented: true,
        },
        PosixMapping {
            posix_name: "connect",
            wasi_interface: "wasi:sockets/tcp.connect",
            description: "Connect TCP socket to remote address",
            implemented: true,
        },
        PosixMapping {
            posix_name: "bind",
            wasi_interface: "wasi:sockets/tcp.bind",
            description: "Bind socket to local address",
            implemented: true,
        },
        PosixMapping {
            posix_name: "listen",
            wasi_interface: "wasi:sockets/tcp.listen",
            description: "Start listening for connections",
            implemented: true,
        },
        PosixMapping {
            posix_name: "accept",
            wasi_interface: "wasi:sockets/tcp.accept",
            description: "Accept incoming connection",
            implemented: true,
        },
        PosixMapping {
            posix_name: "send",
            wasi_interface: "wasi:io/streams.write",
            description: "Send data via connected socket's output stream",
            implemented: true,
        },
        PosixMapping {
            posix_name: "recv",
            wasi_interface: "wasi:io/streams.read",
            description: "Receive data via connected socket's input stream",
            implemented: true,
        },
        PosixMapping {
            posix_name: "malloc",
            wasi_interface: "wasm.memory.grow",
            description: "Allocate memory via WASM linear memory growth",
            implemented: true,
        },
        PosixMapping {
            posix_name: "free",
            wasi_interface: "wasm.memory (allocator)",
            description: "Free memory via userspace allocator on linear memory",
            implemented: true,
        },
        PosixMapping {
            posix_name: "clock_gettime",
            wasi_interface: "wasi:clocks/monotonic-clock.now",
            description: "Get monotonic or wall clock time",
            implemented: true,
        },
        PosixMapping {
            posix_name: "getenv",
            wasi_interface: "wasi:cli/environment.get-environment",
            description: "Get environment variable via WASI CLI interface",
            implemented: true,
        },
        PosixMapping {
            posix_name: "getrandom",
            wasi_interface: "wasi:random/random.get-random-bytes",
            description: "Get cryptographically secure random bytes",
            implemented: true,
        },
        PosixMapping {
            posix_name: "poll",
            wasi_interface: "wasi:io/poll.poll",
            description: "Poll multiple file descriptors / streams for readiness",
            implemented: true,
        },
        PosixMapping {
            posix_name: "exit",
            wasi_interface: "proc_exit",
            description: "Terminate process with exit code",
            implemented: true,
        },
    ]
}

/// Look up the WASI equivalent for a POSIX function.
pub fn lookup_posix(posix_name: &str) -> Option<PosixMapping> {
    posix_mappings()
        .into_iter()
        .find(|m| m.posix_name == posix_name)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_posix_mappings_count() {
        let mappings = posix_mappings();
        assert!(mappings.len() >= 10, "must map at least 10 POSIX functions");
        // Actually maps 22 functions
        assert_eq!(mappings.len(), 22);
    }

    #[test]
    fn test_lookup_open() {
        let m = lookup_posix("open").unwrap();
        assert_eq!(m.wasi_interface, "wasi:filesystem/types.open-at");
        assert!(m.implemented);
    }

    #[test]
    fn test_lookup_read() {
        let m = lookup_posix("read").unwrap();
        assert_eq!(m.wasi_interface, "wasi:io/streams.read");
    }

    #[test]
    fn test_lookup_write() {
        let m = lookup_posix("write").unwrap();
        assert_eq!(m.wasi_interface, "wasi:io/streams.write");
    }

    #[test]
    fn test_lookup_close() {
        let m = lookup_posix("close").unwrap();
        assert!(m.implemented);
    }

    #[test]
    fn test_lookup_stat() {
        let m = lookup_posix("stat").unwrap();
        assert_eq!(m.wasi_interface, "wasi:filesystem/types.stat");
    }

    #[test]
    fn test_lookup_socket() {
        let m = lookup_posix("socket").unwrap();
        assert_eq!(m.wasi_interface, "wasi:sockets/tcp.create-tcp-socket");
    }

    #[test]
    fn test_lookup_connect() {
        let m = lookup_posix("connect").unwrap();
        assert_eq!(m.wasi_interface, "wasi:sockets/tcp.connect");
    }

    #[test]
    fn test_lookup_malloc() {
        let m = lookup_posix("malloc").unwrap();
        assert_eq!(m.wasi_interface, "wasm.memory.grow");
    }

    #[test]
    fn test_lookup_clock_gettime() {
        let m = lookup_posix("clock_gettime").unwrap();
        assert_eq!(m.wasi_interface, "wasi:clocks/monotonic-clock.now");
    }

    #[test]
    fn test_lookup_getrandom() {
        let m = lookup_posix("getrandom").unwrap();
        assert_eq!(m.wasi_interface, "wasi:random/random.get-random-bytes");
    }

    #[test]
    fn test_lookup_poll() {
        let m = lookup_posix("poll").unwrap();
        assert_eq!(m.wasi_interface, "wasi:io/poll.poll");
    }

    #[test]
    fn test_lookup_nonexistent() {
        assert!(lookup_posix("fork").is_none());
        assert!(lookup_posix("exec").is_none());
    }

    #[test]
    fn test_all_mappings_implemented() {
        for m in posix_mappings() {
            assert!(m.implemented, "{} should be implemented", m.posix_name);
        }
    }

    #[test]
    fn test_all_mappings_have_descriptions() {
        for m in posix_mappings() {
            assert!(
                !m.description.is_empty(),
                "{} has empty description",
                m.posix_name
            );
        }
    }
}
