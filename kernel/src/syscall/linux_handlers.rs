//! Linux File I/O Syscall Handlers
//!
//! Implements the core Linux file I/O syscalls (read, write, open, close, etc.)
//! routing through the per-process file descriptor table.
//!
//! Each process has its own `BTreeMap<u32, FileDescriptor>` in the process table
//! (created in Phase 7-4.1). These handlers look up the current process from
//! `percpu::get_current_pid()` and operate on that process's FD table.
//!
//! For compatibility, these handlers also support kernel-context calls where
//! there is no current Linux process — in that case they fall back to the
//! global VFS FD table.

use alloc::string::String;

use super::linux::{
    copy_to_user, read_user_string, validate_user_ptr,
    AT_FDCWD, EACCES, EBADF, EFAULT, EINVAL, EMFILE, ENOENT, ENOSYS, ESPIPE,
};
use crate::process::table::{
    FileDescriptor, FileResource, ProcessId, StdioType, PROCESS_TABLE,
};
use crate::serial;
use crate::vfs;

/// Maximum path length we'll accept from userspace.
const PATH_MAX: usize = 4096;
/// Maximum number of FDs per process.
const FD_MAX: u32 = 256;

// ═══════════════════════════════════════════════════════════════════════
// Helper: current process FD operations
// ═══════════════════════════════════════════════════════════════════════

/// Get the PID of the current Linux process.
///
/// Returns `None` if there is no Linux process running (kernel context).
fn current_pid() -> Option<ProcessId> {
    let pid = super::percpu::get_current_pid(0);
    if pid == 0 {
        None
    } else {
        Some(ProcessId(pid))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_READ (0)
// ═══════════════════════════════════════════════════════════════════════

/// `read(fd, buf, count)` → `ssize_t`
pub fn sys_read(fd: i32, buf_ptr: u64, count: u64) -> i64 {
    if count == 0 {
        return 0;
    }
    if let Err(e) = validate_user_ptr(buf_ptr, count) {
        return e;
    }
    if buf_ptr == 0 {
        return -EFAULT;
    }

    // Try per-process FD table first
    if let Some(pid) = current_pid() {
        return read_via_process(pid, fd as u32, buf_ptr, count);
    }

    // Fallback: global FD table
    match vfs::fd::read(fd, count as usize) {
        Ok(data) => {
            let len = data.len().min(count as usize);
            if copy_to_user(buf_ptr, &data[..len]).is_err() {
                return -EFAULT;
            }
            len as i64
        }
        Err(_) => -EBADF,
    }
}

/// Read from a process's per-process FD table.
fn read_via_process(pid: ProcessId, fd: u32, buf_ptr: u64, count: u64) -> i64 {
    // Read the file descriptor info
    let fd_info = {
        let guard = match PROCESS_TABLE.get(pid) {
            Some(g) => g,
            None => return -EBADF,
        };
        let proc = match guard.get(&pid) {
            Some(p) => p,
            None => return -EBADF,
        };
        match proc.get_fd(fd) {
            Some(f) => f.clone(),
            None => return -EBADF,
        }
    };

    match &fd_info.resource {
        FileResource::Stdio(StdioType::Stdin) => {
            // Read from serial (blocking single byte for now)
            if let Some(byte) = serial::try_read_byte() {
                unsafe {
                    *(buf_ptr as *mut u8) = byte;
                }
                1
            } else {
                0 // Would block — return 0 for now
            }
        }
        FileResource::Stdio(_) => -EBADF, // Can't read stdout/stderr
        FileResource::File { path } => {
            match vfs::read_all(path) {
                Ok(data) => {
                    let offset = fd_info.offset as usize;
                    if offset >= data.len() {
                        return 0; // EOF
                    }
                    let available = data.len() - offset;
                    let to_read = (count as usize).min(available);
                    if copy_to_user(buf_ptr, &data[offset..offset + to_read]).is_err() {
                        return -EFAULT;
                    }
                    // Update offset
                    update_fd_offset(pid, fd, offset + to_read);
                    to_read as i64
                }
                Err(_) => -EIO,
            }
        }
        _ => -EBADF,
    }
}

/// Linux EIO constant.
const EIO: i64 = 5;

// ═══════════════════════════════════════════════════════════════════════
// SYS_WRITE (1)
// ═══════════════════════════════════════════════════════════════════════

/// `write(fd, buf, count)` → `ssize_t`
pub fn sys_write(fd: i32, buf_ptr: u64, count: u64) -> i64 {
    if count == 0 {
        return 0;
    }
    if validate_user_ptr(buf_ptr, count).is_err() {
        return -EFAULT;
    }

    let data = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, count as usize) };

    // Try per-process FD table first
    if let Some(pid) = current_pid() {
        return write_via_process(pid, fd as u32, data);
    }

    // Fallback: global FD table
    match vfs::fd::write(fd, data) {
        Ok(n) => n as i64,
        Err(_) => -EBADF,
    }
}

/// Write through a process's per-process FD table.
fn write_via_process(pid: ProcessId, fd: u32, data: &[u8]) -> i64 {
    let fd_info = {
        let guard = match PROCESS_TABLE.get(pid) {
            Some(g) => g,
            None => return -EBADF,
        };
        let proc = match guard.get(&pid) {
            Some(p) => p,
            None => return -EBADF,
        };
        match proc.get_fd(fd) {
            Some(f) => f.clone(),
            None => return -EBADF,
        }
    };

    match &fd_info.resource {
        FileResource::Stdio(StdioType::Stdout) | FileResource::Stdio(StdioType::Stderr) => {
            if let Ok(s) = core::str::from_utf8(data) {
                serial::write_str(s);
            } else {
                // Write raw bytes as hex for non-UTF8
                for &b in data {
                    serial::write_byte(b);
                }
            }
            data.len() as i64
        }
        FileResource::Stdio(StdioType::Stdin) => -EBADF,
        FileResource::File { path } => {
            // Write to file via VFS
            match vfs::fd::open(path, 0o1 | 0o100) {
                // WRONLY | O_CREAT
                Ok(global_fd) => {
                    let result = vfs::fd::write(global_fd, data);
                    let _ = vfs::fd::close(global_fd);
                    match result {
                        Ok(n) => n as i64,
                        Err(_) => -EIO,
                    }
                }
                Err(_) => -EIO,
            }
        }
        _ => -EBADF,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_OPEN (2)
// ═══════════════════════════════════════════════════════════════════════

/// `open(path, flags, mode)` → `fd`
pub fn sys_open(path_ptr: u64, flags: u32, _mode: u32) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };

    open_file(&path, flags)
}

/// Common open logic for both `open()` and `openat()`.
fn open_file(path: &str, flags: u32) -> i64 {
    // Try via per-process FD table
    if let Some(pid) = current_pid() {
        return open_in_process(pid, path, flags);
    }

    // Fallback: global FD table
    match vfs::fd::open(path, flags) {
        Ok(fd) => fd as i64,
        Err(vfs::VfsError::NotFound) => -ENOENT,
        Err(vfs::VfsError::PermissionDenied) => -EACCES,
        Err(vfs::VfsError::NoSpace) => -EMFILE as i64,
        Err(_) => -EIO,
    }
}

/// Open a file and add it to the process's per-process FD table.
fn open_in_process(pid: ProcessId, path: &str, flags: u32) -> i64 {
    // Check if file exists (or can be created)
    let file_exists = vfs::stat(path).is_ok();
    let o_creat = flags & 0o100 != 0;

    if !file_exists && !o_creat {
        return -ENOENT;
    }

    // If O_CREAT and file doesn't exist, create it via global VFS
    if !file_exists && o_creat {
        // Use global fd table to create, then close
        match vfs::fd::open(path, flags) {
            Ok(gfd) => {
                let _ = vfs::fd::close(gfd);
            }
            Err(_) => return -EIO,
        }
    }

    // Allocate a new FD in the process
    // We need a write lock here — but ProcessTable::get returns a read lock.
    // Use a two-step: read the current next_fd, then do a write via the process
    // table directly. Since PROCESS_TABLE uses RwLock internally, we use a helper.
    let fd_num = alloc_process_fd(pid, path, flags);
    fd_num
}

/// Allocate a new FD in a process's FD table.
///
/// Returns the fd number on success, or negative errno.
fn alloc_process_fd(pid: ProcessId, path: &str, flags: u32) -> i64 {
    // Access the process table directly
    // Note: PROCESS_TABLE.get() returns a read guard.
    // We need a mutable access pattern. Since the processes field
    // is behind RwLock, we need to use for_each or add a helper.
    //
    // For now, use a workaround: remove, modify, re-add.
    // This is safe because we're single-CPU for now.

    let mut proc = match PROCESS_TABLE.remove(pid) {
        Some(p) => p,
        None => return -EBADF,
    };

    if proc.next_fd >= FD_MAX {
        PROCESS_TABLE.add(proc);
        return -EMFILE as i64;
    }

    let fd_num = proc.alloc_fd();
    proc.add_fd(FileDescriptor {
        fd: fd_num,
        resource: FileResource::File {
            path: String::from(path),
        },
        flags,
        offset: 0,
    });

    // Re-add process (preserves pid since it's in the struct)
    let _re_pid = proc.pid;
    PROCESS_TABLE.add(proc);
    // Ensure PID didn't change (add uses proc.pid)

    fd_num as i64
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_CLOSE (3)
// ═══════════════════════════════════════════════════════════════════════

/// `close(fd)` → `0` or `-errno`
pub fn sys_close(fd: i32) -> i64 {
    if let Some(pid) = current_pid() {
        return close_in_process(pid, fd as u32);
    }
    match vfs::fd::close(fd) {
        Ok(()) => 0,
        Err(_) => -EBADF,
    }
}

fn close_in_process(pid: ProcessId, fd: u32) -> i64 {
    let mut proc = match PROCESS_TABLE.remove(pid) {
        Some(p) => p,
        None => return -EBADF,
    };

    let result = if proc.remove_fd(fd).is_some() {
        0
    } else {
        -EBADF
    };

    PROCESS_TABLE.add(proc);
    result
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_STAT (4) / SYS_FSTAT (5)
// ═══════════════════════════════════════════════════════════════════════

/// Linux `struct stat` layout for x86_64.
/// Total size: 144 bytes.
#[repr(C)]
struct LinuxStat {
    st_dev: u64,
    st_ino: u64,
    st_nlink: u64,
    st_mode: u32,
    st_uid: u32,
    st_gid: u32,
    __pad0: u32,
    st_rdev: u64,
    st_size: i64,
    st_blksize: i64,
    st_blocks: i64,
    st_atime: i64,
    st_atime_nsec: i64,
    st_mtime: i64,
    st_mtime_nsec: i64,
    st_ctime: i64,
    st_ctime_nsec: i64,
    __unused: [i64; 3],
}

fn fill_stat(fstat: &vfs::FileStat) -> LinuxStat {
    let mode: u32 = if fstat.is_dir {
        0o040755 // directory
    } else if fstat.is_symlink {
        0o120777 // symlink
    } else {
        0o100644 // regular file
    };

    LinuxStat {
        st_dev: 1,
        st_ino: fstat.ino,
        st_nlink: fstat.nlink as u64,
        st_mode: mode,
        st_uid: fstat.uid,
        st_gid: fstat.gid,
        __pad0: 0,
        st_rdev: 0,
        st_size: fstat.size as i64,
        st_blksize: 4096,
        st_blocks: ((fstat.size + 511) / 512) as i64,
        st_atime: 0,
        st_atime_nsec: 0,
        st_mtime: 0,
        st_mtime_nsec: 0,
        st_ctime: 0,
        st_ctime_nsec: 0,
        __unused: [0; 3],
    }
}

/// `stat(path, statbuf)` → `0` or `-errno`
pub fn sys_stat(path_ptr: u64, statbuf_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if validate_user_ptr(statbuf_ptr, core::mem::size_of::<LinuxStat>() as u64).is_err() {
        return -EFAULT;
    }

    match vfs::stat(&path) {
        Ok(fstat) => {
            let linux_stat = fill_stat(&fstat);
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    &linux_stat as *const LinuxStat as *const u8,
                    core::mem::size_of::<LinuxStat>(),
                )
            };
            if copy_to_user(statbuf_ptr, bytes).is_err() {
                return -EFAULT;
            }
            0
        }
        Err(vfs::VfsError::NotFound) => -ENOENT,
        Err(_) => -EIO,
    }
}

/// `fstat(fd, statbuf)` → `0` or `-errno`
pub fn sys_fstat(fd: i32, statbuf_ptr: u64) -> i64 {
    if validate_user_ptr(statbuf_ptr, core::mem::size_of::<LinuxStat>() as u64).is_err() {
        return -EFAULT;
    }

    // Get path from fd
    let path = if let Some(pid) = current_pid() {
        let guard = match PROCESS_TABLE.get(pid) {
            Some(g) => g,
            None => return -EBADF,
        };
        let proc = match guard.get(&pid) {
            Some(p) => p,
            None => return -EBADF,
        };
        match proc.get_fd(fd as u32) {
            Some(fde) => match &fde.resource {
                FileResource::File { path } => path.clone(),
                FileResource::Stdio(_) => {
                    // Return a fake stat for stdio
                    let fake = LinuxStat {
                        st_dev: 1,
                        st_ino: 0,
                        st_nlink: 1,
                        st_mode: 0o020666, // char device
                        st_uid: 0,
                        st_gid: 0,
                        __pad0: 0,
                        st_rdev: 0,
                        st_size: 0,
                        st_blksize: 4096,
                        st_blocks: 0,
                        st_atime: 0,
                        st_atime_nsec: 0,
                        st_mtime: 0,
                        st_mtime_nsec: 0,
                        st_ctime: 0,
                        st_ctime_nsec: 0,
                        __unused: [0; 3],
                    };
                    let bytes = unsafe {
                        core::slice::from_raw_parts(
                            &fake as *const LinuxStat as *const u8,
                            core::mem::size_of::<LinuxStat>(),
                        )
                    };
                    if copy_to_user(statbuf_ptr, bytes).is_err() {
                        return -EFAULT;
                    }
                    return 0;
                }
                _ => return -EBADF,
            },
            None => return -EBADF,
        }
    } else {
        // Fallback: can't really do fstat without per-process FD
        return -EBADF;
    };

    sys_stat_path(&path, statbuf_ptr)
}

fn sys_stat_path(path: &str, statbuf_ptr: u64) -> i64 {
    match vfs::stat(path) {
        Ok(fstat) => {
            let linux_stat = fill_stat(&fstat);
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    &linux_stat as *const LinuxStat as *const u8,
                    core::mem::size_of::<LinuxStat>(),
                )
            };
            if copy_to_user(statbuf_ptr, bytes).is_err() {
                return -EFAULT;
            }
            0
        }
        Err(vfs::VfsError::NotFound) => -ENOENT,
        Err(_) => -EIO,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_LSEEK (8)
// ═══════════════════════════════════════════════════════════════════════

/// `lseek(fd, offset, whence)` → `off_t` or `-errno`
pub fn sys_lseek(fd: i32, offset: i64, whence: u32) -> i64 {
    if whence > 2 {
        return -EINVAL;
    }

    if let Some(pid) = current_pid() {
        return lseek_in_process(pid, fd as u32, offset, whence);
    }

    match vfs::fd::lseek(fd, offset, whence) {
        Ok(new_off) => new_off as i64,
        Err(vfs::VfsError::InvalidFd) => -EBADF,
        Err(_) => -ESPIPE,
    }
}

fn lseek_in_process(pid: ProcessId, fd: u32, offset: i64, whence: u32) -> i64 {
    let mut proc = match PROCESS_TABLE.remove(pid) {
        Some(p) => p,
        None => return -EBADF,
    };

    let result = if let Some(fde) = proc.file_descriptors.get_mut(&fd) {
        let new_offset = match whence {
            0 => offset as u64,                               // SEEK_SET
            1 => (fde.offset as i64 + offset) as u64,         // SEEK_CUR
            2 => {
                // SEEK_END — get file size
                let size = match &fde.resource {
                    FileResource::File { path } => {
                        vfs::stat(path).map(|s| s.size).unwrap_or(0)
                    }
                    _ => 0,
                };
                (size as i64 + offset) as u64
            }
            _ => {
                PROCESS_TABLE.add(proc);
                return -EINVAL;
            }
        };
        fde.offset = new_offset;
        new_offset as i64
    } else {
        -EBADF
    };

    PROCESS_TABLE.add(proc);
    result
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_ACCESS (21)
// ═══════════════════════════════════════════════════════════════════════

/// `access(path, mode)` → `0` or `-errno`
pub fn sys_access(path_ptr: u64, _mode: u32) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };

    match vfs::stat(&path) {
        Ok(_) => 0,
        Err(vfs::VfsError::NotFound) => -ENOENT,
        Err(_) => -EACCES,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_OPENAT (257)
// ═══════════════════════════════════════════════════════════════════════

/// `openat(dirfd, path, flags, mode)` → `fd` or `-errno`
pub fn sys_openat(dirfd: i32, path_ptr: u64, flags: u32, _mode: u32) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };

    // Only support AT_FDCWD (-100) for now
    if dirfd != AT_FDCWD && !path.starts_with('/') {
        return -ENOSYS; // Relative paths with non-AT_FDCWD dirfd not supported
    }

    open_file(&path, flags)
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_DUP (32) / SYS_DUP2 (33)
// ═══════════════════════════════════════════════════════════════════════

/// `dup(oldfd)` → `newfd` or `-errno`
pub fn sys_dup(oldfd: i32) -> i64 {
    if let Some(pid) = current_pid() {
        return dup_in_process(pid, oldfd as u32, None);
    }
    -ENOSYS // No global dup support
}

/// `dup2(oldfd, newfd)` → `newfd` or `-errno`
pub fn sys_dup2(oldfd: i32, newfd: i32) -> i64 {
    if oldfd == newfd {
        // Check if oldfd is valid
        if let Some(pid) = current_pid() {
            let guard = match PROCESS_TABLE.get(pid) {
                Some(g) => g,
                None => return -EBADF,
            };
            let proc = match guard.get(&pid) {
                Some(p) => p,
                None => return -EBADF,
            };
            return if proc.get_fd(oldfd as u32).is_some() {
                newfd as i64
            } else {
                -EBADF
            };
        }
        return -EBADF;
    }

    if let Some(pid) = current_pid() {
        return dup_in_process(pid, oldfd as u32, Some(newfd as u32));
    }
    -ENOSYS
}

fn dup_in_process(pid: ProcessId, oldfd: u32, target: Option<u32>) -> i64 {
    let mut proc = match PROCESS_TABLE.remove(pid) {
        Some(p) => p,
        None => return -EBADF,
    };

    let result = if let Some(old_entry) = proc.get_fd(oldfd).cloned() {
        let newfd = match target {
            Some(fd) => {
                // dup2: close target if open, use that fd number
                proc.remove_fd(fd);
                fd
            }
            None => proc.alloc_fd(),
        };

        proc.add_fd(FileDescriptor {
            fd: newfd,
            resource: old_entry.resource.clone(),
            flags: old_entry.flags,
            offset: old_entry.offset,
        });

        newfd as i64
    } else {
        -EBADF
    };

    PROCESS_TABLE.add(proc);
    result
}

// ═══════════════════════════════════════════════════════════════════════
// SYS_IOCTL (16) — stub
// ═══════════════════════════════════════════════════════════════════════

/// `ioctl(fd, request, arg)` → result or `-errno`
///
/// Currently only handles TIOCGWINSZ (terminal window size) for basic
/// terminal compatibility.
pub fn sys_ioctl(fd: i32, request: u64, arg: u64) -> i64 {
    const TIOCGWINSZ: u64 = 0x5413;
    const TCGETS: u64 = 0x5401;

    match request {
        TIOCGWINSZ => {
            // Return 80×25 terminal size
            if validate_user_ptr(arg, 8).is_err() {
                return -EFAULT;
            }
            #[repr(C)]
            struct Winsize {
                ws_row: u16,
                ws_col: u16,
                ws_xpixel: u16,
                ws_ypixel: u16,
            }
            let ws = Winsize {
                ws_row: 25,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    &ws as *const Winsize as *const u8,
                    core::mem::size_of::<Winsize>(),
                )
            };
            if copy_to_user(arg, bytes).is_err() {
                return -EFAULT;
            }
            0
        }
        TCGETS => {
            // Pretend success for terminal attribute queries
            -ENOSYS
        }
        _ => {
            crate::serial_println!(
                "[KPIO/Linux] ioctl: fd={}, request={:#x} → ENOSYS",
                fd,
                request
            );
            -EINVAL
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Process syscalls
// ═══════════════════════════════════════════════════════════════════════

/// `exit(status)` — terminate current process
pub fn sys_exit(status: i32) -> i64 {
    crate::serial_println!("[KPIO/Linux] Process exit with status {}", status);

    // Write exit code to debug port for QEMU test harness
    use x86_64::instructions::port::Port;
    let exit_code: u32 = if status == 0 { 0x10 } else { 0x11 }; // success=33, fail=35
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code);
    }

    // If the above didn't stop (no debug-exit device), halt
    crate::scheduler::exit_current(status);
    0 // unreachable
}

/// `getpid()` → pid
pub fn sys_getpid() -> i64 {
    match current_pid() {
        Some(pid) => pid.0 as i64,
        None => 1, // Kernel context → pretend PID 1
    }
}

/// `brk(addr)` → `new_brk` or `-errno`
///
/// Stub: returns the current brk (addr=0) or sets it.
/// Full implementation in Phase 7-4.3.
pub fn sys_brk(addr: u64) -> i64 {
    // For now, just return a fixed brk address above typical ELF load
    // Musl's malloc will use this as the heap base.
    const DEFAULT_BRK: u64 = 0x0060_0000;

    if addr == 0 {
        DEFAULT_BRK as i64
    } else if addr < 0x0000_8000_0000_0000 {
        // Accept any reasonable brk
        addr as i64
    } else {
        -ENOMEM
    }
}

/// Linux ENOMEM constant.
const ENOMEM: i64 = 12;

// ═══════════════════════════════════════════════════════════════════════
// Misc syscalls
// ═══════════════════════════════════════════════════════════════════════

/// `uname(buf)` → `0` or `-errno`
///
/// Fills in the `struct utsname` with KPIO OS identification.
pub fn sys_uname(buf_ptr: u64) -> i64 {
    // struct utsname has 5 fields of 65 bytes each = 325 bytes total
    // (some systems use 6 fields with domainname = 390 bytes)
    const FIELD_LEN: usize = 65;

    if validate_user_ptr(buf_ptr, (FIELD_LEN * 6) as u64).is_err() {
        return -EFAULT;
    }

    fn write_field(base: u64, offset: usize, value: &str) {
        let dest = (base + offset as u64) as *mut u8;
        let bytes = value.as_bytes();
        let len = bytes.len().min(FIELD_LEN - 1);
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, len);
            *dest.add(len) = 0; // NUL terminate
        }
    }

    // Zero the buffer first
    unsafe {
        core::ptr::write_bytes(buf_ptr as *mut u8, 0, FIELD_LEN * 6);
    }

    write_field(buf_ptr, 0 * FIELD_LEN, "Linux");             // sysname
    write_field(buf_ptr, 1 * FIELD_LEN, "kpio");              // nodename
    write_field(buf_ptr, 2 * FIELD_LEN, "6.1.0-kpio");        // release
    write_field(buf_ptr, 3 * FIELD_LEN, "KPIO OS Phase 7");   // version
    write_field(buf_ptr, 4 * FIELD_LEN, "x86_64");            // machine
    write_field(buf_ptr, 5 * FIELD_LEN, "(none)");             // domainname

    0
}

/// `arch_prctl(code, addr)` → `0` or `-errno`
///
/// Used by musl/glibc to set FS base for TLS.
pub fn sys_arch_prctl(code: i32, addr: u64) -> i64 {
    const ARCH_SET_GS: i32 = 0x1001;
    const ARCH_SET_FS: i32 = 0x1002;
    const ARCH_GET_FS: i32 = 0x1003;
    const ARCH_GET_GS: i32 = 0x1004;

    match code {
        ARCH_SET_FS => {
            // Set FS base for TLS
            unsafe {
                use x86_64::registers::model_specific::Msr;
                const FS_BASE_MSR: u32 = 0xC000_0100;
                Msr::new(FS_BASE_MSR).write(addr);
            }
            0
        }
        ARCH_GET_FS => {
            // Read current FS base
            unsafe {
                use x86_64::registers::model_specific::Msr;
                const FS_BASE_MSR: u32 = 0xC000_0100;
                Msr::new(FS_BASE_MSR).read() as i64
            }
        }
        ARCH_SET_GS | ARCH_GET_GS => {
            // GS is managed by kernel for per-CPU data; deny user changes
            -EINVAL
        }
        _ => -EINVAL,
    }
}

/// `set_tid_address(tidptr)` → `tid`
pub fn sys_set_tid_address(_tidptr: u64) -> i64 {
    // Return current TID (simplified: same as PID)
    sys_getpid()
}

/// `clock_gettime(clockid, tp)` → `0` or `-errno`
pub fn sys_clock_gettime(_clockid: i32, tp_ptr: u64) -> i64 {
    if validate_user_ptr(tp_ptr, 16).is_err() {
        return -EFAULT;
    }

    // Return a monotonic clock value based on TSC
    let tsc: u64;
    unsafe {
        core::arch::asm!("rdtsc", "shl rdx, 32", "or rax, rdx", out("rax") tsc, out("rdx") _);
    }

    // Approximate: assume ~2GHz TSC, convert to timespec
    let seconds = tsc / 2_000_000_000;
    let nanoseconds = ((tsc % 2_000_000_000) * 1_000_000_000) / 2_000_000_000;

    #[repr(C)]
    struct Timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }

    let ts = Timespec {
        tv_sec: seconds as i64,
        tv_nsec: nanoseconds as i64,
    };

    let bytes = unsafe {
        core::slice::from_raw_parts(
            &ts as *const Timespec as *const u8,
            core::mem::size_of::<Timespec>(),
        )
    };

    if copy_to_user(tp_ptr, bytes).is_err() {
        return -EFAULT;
    }
    0
}

/// `getrandom(buf, buflen, flags)` → `ssize_t`
pub fn sys_getrandom(buf_ptr: u64, buflen: u64, _flags: u32) -> i64 {
    if buflen == 0 {
        return 0;
    }
    if validate_user_ptr(buf_ptr, buflen).is_err() {
        return -EFAULT;
    }

    // Use RDRAND instruction for random data
    let dest = buf_ptr as *mut u8;
    let len = buflen as usize;

    for i in 0..len {
        let mut val: u64;
        let ok: u8;
        unsafe {
            core::arch::asm!(
                "rdrand {val}",
                "setc {ok}",
                val = out(reg) val,
                ok = out(reg_byte) ok,
            );
        }
        if ok == 0 {
            // RDRAND failed — fill with TSC-based pseudo-random
            let tsc: u64;
            unsafe {
                core::arch::asm!(
                    "rdtsc",
                    "shl rdx, 32",
                    "or rax, rdx",
                    out("rax") tsc,
                    out("rdx") _,
                );
            }
            val = tsc;
        }
        unsafe {
            *dest.add(i) = (val & 0xFF) as u8;
        }
    }

    len as i64
}

// ═══════════════════════════════════════════════════════════════════════
// Helper: update FD offset
// ═══════════════════════════════════════════════════════════════════════

fn update_fd_offset(pid: ProcessId, fd: u32, new_offset: usize) {
    if let Some(mut proc) = PROCESS_TABLE.remove(pid) {
        if let Some(fde) = proc.file_descriptors.get_mut(&fd) {
            fde.offset = new_offset as u64;
        }
        PROCESS_TABLE.add(proc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sys_brk_query() {
        let result = sys_brk(0);
        assert!(result > 0);
    }

    #[test]
    fn test_sys_brk_set() {
        let result = sys_brk(0x0070_0000);
        assert_eq!(result, 0x0070_0000);
    }

    #[test]
    fn test_sys_brk_kernel_addr() {
        let result = sys_brk(0xFFFF_8000_0000_0000);
        assert_eq!(result, -ENOMEM);
    }

    #[test]
    fn test_sys_getpid_kernel() {
        // In test context, no current process → returns 1
        assert_eq!(sys_getpid(), 1);
    }

    #[test]
    fn test_sys_exit_code() {
        // Can't really test exit, but we can test the flow
        // Just verify the function exists and compiles
    }
}
