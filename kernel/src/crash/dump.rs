//! Crash Dump Generation
//!
//! Generates crash dumps for post-mortem analysis.

use alloc::string::String;
use alloc::vec::Vec;

use super::{CrashInfo, CrashType};

/// Crash dump format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpFormat {
    /// Minimal dump (registers and backtrace only)
    Minimal,
    /// Standard dump (includes some memory)
    Standard,
    /// Full dump (complete memory snapshot)
    Full,
}

impl Default for DumpFormat {
    fn default() -> Self {
        Self::Standard
    }
}

/// Crash dump
pub struct CrashDump {
    /// Header
    header: DumpHeader,
    /// Crash info
    crash_info: CrashInfo,
    /// Memory regions
    memory_regions: Vec<MemoryRegion>,
    /// Total size
    size: usize,
}

impl CrashDump {
    /// Create new crash dump
    pub fn new(crash_info: CrashInfo, format: DumpFormat) -> Self {
        let header = DumpHeader::new(format);
        
        Self {
            header,
            crash_info,
            memory_regions: Vec::new(),
            size: 0,
        }
    }

    /// Add memory region
    pub fn add_memory_region(&mut self, region: MemoryRegion) {
        self.size += region.data.len();
        self.memory_regions.push(region);
    }

    /// Capture stack memory
    pub fn capture_stack(&mut self, stack_top: u64, size: usize) {
        // Would copy stack memory
        let data = vec![0u8; size.min(0x10000)]; // Max 64KB stack
        
        self.add_memory_region(MemoryRegion {
            address: stack_top.saturating_sub(size as u64),
            size,
            region_type: MemoryRegionType::Stack,
            data,
        });
    }

    /// Capture kernel memory around crash address
    pub fn capture_crash_area(&mut self, address: u64) {
        // Would copy memory around crash address
        let size = 0x1000; // 4KB
        let data = vec![0u8; size];
        
        let aligned = address & !0xFFF;
        self.add_memory_region(MemoryRegion {
            address: aligned,
            size,
            region_type: MemoryRegionType::CrashArea,
            data,
        });
    }

    /// Get total size
    pub fn size(&self) -> usize {
        self.size + core::mem::size_of::<DumpHeader>()
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        
        // Header
        data.extend_from_slice(&self.header.magic.to_le_bytes());
        data.extend_from_slice(&self.header.version.to_le_bytes());
        data.extend_from_slice(&(self.header.format as u32).to_le_bytes());
        data.extend_from_slice(&self.header.timestamp.to_le_bytes());
        
        // Crash type
        data.extend_from_slice(&(self.crash_info.crash_type as u32).to_le_bytes());
        
        // Message length and data
        let msg_bytes = self.crash_info.message.as_bytes();
        data.extend_from_slice(&(msg_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(msg_bytes);
        
        // CPU state
        data.extend_from_slice(&self.crash_info.cpu_state.rip.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.rsp.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.rbp.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.rax.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.rbx.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.rcx.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.rdx.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.rflags.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.cr2.to_le_bytes());
        data.extend_from_slice(&self.crash_info.cpu_state.error_code.to_le_bytes());
        
        // Backtrace
        data.extend_from_slice(&(self.crash_info.backtrace.len() as u32).to_le_bytes());
        for frame in &self.crash_info.backtrace {
            data.extend_from_slice(&frame.address.to_le_bytes());
        }
        
        // Memory regions
        data.extend_from_slice(&(self.memory_regions.len() as u32).to_le_bytes());
        for region in &self.memory_regions {
            data.extend_from_slice(&region.address.to_le_bytes());
            data.extend_from_slice(&(region.size as u64).to_le_bytes());
            data.extend_from_slice(&(region.region_type as u32).to_le_bytes());
            data.extend_from_slice(&region.data);
        }
        
        data
    }

    /// Write to storage
    pub fn write_to_storage(&self) -> Result<String, DumpError> {
        // Would write to disk
        let filename = alloc::format!("crash_{}.dmp", self.header.timestamp);
        Ok(filename)
    }
}

/// Dump header
#[derive(Debug, Clone)]
pub struct DumpHeader {
    /// Magic number
    pub magic: u32,
    /// Version
    pub version: u32,
    /// Format
    pub format: DumpFormat,
    /// Timestamp
    pub timestamp: u64,
}

impl DumpHeader {
    /// Magic number for KPIO dumps
    pub const MAGIC: u32 = 0x4B50494F; // "KPIO"
    /// Current version
    pub const VERSION: u32 = 1;

    /// Create new header
    pub fn new(format: DumpFormat) -> Self {
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            format,
            timestamp: 0, // Would get current time
        }
    }

    /// Validate header
    pub fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC && self.version <= Self::VERSION
    }
}

/// Memory region in dump
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Start address
    pub address: u64,
    /// Size
    pub size: usize,
    /// Region type
    pub region_type: MemoryRegionType,
    /// Data
    pub data: Vec<u8>,
}

/// Memory region type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    /// Stack
    Stack,
    /// Heap
    Heap,
    /// Code
    Code,
    /// Area around crash
    CrashArea,
    /// Kernel data
    KernelData,
    /// Other
    Other,
}

/// Dump error
#[derive(Debug, Clone)]
pub enum DumpError {
    /// Storage error
    StorageError(String),
    /// No space
    NoSpace,
    /// Write failed
    WriteFailed,
}

/// Dump configuration
#[derive(Debug, Clone)]
pub struct DumpConfig {
    /// Dump format
    pub format: DumpFormat,
    /// Max dump size
    pub max_size: usize,
    /// Include all threads
    pub include_all_threads: bool,
    /// Dump location
    pub location: String,
}

impl Default for DumpConfig {
    fn default() -> Self {
        Self {
            format: DumpFormat::Standard,
            max_size: 64 * 1024 * 1024, // 64 MB
            include_all_threads: true,
            location: "/var/crash".into(),
        }
    }
}

use alloc::string::ToString;
use alloc::vec;
