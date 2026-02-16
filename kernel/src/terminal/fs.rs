//! In-Memory Filesystem
//!
//! Tree-structured filesystem with directories, regular files,
//! symbolic links, and virtual /proc entries. All data lives in
//! kernel heap — no disk backend required.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

// ────────────────────────── Types ──────────────────────────

/// Inode number
pub type Ino = u64;

/// Unix-style permission bits
#[derive(Debug, Clone, Copy)]
pub struct FileMode(pub u16);

impl FileMode {
    pub const DIR_755: FileMode = FileMode(0o40755);
    pub const FILE_644: FileMode = FileMode(0o100644);
    pub const FILE_755: FileMode = FileMode(0o100755);
    pub const LINK_777: FileMode = FileMode(0o120777);

    pub fn is_dir(self) -> bool {
        self.0 & 0o170000 == 0o040000
    }
    pub fn is_file(self) -> bool {
        self.0 & 0o170000 == 0o100000
    }
    pub fn is_symlink(self) -> bool {
        self.0 & 0o170000 == 0o120000
    }

    /// Pretty-print like `drwxr-xr-x`
    pub fn display(&self) -> String {
        let mut s = String::with_capacity(10);
        if self.is_dir() {
            s.push('d');
        } else if self.is_symlink() {
            s.push('l');
        } else {
            s.push('-');
        }
        let perms = self.0 & 0o777;
        for shift in [6u16, 3, 0] {
            let bits = (perms >> shift) & 7;
            s.push(if bits & 4 != 0 { 'r' } else { '-' });
            s.push(if bits & 2 != 0 { 'w' } else { '-' });
            s.push(if bits & 1 != 0 { 'x' } else { '-' });
        }
        s
    }
}

/// Timestamp (seconds since boot — no RTC yet)
#[derive(Debug, Clone, Copy, Default)]
pub struct Timestamp {
    pub secs: u64,
}

/// Inode — metadata + content
#[derive(Debug, Clone)]
pub struct Inode {
    pub ino: Ino,
    pub mode: FileMode,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub created: Timestamp,
    pub modified: Timestamp,
    pub nlink: u32,
    pub content: InodeContent,
}

#[derive(Debug, Clone)]
pub enum InodeContent {
    Directory(BTreeMap<String, Ino>),
    File(Vec<u8>),
    Symlink(String),
    /// Virtual file whose content is generated at read-time
    ProcFile(ProcFileKind),
}

#[derive(Debug, Clone, Copy)]
pub enum ProcFileKind {
    CpuInfo,
    MemInfo,
    Uptime,
    Version,
    Mounts,
    Cmdline,
    Loadavg,
}

// ────────────────────────── Filesystem ──────────────────────────

pub struct MemFs {
    inodes: BTreeMap<Ino, Inode>,
    next_ino: Ino,
    boot_ticks: u64,
}

/// Global filesystem instance
static FS: Mutex<Option<MemFs>> = Mutex::new(None);

/// Initialise the global filesystem with default structure.
pub fn init() {
    let mut fs = MemFs::new();
    fs.populate_defaults();
    *FS.lock() = Some(fs);
}

/// Run a closure with the global filesystem.
pub fn with_fs<F, R>(f: F) -> R
where
    F: FnOnce(&mut MemFs) -> R,
{
    let mut guard = FS.lock();
    let fs = guard
        .as_mut()
        .expect("MemFs not initialised — call terminal::fs::init()");
    f(fs)
}

impl MemFs {
    pub fn new() -> Self {
        let mut fs = Self {
            inodes: BTreeMap::new(),
            next_ino: 1,
            boot_ticks: 0,
        };
        // Create root inode
        let root_ino = fs.alloc_ino();
        fs.inodes.insert(
            root_ino,
            Inode {
                ino: root_ino,
                mode: FileMode::DIR_755,
                uid: 0,
                gid: 0,
                size: 0,
                created: Timestamp::default(),
                modified: Timestamp::default(),
                nlink: 2,
                content: InodeContent::Directory({
                    let mut m = BTreeMap::new();
                    m.insert(String::from("."), root_ino);
                    m.insert(String::from(".."), root_ino);
                    m
                }),
            },
        );
        fs
    }

    fn alloc_ino(&mut self) -> Ino {
        let i = self.next_ino;
        self.next_ino += 1;
        i
    }

    // ── Navigation ────────────────────────────────────────────

    pub fn root_ino(&self) -> Ino {
        1
    }

    /// Resolve an absolute path to its inode. Returns `None` if not found.
    pub fn resolve(&self, path: &str) -> Option<Ino> {
        let path = if path.is_empty() || path == "/" {
            "/"
        } else {
            path
        };
        let mut ino = self.root_ino();
        if path == "/" {
            return Some(ino);
        }

        for component in path.trim_start_matches('/').split('/') {
            if component.is_empty() {
                continue;
            }
            ino = self.lookup(ino, component)?;
        }
        Some(ino)
    }

    /// Look up a name inside a directory.
    pub fn lookup(&self, dir_ino: Ino, name: &str) -> Option<Ino> {
        let dir = self.inodes.get(&dir_ino)?;
        match &dir.content {
            InodeContent::Directory(entries) => entries.get(name).copied(),
            _ => None,
        }
    }

    /// Get inode reference.
    pub fn get(&self, ino: Ino) -> Option<&Inode> {
        self.inodes.get(&ino)
    }

    /// Get mutable inode reference.
    pub fn get_mut(&mut self, ino: Ino) -> Option<&mut Inode> {
        self.inodes.get_mut(&ino)
    }

    // ── Directory operations ──────────────────────────────────

    /// List directory entries (excluding . and ..)
    pub fn readdir(&self, ino: Ino) -> Option<Vec<(String, Ino)>> {
        let dir = self.inodes.get(&ino)?;
        match &dir.content {
            InodeContent::Directory(entries) => Some(
                entries
                    .iter()
                    .filter(|(n, _)| n.as_str() != "." && n.as_str() != "..")
                    .map(|(n, i)| (n.clone(), *i))
                    .collect(),
            ),
            _ => None,
        }
    }

    /// List all entries including . and ..
    pub fn readdir_all(&self, ino: Ino) -> Option<Vec<(String, Ino)>> {
        let dir = self.inodes.get(&ino)?;
        match &dir.content {
            InodeContent::Directory(entries) => {
                Some(entries.iter().map(|(n, i)| (n.clone(), *i)).collect())
            }
            _ => None,
        }
    }

    /// Create a sub-directory. Returns its inode.
    pub fn mkdir(&mut self, parent: Ino, name: &str) -> Result<Ino, FsError> {
        if name.contains('/') || name == "." || name == ".." {
            return Err(FsError::InvalidName);
        }
        // Check parent is a directory and name doesn't exist
        {
            let parent_node = self.inodes.get(&parent).ok_or(FsError::NotFound)?;
            match &parent_node.content {
                InodeContent::Directory(e) => {
                    if e.contains_key(name) {
                        return Err(FsError::AlreadyExists);
                    }
                }
                _ => return Err(FsError::NotADirectory),
            }
        }

        let child_ino = self.alloc_ino();
        let child = Inode {
            ino: child_ino,
            mode: FileMode::DIR_755,
            uid: 0,
            gid: 0,
            size: 0,
            created: Timestamp::default(),
            modified: Timestamp::default(),
            nlink: 2,
            content: InodeContent::Directory({
                let mut m = BTreeMap::new();
                m.insert(String::from("."), child_ino);
                m.insert(String::from(".."), parent);
                m
            }),
        };
        self.inodes.insert(child_ino, child);

        // Add entry to parent
        if let Some(p) = self.inodes.get_mut(&parent) {
            if let InodeContent::Directory(ref mut entries) = p.content {
                entries.insert(String::from(name), child_ino);
            }
            p.nlink += 1;
        }
        Ok(child_ino)
    }

    /// Create (or truncate) a regular file. Returns inode.
    pub fn create_file(&mut self, parent: Ino, name: &str, data: &[u8]) -> Result<Ino, FsError> {
        if name.contains('/') || name == "." || name == ".." {
            return Err(FsError::InvalidName);
        }
        // If file already exists, truncate
        {
            let parent_node = self.inodes.get(&parent).ok_or(FsError::NotFound)?;
            match &parent_node.content {
                InodeContent::Directory(e) => {
                    if let Some(&existing) = e.get(name) {
                        // Truncate existing file
                        if let Some(node) = self.inodes.get_mut(&existing) {
                            if node.mode.is_file() {
                                node.content = InodeContent::File(Vec::from(data));
                                node.size = data.len() as u64;
                                return Ok(existing);
                            }
                        }
                        return Err(FsError::AlreadyExists);
                    }
                }
                _ => return Err(FsError::NotADirectory),
            }
        }

        let child_ino = self.alloc_ino();
        let child = Inode {
            ino: child_ino,
            mode: FileMode::FILE_644,
            uid: 0,
            gid: 0,
            size: data.len() as u64,
            created: Timestamp::default(),
            modified: Timestamp::default(),
            nlink: 1,
            content: InodeContent::File(Vec::from(data)),
        };
        self.inodes.insert(child_ino, child);

        if let Some(p) = self.inodes.get_mut(&parent) {
            if let InodeContent::Directory(ref mut entries) = p.content {
                entries.insert(String::from(name), child_ino);
            }
        }
        Ok(child_ino)
    }

    /// Read file content (for regular and proc files).
    pub fn read_file(&self, ino: Ino) -> Result<Vec<u8>, FsError> {
        let node = self.inodes.get(&ino).ok_or(FsError::NotFound)?;
        match &node.content {
            InodeContent::File(data) => Ok(data.clone()),
            InodeContent::ProcFile(kind) => Ok(self.generate_proc(*kind).into_bytes()),
            InodeContent::Directory(_) => Err(FsError::IsADirectory),
            InodeContent::Symlink(_) => Err(FsError::InvalidOperation),
        }
    }

    /// Write (overwrite) file content.
    pub fn write_file(&mut self, ino: Ino, data: &[u8]) -> Result<(), FsError> {
        let node = self.inodes.get_mut(&ino).ok_or(FsError::NotFound)?;
        match &mut node.content {
            InodeContent::File(ref mut buf) => {
                *buf = Vec::from(data);
                node.size = data.len() as u64;
                Ok(())
            }
            InodeContent::ProcFile(_) => Err(FsError::ReadOnly),
            InodeContent::Directory(_) => Err(FsError::IsADirectory),
            InodeContent::Symlink(_) => Err(FsError::InvalidOperation),
        }
    }

    /// Append to a file.
    pub fn append_file(&mut self, ino: Ino, data: &[u8]) -> Result<(), FsError> {
        let node = self.inodes.get_mut(&ino).ok_or(FsError::NotFound)?;
        match &mut node.content {
            InodeContent::File(ref mut buf) => {
                buf.extend_from_slice(data);
                node.size = buf.len() as u64;
                Ok(())
            }
            _ => Err(FsError::InvalidOperation),
        }
    }

    /// Remove an entry from a directory.
    pub fn remove(&mut self, parent: Ino, name: &str) -> Result<(), FsError> {
        if name == "." || name == ".." {
            return Err(FsError::InvalidName);
        }

        let child_ino = {
            let parent_node = self.inodes.get(&parent).ok_or(FsError::NotFound)?;
            match &parent_node.content {
                InodeContent::Directory(e) => *e.get(name).ok_or(FsError::NotFound)?,
                _ => return Err(FsError::NotADirectory),
            }
        };

        // If directory, check it's empty
        if let Some(child) = self.inodes.get(&child_ino) {
            if let InodeContent::Directory(e) = &child.content {
                if e.len() > 2 {
                    return Err(FsError::DirectoryNotEmpty);
                }
            }
        }

        // Remove from parent
        if let Some(p) = self.inodes.get_mut(&parent) {
            if let InodeContent::Directory(ref mut entries) = p.content {
                entries.remove(name);
            }
        }

        // Decrement nlink and remove if 0
        let should_remove = if let Some(child) = self.inodes.get_mut(&child_ino) {
            child.nlink = child.nlink.saturating_sub(1);
            child.nlink == 0
        } else {
            false
        };

        if should_remove {
            self.inodes.remove(&child_ino);
        }

        Ok(())
    }

    /// Rename / move entry.
    pub fn rename(
        &mut self,
        src_parent: Ino,
        src_name: &str,
        dst_parent: Ino,
        dst_name: &str,
    ) -> Result<(), FsError> {
        let ino = {
            let p = self.inodes.get(&src_parent).ok_or(FsError::NotFound)?;
            match &p.content {
                InodeContent::Directory(e) => *e.get(src_name).ok_or(FsError::NotFound)?,
                _ => return Err(FsError::NotADirectory),
            }
        };

        // Remove from source
        if let Some(p) = self.inodes.get_mut(&src_parent) {
            if let InodeContent::Directory(ref mut e) = p.content {
                e.remove(src_name);
            }
        }

        // Remove existing destination if any
        let _ = self.remove(dst_parent, dst_name);

        // Add to destination
        if let Some(p) = self.inodes.get_mut(&dst_parent) {
            if let InodeContent::Directory(ref mut e) = p.content {
                e.insert(String::from(dst_name), ino);
            }
        }

        // Update .. in moved directory
        if let Some(n) = self.inodes.get_mut(&ino) {
            if let InodeContent::Directory(ref mut e) = n.content {
                e.insert(String::from(".."), dst_parent);
            }
        }

        Ok(())
    }

    /// Copy a file (not directories).
    pub fn copy_file(
        &mut self,
        src_ino: Ino,
        dst_parent: Ino,
        dst_name: &str,
    ) -> Result<Ino, FsError> {
        let data = self.read_file(src_ino)?;
        self.create_file(dst_parent, dst_name, &data)
    }

    /// Get the total size of a subtree (for `du`).
    pub fn tree_size(&self, ino: Ino) -> u64 {
        let node = match self.inodes.get(&ino) {
            Some(n) => n,
            None => return 0,
        };
        match &node.content {
            InodeContent::File(data) => data.len() as u64,
            InodeContent::Directory(entries) => entries
                .iter()
                .filter(|(n, _)| n.as_str() != "." && n.as_str() != "..")
                .map(|(_, &child)| self.tree_size(child))
                .sum(),
            _ => 0,
        }
    }

    /// Count files in a subtree.
    pub fn tree_count(&self, ino: Ino) -> u64 {
        let node = match self.inodes.get(&ino) {
            Some(n) => n,
            None => return 0,
        };
        match &node.content {
            InodeContent::File(_) => 1,
            InodeContent::Directory(entries) => entries
                .iter()
                .filter(|(n, _)| n.as_str() != "." && n.as_str() != "..")
                .map(|(_, &child)| self.tree_count(child))
                .sum(),
            _ => 1,
        }
    }

    // ── /proc generators ──────────────────────────────────────

    fn generate_proc(&self, kind: ProcFileKind) -> String {
        match kind {
            ProcFileKind::CpuInfo => {
                let mut s = String::from("processor\t: 0\n");
                s.push_str("vendor_id\t: GenuineIntel\n");
                s.push_str("model name\t: KPIO Virtual CPU\n");
                s.push_str("cpu MHz\t\t: 2000.000\n");
                s.push_str("cache size\t: 4096 KB\n");
                s.push_str("flags\t\t: fpu sse sse2 sse3 ssse3 sse4_1 sse4_2\n");
                s
            }
            ProcFileKind::MemInfo => {
                let stats = crate::allocator::heap_stats();
                let total_kb = stats.total / 1024;
                let free_kb = stats.free / 1024;
                let used_kb = stats.used / 1024;
                let avail_kb = free_kb;
                alloc::format!(
                    "MemTotal:       {:>8} kB\n\
                     MemFree:        {:>8} kB\n\
                     MemAvailable:   {:>8} kB\n\
                     MemUsed:        {:>8} kB\n\
                     Buffers:               0 kB\n\
                     Cached:                0 kB\n\
                     SwapTotal:             0 kB\n\
                     SwapFree:              0 kB\n",
                    total_kb,
                    free_kb,
                    avail_kb,
                    used_kb,
                )
            }
            ProcFileKind::Uptime => {
                let ticks = crate::scheduler::boot_ticks();
                let secs = ticks / 100; // assumes 100 Hz APIC timer
                let centisecs = ticks % 100;
                alloc::format!("{}.{:02} 0.00\n", secs, centisecs)
            }
            ProcFileKind::Version => String::from(
                "KPIO version 1.0.0 (root@kpio) (rustc 1.82-nightly) #1 SMP PREEMPT_DYNAMIC\n",
            ),
            ProcFileKind::Mounts => {
                String::from("ramfs / ramfs rw,relatime 0 0\nproc /proc proc rw,nosuid,nodev 0 0\n")
            }
            ProcFileKind::Cmdline => String::from("BOOT_IMAGE=/boot/kpio root=/dev/ram0\n"),
            ProcFileKind::Loadavg => String::from("0.00 0.00 0.00 1/32 42\n"),
        }
    }

    /// Bump the tick counter (called from timer interrupt).
    pub fn tick(&mut self) {
        self.boot_ticks += 1;
    }

    pub fn uptime_secs(&self) -> u64 {
        crate::scheduler::boot_ticks() / 100
    }

    // ── Default population ────────────────────────────────────

    fn populate_defaults(&mut self) {
        let root = self.root_ino();

        // Top-level directories
        let home = self.mkdir(root, "home").unwrap();
        let etc = self.mkdir(root, "etc").unwrap();
        let usr = self.mkdir(root, "usr").unwrap();
        let var = self.mkdir(root, "var").unwrap();
        let _tmp = self.mkdir(root, "tmp").unwrap();
        let _bin = self.mkdir(root, "bin").unwrap();
        let dev = self.mkdir(root, "dev").unwrap();
        let proc_dir = self.mkdir(root, "proc").unwrap();
        let _sbin = self.mkdir(root, "sbin").unwrap();
        let _opt = self.mkdir(root, "opt").unwrap();
        let boot = self.mkdir(root, "boot").unwrap();

        // /home/root
        let home_root = self.mkdir(home, "root").unwrap();
        let docs = self.mkdir(home_root, "Documents").unwrap();
        let _dl = self.mkdir(home_root, "Downloads").unwrap();
        let _pics = self.mkdir(home_root, "Pictures").unwrap();
        let _music = self.mkdir(home_root, "Music").unwrap();
        let _vids = self.mkdir(home_root, "Videos").unwrap();
        let dsk = self.mkdir(home_root, "Desktop").unwrap();

        // Sample files
        self.create_file(
            docs,
            "readme.txt",
            b"Welcome to KPIO OS!\nThis is a sample text file.\n",
        )
        .unwrap();
        self.create_file(
            docs,
            "notes.txt",
            b"TODO:\n- Implement more syscalls\n- Add network drivers\n- Build browser engine\n",
        )
        .unwrap();
        self.create_file(dsk, "hello.sh", b"#!/bin/sh\necho \"Hello from KPIO!\"\n")
            .unwrap();
        self.create_file(home_root, ".bashrc",
            b"# KPIO Shell Configuration\nexport PS1='\\u@\\h:\\w$ '\nalias ll='ls -la'\nalias la='ls -a'\n").unwrap();
        self.create_file(
            home_root,
            ".profile",
            b"# KPIO Profile\nexport PATH=/usr/bin:/bin:/sbin\nexport LANG=en_US.UTF-8\n",
        )
        .unwrap();

        // /etc files
        self.create_file(etc, "passwd",
            b"root:x:0:0:root:/home/root:/bin/sh\nnobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin\n").unwrap();
        self.create_file(etc, "hostname", b"kpio\n").unwrap();
        self.create_file(
            etc,
            "hosts",
            b"127.0.0.1\tlocalhost\n::1\t\tlocalhost\n127.0.1.1\tkpio\n",
        )
        .unwrap();
        self.create_file(
            etc,
            "os-release",
            b"NAME=\"KPIO OS\"\nVERSION=\"1.0.0\"\nID=kpio\nPRETTY_NAME=\"KPIO OS 1.0.0\"\n",
        )
        .unwrap();
        self.create_file(
            etc,
            "fstab",
            b"# /etc/fstab: static file system information\nramfs\t/\tramfs\tdefaults\t0 0\n",
        )
        .unwrap();
        let etc_conf = self.mkdir(etc, "config").unwrap();
        self.create_file(
            etc_conf,
            "kernel.conf",
            b"# Kernel configuration\nloglevel=3\n",
        )
        .unwrap();

        // /usr hierarchy
        let _usr_bin = self.mkdir(usr, "bin").unwrap();
        let _usr_lib = self.mkdir(usr, "lib").unwrap();
        let _usr_share = self.mkdir(usr, "share").unwrap();
        let _usr_local = self.mkdir(usr, "local").unwrap();

        // /var hierarchy
        let var_log = self.mkdir(var, "log").unwrap();
        let _var_cache = self.mkdir(var, "cache").unwrap();
        let _var_run = self.mkdir(var, "run").unwrap();
        self.create_file(var_log, "syslog",
            b"[    0.000] KPIO kernel booting\n[    0.010] GDT initialized\n[    0.020] IDT initialized\n[    0.050] Memory management ready\n[    0.100] Scheduler started\n").unwrap();
        self.create_file(var_log, "dmesg",
            b"[    0.000] KPIO 1.0.0 x86_64\n[    0.001] ACPI: RSDP search\n[    0.010] PCI: Enumerating bus\n[    0.050] VirtIO: Block device detected\n").unwrap();

        // /proc virtual files
        self.create_proc_file(proc_dir, "cpuinfo", ProcFileKind::CpuInfo);
        self.create_proc_file(proc_dir, "meminfo", ProcFileKind::MemInfo);
        self.create_proc_file(proc_dir, "uptime", ProcFileKind::Uptime);
        self.create_proc_file(proc_dir, "version", ProcFileKind::Version);
        self.create_proc_file(proc_dir, "mounts", ProcFileKind::Mounts);
        self.create_proc_file(proc_dir, "cmdline", ProcFileKind::Cmdline);
        self.create_proc_file(proc_dir, "loadavg", ProcFileKind::Loadavg);

        // /dev entries (as regular files for now)
        self.create_file(dev, "null", b"").unwrap();
        self.create_file(dev, "zero", b"").unwrap();
        self.create_file(dev, "random", b"").unwrap();
        self.create_file(dev, "console", b"").unwrap();
        self.create_file(dev, "tty0", b"").unwrap();

        // /boot
        self.create_file(boot, "kpio-kernel", b"[binary: KPIO kernel image]\n")
            .unwrap();
    }

    fn create_proc_file(&mut self, parent: Ino, name: &str, kind: ProcFileKind) {
        let ino = self.alloc_ino();
        let node = Inode {
            ino,
            mode: FileMode(0o100444), // r--r--r--
            uid: 0,
            gid: 0,
            size: 0,
            created: Timestamp::default(),
            modified: Timestamp::default(),
            nlink: 1,
            content: InodeContent::ProcFile(kind),
        };
        self.inodes.insert(ino, node);

        if let Some(p) = self.inodes.get_mut(&parent) {
            if let InodeContent::Directory(ref mut e) = p.content {
                e.insert(String::from(name), ino);
            }
        }
    }

    /// Resolve an absolute path, returning (parent_ino, basename)
    pub fn resolve_parent(&self, path: &str) -> Option<(Ino, String)> {
        let path = path.trim_end_matches('/');
        if path.is_empty() || path == "/" {
            return None;
        }
        let last_slash = path.rfind('/')?;
        let parent_path = if last_slash == 0 {
            "/"
        } else {
            &path[..last_slash]
        };
        let name = &path[last_slash + 1..];
        if name.is_empty() {
            return None;
        }
        let parent_ino = self.resolve(parent_path)?;
        Some((parent_ino, String::from(name)))
    }
}

// ────────────────────────── Errors ──────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    AlreadyExists,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    InvalidName,
    ReadOnly,
    InvalidOperation,
    PermissionDenied,
}

impl FsError {
    pub fn as_str(self) -> &'static str {
        match self {
            FsError::NotFound => "No such file or directory",
            FsError::AlreadyExists => "File exists",
            FsError::NotADirectory => "Not a directory",
            FsError::IsADirectory => "Is a directory",
            FsError::DirectoryNotEmpty => "Directory not empty",
            FsError::InvalidName => "Invalid file name",
            FsError::ReadOnly => "Read-only file system",
            FsError::InvalidOperation => "Invalid operation",
            FsError::PermissionDenied => "Permission denied",
        }
    }
}

impl core::fmt::Display for FsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
