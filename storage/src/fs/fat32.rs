//! FAT32 filesystem implementation (read-focused).

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use spin::RwLock;

use crate::driver::BlockDevice;
use crate::vfs::{Filesystem, FsStats};
use crate::{DirEntry, FileMetadata, FilePermissions, FileType, OpenFlags, StorageError};

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Fat32Bpb {
    pub jmp_boot: [u8; 3],
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_entry_count: u16,
    pub total_sectors_16: u16,
    pub media: u8,
    pub fat_size_16: u16,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    pub fat_size_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    pub reserved: [u8; 12],
    pub drive_number: u8,
    pub reserved1: u8,
    pub boot_sig: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

impl Fat32Bpb {
    pub fn cluster_size(&self) -> u32 {
        self.bytes_per_sector as u32 * self.sectors_per_cluster as u32
    }

    pub fn fat_size(&self) -> u32 {
        if self.fat_size_16 != 0 {
            self.fat_size_16 as u32
        } else {
            self.fat_size_32
        }
    }

    pub fn total_sectors(&self) -> u32 {
        if self.total_sectors_16 != 0 {
            self.total_sectors_16 as u32
        } else {
            self.total_sectors_32
        }
    }

    pub fn first_data_sector(&self) -> u32 {
        self.reserved_sectors as u32 + self.num_fats as u32 * self.fat_size()
    }

    pub fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.first_data_sector() + (cluster - 2) * self.sectors_per_cluster as u32
    }

    pub fn total_clusters(&self) -> u32 {
        (self.total_sectors() - self.first_data_sector()) / self.sectors_per_cluster as u32
    }
}

pub mod fat_entry {
    pub const BAD: u32 = 0x0FFFFFF7;
    pub const EOC_MIN: u32 = 0x0FFFFFF8;
    pub const MASK: u32 = 0x0FFFFFFF;
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Fat32DirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub nt_res: u8,
    pub crt_time_tenth: u8,
    pub crt_time: u16,
    pub crt_date: u16,
    pub lst_acc_date: u16,
    pub fst_clus_hi: u16,
    pub wrt_time: u16,
    pub wrt_date: u16,
    pub fst_clus_lo: u16,
    pub file_size: u32,
}

impl Fat32DirEntry {
    pub const SIZE: usize = 32;
    pub const ATTR_READ_ONLY: u8 = 0x01;
    pub const ATTR_VOLUME_ID: u8 = 0x08;
    pub const ATTR_DIRECTORY: u8 = 0x10;
    pub const ATTR_LONG_NAME: u8 = 0x0F;
    pub const DELETED: u8 = 0xE5;
    pub const LAST: u8 = 0x00;

    pub fn is_free(&self) -> bool {
        self.name[0] == Self::DELETED || self.name[0] == Self::LAST
    }

    pub fn is_last(&self) -> bool {
        self.name[0] == Self::LAST
    }

    pub fn is_long_name(&self) -> bool {
        self.attr == Self::ATTR_LONG_NAME
    }

    pub fn is_dir(&self) -> bool {
        self.attr & Self::ATTR_DIRECTORY != 0
    }

    pub fn is_volume_label(&self) -> bool {
        self.attr & Self::ATTR_VOLUME_ID != 0
    }

    pub fn first_cluster(&self) -> u32 {
        ((self.fst_clus_hi as u32) << 16) | self.fst_clus_lo as u32
    }

    pub fn short_name(&self) -> [u8; 13] {
        let mut result = [0u8; 13];
        let mut pos = 0usize;

        for i in 0..8 {
            if self.name[i] != b' ' {
                result[pos] = self.name[i];
                pos += 1;
            }
        }

        let has_ext = self.name[8] != b' ' || self.name[9] != b' ' || self.name[10] != b' ';
        if has_ext {
            result[pos] = b'.';
            pos += 1;
            for i in 8..11 {
                if self.name[i] != b' ' {
                    result[pos] = self.name[i];
                    pos += 1;
                }
            }
        }

        result
    }

    pub fn to_dir_entry(&self) -> DirEntry {
        let short_name = self.short_name();
        let mut name = [0u8; 256];
        let mut name_len = 0usize;

        for &b in &short_name {
            if b == 0 {
                break;
            }
            name[name_len] = b;
            name_len += 1;
        }

        DirEntry {
            name,
            name_len,
            inode: self.first_cluster() as u64,
            file_type: if self.is_dir() {
                FileType::Directory
            } else {
                FileType::Regular
            },
        }
    }

    pub fn to_metadata(&self) -> FileMetadata {
        let permissions = if self.attr & Self::ATTR_READ_ONLY != 0 {
            FilePermissions(0o444)
        } else {
            FilePermissions(0o644)
        };

        FileMetadata {
            file_type: if self.is_dir() {
                FileType::Directory
            } else {
                FileType::Regular
            },
            permissions,
            size: self.file_size as u64,
            nlink: 1,
            uid: 0,
            gid: 0,
            block_size: 4096,
            blocks: (self.file_size as u64).div_ceil(512),
            atime: 0,
            mtime: 0,
            ctime: 0,
            crtime: 0,
            inode: self.first_cluster() as u64,
            dev: 0,
            rdev: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct OpenFile {
    first_cluster: u32,
    size: u64,
    is_dir: bool,
}

pub struct Fat32Filesystem {
    device: &'static dyn BlockDevice,
    bpb: Fat32Bpb,
    read_only: bool,
    free_clusters: u32,
    open_files: RwLock<[Option<OpenFile>; 128]>,
}

impl Fat32Filesystem {
    pub fn mount(device: &'static dyn BlockDevice) -> Result<Self, StorageError> {
        let mut sector = [0u8; 512];
        device.read_blocks(0, &mut sector)?;

        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err(StorageError::InvalidFilesystem);
        }

        let bpb: Fat32Bpb = unsafe { core::ptr::read_unaligned(sector.as_ptr() as *const _) };
        if bpb.bytes_per_sector == 0 || bpb.sectors_per_cluster == 0 || bpb.fat_size() == 0 {
            return Err(StorageError::InvalidFilesystem);
        }

        Ok(Self {
            device,
            bpb,
            read_only: true,
            free_clusters: 0,
            open_files: RwLock::new([None; 128]),
        })
    }

    fn bps(&self) -> usize {
        self.bpb.bytes_per_sector as usize
    }

    fn cluster_size(&self) -> usize {
        self.bpb.cluster_size() as usize
    }

    fn read_sector(&self, sector: u32, out: &mut [u8]) -> Result<(), StorageError> {
        if out.len() < self.bps() {
            return Err(StorageError::BufferTooSmall);
        }
        let n = self.device.read_blocks(sector as u64, out)?;
        if n < self.bps() {
            return Err(StorageError::IoError);
        }
        Ok(())
    }

    fn read_cluster(&self, cluster: u32, out: &mut [u8]) -> Result<(), StorageError> {
        let cluster_size = self.cluster_size();
        if out.len() < cluster_size {
            return Err(StorageError::BufferTooSmall);
        }

        let first_sector = self.bpb.cluster_to_sector(cluster);
        let bps = self.bps();
        for i in 0..self.bpb.sectors_per_cluster as usize {
            let sector = first_sector + i as u32;
            let offset = i * bps;
            self.read_sector(sector, &mut out[offset..offset + bps])?;
        }
        Ok(())
    }

    fn read_fat_entry(&self, cluster: u32) -> Result<u32, StorageError> {
        let fat_offset = cluster as usize * 4;
        let bps = self.bps();
        let fat_sector = self.bpb.reserved_sectors as u32 + (fat_offset / bps) as u32;
        let entry_offset = fat_offset % bps;

        let mut sector = vec![0u8; bps];
        self.read_sector(fat_sector, &mut sector)?;
        if entry_offset + 4 > sector.len() {
            return Err(StorageError::Corrupted);
        }

        Ok(u32::from_le_bytes([
            sector[entry_offset],
            sector[entry_offset + 1],
            sector[entry_offset + 2],
            sector[entry_offset + 3],
        ]) & fat_entry::MASK)
    }

    fn read_dir_entries(&self, start_cluster: u32) -> Result<Vec<Fat32DirEntry>, StorageError> {
        let mut entries = Vec::new();
        let mut cluster = start_cluster;
        let mut cluster_buf = vec![0u8; self.cluster_size()];

        for _ in 0..4096 {
            self.read_cluster(cluster, &mut cluster_buf)?;
            for chunk in cluster_buf.chunks_exact(Fat32DirEntry::SIZE) {
                let entry: Fat32DirEntry = unsafe {
                    core::ptr::read_unaligned(chunk.as_ptr() as *const Fat32DirEntry)
                };
                if entry.is_last() {
                    return Ok(entries);
                }
                if entry.is_free() || entry.is_long_name() || entry.is_volume_label() {
                    continue;
                }
                entries.push(entry);
            }

            let next = self.read_fat_entry(cluster)?;
            if next >= fat_entry::EOC_MIN || next == fat_entry::BAD {
                break;
            }
            if next < 2 {
                return Err(StorageError::Corrupted);
            }
            cluster = next;
        }

        Ok(entries)
    }

    fn resolve_path(&self, path: &str) -> Result<Fat32DirEntry, StorageError> {
        if path == "/" {
            let root_cluster = self.bpb.root_cluster;
            return Ok(Fat32DirEntry {
                name: [b' '; 11],
                attr: Fat32DirEntry::ATTR_DIRECTORY,
                nt_res: 0,
                crt_time_tenth: 0,
                crt_time: 0,
                crt_date: 0,
                lst_acc_date: 0,
                fst_clus_hi: (root_cluster >> 16) as u16,
                wrt_time: 0,
                wrt_date: 0,
                fst_clus_lo: (root_cluster & 0xFFFF) as u16,
                file_size: 0,
            });
        }

        let mut current_cluster = self.bpb.root_cluster;
        let mut current_entry: Option<Fat32DirEntry> = None;
        let components = path
            .trim_start_matches('/')
            .split('/')
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>();

        for (idx, component) in components.iter().enumerate() {
            let mut found = None;
            for entry in self.read_dir_entries(current_cluster)? {
                let short = entry.short_name();
                let mut len = 0usize;
                while len < short.len() && short[len] != 0 {
                    len += 1;
                }
                let entry_name = core::str::from_utf8(&short[..len]).unwrap_or("");
                if entry_name.eq_ignore_ascii_case(component) {
                    found = Some(entry);
                    break;
                }
            }

            let entry = found.ok_or(StorageError::FileNotFound)?;
            current_entry = Some(entry);

            if entry.is_dir() {
                current_cluster = entry.first_cluster();
            } else if idx + 1 < components.len() {
                return Err(StorageError::NotADirectory);
             }
         }

         current_entry.ok_or(StorageError::FileNotFound)
     }

     fn alloc_handle(&self, of: OpenFile) -> Result<u64, StorageError> {
         let mut files = self.open_files.write();
         for (idx, slot) in files.iter_mut().enumerate() {
             if slot.is_none() {
                 *slot = Some(of);
                 return Ok(idx as u64);
             }
         }
         Err(StorageError::TooManyOpenFiles)
     }

     fn get_handle(&self, handle: u64) -> Result<OpenFile, StorageError> {
         let files = self.open_files.read();
         files
             .get(handle as usize)
             .and_then(|slot| *slot)
             .ok_or(StorageError::InvalidFd)
     }

     fn free_handle(&self, handle: u64) -> Result<(), StorageError> {
         let mut files = self.open_files.write();
         let slot = files
             .get_mut(handle as usize)
             .ok_or(StorageError::InvalidFd)?;
         if slot.is_none() {
             return Err(StorageError::InvalidFd);
         }
         *slot = None;
         Ok(())
     }

     fn read_file_data(&self, file: OpenFile, offset: u64, out: &mut [u8]) -> Result<usize, StorageError> {
         if file.is_dir {
             return Err(StorageError::NotAFile);
         }
         if offset >= file.size {
             return Ok(0);
         }

         let to_read = core::cmp::min(out.len() as u64, file.size - offset) as usize;
         let cluster_size = self.cluster_size();
         let skip_clusters = (offset as usize) / cluster_size;
         let mut cluster_offset = (offset as usize) % cluster_size;

         let mut cluster = file.first_cluster;
         for _ in 0..skip_clusters {
             let next = self.read_fat_entry(cluster)?;
             if next >= fat_entry::EOC_MIN {
                 return Ok(0);
             }
             cluster = next;
         }

         let mut cluster_buf = vec![0u8; cluster_size];
         let mut read_total = 0usize;

         while read_total < to_read {
             self.read_cluster(cluster, &mut cluster_buf)?;
             let available = cluster_size - cluster_offset;
             let needed = to_read - read_total;
             let copy_len = core::cmp::min(available, needed);

             out[read_total..read_total + copy_len]
                 .copy_from_slice(&cluster_buf[cluster_offset..cluster_offset + copy_len]);
             read_total += copy_len;
             cluster_offset = 0;

             if read_total >= to_read {
                 break;
             }

             let next = self.read_fat_entry(cluster)?;
             if next >= fat_entry::EOC_MIN {
                 break;
             }
             cluster = next;
         }

         Ok(read_total)
     }
 }

 impl Default for Fat32Filesystem {
     fn default() -> Self {
         struct Dummy;
         impl BlockDevice for Dummy {
             fn info(&self) -> crate::BlockDeviceInfo {
                 crate::BlockDeviceInfo {
                     name: [0; 32],
                     name_len: 0,
                     block_size: 512,
                     total_blocks: 0,
                     read_only: true,
                     supports_trim: false,
                     optimal_io_size: 1,
                     physical_block_size: 512,
                 }
             }

             fn read_blocks(&self, _start_block: u64, _buffer: &mut [u8]) -> Result<usize, StorageError> {
                 Err(StorageError::NotReady)
             }

             fn write_blocks(&self, _start_block: u64, _data: &[u8]) -> Result<usize, StorageError> {
                 Err(StorageError::ReadOnly)
             }

             fn flush(&self) -> Result<(), StorageError> {
                 Ok(())
             }

             fn discard(&self, _start_block: u64, _num_blocks: u64) -> Result<(), StorageError> {
                 Err(StorageError::Unsupported)
             }

             fn is_ready(&self) -> bool {
                 false
             }
         }

         static DUMMY: Dummy = Dummy;

         Self {
             device: &DUMMY,
             bpb: Fat32Bpb {
                 jmp_boot: [0; 3],
                 oem_name: [0; 8],
                 bytes_per_sector: 512,
                 sectors_per_cluster: 1,
                 reserved_sectors: 32,
                 num_fats: 2,
                 root_entry_count: 0,
                 total_sectors_16: 0,
                 media: 0xF8,
                 fat_size_16: 0,
                 sectors_per_track: 0,
                 num_heads: 0,
                 hidden_sectors: 0,
                 total_sectors_32: 0,
                 fat_size_32: 0,
                 ext_flags: 0,
                 fs_version: 0,
                 root_cluster: 2,
                 fs_info: 1,
                 backup_boot_sector: 6,
                 reserved: [0; 12],
                 drive_number: 0x80,
                 reserved1: 0,
                 boot_sig: 0x29,
                 volume_id: 0,
                 volume_label: [0; 11],
                 fs_type: [0; 8],
             },
             read_only: true,
             free_clusters: 0,
             open_files: RwLock::new([None; 128]),
         }
     }
 }

 impl Filesystem for Fat32Filesystem {
     fn fs_type(&self) -> &str {
         "fat32"
     }

     fn statfs(&self) -> Result<FsStats, StorageError> {
         Ok(FsStats {
             fs_type: 0x4D44,
             block_size: self.bpb.cluster_size(),
             total_blocks: self.bpb.total_clusters() as u64,
             free_blocks: self.free_clusters as u64,
             available_blocks: self.free_clusters as u64,
             total_inodes: 0,
             free_inodes: 0,
             fs_id: self.bpb.volume_id as u64,
             max_name_len: 255,
             fragment_size: self.bpb.cluster_size(),
             flags: crate::MountFlags::empty(),
         })
     }

     fn lookup(&self, path: &str) -> Result<FileMetadata, StorageError> {
         if path == "/" {
             let mut meta = FileMetadata::default();
             meta.file_type = FileType::Directory;
             meta.permissions = FilePermissions::DEFAULT_DIR;
             return Ok(meta);
         }
         let entry = self.resolve_path(path)?;
         Ok(entry.to_metadata())
     }

     fn readdir(&self, path: &str, offset: u64) -> Result<Vec<DirEntry>, StorageError> {
         let cluster = if path == "/" {
             self.bpb.root_cluster
         } else {
             let entry = self.resolve_path(path)?;
             if !entry.is_dir() {
                 return Err(StorageError::NotADirectory);
             }
             entry.first_cluster()
         };

         let mut entries = self
             .read_dir_entries(cluster)?
             .into_iter()
             .map(|e| e.to_dir_entry())
             .collect::<Vec<_>>();

         if offset as usize >= entries.len() {
             return Ok(Vec::new());
         }

         Ok(entries.split_off(offset as usize))
     }

     fn create(&self, _path: &str, _mode: u16) -> Result<u64, StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn mkdir(&self, _path: &str, _mode: u16) -> Result<(), StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn unlink(&self, _path: &str) -> Result<(), StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn rmdir(&self, _path: &str) -> Result<(), StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn rename(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn symlink(&self, _target: &str, _link: &str) -> Result<(), StorageError> {
         Err(StorageError::Unsupported)
     }

     fn readlink(&self, _path: &str) -> Result<String, StorageError> {
         Err(StorageError::Unsupported)
     }

     fn link(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
         Err(StorageError::Unsupported)
     }

     fn setattr(&self, _path: &str, _attr: &FileMetadata) -> Result<(), StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn open(&self, path: &str, flags: OpenFlags) -> Result<u64, StorageError> {
         let entry = self.resolve_path(path)?;
         let is_dir = entry.is_dir();

         if is_dir && !flags.contains(OpenFlags::DIRECTORY) {
             return Err(StorageError::NotAFile);
         }
         if !is_dir && flags.contains(OpenFlags::DIRECTORY) {
             return Err(StorageError::NotADirectory);
         }

         self.alloc_handle(OpenFile {
             first_cluster: entry.first_cluster(),
             size: entry.file_size as u64,
             is_dir,
         })
     }

     fn close(&self, handle: u64) -> Result<(), StorageError> {
         self.free_handle(handle)
     }

     fn read(&self, handle: u64, offset: u64, buffer: &mut [u8]) -> Result<usize, StorageError> {
         let file = self.get_handle(handle)?;
         self.read_file_data(file, offset, buffer)
     }

     fn write(&self, _handle: u64, _offset: u64, _data: &[u8]) -> Result<usize, StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn flush(&self, _handle: u64) -> Result<(), StorageError> {
         Ok(())
     }

     fn fsync(&self, _handle: u64, _data_only: bool) -> Result<(), StorageError> {
         Ok(())
     }

     fn truncate(&self, _path: &str, _size: u64) -> Result<(), StorageError> {
         Err(StorageError::ReadOnly)
     }

     fn fallocate(&self, _handle: u64, _offset: u64, _len: u64) -> Result<(), StorageError> {
         Err(StorageError::Unsupported)
     }

     fn sync(&self) -> Result<(), StorageError> {
         self.device.flush()
     }
 }
