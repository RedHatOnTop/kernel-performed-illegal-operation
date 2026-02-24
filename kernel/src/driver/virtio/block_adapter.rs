use crate::driver::virtio::block;
use storage::driver::BlockDevice;
use storage::{BlockDeviceInfo, StorageError};

/// Thin adapter that exposes kernel VirtIO block devices to the storage crate.
pub struct KernelBlockAdapter {
    pub device_index: usize,
}

impl KernelBlockAdapter {
    pub const fn new(device_index: usize) -> Self {
        Self { device_index }
    }
}

impl BlockDevice for KernelBlockAdapter {
    fn info(&self) -> BlockDeviceInfo {
        let mut name = [0u8; 32];
        let label = if self.device_index == 0 {
            b"virtio-blk0".as_slice()
        } else {
            b"virtio-blk1".as_slice()
        };
        name[..label.len()].copy_from_slice(label);

        let info = block::device_info();
        let total_blocks = info
            .iter()
            .find_map(|(idx, sectors, _)| {
                if *idx == self.device_index {
                    Some(*sectors)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        BlockDeviceInfo {
            name,
            name_len: label.len(),
            block_size: block::BLOCK_SIZE as u32,
            total_blocks,
            read_only: false,
            supports_trim: false,
            optimal_io_size: 1,
            physical_block_size: block::BLOCK_SIZE as u32,
        }
    }

    fn read_blocks(&self, start_block: u64, buffer: &mut [u8]) -> Result<usize, StorageError> {
        if buffer.is_empty() {
            return Ok(0);
        }

        if !buffer.len().is_multiple_of(block::BLOCK_SIZE) {
            return Err(StorageError::BufferTooSmall);
        }

        for (i, chunk) in buffer.chunks_exact_mut(block::BLOCK_SIZE).enumerate() {
            let sector = start_block + i as u64;
            let mut sector_buf = [0u8; block::BLOCK_SIZE];
            if !block::read_sector(self.device_index, sector, &mut sector_buf) {
                return Err(StorageError::IoError);
            }
            chunk.copy_from_slice(&sector_buf);
        }

        Ok(buffer.len())
    }

    fn write_blocks(&self, start_block: u64, data: &[u8]) -> Result<usize, StorageError> {
        if data.is_empty() {
            return Ok(0);
        }

        if !data.len().is_multiple_of(block::BLOCK_SIZE) {
            return Err(StorageError::InvalidArgument);
        }

        for (i, chunk) in data.chunks_exact(block::BLOCK_SIZE).enumerate() {
            let sector = start_block + i as u64;
            let mut sector_buf = [0u8; block::BLOCK_SIZE];
            sector_buf.copy_from_slice(chunk);
            if !block::write_sector(self.device_index, sector, &sector_buf) {
                return Err(StorageError::IoError);
            }
        }

        Ok(data.len())
    }

    fn flush(&self) -> Result<(), StorageError> {
        Ok(())
    }

    fn discard(&self, _start_block: u64, _num_blocks: u64) -> Result<(), StorageError> {
        Err(StorageError::Unsupported)
    }

    fn is_ready(&self) -> bool {
        self.device_index < block::device_count()
    }
}
