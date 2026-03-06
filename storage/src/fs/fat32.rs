//! FAT32 filesystem implementation with read and write support.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
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
    /// Start cluster of the parent directory containing this file's dir entry.
    parent_dir_cluster: u32,
    /// Byte offset of the 32-byte dir entry within the parent directory's cluster chain.
    dir_entry_chain_offset: u32,
}

pub struct Fat32Filesystem {
    device: &'static dyn BlockDevice,
    bpb: Fat32Bpb,
    read_only: bool,
    free_clusters: AtomicU32,
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
            read_only: false,
            free_clusters: AtomicU32::new(0),
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

     // ── Write support helpers ───────────────────────────────────────

     fn write_sector(&self, sector: u32, data: &[u8]) -> Result<(), StorageError> {
         if self.read_only {
             return Err(StorageError::ReadOnly);
         }
         self.device.write_blocks(sector as u64, data)?;
         Ok(())
     }

     fn write_fat_entry(&self, cluster: u32, value: u32) -> Result<(), StorageError> {
         let fat_offset = cluster as usize * 4;
         let bps = self.bps();
         let fat_sector = self.bpb.reserved_sectors as u32 + (fat_offset / bps) as u32;
         let entry_offset = fat_offset % bps;

         let mut sector_buf = vec![0u8; bps];
         self.read_sector(fat_sector, &mut sector_buf)?;

         // Preserve upper 4 bits of existing entry per FAT32 spec.
         let existing_hi = sector_buf[entry_offset + 3] & 0xF0;
         let bytes = value.to_le_bytes();
         sector_buf[entry_offset] = bytes[0];
         sector_buf[entry_offset + 1] = bytes[1];
         sector_buf[entry_offset + 2] = bytes[2];
         sector_buf[entry_offset + 3] = (bytes[3] & 0x0F) | existing_hi;

         self.write_sector(fat_sector, &sector_buf)?;

         // Mirror to backup FAT if present.
         if self.bpb.num_fats == 2 {
             let backup_sector = fat_sector + self.bpb.fat_size_32;
             self.write_sector(backup_sector, &sector_buf)?;
         }

         Ok(())
     }

     fn alloc_cluster(&self) -> Result<u32, StorageError> {
         let total = self.bpb.total_clusters();
         for cluster in 2..total + 2 {
             let entry = self.read_fat_entry(cluster)?;
             if entry == 0 {
                 // Mark as end-of-chain.
                 self.write_fat_entry(cluster, 0x0FFF_FFFF)?;
                 self.free_clusters.fetch_sub(1, Ordering::Relaxed);

                 // Zero the new cluster.
                 let zero_buf = vec![0u8; self.bps()];
                 let first_sector = self.bpb.cluster_to_sector(cluster);
                 for i in 0..self.bpb.sectors_per_cluster as u32 {
                     self.write_sector(first_sector + i, &zero_buf)?;
                 }
                 return Ok(cluster);
             }
         }
         Err(StorageError::NoSpace)
     }

     fn extend_chain(&self, last_cluster: u32, new_cluster: u32) -> Result<(), StorageError> {
         self.write_fat_entry(last_cluster, new_cluster)
     }

     fn free_chain(&self, start_cluster: u32) -> Result<(), StorageError> {
         if start_cluster < 2 {
             return Ok(());
         }
         let mut cluster = start_cluster;
         for _ in 0..0x10_0000 {
             let next = self.read_fat_entry(cluster)?;
             self.write_fat_entry(cluster, 0)?;
             self.free_clusters.fetch_add(1, Ordering::Relaxed);
             if next >= fat_entry::EOC_MIN || next < 2 {
                 break;
             }
             cluster = next;
         }
         Ok(())
     }

     /// Split a path into (parent_dir, filename).
     fn split_parent_name(path: &str) -> Result<(&str, &str), StorageError> {
         let path = path.trim_end_matches('/');
         if path.is_empty() || path == "/" {
             return Err(StorageError::InvalidPath);
         }
         if let Some(pos) = path.rfind('/') {
             let parent = if pos == 0 { "/" } else { &path[..pos] };
             let name = &path[pos + 1..];
             if name.is_empty() {
                 return Err(StorageError::InvalidName);
             }
             Ok((parent, name))
         } else {
             Ok(("/", path))
         }
     }

     /// Convert a filename to FAT32 8.3 short name format (uppercase, space-padded).
     fn make_short_name(name: &str) -> Result<[u8; 11], StorageError> {
         if name.is_empty() || name.len() > 12 {
             return Err(StorageError::InvalidName);
         }
         let mut result = [b' '; 11];
         let (base, ext) = if let Some(dot_pos) = name.rfind('.') {
             (&name[..dot_pos], &name[dot_pos + 1..])
         } else {
             (name, "")
         };
         if base.is_empty() || base.len() > 8 || ext.len() > 3 {
             return Err(StorageError::InvalidName);
         }
         for (i, &b) in base.as_bytes().iter().enumerate().take(8) {
             result[i] = b.to_ascii_uppercase();
         }
         for (i, &b) in ext.as_bytes().iter().enumerate().take(3) {
             result[8 + i] = b.to_ascii_uppercase();
         }
         Ok(result)
     }

     /// Get the start cluster of a directory given its path.
     fn parent_cluster(&self, parent_path: &str) -> Result<u32, StorageError> {
         if parent_path == "/" {
             Ok(self.bpb.root_cluster)
         } else {
             let entry = self.resolve_path(parent_path)?;
             if !entry.is_dir() {
                 return Err(StorageError::NotADirectory);
             }
             Ok(entry.first_cluster())
         }
     }

     /// Compare two 11-byte FAT short names (case-insensitive).
     fn names_match(a: &[u8; 11], b: &[u8; 11]) -> bool {
         for i in 0..11 {
             if a[i].to_ascii_uppercase() != b[i].to_ascii_uppercase() {
                 return false;
             }
         }
         true
     }

     /// Find a directory entry by short name within a directory cluster chain.
     /// Returns (entry, byte_offset_within_chain).
     fn find_entry_in_dir(
         &self,
         dir_cluster: u32,
         target: &[u8; 11],
     ) -> Result<(Fat32DirEntry, u32), StorageError> {
         let mut cluster = dir_cluster;
         let cluster_size = self.cluster_size();
         let entries_per_cluster = cluster_size / Fat32DirEntry::SIZE;
         let mut chain_offset: u32 = 0;

         for _ in 0..4096 {
             let mut cluster_buf = vec![0u8; cluster_size];
             self.read_cluster(cluster, &mut cluster_buf)?;

             for i in 0..entries_per_cluster {
                 let offset = i * Fat32DirEntry::SIZE;
                 // SAFETY: Reading a packed 32-byte struct from an aligned buffer slice.
                 let entry: Fat32DirEntry = unsafe {
                     core::ptr::read_unaligned(cluster_buf[offset..].as_ptr() as *const _)
                 };
                 if entry.is_last() {
                     return Err(StorageError::FileNotFound);
                 }
                 if entry.is_free() || entry.is_long_name() || entry.is_volume_label() {
                     continue;
                 }
                 if Self::names_match(&entry.name, target) {
                     return Ok((entry, chain_offset + offset as u32));
                 }
             }

             chain_offset += cluster_size as u32;
             let next = self.read_fat_entry(cluster)?;
             if next >= fat_entry::EOC_MIN || next == fat_entry::BAD {
                 break;
             }
             if next < 2 {
                 return Err(StorageError::Corrupted);
             }
             cluster = next;
         }

         Err(StorageError::FileNotFound)
     }

     /// Find a free 32-byte slot in a directory's cluster chain.
     /// Returns the byte offset within the chain. Extends the chain if full.
     fn find_free_dir_slot(&self, dir_cluster: u32) -> Result<u32, StorageError> {
         let mut cluster = dir_cluster;
         let cluster_size = self.cluster_size();
         let entries_per_cluster = cluster_size / Fat32DirEntry::SIZE;
         let mut chain_offset: u32 = 0;

         for _ in 0..4096 {
             let mut cluster_buf = vec![0u8; cluster_size];
             self.read_cluster(cluster, &mut cluster_buf)?;

             for i in 0..entries_per_cluster {
                 let offset = i * Fat32DirEntry::SIZE;
                 let first_byte = cluster_buf[offset];
                 if first_byte == Fat32DirEntry::DELETED || first_byte == Fat32DirEntry::LAST {
                     return Ok(chain_offset + offset as u32);
                 }
             }

             chain_offset += cluster_size as u32;
             let next = self.read_fat_entry(cluster)?;
             if next >= fat_entry::EOC_MIN {
                 // Directory full — extend with a new cluster.
                 let new_cluster = self.alloc_cluster()?;
                 self.extend_chain(cluster, new_cluster)?;
                 return Ok(chain_offset);
             }
             if next < 2 {
                 return Err(StorageError::Corrupted);
             }
             cluster = next;
         }

         Err(StorageError::NoSpace)
     }

     /// Convert a chain byte-offset to (actual_cluster, disk_sector, offset_within_sector).
     fn chain_offset_to_sector(
         &self,
         start_cluster: u32,
         chain_byte_offset: u32,
     ) -> Result<(u32, u32, usize), StorageError> {
         let cluster_size = self.cluster_size();
         let cluster_index = chain_byte_offset as usize / cluster_size;
         let intra = chain_byte_offset as usize % cluster_size;

         let mut cluster = start_cluster;
         for _ in 0..cluster_index {
             let next = self.read_fat_entry(cluster)?;
             if next >= fat_entry::EOC_MIN || next < 2 {
                 return Err(StorageError::Corrupted);
             }
             cluster = next;
         }

         let bps = self.bps();
         let sector_in_cluster = intra / bps;
         let offset_in_sector = intra % bps;
         let sector = self.bpb.cluster_to_sector(cluster) + sector_in_cluster as u32;

         Ok((cluster, sector, offset_in_sector))
     }

     /// Write a raw 32-byte directory entry at the given chain byte-offset.
     fn write_dir_entry_at(
         &self,
         dir_cluster: u32,
         chain_byte_offset: u32,
         entry: &Fat32DirEntry,
     ) -> Result<(), StorageError> {
         let (_cluster, sector, offset_in_sector) =
             self.chain_offset_to_sector(dir_cluster, chain_byte_offset)?;

         let bps = self.bps();
         let mut sector_buf = vec![0u8; bps];
         self.read_sector(sector, &mut sector_buf)?;

         // SAFETY: Reinterpreting a packed repr(C) struct as raw bytes.
         let entry_bytes: &[u8] = unsafe {
             core::slice::from_raw_parts(
                 entry as *const Fat32DirEntry as *const u8,
                 Fat32DirEntry::SIZE,
             )
         };
         sector_buf[offset_in_sector..offset_in_sector + Fat32DirEntry::SIZE]
             .copy_from_slice(entry_bytes);

         self.write_sector(sector, &sector_buf)
     }

     /// Update the first-cluster field of a directory entry on disk.
     fn update_dir_entry_cluster(
         &self,
         dir_cluster: u32,
         chain_byte_offset: u32,
         first_cluster: u32,
     ) -> Result<(), StorageError> {
         let (_cluster, sector, ofs) =
             self.chain_offset_to_sector(dir_cluster, chain_byte_offset)?;

         let bps = self.bps();
         let mut sector_buf = vec![0u8; bps];
         self.read_sector(sector, &mut sector_buf)?;

         // fst_clus_hi is at byte 20, fst_clus_lo is at byte 26 within the 32-byte entry.
         sector_buf[ofs + 20..ofs + 22]
             .copy_from_slice(&((first_cluster >> 16) as u16).to_le_bytes());
         sector_buf[ofs + 26..ofs + 28]
             .copy_from_slice(&((first_cluster & 0xFFFF) as u16).to_le_bytes());

         self.write_sector(sector, &sector_buf)
     }

     /// Update the file_size field of a directory entry on disk.
     fn update_dir_entry_size(
         &self,
         dir_cluster: u32,
         chain_byte_offset: u32,
         size: u32,
     ) -> Result<(), StorageError> {
         let (_cluster, sector, ofs) =
             self.chain_offset_to_sector(dir_cluster, chain_byte_offset)?;

         let bps = self.bps();
         let mut sector_buf = vec![0u8; bps];
         self.read_sector(sector, &mut sector_buf)?;

         // file_size is at byte 28 within the 32-byte entry.
         sector_buf[ofs + 28..ofs + 32].copy_from_slice(&size.to_le_bytes());

         self.write_sector(sector, &sector_buf)
     }

     /// Mark a directory entry as deleted (first byte = 0xE5) on disk.
     fn mark_dir_entry_deleted(
         &self,
         dir_cluster: u32,
         chain_byte_offset: u32,
     ) -> Result<(), StorageError> {
         let (_cluster, sector, ofs) =
             self.chain_offset_to_sector(dir_cluster, chain_byte_offset)?;

         let bps = self.bps();
         let mut sector_buf = vec![0u8; bps];
         self.read_sector(sector, &mut sector_buf)?;

         sector_buf[ofs] = Fat32DirEntry::DELETED;

         self.write_sector(sector, &sector_buf)
     }

     /// Count existing clusters in a chain and return (count, last_cluster).
     fn count_chain(&self, first_cluster: u32) -> Result<(u32, u32), StorageError> {
         if first_cluster < 2 {
             return Ok((0, 0));
         }
         let mut count = 0u32;
         let mut last = first_cluster;
         let mut cluster = first_cluster;
         for _ in 0..0x10_0000 {
             count += 1;
             last = cluster;
             let next = self.read_fat_entry(cluster)?;
             if next >= fat_entry::EOC_MIN || next < 2 {
                 break;
             }
             cluster = next;
         }
         Ok((count, last))
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
             free_clusters: AtomicU32::new(0),
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
             free_blocks: self.free_clusters.load(Ordering::Relaxed) as u64,
             available_blocks: self.free_clusters.load(Ordering::Relaxed) as u64,
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

     fn create(&self, path: &str, _mode: u16) -> Result<u64, StorageError> {
         if self.read_only {
             return Err(StorageError::ReadOnly);
         }
         // Fail if already exists.
         if self.resolve_path(path).is_ok() {
             return Err(StorageError::AlreadyExists);
         }

         let (parent_path, name) = Self::split_parent_name(path)?;
         let short_name = Self::make_short_name(name)?;
         let parent_cluster = self.parent_cluster(parent_path)?;

         let slot_offset = self.find_free_dir_slot(parent_cluster)?;

         let entry = Fat32DirEntry {
             name: short_name,
             attr: 0x20, // ARCHIVE
             nt_res: 0,
             crt_time_tenth: 0,
             crt_time: 0,
             crt_date: 0,
             lst_acc_date: 0,
             fst_clus_hi: 0,
             wrt_time: 0,
             wrt_date: 0,
             fst_clus_lo: 0,
             file_size: 0,
         };

         self.write_dir_entry_at(parent_cluster, slot_offset, &entry)?;

         // Return a synthetic inode: encode parent cluster + offset.
         Ok((parent_cluster as u64) << 32 | slot_offset as u64)
     }

     fn mkdir(&self, path: &str, _mode: u16) -> Result<(), StorageError> {
         if self.read_only {
             return Err(StorageError::ReadOnly);
         }
         if self.resolve_path(path).is_ok() {
             return Err(StorageError::AlreadyExists);
         }

         let (parent_path, name) = Self::split_parent_name(path)?;
         let short_name = Self::make_short_name(name)?;
         let parent_cluster = self.parent_cluster(parent_path)?;

         // Allocate a cluster for the new directory.
         let dir_cluster = self.alloc_cluster()?;

         // Create "." entry.
         let mut dot_name = [b' '; 11];
         dot_name[0] = b'.';
         let dot = Fat32DirEntry {
             name: dot_name,
             attr: Fat32DirEntry::ATTR_DIRECTORY,
             nt_res: 0,
             crt_time_tenth: 0,
             crt_time: 0,
             crt_date: 0,
             lst_acc_date: 0,
             fst_clus_hi: (dir_cluster >> 16) as u16,
             wrt_time: 0,
             wrt_date: 0,
             fst_clus_lo: (dir_cluster & 0xFFFF) as u16,
             file_size: 0,
         };
         self.write_dir_entry_at(dir_cluster, 0, &dot)?;

         // Create ".." entry.
         let mut dotdot_name = [b' '; 11];
         dotdot_name[0] = b'.';
         dotdot_name[1] = b'.';
         let parent_for_dotdot = if parent_cluster == self.bpb.root_cluster {
             0
         } else {
             parent_cluster
         };
         let dotdot = Fat32DirEntry {
             name: dotdot_name,
             attr: Fat32DirEntry::ATTR_DIRECTORY,
             nt_res: 0,
             crt_time_tenth: 0,
             crt_time: 0,
             crt_date: 0,
             lst_acc_date: 0,
             fst_clus_hi: (parent_for_dotdot >> 16) as u16,
             wrt_time: 0,
             wrt_date: 0,
             fst_clus_lo: (parent_for_dotdot & 0xFFFF) as u16,
             file_size: 0,
         };
         self.write_dir_entry_at(dir_cluster, Fat32DirEntry::SIZE as u32, &dotdot)?;

         // Add entry in parent directory.
         let slot_offset = self.find_free_dir_slot(parent_cluster)?;
         let parent_entry = Fat32DirEntry {
             name: short_name,
             attr: Fat32DirEntry::ATTR_DIRECTORY,
             nt_res: 0,
             crt_time_tenth: 0,
             crt_time: 0,
             crt_date: 0,
             lst_acc_date: 0,
             fst_clus_hi: (dir_cluster >> 16) as u16,
             wrt_time: 0,
             wrt_date: 0,
             fst_clus_lo: (dir_cluster & 0xFFFF) as u16,
             file_size: 0,
         };
         self.write_dir_entry_at(parent_cluster, slot_offset, &parent_entry)?;

         Ok(())
     }

     fn unlink(&self, path: &str) -> Result<(), StorageError> {
         if self.read_only {
             return Err(StorageError::ReadOnly);
         }

         let (parent_path, name) = Self::split_parent_name(path)?;
         let short_name = Self::make_short_name(name)?;
         let parent_cluster = self.parent_cluster(parent_path)?;

         let (entry, offset) = self.find_entry_in_dir(parent_cluster, &short_name)?;
         if entry.is_dir() {
             return Err(StorageError::NotAFile);
         }

         // Free the cluster chain.
         self.free_chain(entry.first_cluster())?;

         // Mark directory entry as deleted.
         self.mark_dir_entry_deleted(parent_cluster, offset)?;

         Ok(())
     }

     fn rmdir(&self, path: &str) -> Result<(), StorageError> {
         if self.read_only {
             return Err(StorageError::ReadOnly);
         }

         let (parent_path, name) = Self::split_parent_name(path)?;
         let short_name = Self::make_short_name(name)?;
         let parent_cluster = self.parent_cluster(parent_path)?;

         let (entry, offset) = self.find_entry_in_dir(parent_cluster, &short_name)?;
         if !entry.is_dir() {
             return Err(StorageError::NotADirectory);
         }

         // Check that directory is empty (only . and .. allowed).
         let children = self.read_dir_entries(entry.first_cluster())?;
         if !children.is_empty() {
             return Err(StorageError::DirectoryNotEmpty);
         }

         // Free the directory's cluster chain.
         self.free_chain(entry.first_cluster())?;

         // Mark the parent's dir entry as deleted.
         self.mark_dir_entry_deleted(parent_cluster, offset)?;

         Ok(())
     }

     fn rename(&self, _old: &str, _new: &str) -> Result<(), StorageError> {
         Err(StorageError::Unsupported)
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
         // Handle CREATE: if file does not exist and CREATE is requested, create it.
         let (entry, parent_cluster, entry_offset) = match self.resolve_path(path) {
             Ok(e) => {
                 // File exists — find its parent and offset for tracking.
                 let (parent_path, name) =
                     Self::split_parent_name(path).unwrap_or(("/", ""));
                 let pclus = self.parent_cluster(parent_path).unwrap_or(self.bpb.root_cluster);
                 let short = Self::make_short_name(name).unwrap_or([b' '; 11]);
                 let (_found, off) = self.find_entry_in_dir(pclus, &short).unwrap_or((e, 0));
                 (e, pclus, off)
             }
             Err(StorageError::FileNotFound) if flags.contains(OpenFlags::CREATE) => {
                 if self.read_only {
                     return Err(StorageError::ReadOnly);
                 }
                 self.create(path, 0o644)?;
                 let e = self.resolve_path(path)?;
                 let (parent_path, name) =
                     Self::split_parent_name(path).unwrap_or(("/", ""));
                 let pclus = self.parent_cluster(parent_path).unwrap_or(self.bpb.root_cluster);
                 let short = Self::make_short_name(name).unwrap_or([b' '; 11]);
                 let (_found, off) = self.find_entry_in_dir(pclus, &short).unwrap_or((e, 0));
                 (e, pclus, off)
             }
             Err(e) => return Err(e),
         };

         let is_dir = entry.is_dir();

         if is_dir && !flags.contains(OpenFlags::DIRECTORY) {
             return Err(StorageError::NotAFile);
         }
         if !is_dir && flags.contains(OpenFlags::DIRECTORY) {
             return Err(StorageError::NotADirectory);
         }

         // Handle TRUNCATE on existing file.
         let (first_cluster, size) = if flags.contains(OpenFlags::TRUNCATE) && !is_dir {
             if entry.first_cluster() >= 2 {
                 self.free_chain(entry.first_cluster())?;
                 self.update_dir_entry_cluster(parent_cluster, entry_offset, 0)?;
                 self.update_dir_entry_size(parent_cluster, entry_offset, 0)?;
             }
             (0, 0u64)
         } else {
             (entry.first_cluster(), entry.file_size as u64)
         };

         self.alloc_handle(OpenFile {
             first_cluster,
             size,
             is_dir,
             parent_dir_cluster: parent_cluster,
             dir_entry_chain_offset: entry_offset,
         })
     }

     fn close(&self, handle: u64) -> Result<(), StorageError> {
         self.free_handle(handle)
     }

     fn read(&self, handle: u64, offset: u64, buffer: &mut [u8]) -> Result<usize, StorageError> {
         let file = self.get_handle(handle)?;
         self.read_file_data(file, offset, buffer)
     }

     fn write(&self, handle: u64, offset: u64, data: &[u8]) -> Result<usize, StorageError> {
         if self.read_only {
             return Err(StorageError::ReadOnly);
         }
         if data.is_empty() {
             return Ok(0);
         }

         let mut files = self.open_files.write();
         let file = files
             .get_mut(handle as usize)
             .ok_or(StorageError::InvalidFd)?
             .as_mut()
             .ok_or(StorageError::InvalidFd)?;

         if file.is_dir {
             return Err(StorageError::NotAFile);
         }

         let write_end = offset + data.len() as u64;
         let cluster_size = self.cluster_size() as u64;
         let needed_clusters = ((write_end + cluster_size - 1) / cluster_size) as u32;

         // Count current clusters.
         let (mut current_clusters, mut last_cluster) = self.count_chain(file.first_cluster)?;

         // Allocate first cluster if file has none.
         if file.first_cluster < 2 && needed_clusters > 0 {
             let new_cluster = self.alloc_cluster()?;
             file.first_cluster = new_cluster;
             last_cluster = new_cluster;
             current_clusters = 1;
             self.update_dir_entry_cluster(
                 file.parent_dir_cluster,
                 file.dir_entry_chain_offset,
                 new_cluster,
             )?;
         }

         // Allocate additional clusters as needed.
         while current_clusters < needed_clusters {
             let new_cluster = self.alloc_cluster()?;
             self.extend_chain(last_cluster, new_cluster)?;
             last_cluster = new_cluster;
             current_clusters += 1;
         }

         // Navigate to the starting cluster for the write offset.
         let csize = self.cluster_size();
         let skip_clusters = (offset as usize) / csize;
         let mut cluster_offset = (offset as usize) % csize;

         let mut cluster = file.first_cluster;
         for _ in 0..skip_clusters {
             let next = self.read_fat_entry(cluster)?;
             if next >= fat_entry::EOC_MIN || next < 2 {
                 return Err(StorageError::IoError);
             }
             cluster = next;
         }

         // Write data sector-by-sector (read-modify-write).
         let bps = self.bps();
         let mut written = 0usize;

         while written < data.len() {
             let sector_in_cluster = cluster_offset / bps;
             let offset_in_sector = cluster_offset % bps;
             let sector = self.bpb.cluster_to_sector(cluster) + sector_in_cluster as u32;

             let mut sector_buf = vec![0u8; bps];
             self.read_sector(sector, &mut sector_buf)?;

             let available = bps - offset_in_sector;
             let to_write = core::cmp::min(available, data.len() - written);
             sector_buf[offset_in_sector..offset_in_sector + to_write]
                 .copy_from_slice(&data[written..written + to_write]);

             self.write_sector(sector, &sector_buf)?;
             written += to_write;
             cluster_offset += to_write;

             if cluster_offset >= csize {
                 cluster_offset = 0;
                 if written < data.len() {
                     let next = self.read_fat_entry(cluster)?;
                     if next >= fat_entry::EOC_MIN || next < 2 {
                         break;
                     }
                     cluster = next;
                 }
             }
         }

         // Update file size if the write extended the file.
         if write_end > file.size {
             file.size = write_end;
             self.update_dir_entry_size(
                 file.parent_dir_cluster,
                 file.dir_entry_chain_offset,
                 write_end as u32,
             )?;
         }

         Ok(written)
     }

     fn flush(&self, _handle: u64) -> Result<(), StorageError> {
         self.device.flush()
     }

     fn fsync(&self, _handle: u64, _data_only: bool) -> Result<(), StorageError> {
         self.device.flush()
     }

     fn truncate(&self, path: &str, size: u64) -> Result<(), StorageError> {
         if self.read_only {
             return Err(StorageError::ReadOnly);
         }

         let (parent_path, name) = Self::split_parent_name(path)?;
         let short_name = Self::make_short_name(name)?;
         let parent_cluster = self.parent_cluster(parent_path)?;

         let (entry, offset) = self.find_entry_in_dir(parent_cluster, &short_name)?;
         if entry.is_dir() {
             return Err(StorageError::NotAFile);
         }

         let cluster_size = self.cluster_size() as u64;
         let new_clusters_needed = if size == 0 {
             0u32
         } else {
             ((size + cluster_size - 1) / cluster_size) as u32
         };

         let (current_count, _last) = self.count_chain(entry.first_cluster())?;

         if new_clusters_needed == 0 && current_count > 0 {
             // Free entire chain.
             self.free_chain(entry.first_cluster())?;
             self.update_dir_entry_cluster(parent_cluster, offset, 0)?;
         } else if new_clusters_needed < current_count {
             // Walk to the cluster at position new_clusters_needed-1, then free the rest.
             let mut cluster = entry.first_cluster();
             for _ in 1..new_clusters_needed {
                 let next = self.read_fat_entry(cluster)?;
                 if next >= fat_entry::EOC_MIN || next < 2 {
                     break;
                 }
                 cluster = next;
             }
             let tail = self.read_fat_entry(cluster)?;
             self.write_fat_entry(cluster, 0x0FFF_FFFF)?; // Mark new last as EOC.
             if tail >= 2 && tail < fat_entry::EOC_MIN {
                 self.free_chain(tail)?;
             }
         }

         self.update_dir_entry_size(parent_cluster, offset, size as u32)?;
         Ok(())
     }

     fn fallocate(&self, _handle: u64, _offset: u64, _len: u64) -> Result<(), StorageError> {
         Err(StorageError::Unsupported)
     }

     fn sync(&self) -> Result<(), StorageError> {
         self.device.flush()
     }
 }
