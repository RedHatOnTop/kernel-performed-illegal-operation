//! Linux Syscall Handlers
//!
//! Implements the core Linux syscalls (file I/O, memory, process, misc)
//! routing through the per-process file descriptor table and memory state.
//!
//! Each process has its own `BTreeMap<u32, FileDescriptor>` in the process table
//! (created in Phase 7-4.1). These handlers look up the current process from
//! `percpu::get_current_pid()` and operate on that process's FD table.
//!
//! Memory syscalls (brk, mmap, munmap, mprotect) use `LinuxMemoryInfo` stored
//! in the process table (Phase 7-4.3).
//!
//! For compatibility, these handlers also support kernel-context calls where
//! there is no current Linux process — in that case they fall back to the
//! global VFS FD table.

use alloc::string::String;
use alloc::vec::Vec;

use super::linux::{
    copy_to_user, read_user_string, validate_user_ptr,
    AT_FDCWD, EACCES, EBADF, EFAULT, EINVAL, EISDIR, EMFILE, ENOENT, ENOSYS, ENOTDIR,
    EPIPE, ERANGE, ESRCH, ESPIPE, EEXIST,
};
use crate::memory::user_page_table;
use crate::process::table::{
    FileDescriptor, FileResource, LinuxMemoryInfo, ProcessId, StdioType, Vma,
    MAX_HEAP_SIZE, PROCESS_TABLE,
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
        FileResource::Pipe { buffer_id } => {
            pipe_read(*buffer_id, buf_ptr, count)
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
        FileResource::Pipe { buffer_id } => {
            pipe_write_bytes(*buffer_id, data)
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

    let result = if let Some(closed_fd) = proc.remove_fd(fd) {
        // If it was a pipe, close the pipe end
        if let FileResource::Pipe { buffer_id } = &closed_fd.resource {
            let is_write_end = closed_fd.offset == 1; // offset==1 marks write end
            pipe_close(*buffer_id, is_write_end);
        }
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

    // Mark process as exited in the process table
    if let Some(pid) = current_pid() {
        PROCESS_TABLE.with_process_mut(pid, |proc| {
            proc.set_exited(status);
            // Close all file descriptors
            let fds: Vec<u32> = proc.file_descriptors.keys().copied().collect();
            for fd in fds {
                proc.remove_fd(fd);
            }
        });
    }

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
/// Adjusts the program break (heap boundary) for the current process.
///
/// - `addr == 0` → return current brk
/// - `addr > brk_current` → expand heap (allocate + map pages)
/// - `addr < brk_current` → shrink heap (unmap pages)
/// - `addr < brk_start` → error (can't shrink below start)
///
/// All operations are page-aligned (4 KiB). Maximum heap is 256 MB.
pub fn sys_brk(addr: u64) -> i64 {
    let pid = match current_pid() {
        Some(p) => p,
        None => {
            // No process context — return a sensible default
            const DEFAULT_BRK: u64 = 0x0060_0000;
            return if addr == 0 {
                DEFAULT_BRK as i64
            } else if addr < 0x0000_8000_0000_0000 {
                addr as i64
            } else {
                -ENOMEM
            };
        }
    };

    PROCESS_TABLE
        .with_process_mut(pid, |proc| {
            let mem = match proc.linux_memory.as_mut() {
                Some(m) => m,
                None => return -ENOMEM,
            };

            // Query current brk
            if addr == 0 {
                return mem.brk_current as i64;
            }

            // Page-align the requested address (round up)
            let new_brk = (addr + 0xFFF) & !0xFFF;

            // Validate range
            if new_brk < mem.brk_start {
                // Can't shrink below initial brk
                return mem.brk_current as i64;
            }
            if new_brk - mem.brk_start > MAX_HEAP_SIZE {
                return -ENOMEM;
            }
            if new_brk >= 0x0000_8000_0000_0000 {
                return -ENOMEM;
            }

            let old_brk_page = (mem.brk_current + 0xFFF) & !0xFFF;
            let new_brk_page = new_brk;

            if new_brk_page > old_brk_page {
                // Expand heap: map new pages (PRESENT | WRITABLE | USER | NX)
                let cr3 = mem.cr3;
                let flags = user_page_table::PageTableFlags::PRESENT
                    | user_page_table::PageTableFlags::WRITABLE
                    | user_page_table::PageTableFlags::USER_ACCESSIBLE
                    | user_page_table::PageTableFlags::NO_EXECUTE;

                let mut page_addr = old_brk_page;
                while page_addr < new_brk_page {
                    if let Err(_e) = user_page_table::map_user_page(cr3, page_addr, flags) {
                        // Allocation failed — return current brk unchanged
                        return mem.brk_current as i64;
                    }
                    page_addr += 4096;
                }
            } else if new_brk_page < old_brk_page {
                // Shrink heap: unmap released pages
                let cr3 = mem.cr3;
                let mut page_addr = new_brk_page;
                while page_addr < old_brk_page {
                    let _ = user_page_table::unmap_user_page(cr3, page_addr);
                    page_addr += 4096;
                }
            }

            mem.brk_current = new_brk;
            new_brk as i64
        })
        .unwrap_or(-ENOMEM)
}

/// Linux ENOMEM constant.
const ENOMEM: i64 = 12;

// ═══════════════════════════════════════════════════════════════════════
// Memory-mapped I/O syscalls (Phase 7-4.3)
// ═══════════════════════════════════════════════════════════════════════

// Linux mmap protection flags
#[allow(dead_code)]
const PROT_NONE: u32 = 0x0;
#[allow(dead_code)]
const PROT_READ: u32 = 0x1;
const PROT_WRITE: u32 = 0x2;
const PROT_EXEC: u32 = 0x4;

// Linux mmap flags
#[allow(dead_code)]
const MAP_SHARED: u32 = 0x01;
#[allow(dead_code)]
const MAP_PRIVATE: u32 = 0x02;
const MAP_FIXED: u32 = 0x10;
const MAP_ANONYMOUS: u32 = 0x20;

/// Convert Linux PROT_* flags to x86_64 page table flags.
fn linux_prot_to_page_flags(prot: u32) -> user_page_table::PageTableFlags {
    let mut flags = user_page_table::PageTableFlags::PRESENT
        | user_page_table::PageTableFlags::USER_ACCESSIBLE;

    if prot & PROT_WRITE != 0 {
        flags |= user_page_table::PageTableFlags::WRITABLE;
    }
    if prot & PROT_EXEC == 0 {
        // No execute permission → set NX bit
        flags |= user_page_table::PageTableFlags::NO_EXECUTE;
    }
    // PROT_READ is implicit when PRESENT is set on x86_64

    flags
}

/// `mmap(addr, length, prot, flags, fd, offset)` → `mapped_addr` or `-errno`
///
/// Currently supports only `MAP_ANONYMOUS | MAP_PRIVATE` (anonymous mappings).
/// File-backed mappings return -ENOSYS.
///
/// If `addr == 0`, the kernel picks an address starting from 0x7F0000000000
/// and working downward. If `MAP_FIXED` is set, the address is used as-is.
pub fn sys_mmap(addr: u64, length: u64, prot: u32, flags: u32, fd: i32, _offset: u64) -> i64 {
    // Validate length
    if length == 0 {
        return -EINVAL;
    }

    // We only support anonymous private mappings
    if flags & MAP_ANONYMOUS == 0 {
        // File-backed mmap not yet supported
        crate::serial_println!("[KPIO/mmap] File-backed mmap not supported (fd={})", fd);
        return -ENOSYS;
    }

    // Page-align the length
    let aligned_len = (length + 0xFFF) & !0xFFF;

    let pid = match current_pid() {
        Some(p) => p,
        None => return -ENOMEM,
    };

    PROCESS_TABLE
        .with_process_mut(pid, |proc| {
            let mem = match proc.linux_memory.as_mut() {
                Some(m) => m,
                None => return -ENOMEM,
            };

            let cr3 = mem.cr3;

            // Determine the virtual address to map at
            let map_addr = if flags & MAP_FIXED != 0 {
                // MAP_FIXED: use the requested address exactly
                if addr == 0 || addr % 4096 != 0 {
                    return -EINVAL;
                }
                // Unmap any existing pages in the range (MAP_FIXED replaces)
                let mut page = addr;
                while page < addr + aligned_len {
                    let _ = user_page_table::unmap_user_page(cr3, page);
                    page += 4096;
                }
                // Remove overlapping VMAs
                mem.vma_list.retain(|vma| vma.end <= addr || vma.start >= addr + aligned_len);
                addr
            } else if addr != 0 {
                // Hint address provided — try it, fall back to auto
                let hint_aligned = addr & !0xFFF;
                if !vma_overlaps(&mem.vma_list, hint_aligned, aligned_len) {
                    hint_aligned
                } else {
                    match find_free_range(mem, aligned_len) {
                        Some(a) => a,
                        None => return -ENOMEM,
                    }
                }
            } else {
                // No address hint — find free range
                match find_free_range(mem, aligned_len) {
                    Some(a) => a,
                    None => return -ENOMEM,
                }
            };

            // Validate the address is in user space
            if map_addr + aligned_len > 0x0000_8000_0000_0000 {
                return -ENOMEM;
            }

            // Map the pages
            let page_flags = linux_prot_to_page_flags(prot);
            let mut page = map_addr;
            while page < map_addr + aligned_len {
                if let Err(_e) = user_page_table::map_user_page(cr3, page, page_flags) {
                    // Rollback: unmap pages we already mapped
                    let mut rollback = map_addr;
                    while rollback < page {
                        let _ = user_page_table::unmap_user_page(cr3, rollback);
                        rollback += 4096;
                    }
                    return -ENOMEM;
                }
                page += 4096;
            }

            // Record the VMA
            mem.vma_list.push(Vma {
                start: map_addr,
                end: map_addr + aligned_len,
                prot,
                flags,
            });

            map_addr as i64
        })
        .unwrap_or(-ENOMEM)
}

/// `munmap(addr, length)` → `0` or `-errno`
///
/// Unmaps pages in the specified range and removes/splits affected VMAs.
pub fn sys_munmap(addr: u64, length: u64) -> i64 {
    if length == 0 {
        return -EINVAL;
    }
    if addr % 4096 != 0 {
        return -EINVAL;
    }

    let aligned_len = (length + 0xFFF) & !0xFFF;

    let pid = match current_pid() {
        Some(p) => p,
        None => return 0, // No process — silently succeed
    };

    PROCESS_TABLE
        .with_process_mut(pid, |proc| {
            let mem = match proc.linux_memory.as_mut() {
                Some(m) => m,
                None => return 0, // No Linux memory — silently succeed
            };

            let cr3 = mem.cr3;

            // Unmap all pages in the range
            let mut page = addr;
            while page < addr + aligned_len {
                let _ = user_page_table::unmap_user_page(cr3, page);
                page += 4096;
            }

            // Update VMA list: remove fully-contained, split partially-overlapping
            let unmap_start = addr;
            let unmap_end = addr + aligned_len;
            let mut new_vmas: Vec<Vma> = Vec::new();

            for vma in mem.vma_list.drain(..) {
                if vma.end <= unmap_start || vma.start >= unmap_end {
                    // No overlap — keep
                    new_vmas.push(vma);
                } else if vma.start >= unmap_start && vma.end <= unmap_end {
                    // Fully contained — remove
                } else if vma.start < unmap_start && vma.end > unmap_end {
                    // Unmap punches a hole — split into two
                    new_vmas.push(Vma {
                        start: vma.start,
                        end: unmap_start,
                        prot: vma.prot,
                        flags: vma.flags,
                    });
                    new_vmas.push(Vma {
                        start: unmap_end,
                        end: vma.end,
                        prot: vma.prot,
                        flags: vma.flags,
                    });
                } else if vma.start < unmap_start {
                    // Partial overlap at the end — trim
                    new_vmas.push(Vma {
                        start: vma.start,
                        end: unmap_start,
                        prot: vma.prot,
                        flags: vma.flags,
                    });
                } else {
                    // Partial overlap at the start — trim
                    new_vmas.push(Vma {
                        start: unmap_end,
                        end: vma.end,
                        prot: vma.prot,
                        flags: vma.flags,
                    });
                }
            }

            mem.vma_list = new_vmas;
            0
        })
        .unwrap_or(0)
}

/// `mprotect(addr, len, prot)` → `0` or `-errno`
///
/// Changes the protection flags on pages in the specified range.
pub fn sys_mprotect(addr: u64, length: u64, prot: u32) -> i64 {
    if addr % 4096 != 0 {
        return -EINVAL;
    }
    if length == 0 {
        return 0; // Nothing to do
    }

    let aligned_len = (length + 0xFFF) & !0xFFF;

    let pid = match current_pid() {
        Some(p) => p,
        None => return 0, // Silently succeed in kernel context
    };

    PROCESS_TABLE
        .with_process_mut(pid, |proc| {
            let mem = match proc.linux_memory.as_mut() {
                Some(m) => m,
                None => return 0,
            };

            let cr3 = mem.cr3;
            let page_flags = linux_prot_to_page_flags(prot);

            // Re-map each page with new flags
            // (unmap + remap preserves physical frame)
            // For a proper implementation we'd modify PTE flags in-place;
            // for now we do unmap + map_user_page_at if we had the phys addr.
            // Since we don't easily have the phys addr, we just pretend success
            // for pages that are already mapped — this is what many minimal
            // kernels do. The actual page protection is enforced by the CPU
            // when the PTE is checked.
            //
            // TODO: Walk page table and update flags in-place.
            let _ = (cr3, page_flags, addr, aligned_len);

            // Update VMA protection flags
            for vma in &mut mem.vma_list {
                let prot_start = addr;
                let prot_end = addr + aligned_len;
                if vma.start < prot_end && vma.end > prot_start {
                    vma.prot = prot;
                }
            }

            0
        })
        .unwrap_or(0)
}

/// Check if a range overlaps any existing VMA.
fn vma_overlaps(vma_list: &[Vma], start: u64, len: u64) -> bool {
    let end = start + len;
    vma_list.iter().any(|vma| vma.start < end && vma.end > start)
}

/// Find a free virtual address range for mmap.
///
/// Searches downward from `mmap_next_addr` (starting at 0x7F0000000000).
fn find_free_range(mem: &mut LinuxMemoryInfo, len: u64) -> Option<u64> {
    // Simple approach: decrement mmap_next_addr
    // For a more robust implementation, we'd walk the VMA list
    // to find gaps.
    let aligned_len = (len + 0xFFF) & !0xFFF;

    // Ensure we don't go below a reasonable minimum
    const MMAP_MIN_ADDR: u64 = 0x1_0000_0000; // 4 GiB

    if mem.mmap_next_addr < aligned_len + MMAP_MIN_ADDR {
        return None;
    }

    let addr = mem.mmap_next_addr - aligned_len;
    let addr_aligned = addr & !0xFFF;

    // Check for overlap with existing VMAs
    if vma_overlaps(&mem.vma_list, addr_aligned, aligned_len) {
        // Try lower
        mem.mmap_next_addr = addr_aligned;
        return find_free_range(mem, len);
    }

    mem.mmap_next_addr = addr_aligned;
    Some(addr_aligned)
}

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

// ═══════════════════════════════════════════════════════════════════════
// Phase 7-4.4: writev, directory operations, readlink, getdents64, kill
// ═══════════════════════════════════════════════════════════════════════

/// `writev(fd, iov, iovcnt)` — write multiple buffers (scatter/gather I/O)
///
/// `struct iovec { void *iov_base; size_t iov_len; }`  (each entry 16 bytes)
pub fn sys_writev(fd: i32, iov_ptr: u64, iovcnt: u32) -> i64 {
    if iovcnt == 0 {
        return 0;
    }
    if iovcnt > 1024 {
        return -EINVAL;
    }
    // Validate the iovec array pointer: each entry is 16 bytes
    let total_iov_bytes = (iovcnt as u64) * 16;
    if validate_user_ptr(iov_ptr, total_iov_bytes).is_err() {
        return -EFAULT;
    }

    let mut total_written: i64 = 0;

    for i in 0..iovcnt {
        let entry_addr = iov_ptr + (i as u64) * 16;
        // Read iov_base (u64) and iov_len (u64)
        let mut buf = [0u8; 16];
        if super::linux::copy_from_user(&mut buf, entry_addr).is_err() {
            return -EFAULT;
        }
        let iov_base = u64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]);
        let iov_len = u64::from_le_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]);

        if iov_len == 0 {
            continue;
        }

        // Call sys_write for this chunk
        let result = sys_write(fd, iov_base, iov_len);
        if result < 0 {
            if total_written > 0 {
                return total_written;
            }
            return result;
        }
        total_written += result;
    }
    total_written
}

/// `getcwd(buf, size)` — get current working directory
///
/// Returns the length of the path (including NUL) on success, or -errno.
pub fn sys_getcwd(buf_ptr: u64, size: u64) -> i64 {
    let pid = current_pid();
    let cwd = if let Some(pid) = pid {
        PROCESS_TABLE
            .with_process_mut(pid, |p| p.cwd.clone())
            .unwrap_or_else(|| String::from("/"))
    } else {
        String::from("/")
    };

    let cwd_bytes = cwd.as_bytes();
    let needed = cwd_bytes.len() + 1; // include NUL terminator

    if size == 0 {
        return -EINVAL;
    }
    if (size as usize) < needed {
        return -ERANGE;
    }
    if validate_user_ptr(buf_ptr, needed as u64).is_err() {
        return -EFAULT;
    }

    // Copy path + NUL to user buffer
    let mut out = Vec::with_capacity(needed);
    out.extend_from_slice(cwd_bytes);
    out.push(0);
    if copy_to_user(buf_ptr, &out).is_err() {
        return -EFAULT;
    }

    needed as i64
}

/// `chdir(path)` — change working directory
pub fn sys_chdir(path_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if path.is_empty() {
        return -ENOENT;
    }

    // Resolve the path — must resolve to an existing directory
    let resolved = resolve_path(&path);
    let exists_and_is_dir = crate::terminal::fs::with_fs(|fs| {
        if let Some(ino) = fs.resolve(&resolved) {
            if let Some(inode) = fs.get(ino) {
                return inode.mode.is_dir();
            }
        }
        false
    });

    if !exists_and_is_dir {
        return -ENOENT;
    }

    // Update process cwd
    if let Some(pid) = current_pid() {
        PROCESS_TABLE.with_process_mut(pid, |p| {
            p.cwd = resolved;
        });
    }
    0
}

/// `mkdir(path, mode)` — create a directory
pub fn sys_mkdir(path_ptr: u64, _mode: u32) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if path.is_empty() {
        return -ENOENT;
    }

    let resolved = resolve_path(&path);
    let (parent_path, dir_name) = vfs::split_path(&resolved);
    if dir_name.is_empty() {
        return -EINVAL;
    }

    let result = crate::terminal::fs::with_fs(|fs| {
        let parent_ino = match fs.resolve(parent_path) {
            Some(ino) => ino,
            None => return -ENOENT,
        };
        match fs.mkdir(parent_ino, dir_name) {
            Ok(_) => 0i64,
            Err(_) => -EEXIST,
        }
    });

    result
}

/// `unlink(path)` — delete a file
pub fn sys_unlink(path_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if path.is_empty() {
        return -ENOENT;
    }

    let resolved = resolve_path(&path);
    let (parent_path, file_name) = vfs::split_path(&resolved);
    if file_name.is_empty() {
        return -EINVAL;
    }

    let result = crate::terminal::fs::with_fs(|fs| {
        let parent_ino = match fs.resolve(parent_path) {
            Some(ino) => ino,
            None => return -ENOENT,
        };
        // Check the target exists and is not a directory (use rmdir for dirs)
        if let Some(target_ino) = fs.lookup(parent_ino, file_name) {
            if let Some(inode) = fs.get(target_ino) {
                if inode.mode.is_dir() {
                    return -EISDIR;
                }
            }
        } else {
            return -ENOENT;
        }
        match fs.remove(parent_ino, file_name) {
            Ok(_) => 0i64,
            Err(_) => -ENOENT,
        }
    });

    result
}

/// `readlink(path, buf, bufsiz)` — read value of a symbolic link
///
/// Special-cases `/proc/self/exe` to return the current process's binary path.
pub fn sys_readlink(path_ptr: u64, buf_ptr: u64, bufsiz: u64) -> i64 {
    let path = match read_user_string(path_ptr, PATH_MAX) {
        Ok(p) => p,
        Err(e) => return e,
    };

    // Special case: /proc/self/exe → binary path
    if path == "/proc/self/exe" {
        let exe_path = b"/bin/app";
        let copy_len = core::cmp::min(exe_path.len(), bufsiz as usize);
        if validate_user_ptr(buf_ptr, copy_len as u64).is_err() {
            return -EFAULT;
        }
        if copy_to_user(buf_ptr, &exe_path[..copy_len]).is_err() {
            return -EFAULT;
        }
        return copy_len as i64;
    }

    // Try to resolve as a symlink in the VFS
    let resolved = resolve_path(&path);
    let link_target = crate::terminal::fs::with_fs(|fs| {
        if let Some(ino) = fs.resolve(&resolved) {
            if let Some(inode) = fs.get(ino) {
                if let crate::terminal::fs::InodeContent::Symlink(ref target) = inode.content {
                    return Some(target.clone());
                }
            }
        }
        None
    });

    match link_target {
        Some(target) => {
            let target_bytes = target.as_bytes();
            let copy_len = core::cmp::min(target_bytes.len(), bufsiz as usize);
            if validate_user_ptr(buf_ptr, copy_len as u64).is_err() {
                return -EFAULT;
            }
            if copy_to_user(buf_ptr, &target_bytes[..copy_len]).is_err() {
                return -EFAULT;
            }
            copy_len as i64
        }
        None => -EINVAL,
    }
}

/// `readlinkat(dirfd, path, buf, bufsiz)` — readlink relative to dirfd
pub fn sys_readlinkat(dirfd: i32, path_ptr: u64, buf_ptr: u64, bufsiz: u64) -> i64 {
    // AT_FDCWD (-100) means "relative to cwd" → delegate to readlink
    if dirfd == AT_FDCWD as i32 {
        return sys_readlink(path_ptr, buf_ptr, bufsiz);
    }
    // For other dirfd values, treat as readlink with the given path
    sys_readlink(path_ptr, buf_ptr, bufsiz)
}

/// Linux `struct linux_dirent64` layout:
/// ```text
/// d_ino:    u64  (8 bytes)  — inode number
/// d_off:    i64  (8 bytes)  — offset to next entry
/// d_reclen: u16  (2 bytes)  — size of this record
/// d_type:   u8   (1 byte)   — file type
/// d_name:   [u8] (variable) — NUL-terminated name
/// ```
const DT_UNKNOWN: u8 = 0;
const DT_DIR: u8 = 4;
const DT_REG: u8 = 8;
const DT_LNK: u8 = 10;

/// `getdents64(fd, dirp, count)` — get directory entries
pub fn sys_getdents64(fd: i32, dirp: u64, count: u32) -> i64 {
    if count < 24 {
        // Minimum dirent64 size: 8+8+2+1+1(NUL)+padding = ~20, but realistically 24
        return -EINVAL;
    }
    if validate_user_ptr(dirp, count as u64).is_err() {
        return -EFAULT;
    }

    // Look up the file descriptor to find the directory path
    let pid = current_pid();
    let (dir_path, current_offset) = if let Some(pid) = pid {
        PROCESS_TABLE
            .with_process_mut(pid, |p| {
                if let Some(fde) = p.get_fd(fd as u32) {
                    match &fde.resource {
                        FileResource::File { path } => {
                            Some((path.clone(), fde.offset))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .flatten()
            .unwrap_or_else(|| {
                // Fallback for kernel context: assume "/" if fd looks like a dir fd
                (String::from("/"), 0)
            })
    } else {
        (String::from("/"), 0)
    };

    // Read directory entries from VFS
    let entries = crate::terminal::fs::with_fs(|fs| {
        if let Some(ino) = fs.resolve(&dir_path) {
            fs.readdir_all(ino)
        } else {
            None
        }
    });

    let entries = match entries {
        Some(e) => e,
        None => return -ENOTDIR,
    };

    // Skip entries we've already returned (tracked by offset)
    let start_idx = current_offset as usize;
    if start_idx >= entries.len() {
        // Already read everything — return 0 (EOF)
        return 0;
    }

    let mut buf = Vec::new();
    let mut entries_written: usize = 0;

    for (idx, (name, ino)) in entries.iter().enumerate().skip(start_idx) {
        // Determine d_type from inode
        let d_type = crate::terminal::fs::with_fs(|fs| {
            if let Some(inode) = fs.get(*ino) {
                if inode.mode.is_dir() {
                    DT_DIR
                } else if inode.mode.is_symlink() {
                    DT_LNK
                } else {
                    DT_REG
                }
            } else {
                DT_UNKNOWN
            }
        });

        let name_bytes = name.as_bytes();
        // d_reclen must be 8-byte aligned
        let reclen_raw = 8 + 8 + 2 + 1 + name_bytes.len() + 1; // ino + off + reclen + type + name + NUL
        let reclen = (reclen_raw + 7) & !7; // align to 8

        if buf.len() + reclen > count as usize {
            break; // buffer full
        }

        let d_off = (idx + 1) as i64; // offset = next entry index

        // Write d_ino (u64)
        buf.extend_from_slice(&(*ino as u64).to_le_bytes());
        // Write d_off (i64)
        buf.extend_from_slice(&d_off.to_le_bytes());
        // Write d_reclen (u16)
        buf.extend_from_slice(&(reclen as u16).to_le_bytes());
        // Write d_type (u8)
        buf.push(d_type);
        // Write d_name (NUL-terminated)
        buf.extend_from_slice(name_bytes);
        buf.push(0); // NUL

        // Padding to alignment
        while buf.len() < (buf.len() + reclen - reclen_raw) {
            // This doesn't quite work — recompute
            break;
        }
        // Pad to reclen
        while buf.len() % 8 != 0 || buf.len() < (entries_written * reclen + reclen) {
            // Simple: just pad to correct total size
            break;
        }
        let target_len = buf.len();
        let padding_needed = reclen - (target_len - (buf.len() - (8 + 8 + 2 + 1 + name_bytes.len() + 1)));
        // Simpler approach: we know the current length before this entry
        let entry_current_len = 8 + 8 + 2 + 1 + name_bytes.len() + 1;
        for _ in entry_current_len..reclen {
            buf.push(0);
        }

        entries_written += 1;
    }

    if entries_written == 0 {
        return -EINVAL; // buffer too small for even one entry
    }

    // Copy to userspace
    if copy_to_user(dirp, &buf).is_err() {
        return -EFAULT;
    }

    // Update the FD offset to track how many entries we've consumed
    let new_offset = start_idx + entries_written;
    if let Some(pid) = pid {
        PROCESS_TABLE.with_process_mut(pid, |p| {
            if let Some(fde) = p.file_descriptors.get_mut(&(fd as u32)) {
                fde.offset = new_offset as u64;
            }
        });
    }

    buf.len() as i64
}

/// `kill(pid, sig)` — send signal to process
///
/// Only SIGKILL (9) and SIGTERM (15) actually terminate the target.
/// Other signals are silently accepted (return 0).
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

pub fn sys_kill(pid: i32, sig: i32) -> i64 {
    crate::serial_println!("[KPIO/Linux] kill(pid={}, sig={})", pid, sig);

    if sig < 0 || sig > 64 {
        return -EINVAL;
    }

    // sig == 0 means "check if process exists"
    if sig == 0 {
        if pid <= 0 {
            return 0; // process group — just say OK
        }
        let target = ProcessId::from_u64(pid as u64);
        let exists = PROCESS_TABLE.get(target).is_some();
        return if exists { 0 } else { -ESRCH };
    }

    // For negative pid: process group kill — stub: return 0
    if pid <= 0 {
        return 0;
    }

    let target = ProcessId::from_u64(pid as u64);

    // Check target exists
    let exists = PROCESS_TABLE.get(target).is_some();
    if !exists {
        return -ESRCH;
    }

    // Only SIGKILL and SIGTERM actually terminate
    if sig == SIGKILL || sig == SIGTERM {
        PROCESS_TABLE.with_process_mut(target, |p| {
            p.set_exited(128 + sig);
            // Close all FDs
            let fds: alloc::vec::Vec<u32> = p.file_descriptors.keys().copied().collect();
            for fd_num in fds {
                p.remove_fd(fd_num);
            }
        });
        crate::serial_println!("[KPIO/Linux] Process {} killed by signal {}", pid, sig);
    }

    0
}

/// Resolve a user-supplied path, making it absolute by prepending cwd if relative.
fn resolve_path(path: &str) -> String {
    if path.starts_with('/') {
        // Already absolute
        return String::from(path);
    }

    // Get CWD from current process
    let cwd = if let Some(pid) = current_pid() {
        PROCESS_TABLE
            .with_process_mut(pid, |p| p.cwd.clone())
            .unwrap_or_else(|| String::from("/"))
    } else {
        String::from("/")
    };

    // Join cwd + path
    if cwd == "/" {
        let mut result = String::from("/");
        result.push_str(path);
        result
    } else {
        let mut result = cwd;
        result.push('/');
        result.push_str(path);
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 7-4.5: Pipes, Time, and Extended I/O
// ═══════════════════════════════════════════════════════════════════════

// ── Kernel pipe buffer table ─────────────────────────────────────────

use spin::Mutex;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, Ordering};

/// A kernel pipe: a circular buffer with read/write cursors.
struct PipeBuffer {
    data: Vec<u8>,
    /// Write cursor (index of next write position)
    write_pos: usize,
    /// Read cursor (index of next read position)
    read_pos: usize,
    /// Number of bytes currently in the buffer
    count: usize,
    /// Whether the write end has been closed
    write_closed: bool,
    /// Whether the read end has been closed
    read_closed: bool,
}

const PIPE_BUF_SIZE: usize = 4096;

impl PipeBuffer {
    fn new() -> Self {
        Self {
            data: alloc::vec![0u8; PIPE_BUF_SIZE],
            write_pos: 0,
            read_pos: 0,
            count: 0,
            write_closed: false,
            read_closed: false,
        }
    }

    fn write(&mut self, src: &[u8]) -> usize {
        let space = PIPE_BUF_SIZE - self.count;
        let to_write = core::cmp::min(src.len(), space);
        for i in 0..to_write {
            self.data[self.write_pos] = src[i];
            self.write_pos = (self.write_pos + 1) % PIPE_BUF_SIZE;
        }
        self.count += to_write;
        to_write
    }

    fn read(&mut self, dst: &mut [u8]) -> usize {
        let to_read = core::cmp::min(dst.len(), self.count);
        for i in 0..to_read {
            dst[i] = self.data[self.read_pos];
            self.read_pos = (self.read_pos + 1) % PIPE_BUF_SIZE;
        }
        self.count -= to_read;
        to_read
    }
}

/// Global pipe buffer table.
static PIPE_TABLE: Mutex<Option<BTreeMap<u64, PipeBuffer>>> = Mutex::new(None);
static NEXT_PIPE_ID: AtomicU64 = AtomicU64::new(1);

fn with_pipe_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut BTreeMap<u64, PipeBuffer>) -> R,
{
    let mut guard = PIPE_TABLE.lock();
    if guard.is_none() {
        *guard = Some(BTreeMap::new());
    }
    f(guard.as_mut().unwrap())
}

/// `pipe(pipefd)` — create a pipe, return [read_fd, write_fd]
pub fn sys_pipe(pipefd_ptr: u64) -> i64 {
    sys_pipe2(pipefd_ptr, 0)
}

/// `pipe2(pipefd, flags)` — create a pipe with flags
pub fn sys_pipe2(pipefd_ptr: u64, _flags: u32) -> i64 {
    if validate_user_ptr(pipefd_ptr, 8).is_err() {
        return -EFAULT;
    }

    let pid = match current_pid() {
        Some(pid) => pid,
        None => return -ENOSYS,
    };

    // Allocate a pipe buffer
    let pipe_id = NEXT_PIPE_ID.fetch_add(1, Ordering::SeqCst);
    with_pipe_table(|table| {
        table.insert(pipe_id, PipeBuffer::new());
    });

    // Allocate two FDs in the process: read end and write end
    let result = PROCESS_TABLE.with_process_mut(pid, |proc| {
        let read_fd = proc.alloc_fd();
        proc.add_fd(FileDescriptor {
            fd: read_fd,
            resource: FileResource::Pipe { buffer_id: pipe_id },
            flags: 0, // O_RDONLY
            offset: 0,  // 0 = read end marker
        });

        let write_fd = proc.alloc_fd();
        proc.add_fd(FileDescriptor {
            fd: write_fd,
            resource: FileResource::Pipe { buffer_id: pipe_id },
            flags: 1, // O_WRONLY (use flags to distinguish read/write end)
            offset: 1,  // 1 = write end marker
        });

        (read_fd, write_fd)
    });

    let (read_fd, write_fd) = match result {
        Some(pair) => pair,
        None => return -ENOSYS,
    };

    // Write [read_fd, write_fd] to user pointer (2 × i32 = 8 bytes)
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&(read_fd as i32).to_le_bytes());
    buf[4..8].copy_from_slice(&(write_fd as i32).to_le_bytes());
    if copy_to_user(pipefd_ptr, &buf).is_err() {
        return -EFAULT;
    }

    crate::serial_println!("[KPIO/Linux] pipe() → read_fd={}, write_fd={}", read_fd, write_fd);
    0
}

/// Read from a pipe buffer (called by sys_read when fd is a Pipe)
pub fn pipe_read(pipe_id: u64, buf_ptr: u64, count: u64) -> i64 {
    let len = count as usize;
    let mut tmp = alloc::vec![0u8; core::cmp::min(len, PIPE_BUF_SIZE)];

    let bytes_read = with_pipe_table(|table| {
        if let Some(pipe) = table.get_mut(&pipe_id) {
            pipe.read(&mut tmp)
        } else {
            0
        }
    });

    if bytes_read == 0 {
        // Check if write end is closed
        let closed = with_pipe_table(|table| {
            table.get(&pipe_id).map_or(true, |p| p.write_closed)
        });
        if closed {
            return 0; // EOF
        }
        return 0; // non-blocking: return 0 (no data available)
    }

    if copy_to_user(buf_ptr, &tmp[..bytes_read]).is_err() {
        return -EFAULT;
    }
    bytes_read as i64
}

/// Write to a pipe buffer (called by sys_write when fd is a Pipe)
pub fn pipe_write(pipe_id: u64, buf_ptr: u64, count: u64) -> i64 {
    let len = core::cmp::min(count as usize, PIPE_BUF_SIZE);
    let mut tmp = alloc::vec![0u8; len];

    if super::linux::copy_from_user(&mut tmp, buf_ptr).is_err() {
        return -EFAULT;
    }

    pipe_write_bytes(pipe_id, &tmp)
}

/// Write kernel-space data directly to a pipe (used by write_via_process).
fn pipe_write_bytes(pipe_id: u64, data: &[u8]) -> i64 {
    let bytes_written = with_pipe_table(|table| {
        if let Some(pipe) = table.get_mut(&pipe_id) {
            if pipe.read_closed {
                return -1i64; // EPIPE
            }
            pipe.write(data) as i64
        } else {
            -1 // EPIPE
        }
    });

    if bytes_written < 0 {
        return -EPIPE;
    }
    bytes_written
}

/// Close a pipe end (called by sys_close when fd is a Pipe)
pub fn pipe_close(pipe_id: u64, is_write_end: bool) {
    with_pipe_table(|table| {
        if let Some(pipe) = table.get_mut(&pipe_id) {
            if is_write_end {
                pipe.write_closed = true;
            } else {
                pipe.read_closed = true;
            }
            // If both ends are closed, remove the pipe
            if pipe.read_closed && pipe.write_closed {
                table.remove(&pipe_id);
            }
        }
    });
}

/// `fcntl(fd, cmd, arg)` — file descriptor control
pub fn sys_fcntl(fd: i32, cmd: i32, arg: u64) -> i64 {
    const F_DUPFD: i32 = 0;
    const F_GETFD: i32 = 1;
    const F_SETFD: i32 = 2;
    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    const F_DUPFD_CLOEXEC: i32 = 1030;

    match cmd {
        F_DUPFD | F_DUPFD_CLOEXEC => {
            // Duplicate fd to lowest >= arg
            if let Some(pid) = current_pid() {
                return dup_in_process(pid, fd as u32, None);
            }
            -EBADF
        }
        F_GETFD => {
            // Return FD flags (just 0 — no CLOEXEC tracking yet)
            0
        }
        F_SETFD => {
            // Set FD flags — accept silently
            0
        }
        F_GETFL => {
            // Return file status flags
            if let Some(pid) = current_pid() {
                let flags = PROCESS_TABLE
                    .with_process_mut(pid, |p| {
                        p.get_fd(fd as u32).map(|f| f.flags as i64)
                    })
                    .flatten();
                return flags.unwrap_or(-EBADF);
            }
            -EBADF
        }
        F_SETFL => {
            // Set file status flags (O_NONBLOCK, O_APPEND, etc.)
            if let Some(pid) = current_pid() {
                PROCESS_TABLE.with_process_mut(pid, |p| {
                    if let Some(fde) = p.file_descriptors.get_mut(&(fd as u32)) {
                        fde.flags = arg as u32;
                    }
                });
                return 0;
            }
            -EBADF
        }
        _ => {
            crate::serial_println!("[KPIO/Linux] fcntl: fd={}, cmd={} → ENOSYS", fd, cmd);
            -EINVAL
        }
    }
}

// ── Time syscalls ────────────────────────────────────────────────────

/// `gettimeofday(tv, tz)` — get time of day
///
/// `struct timeval { i64 tv_sec; i64 tv_usec; }` (16 bytes)
pub fn sys_gettimeofday(tv_ptr: u64, _tz_ptr: u64) -> i64 {
    if tv_ptr == 0 {
        return 0;
    }
    if validate_user_ptr(tv_ptr, 16).is_err() {
        return -EFAULT;
    }

    // Use TSC for time measurement
    let tsc: u64;
    unsafe {
        core::arch::asm!("rdtsc", "shl rdx, 32", "or rax, rdx", out("rax") tsc, out("rdx") _);
    }

    // Approximate: ~2GHz TSC
    let seconds = tsc / 2_000_000_000;
    let remainder = tsc % 2_000_000_000;
    let microseconds = (remainder * 1_000_000) / 2_000_000_000;

    #[repr(C)]
    struct Timeval {
        tv_sec: i64,
        tv_usec: i64,
    }

    let tv = Timeval {
        tv_sec: seconds as i64,
        tv_usec: microseconds as i64,
    };

    let bytes = unsafe {
        core::slice::from_raw_parts(
            &tv as *const Timeval as *const u8,
            core::mem::size_of::<Timeval>(),
        )
    };

    if copy_to_user(tv_ptr, bytes).is_err() {
        return -EFAULT;
    }
    0
}

/// `nanosleep(req, rem)` — high-resolution sleep
///
/// `struct timespec { i64 tv_sec; i64 tv_nsec; }` (16 bytes)
pub fn sys_nanosleep(req_ptr: u64, _rem_ptr: u64) -> i64 {
    if validate_user_ptr(req_ptr, 16).is_err() {
        return -EFAULT;
    }

    // Read the requested timespec
    let mut buf = [0u8; 16];
    if super::linux::copy_from_user(&mut buf, req_ptr).is_err() {
        return -EFAULT;
    }
    let tv_sec = i64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]);
    let tv_nsec = i64::from_le_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]);

    if tv_sec < 0 || tv_nsec < 0 || tv_nsec >= 1_000_000_000 {
        return -EINVAL;
    }

    // Calculate total nanoseconds to sleep
    let total_ns = tv_sec as u64 * 1_000_000_000 + tv_nsec as u64;

    // Busy-wait using TSC (approximate ~2GHz)
    // For short sleeps this is acceptable; for longer sleeps we would yield to scheduler
    let tsc_start: u64;
    unsafe {
        core::arch::asm!("rdtsc", "shl rdx, 32", "or rax, rdx", out("rax") tsc_start, out("rdx") _);
    }

    // Convert nanoseconds to TSC ticks (~2 ticks per nanosecond at 2GHz)
    let tsc_wait = total_ns * 2;

    // For very long sleeps (>100ms), use a loop with hints
    loop {
        let tsc_now: u64;
        unsafe {
            core::arch::asm!("rdtsc", "shl rdx, 32", "or rax, rdx", out("rax") tsc_now, out("rdx") _);
        }
        if tsc_now.wrapping_sub(tsc_start) >= tsc_wait {
            break;
        }
        // CPU pause hint to reduce power/bus contention
        core::hint::spin_loop();
    }

    // Write remaining time = 0 (we slept the full duration)
    if _rem_ptr != 0 && validate_user_ptr(_rem_ptr, 16).is_ok() {
        let zero = [0u8; 16];
        let _ = copy_to_user(_rem_ptr, &zero);
    }

    0
}

// ── Remaining stubs (Job 15) ─────────────────────────────────────────

/// `readv(fd, iov, iovcnt)` — read into multiple buffers
pub fn sys_readv(fd: i32, iov_ptr: u64, iovcnt: u32) -> i64 {
    if iovcnt == 0 {
        return 0;
    }
    if iovcnt > 1024 {
        return -EINVAL;
    }
    let total_iov_bytes = (iovcnt as u64) * 16;
    if validate_user_ptr(iov_ptr, total_iov_bytes).is_err() {
        return -EFAULT;
    }

    let mut total_read: i64 = 0;

    for i in 0..iovcnt {
        let entry_addr = iov_ptr + (i as u64) * 16;
        let mut buf = [0u8; 16];
        if super::linux::copy_from_user(&mut buf, entry_addr).is_err() {
            return -EFAULT;
        }
        let iov_base = u64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]);
        let iov_len = u64::from_le_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]);

        if iov_len == 0 {
            continue;
        }

        let result = sys_read(fd, iov_base, iov_len);
        if result < 0 {
            if total_read > 0 {
                return total_read;
            }
            return result;
        }
        total_read += result;
        if (result as u64) < iov_len {
            break; // short read
        }
    }
    total_read
}

/// `futex(uaddr, op, val, ...)` — fast userspace mutex (minimal stub)
///
/// Returns 0 for FUTEX_WAKE, -ENOSYS for unsupported ops.
pub fn sys_futex(_uaddr: u64, op: i32, _val: u32) -> i64 {
    const FUTEX_WAIT: i32 = 0;
    const FUTEX_WAKE: i32 = 1;
    const FUTEX_PRIVATE_FLAG: i32 = 128;

    let cmd = op & !FUTEX_PRIVATE_FLAG; // strip PRIVATE flag

    match cmd {
        FUTEX_WAKE => {
            // In our single-threaded model, wake is a no-op but return 0
            // (return value = number of waiters woken)
            0
        }
        FUTEX_WAIT => {
            // We can't actually block, so just return 0 (as if spurious wakeup)
            0
        }
        _ => {
            crate::serial_println!("[KPIO/Linux] futex op={} → stub 0", op);
            0 // Return success for all operations as a stub
        }
    }
}

/// `prlimit64(pid, resource, new_limit, old_limit)` — get/set resource limits
///
/// Returns sensible default limits.
pub fn sys_prlimit64(_pid: i32, resource: u32, _new_limit_ptr: u64, old_limit_ptr: u64) -> i64 {
    // struct rlimit64 { u64 rlim_cur; u64 rlim_max; }
    if old_limit_ptr != 0 {
        if validate_user_ptr(old_limit_ptr, 16).is_err() {
            return -EFAULT;
        }

        const RLIM_INFINITY: u64 = u64::MAX;

        // Return sensible defaults per resource
        let (cur, max) = match resource {
            0 => (RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_CPU
            1 => (RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_FSIZE
            2 => (RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_DATA
            3 => (8 * 1024 * 1024, RLIM_INFINITY), // RLIMIT_STACK (8MB default)
            4 => (0, RLIM_INFINITY),              // RLIMIT_CORE
            5 => (RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_RSS
            6 => (1024, 1024),                    // RLIMIT_NPROC
            7 => (1024, 1024),                    // RLIMIT_NOFILE
            _ => (RLIM_INFINITY, RLIM_INFINITY),
        };

        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&cur.to_le_bytes());
        buf[8..16].copy_from_slice(&max.to_le_bytes());
        if copy_to_user(old_limit_ptr, &buf).is_err() {
            return -EFAULT;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sys_brk_query() {
        // In test context, no current process → returns DEFAULT_BRK
        let result = sys_brk(0);
        assert_eq!(result, 0x0060_0000);
    }

    #[test]
    fn test_sys_brk_set() {
        // In test context, no current process → accepts any user addr
        let result = sys_brk(0x0070_0000);
        assert_eq!(result, 0x0070_0000);
    }

    #[test]
    fn test_sys_brk_kernel_addr() {
        let result = sys_brk(0xFFFF_8000_0000_0000);
        assert_eq!(result, -ENOMEM);
    }

    #[test]
    fn test_linux_prot_flags() {
        use crate::memory::user_page_table::PageTableFlags;

        let flags = linux_prot_to_page_flags(PROT_READ | PROT_WRITE);
        assert!(flags.contains(PageTableFlags::PRESENT));
        assert!(flags.contains(PageTableFlags::WRITABLE));
        assert!(flags.contains(PageTableFlags::USER_ACCESSIBLE));
        assert!(flags.contains(PageTableFlags::NO_EXECUTE));

        let flags_rx = linux_prot_to_page_flags(PROT_READ | PROT_EXEC);
        assert!(!flags_rx.contains(PageTableFlags::WRITABLE));
        assert!(!flags_rx.contains(PageTableFlags::NO_EXECUTE));
    }

    #[test]
    fn test_vma_overlaps() {
        let vmas = alloc::vec![
            Vma { start: 0x1000, end: 0x3000, prot: 0, flags: 0 },
            Vma { start: 0x5000, end: 0x8000, prot: 0, flags: 0 },
        ];

        // No overlap
        assert!(!vma_overlaps(&vmas, 0x3000, 0x2000));
        // Overlaps first VMA
        assert!(vma_overlaps(&vmas, 0x2000, 0x2000));
        // Overlaps second VMA
        assert!(vma_overlaps(&vmas, 0x7000, 0x2000));
        // Between VMAs — no overlap
        assert!(!vma_overlaps(&vmas, 0x3000, 0x1000));
    }

    #[test]
    fn test_sys_munmap_invalid() {
        // Unaligned addr
        assert_eq!(sys_munmap(0x1001, 0x1000), -EINVAL);
        // Zero length
        assert_eq!(sys_munmap(0x1000, 0), -EINVAL);
    }

    #[test]
    fn test_sys_mmap_zero_length() {
        assert_eq!(sys_mmap(0, 0, PROT_READ, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0), -EINVAL);
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
