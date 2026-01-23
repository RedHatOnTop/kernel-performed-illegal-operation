//! ELF64 Parser and Loader
//!
//! Implements ELF64 binary loading for x86_64 userspace processes.

use alloc::vec::Vec;
use alloc::string::String;
use core::mem::size_of;

/// ELF magic number: 0x7F 'E' 'L' 'F'
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// ELF class: 64-bit
pub const ELFCLASS64: u8 = 2;

/// ELF data encoding: little endian
pub const ELFDATA2LSB: u8 = 1;

/// ELF type: executable
pub const ET_EXEC: u16 = 2;

/// ELF type: shared object (PIE)
pub const ET_DYN: u16 = 3;

/// Machine type: x86_64
pub const EM_X86_64: u16 = 62;

/// Program header type: loadable segment
pub const PT_LOAD: u32 = 1;

/// Program header type: dynamic linking info
pub const PT_DYNAMIC: u32 = 2;

/// Program header type: interpreter path
pub const PT_INTERP: u32 = 3;

/// Program header type: program header table
pub const PT_PHDR: u32 = 6;

/// Segment permission: executable
pub const PF_X: u32 = 1;

/// Segment permission: writable
pub const PF_W: u32 = 2;

/// Segment permission: readable
pub const PF_R: u32 = 4;

/// ELF64 file header
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Elf64Header {
    /// Magic number and other info
    pub e_ident: [u8; 16],
    /// Object file type
    pub e_type: u16,
    /// Machine type
    pub e_machine: u16,
    /// Object file version
    pub e_version: u32,
    /// Entry point virtual address
    pub e_entry: u64,
    /// Program header table file offset
    pub e_phoff: u64,
    /// Section header table file offset
    pub e_shoff: u64,
    /// Processor-specific flags
    pub e_flags: u32,
    /// ELF header size
    pub e_ehsize: u16,
    /// Program header table entry size
    pub e_phentsize: u16,
    /// Program header table entry count
    pub e_phnum: u16,
    /// Section header table entry size
    pub e_shentsize: u16,
    /// Section header table entry count
    pub e_shnum: u16,
    /// Section name string table index
    pub e_shstrndx: u16,
}

/// ELF64 program header
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Elf64ProgramHeader {
    /// Segment type
    pub p_type: u32,
    /// Segment flags
    pub p_flags: u32,
    /// Segment file offset
    pub p_offset: u64,
    /// Segment virtual address
    pub p_vaddr: u64,
    /// Segment physical address (unused)
    pub p_paddr: u64,
    /// Segment size in file
    pub p_filesz: u64,
    /// Segment size in memory
    pub p_memsz: u64,
    /// Segment alignment
    pub p_align: u64,
}

/// ELF64 section header
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Elf64SectionHeader {
    /// Section name (string table index)
    pub sh_name: u32,
    /// Section type
    pub sh_type: u32,
    /// Section flags
    pub sh_flags: u64,
    /// Section virtual address
    pub sh_addr: u64,
    /// Section file offset
    pub sh_offset: u64,
    /// Section size
    pub sh_size: u64,
    /// Link to another section
    pub sh_link: u32,
    /// Additional section information
    pub sh_info: u32,
    /// Section alignment
    pub sh_addralign: u64,
    /// Entry size if section holds table
    pub sh_entsize: u64,
}

/// Memory segment to be loaded
#[derive(Debug, Clone)]
pub struct LoadSegment {
    /// Virtual address where segment should be loaded
    pub vaddr: u64,
    /// Size of segment in memory
    pub mem_size: u64,
    /// File offset of segment data
    pub file_offset: u64,
    /// Size of segment data in file
    pub file_size: u64,
    /// Segment flags (PF_R, PF_W, PF_X)
    pub flags: u32,
    /// Alignment requirement
    pub align: u64,
}

impl LoadSegment {
    /// Check if segment is readable
    pub fn is_readable(&self) -> bool {
        self.flags & PF_R != 0
    }

    /// Check if segment is writable
    pub fn is_writable(&self) -> bool {
        self.flags & PF_W != 0
    }

    /// Check if segment is executable
    pub fn is_executable(&self) -> bool {
        self.flags & PF_X != 0
    }
}

/// Loaded ELF program information
#[derive(Debug)]
pub struct LoadedProgram {
    /// Entry point address
    pub entry_point: u64,
    /// Base address (for PIE)
    pub base_address: u64,
    /// Program header virtual address
    pub phdr_vaddr: Option<u64>,
    /// Number of program headers
    pub phdr_count: u16,
    /// Segments to load
    pub segments: Vec<LoadSegment>,
    /// Minimum virtual address
    pub min_vaddr: u64,
    /// Maximum virtual address (end of last segment)
    pub max_vaddr: u64,
    /// Total memory size required
    pub total_size: u64,
    /// Is position independent executable
    pub is_pie: bool,
    /// Interpreter path (if any)
    pub interpreter: Option<String>,
}

/// ELF loading errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    /// Binary too small to contain ELF header
    TooSmall,
    /// Invalid ELF magic number
    InvalidMagic,
    /// Invalid ELF class (not 64-bit)
    InvalidClass,
    /// Invalid data encoding (not little endian)
    InvalidEncoding,
    /// Invalid ELF version
    InvalidVersion,
    /// Invalid ELF type (not executable or shared object)
    InvalidType,
    /// Invalid machine type (not x86_64)
    InvalidMachine,
    /// Invalid program header offset
    InvalidPhoff,
    /// Invalid program header size
    InvalidPhentsize,
    /// Invalid program header count
    InvalidPhnum,
    /// Segment extends beyond file
    SegmentOutOfBounds,
    /// Segment has invalid alignment
    InvalidAlignment,
    /// Overlapping segments
    OverlappingSegments,
    /// No loadable segments
    NoLoadableSegments,
    /// Memory size smaller than file size
    InvalidMemSize,
    /// Interpreter path too long or invalid
    InvalidInterpreter,
}

/// ELF64 loader
pub struct Elf64Loader;

impl Elf64Loader {
    /// Parse and validate an ELF64 binary
    ///
    /// # Arguments
    ///
    /// * `binary` - Raw bytes of the ELF file
    ///
    /// # Returns
    ///
    /// * `Ok(LoadedProgram)` - Successfully parsed program info
    /// * `Err(ElfError)` - Parsing or validation error
    pub fn parse(binary: &[u8]) -> Result<LoadedProgram, ElfError> {
        // Check minimum size
        if binary.len() < size_of::<Elf64Header>() {
            return Err(ElfError::TooSmall);
        }

        // Parse ELF header
        let header = Self::parse_header(binary)?;
        
        // Validate header
        Self::validate_header(&header)?;

        // Parse program headers
        let segments = Self::parse_program_headers(binary, &header)?;

        if segments.is_empty() {
            return Err(ElfError::NoLoadableSegments);
        }

        // Calculate address range
        let min_vaddr = segments.iter().map(|s| s.vaddr).min().unwrap_or(0);
        let max_vaddr = segments
            .iter()
            .map(|s| s.vaddr + s.mem_size)
            .max()
            .unwrap_or(0);
        let total_size = max_vaddr - min_vaddr;

        // Check for PIE
        let is_pie = header.e_type == ET_DYN;

        // Find program header location in memory
        let phdr_vaddr = Self::find_phdr_vaddr(binary, &header)?;

        // Check for interpreter
        let interpreter = Self::find_interpreter(binary, &header)?;

        Ok(LoadedProgram {
            entry_point: header.e_entry,
            base_address: 0, // Set during loading
            phdr_vaddr,
            phdr_count: header.e_phnum,
            segments,
            min_vaddr,
            max_vaddr,
            total_size,
            is_pie,
            interpreter,
        })
    }

    /// Parse ELF header from raw bytes
    fn parse_header(binary: &[u8]) -> Result<Elf64Header, ElfError> {
        if binary.len() < size_of::<Elf64Header>() {
            return Err(ElfError::TooSmall);
        }

        // SAFETY: We've verified the size, and Elf64Header is repr(C, packed)
        let header: Elf64Header = unsafe {
            core::ptr::read_unaligned(binary.as_ptr() as *const Elf64Header)
        };

        Ok(header)
    }

    /// Validate ELF header
    fn validate_header(header: &Elf64Header) -> Result<(), ElfError> {
        // Check magic
        if header.e_ident[0..4] != ELF_MAGIC {
            return Err(ElfError::InvalidMagic);
        }

        // Check class (must be 64-bit)
        if header.e_ident[4] != ELFCLASS64 {
            return Err(ElfError::InvalidClass);
        }

        // Check data encoding (must be little endian)
        if header.e_ident[5] != ELFDATA2LSB {
            return Err(ElfError::InvalidEncoding);
        }

        // Check version
        if header.e_ident[6] != 1 {
            return Err(ElfError::InvalidVersion);
        }

        // Check type (must be executable or shared object/PIE)
        if header.e_type != ET_EXEC && header.e_type != ET_DYN {
            return Err(ElfError::InvalidType);
        }

        // Check machine (must be x86_64)
        if header.e_machine != EM_X86_64 {
            return Err(ElfError::InvalidMachine);
        }

        // Check program header size
        if header.e_phentsize != size_of::<Elf64ProgramHeader>() as u16 {
            return Err(ElfError::InvalidPhentsize);
        }

        Ok(())
    }

    /// Parse program headers and extract loadable segments
    fn parse_program_headers(
        binary: &[u8],
        header: &Elf64Header,
    ) -> Result<Vec<LoadSegment>, ElfError> {
        let phoff = header.e_phoff as usize;
        let phentsize = header.e_phentsize as usize;
        let phnum = header.e_phnum as usize;

        // Validate program header table bounds
        let ph_table_end = phoff
            .checked_add(phnum.checked_mul(phentsize).ok_or(ElfError::InvalidPhnum)?)
            .ok_or(ElfError::InvalidPhoff)?;

        if ph_table_end > binary.len() {
            return Err(ElfError::InvalidPhoff);
        }

        let mut segments = Vec::new();

        for i in 0..phnum {
            let ph_offset = phoff + i * phentsize;
            
            // SAFETY: We've validated bounds above
            let ph: Elf64ProgramHeader = unsafe {
                core::ptr::read_unaligned(
                    binary.as_ptr().add(ph_offset) as *const Elf64ProgramHeader
                )
            };

            // Only process LOAD segments
            if ph.p_type != PT_LOAD {
                continue;
            }

            // Validate segment
            Self::validate_segment(&ph, binary.len())?;

            segments.push(LoadSegment {
                vaddr: ph.p_vaddr,
                mem_size: ph.p_memsz,
                file_offset: ph.p_offset,
                file_size: ph.p_filesz,
                flags: ph.p_flags,
                align: ph.p_align,
            });
        }

        // Sort segments by virtual address
        segments.sort_by_key(|s| s.vaddr);

        // Check for overlapping segments
        for i in 1..segments.len() {
            let prev_end = segments[i - 1].vaddr + segments[i - 1].mem_size;
            if segments[i].vaddr < prev_end {
                return Err(ElfError::OverlappingSegments);
            }
        }

        Ok(segments)
    }

    /// Validate a single segment
    fn validate_segment(ph: &Elf64ProgramHeader, file_size: usize) -> Result<(), ElfError> {
        // Memory size must be at least file size
        if ph.p_memsz < ph.p_filesz {
            return Err(ElfError::InvalidMemSize);
        }

        // Segment data must be within file
        let segment_end = ph
            .p_offset
            .checked_add(ph.p_filesz)
            .ok_or(ElfError::SegmentOutOfBounds)?;

        if segment_end > file_size as u64 {
            return Err(ElfError::SegmentOutOfBounds);
        }

        // Alignment must be a power of 2 (or 0/1)
        if ph.p_align > 1 && !ph.p_align.is_power_of_two() {
            return Err(ElfError::InvalidAlignment);
        }

        Ok(())
    }

    /// Find PT_PHDR segment to get program header virtual address
    fn find_phdr_vaddr(binary: &[u8], header: &Elf64Header) -> Result<Option<u64>, ElfError> {
        let phoff = header.e_phoff as usize;
        let phentsize = header.e_phentsize as usize;
        let phnum = header.e_phnum as usize;

        for i in 0..phnum {
            let ph_offset = phoff + i * phentsize;
            let ph: Elf64ProgramHeader = unsafe {
                core::ptr::read_unaligned(
                    binary.as_ptr().add(ph_offset) as *const Elf64ProgramHeader
                )
            };

            if ph.p_type == PT_PHDR {
                return Ok(Some(ph.p_vaddr));
            }
        }

        Ok(None)
    }

    /// Find PT_INTERP segment to get interpreter path
    fn find_interpreter(binary: &[u8], header: &Elf64Header) -> Result<Option<String>, ElfError> {
        let phoff = header.e_phoff as usize;
        let phentsize = header.e_phentsize as usize;
        let phnum = header.e_phnum as usize;

        for i in 0..phnum {
            let ph_offset = phoff + i * phentsize;
            let ph: Elf64ProgramHeader = unsafe {
                core::ptr::read_unaligned(
                    binary.as_ptr().add(ph_offset) as *const Elf64ProgramHeader
                )
            };

            if ph.p_type == PT_INTERP {
                let offset = ph.p_offset as usize;
                let size = ph.p_filesz as usize;

                if offset + size > binary.len() {
                    return Err(ElfError::InvalidInterpreter);
                }

                // Interpreter path should be null-terminated
                let path_bytes = &binary[offset..offset + size];
                let path_len = path_bytes.iter().position(|&b| b == 0).unwrap_or(size);

                if path_len == 0 || path_len > 255 {
                    return Err(ElfError::InvalidInterpreter);
                }

                let path = core::str::from_utf8(&path_bytes[..path_len])
                    .map_err(|_| ElfError::InvalidInterpreter)?;

                return Ok(Some(String::from(path)));
            }
        }

        Ok(None)
    }

    /// Get segment data from binary
    pub fn get_segment_data<'a>(binary: &'a [u8], segment: &LoadSegment) -> &'a [u8] {
        let start = segment.file_offset as usize;
        let end = start + segment.file_size as usize;
        &binary[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid ELF64 header for x86_64
    fn create_minimal_elf() -> Vec<u8> {
        let mut elf = vec![0u8; 120]; // Minimum ELF + one program header
        
        // ELF header
        elf[0..4].copy_from_slice(&ELF_MAGIC);
        elf[4] = ELFCLASS64;      // 64-bit
        elf[5] = ELFDATA2LSB;     // Little endian
        elf[6] = 1;               // ELF version
        elf[16..18].copy_from_slice(&ET_EXEC.to_le_bytes()); // Type: executable
        elf[18..20].copy_from_slice(&EM_X86_64.to_le_bytes()); // Machine: x86_64
        elf[20..24].copy_from_slice(&1u32.to_le_bytes()); // Version
        elf[24..32].copy_from_slice(&0x400000u64.to_le_bytes()); // Entry point
        elf[32..40].copy_from_slice(&64u64.to_le_bytes()); // Program header offset
        elf[52..54].copy_from_slice(&64u16.to_le_bytes()); // ELF header size
        elf[54..56].copy_from_slice(&56u16.to_le_bytes()); // Program header entry size
        elf[56..58].copy_from_slice(&1u16.to_le_bytes());  // Program header count
        
        // Program header (PT_LOAD)
        elf[64..68].copy_from_slice(&PT_LOAD.to_le_bytes()); // Type
        elf[68..72].copy_from_slice(&(PF_R | PF_X).to_le_bytes()); // Flags
        elf[72..80].copy_from_slice(&0u64.to_le_bytes()); // Offset
        elf[80..88].copy_from_slice(&0x400000u64.to_le_bytes()); // Virtual address
        elf[88..96].copy_from_slice(&0x400000u64.to_le_bytes()); // Physical address
        elf[96..104].copy_from_slice(&120u64.to_le_bytes()); // File size
        elf[104..112].copy_from_slice(&120u64.to_le_bytes()); // Memory size
        elf[112..120].copy_from_slice(&0x1000u64.to_le_bytes()); // Alignment
        
        elf
    }

    #[test]
    fn test_parse_minimal_elf() {
        let elf = create_minimal_elf();
        let result = Elf64Loader::parse(&elf);
        assert!(result.is_ok());
        
        let program = result.unwrap();
        assert_eq!(program.entry_point, 0x400000);
        assert_eq!(program.segments.len(), 1);
        assert!(program.segments[0].is_readable());
        assert!(program.segments[0].is_executable());
        assert!(!program.segments[0].is_writable());
    }

    #[test]
    fn test_invalid_magic() {
        let mut elf = create_minimal_elf();
        elf[0] = 0x00; // Corrupt magic
        
        let result = Elf64Loader::parse(&elf);
        assert_eq!(result, Err(ElfError::InvalidMagic));
    }

    #[test]
    fn test_too_small() {
        let elf = vec![0x7F, b'E', b'L', b'F'];
        let result = Elf64Loader::parse(&elf);
        assert_eq!(result, Err(ElfError::TooSmall));
    }
}
