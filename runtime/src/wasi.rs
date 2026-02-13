//! WASI Preview 1 implementation.
//!
//! This module provides a complete WASI (WebAssembly System Interface) Preview 1
//! implementation with an integrated in-memory virtual filesystem (VFS).
//! All file I/O operations are sandboxed to preopened directories.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::RuntimeError;

// ─── Virtual File System ───────────────────────────────────────────

/// Simple in-memory virtual file system.
///
/// Files are stored as flat path → data mappings. Directories are tracked
/// as a set of known directory paths. This approach is simple and sufficient
/// for a kernel OS VFS.
#[derive(Debug, Clone)]
pub struct Vfs {
    /// File contents keyed by normalized absolute path.
    files: BTreeMap<String, Vec<u8>>,
    /// Known directory paths.
    dirs: BTreeMap<String, ()>,
}

impl Vfs {
    /// Create a new empty VFS with root directory.
    pub fn new() -> Self {
        let mut dirs = BTreeMap::new();
        dirs.insert(String::from("/"), ());
        Vfs {
            files: BTreeMap::new(),
            dirs,
        }
    }

    /// Normalize a path (resolve `.` and `..`, collapse slashes).
    pub fn normalize(path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for component in path.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                other => parts.push(other),
            }
        }
        if parts.is_empty() {
            String::from("/")
        } else {
            let mut result = String::new();
            for part in &parts {
                result.push('/');
                result.push_str(part);
            }
            result
        }
    }

    /// Check if a path is within a given root (sandbox check).
    pub fn is_within(path: &str, root: &str) -> bool {
        let norm_path = Self::normalize(path);
        let norm_root = Self::normalize(root);
        if norm_root == "/" {
            return true;
        }
        norm_path == norm_root
            || norm_path.starts_with(&alloc::format!("{}/", norm_root))
    }

    /// Create a file with given contents. Parent directory must exist.
    pub fn create_file(&mut self, path: &str, data: Vec<u8>) -> Result<(), WasiError> {
        let path = Self::normalize(path);
        if let Some(parent) = Self::parent_dir(&path) {
            if !self.dirs.contains_key(&parent) {
                return Err(WasiError::NoEnt);
            }
        }
        self.files.insert(path, data);
        Ok(())
    }

    /// Read file contents.
    pub fn read_file(&self, path: &str) -> Result<&[u8], WasiError> {
        let path = Self::normalize(path);
        self.files.get(&path).map(|v| v.as_slice()).ok_or(WasiError::NoEnt)
    }

    /// Write to a file at a specific offset. Extends file if needed.
    pub fn write_file(&mut self, path: &str, offset: usize, data: &[u8]) -> Result<(), WasiError> {
        let path = Self::normalize(path);
        let file = self.files.get_mut(&path).ok_or(WasiError::NoEnt)?;
        let end = offset + data.len();
        if end > file.len() {
            file.resize(end, 0);
        }
        file[offset..end].copy_from_slice(data);
        Ok(())
    }

    /// Get file size.
    pub fn file_size(&self, path: &str) -> Result<u64, WasiError> {
        let path = Self::normalize(path);
        if let Some(f) = self.files.get(&path) {
            Ok(f.len() as u64)
        } else if self.dirs.contains_key(&path) {
            Ok(0)
        } else {
            Err(WasiError::NoEnt)
        }
    }

    /// Check if a path exists (file or directory).
    pub fn exists(&self, path: &str) -> bool {
        let path = Self::normalize(path);
        self.files.contains_key(&path) || self.dirs.contains_key(&path)
    }

    /// Check if a path is a directory.
    pub fn is_dir(&self, path: &str) -> bool {
        let path = Self::normalize(path);
        self.dirs.contains_key(&path)
    }

    /// Check if a path is a file.
    pub fn is_file(&self, path: &str) -> bool {
        let path = Self::normalize(path);
        self.files.contains_key(&path)
    }

    /// Create a directory. Parent must exist.
    pub fn create_dir(&mut self, path: &str) -> Result<(), WasiError> {
        let path = Self::normalize(path);
        if self.dirs.contains_key(&path) || self.files.contains_key(&path) {
            return Err(WasiError::Exist);
        }
        if let Some(parent) = Self::parent_dir(&path) {
            if !self.dirs.contains_key(&parent) {
                return Err(WasiError::NoEnt);
            }
        }
        self.dirs.insert(path, ());
        Ok(())
    }

    /// Remove an empty directory.
    pub fn remove_dir(&mut self, path: &str) -> Result<(), WasiError> {
        let path = Self::normalize(path);
        if !self.dirs.contains_key(&path) {
            return Err(WasiError::NoEnt);
        }
        if path == "/" {
            return Err(WasiError::Access);
        }
        let prefix = alloc::format!("{}/", path);
        for key in self.files.keys() {
            if key.starts_with(&prefix) {
                return Err(WasiError::NotEmpty);
            }
        }
        for key in self.dirs.keys() {
            if key != &path && key.starts_with(&prefix) {
                return Err(WasiError::NotEmpty);
            }
        }
        self.dirs.remove(&path);
        Ok(())
    }

    /// Remove a file.
    pub fn remove_file(&mut self, path: &str) -> Result<(), WasiError> {
        let path = Self::normalize(path);
        self.files.remove(&path).ok_or(WasiError::NoEnt)?;
        Ok(())
    }

    /// Rename a file or directory.
    pub fn rename(&mut self, old_path: &str, new_path: &str) -> Result<(), WasiError> {
        let old = Self::normalize(old_path);
        let new = Self::normalize(new_path);

        if let Some(data) = self.files.remove(&old) {
            self.files.insert(new, data);
            Ok(())
        } else if self.dirs.contains_key(&old) {
            let old_prefix = alloc::format!("{}/", old);

            let files_to_move: Vec<(String, Vec<u8>)> = self
                .files
                .iter()
                .filter(|(k, _)| k.starts_with(&old_prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            let dirs_to_move: Vec<String> = self
                .dirs
                .keys()
                .filter(|k| k.starts_with(&old_prefix) || **k == old)
                .cloned()
                .collect();

            for (f, _) in &files_to_move {
                self.files.remove(f);
            }
            for d in &dirs_to_move {
                self.dirs.remove(d);
            }

            let new_prefix = alloc::format!("{}/", new);
            self.dirs.insert(new.clone(), ());

            for (f, data) in files_to_move {
                let relative = &f[old_prefix.len()..];
                let new_f = alloc::format!("{}{}", new_prefix, relative);
                self.files.insert(new_f, data);
            }

            for d in dirs_to_move {
                if d == old {
                    continue;
                }
                let relative = &d[old_prefix.len()..];
                let new_d = alloc::format!("{}{}", new_prefix, relative);
                self.dirs.insert(new_d, ());
            }

            Ok(())
        } else {
            Err(WasiError::NoEnt)
        }
    }

    /// List directory entries (., .., and direct children).
    pub fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, WasiError> {
        let path = Self::normalize(path);
        if !self.dirs.contains_key(&path) {
            return Err(WasiError::NoEnt);
        }

        let prefix = if path == "/" {
            String::from("/")
        } else {
            alloc::format!("{}/", path)
        };
        let mut entries = Vec::new();
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();
        let mut ino: u64 = 1;

        // . and ..
        entries.push(DirEntry {
            d_ino: 0,
            d_next: 1,
            d_namlen: 1,
            d_type: FdType::Directory,
            name: String::from("."),
        });
        entries.push(DirEntry {
            d_ino: 0,
            d_next: 2,
            d_namlen: 2,
            d_type: FdType::Directory,
            name: String::from(".."),
        });

        // Direct children (files)
        for key in self.files.keys() {
            if let Some(rest) = key.strip_prefix(prefix.as_str()) {
                if !rest.contains('/') && !rest.is_empty() {
                    let name = String::from(rest);
                    if !seen.contains_key(&name) {
                        seen.insert(name.clone(), ());
                        entries.push(DirEntry {
                            d_ino: ino,
                            d_next: entries.len() as u64 + 1,
                            d_namlen: name.len() as u32,
                            d_type: FdType::RegularFile,
                            name,
                        });
                        ino += 1;
                    }
                }
            }
        }

        // Direct children (directories)
        for key in self.dirs.keys() {
            if key == &path {
                continue;
            }
            if let Some(rest) = key.strip_prefix(prefix.as_str()) {
                if !rest.contains('/') && !rest.is_empty() {
                    let name = String::from(rest);
                    if !seen.contains_key(&name) {
                        seen.insert(name.clone(), ());
                        entries.push(DirEntry {
                            d_ino: ino,
                            d_next: entries.len() as u64 + 1,
                            d_namlen: name.len() as u32,
                            d_type: FdType::Directory,
                            name,
                        });
                        ino += 1;
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Get stat information for a path.
    pub fn stat(&self, path: &str) -> Result<FileStat, WasiError> {
        let path = Self::normalize(path);
        if let Some(data) = self.files.get(&path) {
            Ok(FileStat {
                dev: 0,
                ino: 0,
                filetype: FdType::RegularFile as u8,
                nlink: 1,
                size: data.len() as u64,
                atim: 0,
                mtim: 0,
                ctim: 0,
            })
        } else if self.dirs.contains_key(&path) {
            Ok(FileStat {
                dev: 0,
                ino: 0,
                filetype: FdType::Directory as u8,
                nlink: 1,
                size: 0,
                atim: 0,
                mtim: 0,
                ctim: 0,
            })
        } else {
            Err(WasiError::NoEnt)
        }
    }

    /// Truncate a file to zero length.
    pub fn truncate(&mut self, path: &str) -> Result<(), WasiError> {
        let path = Self::normalize(path);
        let file = self.files.get_mut(&path).ok_or(WasiError::NoEnt)?;
        file.clear();
        Ok(())
    }

    /// Get parent directory path.
    fn parent_dir(path: &str) -> Option<String> {
        if path == "/" {
            return None;
        }
        match path.rfind('/') {
            Some(0) => Some(String::from("/")),
            Some(idx) => Some(String::from(&path[..idx])),
            None => None,
        }
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new()
    }
}

// ─── WASI Context ──────────────────────────────────────────────────

/// WASI context for a running WASM instance.
///
/// Manages the virtual filesystem, file descriptor table, process
/// arguments, environment variables, clock, and random state.
pub struct WasiCtx {
    /// In-memory virtual filesystem.
    pub vfs: Vfs,
    /// File descriptor table.
    fds: BTreeMap<u32, FileDescriptor>,
    /// Next file descriptor number to allocate.
    next_fd: u32,
    /// Command line arguments.
    args: Vec<String>,
    /// Environment variables.
    env: BTreeMap<String, String>,
    /// Preopened directories: (fd_number, vfs_path).
    preopened_dirs: Vec<(u32, String)>,
    /// stdout capture buffer.
    stdout_buf: Vec<u8>,
    /// stderr capture buffer.
    stderr_buf: Vec<u8>,
    /// Exit code (if proc_exit was called).
    exit_code: Option<u32>,
    /// Monotonic counter (nanoseconds, increments per call).
    monotonic_counter: u64,
    /// PRNG state for random_get.
    random_state: u64,
}

impl WasiCtx {
    /// Create a new WASI context with standard FDs.
    pub fn new() -> Self {
        let mut fds = BTreeMap::new();
        fds.insert(0, FileDescriptor::stdin());
        fds.insert(1, FileDescriptor::stdout());
        fds.insert(2, FileDescriptor::stderr());

        WasiCtx {
            vfs: Vfs::new(),
            fds,
            next_fd: 3,
            args: Vec::new(),
            env: BTreeMap::new(),
            preopened_dirs: Vec::new(),
            stdout_buf: Vec::new(),
            stderr_buf: Vec::new(),
            exit_code: None,
            monotonic_counter: 1_000_000_000, // Start at 1 second in nanoseconds
            random_state: 0xDEAD_BEEF_CAFE_BABE,
        }
    }

    /// Set command line arguments.
    pub fn set_args(&mut self, args: Vec<String>) -> &mut Self {
        self.args = args;
        self
    }

    /// Set an environment variable.
    pub fn set_env(&mut self, key: &str, value: &str) -> &mut Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Get the stdout capture buffer.
    pub fn stdout_buf(&self) -> &[u8] {
        &self.stdout_buf
    }

    /// Get the stderr capture buffer.
    pub fn stderr_buf(&self) -> &[u8] {
        &self.stderr_buf
    }

    /// Get exit code if process has exited.
    pub fn exit_code(&self) -> Option<u32> {
        self.exit_code
    }

    /// Preopen a directory. Creates the directory in VFS if needed.
    /// Returns the assigned FD number.
    pub fn preopen_dir(&mut self, guest_path: &str) -> u32 {
        let vfs_path = Vfs::normalize(guest_path);

        // Ensure the directory exists in VFS
        if !self.vfs.is_dir(&vfs_path) {
            let _ = self.vfs.create_dir(&vfs_path);
        }

        let fd = FileDescriptor {
            fd_type: FdType::Directory,
            rights: FdRights::all(),
            flags: FdFlags::empty(),
            offset: 0,
            path: Some(vfs_path.clone()),
            preopen_guest_path: Some(String::from(guest_path)),
        };

        let fd_num = self.alloc_fd(fd);
        self.preopened_dirs.push((fd_num, vfs_path));
        fd_num
    }

    /// Allocate a new file descriptor.
    fn alloc_fd(&mut self, fd: FileDescriptor) -> u32 {
        let num = self.next_fd;
        self.next_fd += 1;
        self.fds.insert(num, fd);
        num
    }

    // ─── WASI System Calls ─────────────────────────────────────────

    /// fd_read - Read from a file descriptor.
    pub fn fd_read(&mut self, fd: u32, buf: &mut [u8]) -> Result<usize, WasiError> {
        // Validate FD and permissions
        {
            let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
            if !file.rights.contains(FdRights::READ) {
                return Err(WasiError::Access);
            }
            // stdin returns EOF
            if fd == 0 {
                return Ok(0);
            }
            if file.fd_type == FdType::Directory {
                return Err(WasiError::IsDir);
            }
        }

        // Get path and offset
        let (path, offset) = {
            let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
            (
                file.path.clone().ok_or(WasiError::Io)?,
                file.offset as usize,
            )
        };

        // Read from VFS
        let data = self.vfs.read_file(&path)?;
        let available = if offset < data.len() {
            &data[offset..]
        } else {
            &[]
        };
        let to_read = core::cmp::min(available.len(), buf.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);

        // Update offset
        if let Some(file) = self.fds.get_mut(&fd) {
            file.offset += to_read as u64;
        }

        Ok(to_read)
    }

    /// fd_write - Write to a file descriptor.
    pub fn fd_write(&mut self, fd: u32, data: &[u8]) -> Result<usize, WasiError> {
        // Validate FD and permissions
        {
            let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
            if !file.rights.contains(FdRights::WRITE) {
                return Err(WasiError::Access);
            }
        }

        // stdout → capture buffer
        if fd == 1 {
            self.stdout_buf.extend_from_slice(data);
            return Ok(data.len());
        }
        // stderr → capture buffer
        if fd == 2 {
            self.stderr_buf.extend_from_slice(data);
            return Ok(data.len());
        }

        // Get path and offset for regular files
        let (path, offset) = {
            let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
            if file.fd_type == FdType::Directory {
                return Err(WasiError::IsDir);
            }
            (
                file.path.clone().ok_or(WasiError::Io)?,
                file.offset as usize,
            )
        };

        // Write to VFS
        self.vfs.write_file(&path, offset, data)?;

        // Update offset
        if let Some(file) = self.fds.get_mut(&fd) {
            file.offset += data.len() as u64;
        }

        Ok(data.len())
    }

    /// fd_seek - Reposition read/write offset.
    pub fn fd_seek(&mut self, fd: u32, offset: i64, whence: Whence) -> Result<u64, WasiError> {
        let (path, current_offset) = {
            let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
            if !file.rights.contains(FdRights::SEEK) {
                return Err(WasiError::Access);
            }
            (file.path.clone(), file.offset)
        };

        let file_size = if let Some(ref p) = path {
            self.vfs.file_size(p).unwrap_or(0)
        } else {
            0
        };

        let new_offset = match whence {
            Whence::Set => {
                if offset < 0 {
                    return Err(WasiError::Inval);
                }
                offset as u64
            }
            Whence::Cur => {
                let result = current_offset as i64 + offset;
                if result < 0 {
                    return Err(WasiError::Inval);
                }
                result as u64
            }
            Whence::End => {
                let result = file_size as i64 + offset;
                if result < 0 {
                    return Err(WasiError::Inval);
                }
                result as u64
            }
        };

        if let Some(file) = self.fds.get_mut(&fd) {
            file.offset = new_offset;
        }

        Ok(new_offset)
    }

    /// fd_tell - Get current offset.
    pub fn fd_tell(&self, fd: u32) -> Result<u64, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        if !file.rights.contains(FdRights::TELL) {
            return Err(WasiError::Access);
        }
        Ok(file.offset)
    }

    /// fd_close - Close a file descriptor.
    pub fn fd_close(&mut self, fd: u32) -> Result<(), WasiError> {
        if fd < 3 {
            return Err(WasiError::Access);
        }
        self.fds.remove(&fd).ok_or(WasiError::BadF)?;
        Ok(())
    }

    /// fd_fdstat_get - Get file descriptor status.
    pub fn fd_fdstat_get(&self, fd: u32) -> Result<FdStat, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        Ok(FdStat {
            fs_filetype: file.fd_type,
            fs_flags: file.flags,
            fs_rights_base: file.rights,
            fs_rights_inheriting: file.rights,
        })
    }

    /// fd_prestat_get - Get preopened directory info.
    pub fn fd_prestat_get(&self, fd: u32) -> Result<Prestat, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        let guest_path = file.preopen_guest_path.as_ref().ok_or(WasiError::BadF)?;
        Ok(Prestat {
            tag: PrestatTag::Dir,
            inner: PrestatInner {
                dir_name_len: guest_path.len(),
            },
        })
    }

    /// fd_prestat_dir_name - Get preopened directory name.
    pub fn fd_prestat_dir_name(&self, fd: u32, buf: &mut [u8]) -> Result<(), WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        let guest_path = file.preopen_guest_path.as_ref().ok_or(WasiError::BadF)?;
        let name_bytes = guest_path.as_bytes();
        if buf.len() < name_bytes.len() {
            return Err(WasiError::Overflow);
        }
        buf[..name_bytes.len()].copy_from_slice(name_bytes);
        Ok(())
    }

    /// path_open - Open a file or directory relative to a directory FD.
    pub fn path_open(
        &mut self,
        dir_fd: u32,
        _dirflags: LookupFlags,
        path: &str,
        oflags: OFlags,
        rights: FdRights,
        _inheriting_rights: FdRights,
        fdflags: FdFlags,
    ) -> Result<u32, WasiError> {
        // Resolve the full path and do sandbox check
        let full_path = self.resolve_sandboxed_path(dir_fd, path)?;

        let is_create = oflags.contains(OFlags::CREAT);
        let is_excl = oflags.contains(OFlags::EXCL);
        let is_trunc = oflags.contains(OFlags::TRUNC);
        let is_dir = oflags.contains(OFlags::DIRECTORY);

        if is_dir {
            if !self.vfs.is_dir(&full_path) {
                return Err(WasiError::NotDir);
            }
            let new_fd = FileDescriptor {
                fd_type: FdType::Directory,
                rights,
                flags: fdflags,
                offset: 0,
                path: Some(full_path),
                preopen_guest_path: None,
            };
            return Ok(self.alloc_fd(new_fd));
        }

        let file_exists = self.vfs.is_file(&full_path);

        if is_excl && file_exists {
            return Err(WasiError::Exist);
        }

        if is_create && !file_exists {
            self.vfs.create_file(&full_path, Vec::new())?;
        } else if !file_exists {
            return Err(WasiError::NoEnt);
        }

        if is_trunc && self.vfs.is_file(&full_path) {
            self.vfs.truncate(&full_path)?;
        }

        let new_fd = FileDescriptor {
            fd_type: FdType::RegularFile,
            rights,
            flags: fdflags,
            offset: 0,
            path: Some(full_path),
            preopen_guest_path: None,
        };
        Ok(self.alloc_fd(new_fd))
    }

    /// path_create_directory - Create a directory.
    pub fn path_create_directory(&mut self, dir_fd: u32, path: &str) -> Result<(), WasiError> {
        let full_path = self.resolve_sandboxed_path(dir_fd, path)?;

        {
            let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
            if !dir.rights.contains(FdRights::PATH_CREATE_DIR) {
                return Err(WasiError::Access);
            }
        }

        self.vfs.create_dir(&full_path)
    }

    /// path_remove_directory - Remove an empty directory.
    pub fn path_remove_directory(&mut self, dir_fd: u32, path: &str) -> Result<(), WasiError> {
        let full_path = self.resolve_sandboxed_path(dir_fd, path)?;

        {
            let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
            if !dir.rights.contains(FdRights::PATH_REMOVE) {
                return Err(WasiError::Access);
            }
        }

        self.vfs.remove_dir(&full_path)
    }

    /// path_unlink_file - Remove a file.
    pub fn path_unlink_file(&mut self, dir_fd: u32, path: &str) -> Result<(), WasiError> {
        let full_path = self.resolve_sandboxed_path(dir_fd, path)?;

        {
            let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
            if !dir.rights.contains(FdRights::PATH_REMOVE) {
                return Err(WasiError::Access);
            }
        }

        self.vfs.remove_file(&full_path)
    }

    /// path_rename - Rename a file or directory.
    pub fn path_rename(
        &mut self,
        old_dir_fd: u32,
        old_path: &str,
        new_dir_fd: u32,
        new_path: &str,
    ) -> Result<(), WasiError> {
        let old_full = self.resolve_sandboxed_path(old_dir_fd, old_path)?;
        let new_full = self.resolve_sandboxed_path(new_dir_fd, new_path)?;

        {
            let old_dir = self.fds.get(&old_dir_fd).ok_or(WasiError::BadF)?;
            if !old_dir.rights.contains(FdRights::PATH_RENAME) {
                return Err(WasiError::Access);
            }
        }

        self.vfs.rename(&old_full, &new_full)
    }

    /// path_filestat_get - Get file stat for a path.
    pub fn path_filestat_get(
        &self,
        dir_fd: u32,
        _flags: LookupFlags,
        path: &str,
    ) -> Result<FileStat, WasiError> {
        let full_path = self.resolve_sandboxed_path(dir_fd, path)?;
        self.vfs.stat(&full_path)
    }

    /// fd_readdir - Read directory entries.
    pub fn fd_readdir(
        &self,
        fd: u32,
        buf: &mut [u8],
        cookie: u64,
    ) -> Result<usize, WasiError> {
        let file = self.fds.get(&fd).ok_or(WasiError::BadF)?;
        if file.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        if !file.rights.contains(FdRights::READ) {
            return Err(WasiError::Access);
        }

        let path = file.path.as_deref().ok_or(WasiError::Io)?;
        let entries = self.vfs.readdir(path)?;

        // Serialize entries into buffer starting from cookie
        let mut offset = 0usize;
        for (i, entry) in entries.iter().enumerate() {
            if (i as u64) < cookie {
                continue;
            }
            // WASI dirent: d_next(8) + d_ino(8) + d_namlen(4) + d_type(1) + pad(3) + name
            let entry_size = 24 + entry.name.len();
            if offset + entry_size > buf.len() {
                break;
            }

            let d_next = (i as u64) + 1;
            buf[offset..offset + 8].copy_from_slice(&d_next.to_le_bytes());
            buf[offset + 8..offset + 16].copy_from_slice(&entry.d_ino.to_le_bytes());
            buf[offset + 16..offset + 20]
                .copy_from_slice(&(entry.name.len() as u32).to_le_bytes());
            buf[offset + 20] = entry.d_type as u8;
            buf[offset + 21] = 0;
            buf[offset + 22] = 0;
            buf[offset + 23] = 0;
            buf[offset + 24..offset + 24 + entry.name.len()]
                .copy_from_slice(entry.name.as_bytes());

            offset += entry_size;
        }

        Ok(offset)
    }

    /// args_get - Get command line arguments.
    pub fn args_get(&self) -> &[String] {
        &self.args
    }

    /// args_sizes_get - Returns (arg_count, total_buf_size including NUL terminators).
    pub fn args_sizes_get(&self) -> (usize, usize) {
        let count = self.args.len();
        let total_size: usize = self.args.iter().map(|s| s.len() + 1).sum();
        (count, total_size)
    }

    /// environ_get - Get environment variables as key=value strings.
    pub fn environ_get(&self) -> Vec<String> {
        self.env
            .iter()
            .map(|(k, v)| alloc::format!("{}={}", k, v))
            .collect()
    }

    /// environ_sizes_get - Returns (env_count, total_buf_size including NUL).
    pub fn environ_sizes_get(&self) -> (usize, usize) {
        let count = self.env.len();
        let total_size: usize = self
            .env
            .iter()
            .map(|(k, v)| k.len() + 1 + v.len() + 1) // key=value\0
            .sum();
        (count, total_size)
    }

    /// clock_time_get - Get current time in nanoseconds.
    pub fn clock_time_get(
        &mut self,
        clock_id: ClockId,
        _precision: u64,
    ) -> Result<u64, WasiError> {
        match clock_id {
            ClockId::Realtime => {
                self.monotonic_counter += 1_000_000; // +1ms per call
                Ok(1_700_000_000_000_000_000 + self.monotonic_counter)
            }
            ClockId::Monotonic => {
                self.monotonic_counter += 1_000_000; // +1ms per call
                Ok(self.monotonic_counter)
            }
            ClockId::ProcessCputime | ClockId::ThreadCputime => {
                self.monotonic_counter += 100_000;
                Ok(self.monotonic_counter)
            }
        }
    }

    /// random_get - Fill buffer with pseudo-random bytes.
    pub fn random_get(&mut self, buf: &mut [u8]) -> Result<(), WasiError> {
        for byte in buf.iter_mut() {
            self.random_state ^= self.random_state << 13;
            self.random_state ^= self.random_state >> 7;
            self.random_state ^= self.random_state << 17;
            *byte = (self.random_state & 0xFF) as u8;
        }
        Ok(())
    }

    /// proc_exit - Terminate the process with an exit code.
    pub fn proc_exit(&mut self, code: u32) {
        self.exit_code = Some(code);
    }

    // ─── Internal Helpers ──────────────────────────────────────────

    /// Resolve a path relative to a directory FD and verify it stays
    /// within the sandbox (within the directory FD's scope).
    fn resolve_sandboxed_path(&self, dir_fd: u32, path: &str) -> Result<String, WasiError> {
        let dir = self.fds.get(&dir_fd).ok_or(WasiError::BadF)?;
        if dir.fd_type != FdType::Directory {
            return Err(WasiError::NotDir);
        }
        if !dir.rights.contains(FdRights::PATH_OPEN) {
            return Err(WasiError::Access);
        }

        let base = dir.path.as_deref().unwrap_or("/");
        let full = if path.starts_with('/') {
            Vfs::normalize(path)
        } else {
            Vfs::normalize(&alloc::format!("{}/{}", base, path))
        };

        // Sandbox check: resolved path must be within the dir_fd's directory
        if !Vfs::is_within(&full, base) {
            return Err(WasiError::Access);
        }

        Ok(full)
    }
}

impl Default for WasiCtx {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Supporting Types ──────────────────────────────────────────────

/// A file descriptor.
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    /// File descriptor type.
    pub fd_type: FdType,
    /// File descriptor rights.
    pub rights: FdRights,
    /// File descriptor flags.
    pub flags: FdFlags,
    /// Current read/write offset.
    pub offset: u64,
    /// VFS path (if applicable).
    pub path: Option<String>,
    /// Guest-visible preopened directory path (only for preopened dirs).
    pub preopen_guest_path: Option<String>,
}

impl FileDescriptor {
    /// Create stdin file descriptor.
    pub fn stdin() -> Self {
        FileDescriptor {
            fd_type: FdType::CharDevice,
            rights: FdRights::READ,
            flags: FdFlags::empty(),
            offset: 0,
            path: None,
            preopen_guest_path: None,
        }
    }

    /// Create stdout file descriptor.
    pub fn stdout() -> Self {
        FileDescriptor {
            fd_type: FdType::CharDevice,
            rights: FdRights::WRITE,
            flags: FdFlags::empty(),
            offset: 0,
            path: None,
            preopen_guest_path: None,
        }
    }

    /// Create stderr file descriptor.
    pub fn stderr() -> Self {
        FileDescriptor {
            fd_type: FdType::CharDevice,
            rights: FdRights::WRITE,
            flags: FdFlags::empty(),
            offset: 0,
            path: None,
            preopen_guest_path: None,
        }
    }

    /// Create a regular file descriptor.
    pub fn file(path: String, rights: FdRights) -> Self {
        FileDescriptor {
            fd_type: FdType::RegularFile,
            rights,
            flags: FdFlags::empty(),
            offset: 0,
            path: Some(path),
            preopen_guest_path: None,
        }
    }

    /// Create a directory descriptor.
    pub fn directory(path: String) -> Self {
        FileDescriptor {
            fd_type: FdType::Directory,
            rights: FdRights::READ | FdRights::PATH_OPEN,
            flags: FdFlags::empty(),
            offset: 0,
            path: Some(path),
            preopen_guest_path: None,
        }
    }
}

/// File descriptor types (WASI Preview 1 filetype values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FdType {
    /// Unknown.
    Unknown = 0,
    /// Block device.
    BlockDevice = 1,
    /// Character device.
    CharDevice = 2,
    /// Directory.
    Directory = 3,
    /// Regular file.
    RegularFile = 4,
    /// Socket (datagram).
    SocketDgram = 5,
    /// Socket (stream).
    SocketStream = 6,
}

bitflags::bitflags! {
    /// File descriptor rights.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdRights: u64 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const SEEK = 1 << 2;
        const SYNC = 1 << 3;
        const TELL = 1 << 4;
        const ADVISE = 1 << 5;
        const ALLOCATE = 1 << 6;
        const PATH_CREATE_DIR = 1 << 7;
        const PATH_CREATE_FILE = 1 << 8;
        const PATH_OPEN = 1 << 9;
        const PATH_READLINK = 1 << 10;
        const PATH_REMOVE = 1 << 11;
        const PATH_RENAME = 1 << 12;
        const PATH_FILESTAT = 1 << 13;
        const PATH_LINK = 1 << 14;
        const PATH_SYMLINK = 1 << 15;
        const POLL_FD = 1 << 16;
        const SOCK_RECV = 1 << 17;
        const SOCK_SEND = 1 << 18;
    }
}

/// WASI error codes (Preview 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum WasiError {
    /// Success.
    Success = 0,
    /// Argument list too long.
    TooBig = 1,
    /// Permission denied.
    Access = 2,
    /// Address in use.
    AddrInUse = 3,
    /// Address not available.
    AddrNotAvail = 4,
    /// Address family not supported.
    AfNoSupport = 5,
    /// Resource unavailable.
    Again = 6,
    /// Connection already in progress.
    Already = 7,
    /// Bad file descriptor.
    BadF = 8,
    /// Bad message.
    BadMsg = 9,
    /// Device or resource busy.
    Busy = 10,
    /// Operation canceled.
    Canceled = 11,
    /// No child processes.
    Child = 12,
    /// Connection aborted.
    ConnAborted = 13,
    /// Connection refused.
    ConnRefused = 14,
    /// Connection reset.
    ConnReset = 15,
    /// Resource deadlock would occur.
    DeadLk = 16,
    /// Destination address required.
    DestAddrReq = 17,
    /// Mathematics argument out of domain.
    Dom = 18,
    /// File exists.
    Exist = 20,
    /// Bad address.
    Fault = 21,
    /// File too large.
    FBig = 22,
    /// Host unreachable.
    HostUnreach = 23,
    /// Identifier removed.
    IdRm = 24,
    /// Illegal byte sequence.
    IlSeq = 25,
    /// Operation in progress.
    InProgress = 26,
    /// Interrupted function.
    Intr = 27,
    /// Invalid argument.
    Inval = 28,
    /// I/O error.
    Io = 29,
    /// Socket is connected.
    IsConn = 30,
    /// Is a directory.
    IsDir = 31,
    /// Too many levels of symbolic links.
    Loop = 32,
    /// File descriptor value too large.
    MFile = 33,
    /// Too many links.
    MLink = 34,
    /// Message too large.
    MsgSize = 35,
    /// Filename too long.
    NameTooLong = 37,
    /// Network is down.
    NetDown = 38,
    /// Connection aborted by network.
    NetReset = 39,
    /// Network unreachable.
    NetUnreach = 40,
    /// Too many files open in system.
    NFile = 41,
    /// No buffer space available.
    NoBufs = 42,
    /// No such device.
    NoDev = 43,
    /// No such file or directory.
    NoEnt = 44,
    /// Executable file format error.
    NoExec = 45,
    /// No locks available.
    NoLck = 46,
    /// Not enough space.
    NoMem = 48,
    /// No message of the desired type.
    NoMsg = 49,
    /// Protocol not available.
    NoProtoOpt = 50,
    /// No space left on device.
    NoSpc = 51,
    /// Function not supported.
    NoSys = 52,
    /// Socket is not connected.
    NotConn = 53,
    /// Not a directory.
    NotDir = 54,
    /// Directory not empty.
    NotEmpty = 55,
    /// State not recoverable.
    NotRecoverable = 56,
    /// Not a socket.
    NotSock = 57,
    /// Not supported.
    NotSup = 58,
    /// Inappropriate I/O control operation.
    NoTty = 59,
    /// No such device or address.
    NxIo = 60,
    /// Value too large to be stored in data type.
    Overflow = 61,
    /// Previous owner died.
    OwnerDead = 62,
    /// Operation not permitted.
    Perm = 63,
    /// Broken pipe.
    Pipe = 64,
    /// Protocol error.
    Proto = 65,
    /// Protocol not supported.
    ProtoNoSupport = 66,
    /// Protocol wrong type for socket.
    ProtoType = 67,
    /// Result too large.
    Range = 68,
    /// Read-only file system.
    RoFs = 69,
    /// Invalid seek.
    SPipe = 70,
    /// No such process.
    SRch = 71,
    /// Connection timed out.
    TimedOut = 73,
    /// Text file busy.
    TxtBsy = 74,
    /// Cross-device link.
    XDev = 75,
    /// Capabilities insufficient.
    NotCapable = 76,
}

impl WasiError {
    /// Convert to WASI errno integer.
    pub fn to_errno(self) -> i32 {
        self as i32
    }
}

impl From<WasiError> for RuntimeError {
    fn from(err: WasiError) -> Self {
        RuntimeError::WasiError(alloc::format!("WASI error: {:?}", err))
    }
}

/// Seek whence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whence {
    /// Seek from beginning.
    Set = 0,
    /// Seek from current position.
    Cur = 1,
    /// Seek from end.
    End = 2,
}

impl Whence {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Whence::Set),
            1 => Some(Whence::Cur),
            2 => Some(Whence::End),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    /// File descriptor flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: u16 {
        const APPEND = 1 << 0;
        const DSYNC = 1 << 1;
        const NONBLOCK = 1 << 2;
        const RSYNC = 1 << 3;
        const SYNC = 1 << 4;
    }
}

bitflags::bitflags! {
    /// Lookup flags for path operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LookupFlags: u32 {
        const SYMLINK_FOLLOW = 1 << 0;
    }
}

bitflags::bitflags! {
    /// Open flags for path_open.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OFlags: u16 {
        const CREAT = 1 << 0;
        const DIRECTORY = 1 << 1;
        const EXCL = 1 << 2;
        const TRUNC = 1 << 3;
    }
}

/// File descriptor stat.
#[derive(Debug, Clone)]
pub struct FdStat {
    pub fs_filetype: FdType,
    pub fs_flags: FdFlags,
    pub fs_rights_base: FdRights,
    pub fs_rights_inheriting: FdRights,
}

/// Prestat tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrestatTag {
    Dir = 0,
}

/// Prestat inner data.
#[repr(C)]
#[derive(Clone, Copy)]
pub union PrestatInner {
    pub dir_name_len: usize,
}

impl core::fmt::Debug for PrestatInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PrestatInner {{ dir_name_len: {} }}", unsafe {
            self.dir_name_len
        })
    }
}

/// Prestat.
#[derive(Debug, Clone, Copy)]
pub struct Prestat {
    pub tag: PrestatTag,
    pub inner: PrestatInner,
}

/// Clock ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockId {
    Realtime = 0,
    Monotonic = 1,
    ProcessCputime = 2,
    ThreadCputime = 3,
}

impl ClockId {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(ClockId::Realtime),
            1 => Some(ClockId::Monotonic),
            2 => Some(ClockId::ProcessCputime),
            3 => Some(ClockId::ThreadCputime),
            _ => None,
        }
    }
}

/// Directory entry.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Inode number.
    pub d_ino: u64,
    /// Offset to next entry.
    pub d_next: u64,
    /// Name length.
    pub d_namlen: u32,
    /// Entry type.
    pub d_type: FdType,
    /// Entry name.
    pub name: String,
}

/// File stat.
#[derive(Debug, Clone, Default)]
pub struct FileStat {
    pub dev: u64,
    pub ino: u64,
    pub filetype: u8,
    pub nlink: u64,
    pub size: u64,
    pub atim: u64,
    pub mtim: u64,
    pub ctim: u64,
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    // ── VFS Tests ──────────────────────────────────────────────────

    #[test]
    fn test_vfs_normalize() {
        assert_eq!(Vfs::normalize("/"), "/");
        assert_eq!(Vfs::normalize("/foo/bar"), "/foo/bar");
        assert_eq!(Vfs::normalize("/foo/../bar"), "/bar");
        assert_eq!(Vfs::normalize("/foo/./bar"), "/foo/bar");
        assert_eq!(Vfs::normalize("//foo///bar//"), "/foo/bar");
        assert_eq!(Vfs::normalize("/a/b/c/../../d"), "/a/d");
    }

    #[test]
    fn test_vfs_is_within() {
        assert!(Vfs::is_within("/app/data/file.txt", "/app"));
        assert!(Vfs::is_within("/app", "/app"));
        assert!(!Vfs::is_within("/etc/passwd", "/app"));
        assert!(!Vfs::is_within("/app2", "/app"));
        assert!(Vfs::is_within("/anything", "/"));
    }

    #[test]
    fn test_vfs_create_read_file() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/app").unwrap();
        vfs.create_file("/app/test.txt", b"hello".to_vec()).unwrap();
        let data = vfs.read_file("/app/test.txt").unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn test_vfs_write_file() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/data").unwrap();
        vfs.create_file("/data/f.txt", b"abc".to_vec()).unwrap();
        vfs.write_file("/data/f.txt", 1, b"XY").unwrap();
        assert_eq!(vfs.read_file("/data/f.txt").unwrap(), b"aXY");
        // Write beyond end extends the file
        vfs.write_file("/data/f.txt", 5, b"Z").unwrap();
        assert_eq!(vfs.read_file("/data/f.txt").unwrap(), b"aXY\0\0Z");
    }

    #[test]
    fn test_vfs_directory_operations() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/mydir").unwrap();
        assert!(vfs.is_dir("/mydir"));
        assert!(!vfs.is_file("/mydir"));
        assert_eq!(vfs.create_dir("/mydir"), Err(WasiError::Exist));
        vfs.remove_dir("/mydir").unwrap();
        assert!(!vfs.is_dir("/mydir"));
    }

    #[test]
    fn test_vfs_remove_nonempty_dir() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/d").unwrap();
        vfs.create_file("/d/f.txt", vec![]).unwrap();
        assert_eq!(vfs.remove_dir("/d"), Err(WasiError::NotEmpty));
    }

    #[test]
    fn test_vfs_readdir() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/app").unwrap();
        vfs.create_dir("/app/sub").unwrap();
        vfs.create_file("/app/hello.txt", b"hi".to_vec()).unwrap();
        let entries = vfs.readdir("/app").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"."));
        assert!(names.contains(&".."));
        assert!(names.contains(&"hello.txt"));
        assert!(names.contains(&"sub"));
    }

    #[test]
    fn test_vfs_rename() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/a").unwrap();
        vfs.create_file("/a/f.txt", b"data".to_vec()).unwrap();
        vfs.rename("/a/f.txt", "/a/g.txt").unwrap();
        assert!(!vfs.is_file("/a/f.txt"));
        assert_eq!(vfs.read_file("/a/g.txt").unwrap(), b"data");
    }

    // ── WasiCtx Tests (Quality Gates) ──────────────────────────────

    // C-QG1: stdout output
    #[test]
    fn test_cqg1_stdout_output() {
        let mut ctx = WasiCtx::new();
        let msg = b"Hello, WASI!";
        let n = ctx.fd_write(1, msg).unwrap();
        assert_eq!(n, 12);
        assert_eq!(ctx.stdout_buf(), b"Hello, WASI!");
    }

    // C-QG2: File read via preopened dir
    #[test]
    fn test_cqg2_file_read() {
        let mut ctx = WasiCtx::new();
        let dir_fd = ctx.preopen_dir("/app");
        ctx.vfs
            .create_file("/app/hello.txt", b"Hello from VFS!".to_vec())
            .unwrap();
        let file_fd = ctx
            .path_open(
                dir_fd, LookupFlags::empty(), "hello.txt", OFlags::empty(),
                FdRights::READ | FdRights::SEEK, FdRights::empty(), FdFlags::empty(),
            )
            .unwrap();
        let mut buf = [0u8; 64];
        let n = ctx.fd_read(file_fd, &mut buf).unwrap();
        assert_eq!(n, 15);
        assert_eq!(&buf[..n], b"Hello from VFS!");
    }

    // C-QG3: File write via path_open(O_CREAT)
    #[test]
    fn test_cqg3_file_write() {
        let mut ctx = WasiCtx::new();
        let dir_fd = ctx.preopen_dir("/data");
        let file_fd = ctx
            .path_open(
                dir_fd, LookupFlags::empty(), "output.txt", OFlags::CREAT,
                FdRights::WRITE | FdRights::SEEK, FdRights::empty(), FdFlags::empty(),
            )
            .unwrap();
        let n = ctx.fd_write(file_fd, b"Written data!").unwrap();
        assert_eq!(n, 13);
        assert_eq!(ctx.vfs.read_file("/data/output.txt").unwrap(), b"Written data!");
    }

    // C-QG4: Directory create + readdir
    #[test]
    fn test_cqg4_directory() {
        let mut ctx = WasiCtx::new();
        let dir_fd = ctx.preopen_dir("/workspace");
        ctx.path_create_directory(dir_fd, "subdir").unwrap();
        assert!(ctx.vfs.is_dir("/workspace/subdir"));
        let entries = ctx.vfs.readdir("/workspace").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"subdir"));
    }

    // C-QG5: Clock monotonic non-zero
    #[test]
    fn test_cqg5_clock() {
        let mut ctx = WasiCtx::new();
        let t1 = ctx.clock_time_get(ClockId::Monotonic, 0).unwrap();
        let t2 = ctx.clock_time_get(ClockId::Monotonic, 0).unwrap();
        assert!(t1 > 0, "Monotonic time should be non-zero");
        assert!(t2 > t1, "Monotonic time should increase");
    }

    // C-QG6: Random non-zero bytes
    #[test]
    fn test_cqg6_random() {
        let mut ctx = WasiCtx::new();
        let mut buf = [0u8; 32];
        ctx.random_get(&mut buf).unwrap();
        let non_zero_count = buf.iter().filter(|&&b| b != 0).count();
        assert!(non_zero_count > 0, "Expected some non-zero random bytes");
    }

    // C-QG7: Args get/sizes_get
    #[test]
    fn test_cqg7_args() {
        let mut ctx = WasiCtx::new();
        ctx.set_args(vec![String::from("app"), String::from("--flag")]);
        let (argc, buf_size) = ctx.args_sizes_get();
        assert_eq!(argc, 2);
        assert_eq!(buf_size, 4 + 7); // "app\0" + "--flag\0"
        let args = ctx.args_get();
        assert_eq!(args[0], "app");
        assert_eq!(args[1], "--flag");
    }

    // C-QG8: proc_exit
    #[test]
    fn test_cqg8_proc_exit() {
        let mut ctx = WasiCtx::new();
        assert_eq!(ctx.exit_code(), None);
        ctx.proc_exit(42);
        assert_eq!(ctx.exit_code(), Some(42));
    }

    // C-QG9: Sandbox violation
    #[test]
    fn test_cqg9_sandbox() {
        let mut ctx = WasiCtx::new();
        let dir_fd = ctx.preopen_dir("/app");

        // Path traversal outside /app
        let result = ctx.path_open(
            dir_fd, LookupFlags::empty(), "../etc/passwd", OFlags::empty(),
            FdRights::READ, FdRights::empty(), FdFlags::empty(),
        );
        assert_eq!(result, Err(WasiError::Access));

        // Absolute path outside /app
        let result = ctx.path_open(
            dir_fd, LookupFlags::empty(), "/etc/passwd", OFlags::empty(),
            FdRights::READ, FdRights::empty(), FdFlags::empty(),
        );
        assert_eq!(result, Err(WasiError::Access));
    }

    // C-QG10: Full WASI app simulation
    #[test]
    fn test_cqg10_full_wasi_app() {
        let mut ctx = WasiCtx::new();
        ctx.set_args(vec![String::from("myapp"), String::from("run")]);
        ctx.set_env("HOME", "/app");
        let dir_fd = ctx.preopen_dir("/app");

        // Create input file
        ctx.vfs.create_file("/app/input.txt", b"input data 12345".to_vec()).unwrap();

        // Read input
        let input_fd = ctx.path_open(
            dir_fd, LookupFlags::empty(), "input.txt", OFlags::empty(),
            FdRights::READ | FdRights::SEEK, FdRights::empty(), FdFlags::empty(),
        ).unwrap();
        let mut read_buf = [0u8; 256];
        let n = ctx.fd_read(input_fd, &mut read_buf).unwrap();
        assert_eq!(&read_buf[..n], b"input data 12345");
        ctx.fd_close(input_fd).unwrap();

        // Write output
        let output_fd = ctx.path_open(
            dir_fd, LookupFlags::empty(), "output.txt", OFlags::CREAT,
            FdRights::WRITE, FdRights::empty(), FdFlags::empty(),
        ).unwrap();
        let output = alloc::format!("processed: {}", core::str::from_utf8(&read_buf[..n]).unwrap());
        ctx.fd_write(output_fd, output.as_bytes()).unwrap();
        ctx.fd_close(output_fd).unwrap();

        // Verify VFS
        assert_eq!(ctx.vfs.read_file("/app/output.txt").unwrap(), b"processed: input data 12345");

        // Stdout
        ctx.fd_write(1, b"Done!\n").unwrap();
        assert_eq!(ctx.stdout_buf(), b"Done!\n");

        // Environ
        let envs = ctx.environ_get();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0], "HOME=/app");

        // Normal exit
        ctx.proc_exit(0);
        assert_eq!(ctx.exit_code(), Some(0));
    }

    // Additional edge case tests

    #[test]
    fn test_fd_close_prevents_stdio() {
        let mut ctx = WasiCtx::new();
        assert_eq!(ctx.fd_close(0), Err(WasiError::Access));
        assert_eq!(ctx.fd_close(1), Err(WasiError::Access));
        assert_eq!(ctx.fd_close(2), Err(WasiError::Access));
    }

    #[test]
    fn test_fd_seek() {
        let mut ctx = WasiCtx::new();
        let dir_fd = ctx.preopen_dir("/app");
        ctx.vfs.create_file("/app/data.bin", vec![1, 2, 3, 4, 5]).unwrap();
        let fd = ctx.path_open(
            dir_fd, LookupFlags::empty(), "data.bin", OFlags::empty(),
            FdRights::READ | FdRights::SEEK | FdRights::TELL,
            FdRights::empty(), FdFlags::empty(),
        ).unwrap();
        let pos = ctx.fd_seek(fd, 2, Whence::Set).unwrap();
        assert_eq!(pos, 2);
        let mut buf = [0u8; 3];
        let n = ctx.fd_read(fd, &mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf, &[3, 4, 5]);
        let pos = ctx.fd_seek(fd, -1, Whence::End).unwrap();
        assert_eq!(pos, 4);
        let pos = ctx.fd_tell(fd).unwrap();
        assert_eq!(pos, 4);
    }

    #[test]
    fn test_prestat_get_dir_name() {
        let mut ctx = WasiCtx::new();
        let fd = ctx.preopen_dir("/my-app-data");
        let prestat = ctx.fd_prestat_get(fd).unwrap();
        assert_eq!(prestat.tag, PrestatTag::Dir);
        let name_len = unsafe { prestat.inner.dir_name_len };
        assert_eq!(name_len, "/my-app-data".len());
        let mut buf = vec![0u8; name_len];
        ctx.fd_prestat_dir_name(fd, &mut buf).unwrap();
        assert_eq!(&buf, b"/my-app-data");
    }

    #[test]
    fn test_environ_sizes() {
        let mut ctx = WasiCtx::new();
        ctx.set_env("KEY", "VAL");
        ctx.set_env("A", "B");
        let (count, size) = ctx.environ_sizes_get();
        assert_eq!(count, 2);
        // "A=B\0" = 4 bytes, "KEY=VAL\0" = 8 bytes = 12 total
        assert_eq!(size, 12);
    }

    #[test]
    fn test_path_open_excl() {
        let mut ctx = WasiCtx::new();
        let dir_fd = ctx.preopen_dir("/d");
        ctx.vfs.create_file("/d/exists.txt", vec![]).unwrap();
        let result = ctx.path_open(
            dir_fd, LookupFlags::empty(), "exists.txt",
            OFlags::CREAT | OFlags::EXCL,
            FdRights::WRITE, FdRights::empty(), FdFlags::empty(),
        );
        assert_eq!(result, Err(WasiError::Exist));
    }

    #[test]
    fn test_path_open_trunc() {
        let mut ctx = WasiCtx::new();
        let dir_fd = ctx.preopen_dir("/d");
        ctx.vfs.create_file("/d/file.txt", b"old content".to_vec()).unwrap();
        let fd = ctx.path_open(
            dir_fd, LookupFlags::empty(), "file.txt", OFlags::TRUNC,
            FdRights::READ | FdRights::WRITE, FdRights::empty(), FdFlags::empty(),
        ).unwrap();
        let mut buf = [0u8; 64];
        let n = ctx.fd_read(fd, &mut buf).unwrap();
        assert_eq!(n, 0); // File was truncated
    }

    #[test]
    fn test_fdstat_get() {
        let mut ctx = WasiCtx::new();
        let stat = ctx.fd_fdstat_get(1).unwrap();
        assert_eq!(stat.fs_filetype, FdType::CharDevice);
        assert!(stat.fs_rights_base.contains(FdRights::WRITE));
    }
}
