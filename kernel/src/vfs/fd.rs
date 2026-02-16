//! File Descriptor Table
//!
//! Maps integer file descriptors to open files (inode + offset).
//! A single global FdTable suffices for the current single-address-space kernel.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::VfsError;
use crate::terminal::fs;

/// Maximum number of open file descriptors.
const MAX_FDS: usize = 256;

/// Fd 0/1/2 are reserved for stdin/stdout/stderr.
const FIRST_USER_FD: i32 = 3;

/// An open file descriptor entry.
#[derive(Debug, Clone)]
pub struct FdEntry {
    /// Absolute path that was opened.
    pub path: String,
    /// Inode number in terminal::fs.
    pub ino: u64,
    /// Current read/write cursor offset.
    pub offset: usize,
    /// Open flags.
    pub flags: u32,
    /// Whether this fd is a special (stdio) fd.
    pub special: Option<SpecialFd>,
}

/// Special file descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialFd {
    Stdin,
    Stdout,
    Stderr,
}

/// Global file descriptor table.
pub struct FdTable {
    entries: BTreeMap<i32, FdEntry>,
    next_fd: i32,
}

static FD_TABLE: Mutex<Option<FdTable>> = Mutex::new(None);

/// Initialise the global fd table with stdio entries.
pub fn init() {
    let mut table = FdTable {
        entries: BTreeMap::new(),
        next_fd: FIRST_USER_FD,
    };

    // 0 = stdin
    table.entries.insert(
        0,
        FdEntry {
            path: String::from("/dev/stdin"),
            ino: 0,
            offset: 0,
            flags: 0,
            special: Some(SpecialFd::Stdin),
        },
    );
    // 1 = stdout
    table.entries.insert(
        1,
        FdEntry {
            path: String::from("/dev/stdout"),
            ino: 0,
            offset: 0,
            flags: 1,
            special: Some(SpecialFd::Stdout),
        },
    );
    // 2 = stderr
    table.entries.insert(
        2,
        FdEntry {
            path: String::from("/dev/stderr"),
            ino: 0,
            offset: 0,
            flags: 1,
            special: Some(SpecialFd::Stderr),
        },
    );

    *FD_TABLE.lock() = Some(table);
}

fn with_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut FdTable) -> R,
{
    let mut guard = FD_TABLE.lock();
    let table = guard
        .as_mut()
        .expect("FdTable not initialised — call vfs::fd::init()");
    f(table)
}

// ── Public API ──────────────────────────────────────────────

/// Open a file and return an fd.
pub fn open(path: &str, flags: u32) -> Result<i32, VfsError> {
    let ino = fs::with_fs(|f| f.resolve(path));

    match ino {
        Some(ino) => {
            let fd = with_table(|t| {
                if t.entries.len() >= MAX_FDS {
                    return Err(VfsError::NoSpace);
                }
                let fd = t.next_fd;
                t.next_fd += 1;
                t.entries.insert(
                    fd,
                    FdEntry {
                        path: String::from(path),
                        ino,
                        offset: 0,
                        flags,
                        special: None,
                    },
                );
                Ok(fd)
            })?;
            Ok(fd)
        }
        None => {
            // O_CREAT: create file
            if flags & 0o100 != 0 {
                let (parent_path, name) = super::split_path(path);
                let parent_ino =
                    fs::with_fs(|f| f.resolve(parent_path)).ok_or(VfsError::NotFound)?;
                let new_ino = fs::with_fs(|f| f.create_file(parent_ino, name, b""))
                    .map_err(|_| VfsError::IoError)?;
                let fd = with_table(|t| {
                    let fd = t.next_fd;
                    t.next_fd += 1;
                    t.entries.insert(
                        fd,
                        FdEntry {
                            path: String::from(path),
                            ino: new_ino,
                            offset: 0,
                            flags,
                            special: None,
                        },
                    );
                    fd
                });
                Ok(fd)
            } else {
                Err(VfsError::NotFound)
            }
        }
    }
}

/// Read up to `len` bytes from an fd. Returns data read.
pub fn read(fd: i32, len: usize) -> Result<Vec<u8>, VfsError> {
    // Stdio: stdin returns empty (no interactive input via syscall)
    let entry = with_table(|t| t.entries.get(&fd).cloned().ok_or(VfsError::InvalidFd))?;

    if entry.special == Some(SpecialFd::Stdin) {
        return Ok(Vec::new());
    }
    if entry.special.is_some() {
        return Err(VfsError::PermissionDenied);
    }

    let data = fs::with_fs(|f| f.read_file(entry.ino)).map_err(|_| VfsError::IoError)?;

    let start = entry.offset.min(data.len());
    let end = (start + len).min(data.len());
    let chunk = data[start..end].to_vec();

    // Advance offset
    let read_count = chunk.len();
    with_table(|t| {
        if let Some(e) = t.entries.get_mut(&fd) {
            e.offset += read_count;
        }
    });

    Ok(chunk)
}

/// Write bytes to an fd. Returns count written.
pub fn write(fd: i32, data: &[u8]) -> Result<usize, VfsError> {
    let entry = with_table(|t| t.entries.get(&fd).cloned().ok_or(VfsError::InvalidFd))?;

    // Stdout / stderr
    if entry.special == Some(SpecialFd::Stdout) || entry.special == Some(SpecialFd::Stderr) {
        if let Ok(s) = core::str::from_utf8(data) {
            crate::serial::write_str(s);
        }
        return Ok(data.len());
    }

    if entry.special.is_some() {
        return Err(VfsError::PermissionDenied);
    }

    // Append or overwrite
    let append = entry.flags & 0o2000 != 0;
    if append {
        fs::with_fs(|f| f.append_file(entry.ino, data)).map_err(|_| VfsError::IoError)?;
    } else {
        fs::with_fs(|f| f.write_file(entry.ino, data)).map_err(|_| VfsError::IoError)?;
    }

    with_table(|t| {
        if let Some(e) = t.entries.get_mut(&fd) {
            e.offset += data.len();
        }
    });

    Ok(data.len())
}

/// Close an fd.
pub fn close(fd: i32) -> Result<(), VfsError> {
    with_table(|t| {
        if t.entries.remove(&fd).is_some() {
            Ok(())
        } else {
            Err(VfsError::InvalidFd)
        }
    })
}

/// Seek an fd to a new offset. Returns new offset.
pub fn lseek(fd: i32, offset: i64, whence: u32) -> Result<u64, VfsError> {
    with_table(|t| {
        let entry = t.entries.get_mut(&fd).ok_or(VfsError::InvalidFd)?;
        let new_offset = match whence {
            0 => offset as usize,                         // SEEK_SET
            1 => (entry.offset as i64 + offset) as usize, // SEEK_CUR
            2 => {
                // SEEK_END — need file size
                let size = fs::with_fs(|f| f.get(entry.ino).map(|i| i.size as usize).unwrap_or(0));
                (size as i64 + offset) as usize
            }
            _ => return Err(VfsError::IoError),
        };
        entry.offset = new_offset;
        Ok(new_offset as u64)
    })
}

/// Get number of open file descriptors.
pub fn open_count() -> usize {
    with_table(|t| t.entries.len())
}
