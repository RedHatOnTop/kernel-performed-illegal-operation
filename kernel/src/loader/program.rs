//! User Program Management
//!
//! This module manages loaded userspace programs and their execution state.

use alloc::vec::Vec;
use alloc::string::String;
use super::elf::{LoadedProgram, LoadSegment, Elf64Loader, ElfError};

/// Userspace address space layout
pub mod layout {
    /// Start of userspace text segment (4MB)
    pub const USER_TEXT_START: u64 = 0x0000_0000_0040_0000;
    
    /// End of userspace address space
    pub const USER_SPACE_END: u64 = 0x0000_7FFF_FFFF_F000;
    
    /// Default stack top (grows down)
    pub const USER_STACK_TOP: u64 = 0x0000_7FFF_FFFF_F000;
    
    /// Default stack size (8MB)
    pub const USER_STACK_SIZE: u64 = 8 * 1024 * 1024;
    
    /// Default heap start (after program)
    pub const USER_HEAP_START: u64 = 0x0000_0000_1000_0000;
    
    /// Maximum heap size (1GB)
    pub const USER_HEAP_MAX_SIZE: u64 = 1024 * 1024 * 1024;
    
    /// PIE base address (randomizable in future)
    pub const PIE_BASE: u64 = 0x0000_5555_5555_0000;
}

/// Program execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramState {
    /// Program is loaded but not started
    Ready,
    /// Program is currently running
    Running,
    /// Program is waiting for I/O or event
    Blocked,
    /// Program has exited
    Exited(i32),
    /// Program was killed
    Killed,
}

/// Memory region in user address space
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Start virtual address
    pub start: u64,
    /// Size in bytes
    pub size: u64,
    /// Is readable
    pub readable: bool,
    /// Is writable
    pub writable: bool,
    /// Is executable
    pub executable: bool,
    /// Region type
    pub region_type: RegionType,
}

/// Type of memory region
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionType {
    /// Code segment
    Code,
    /// Read-only data
    Rodata,
    /// Writable data
    Data,
    /// BSS (zero-initialized)
    Bss,
    /// Stack
    Stack,
    /// Heap
    Heap,
    /// Memory-mapped file
    Mmap,
    /// Anonymous mapping
    Anonymous,
}

/// Loaded user program
#[derive(Debug)]
pub struct UserProgram {
    /// Program name
    pub name: String,
    /// Entry point address
    pub entry_point: u64,
    /// Base address (for PIE)
    pub base_address: u64,
    /// Stack pointer (initial)
    pub initial_sp: u64,
    /// Program break (heap end)
    pub brk: u64,
    /// Memory regions
    pub regions: Vec<MemoryRegion>,
    /// Current state
    pub state: ProgramState,
    /// Exit code (if exited)
    pub exit_code: Option<i32>,
    /// Arguments
    pub args: Vec<String>,
    /// Environment variables
    pub envp: Vec<String>,
    /// Auxiliary vector entries
    pub auxv: Vec<(u64, u64)>,
}

/// Auxiliary vector entry types (for ELF loading)
pub mod auxv {
    /// End of auxiliary vector
    pub const AT_NULL: u64 = 0;
    /// Program headers location
    pub const AT_PHDR: u64 = 3;
    /// Size of program header entry
    pub const AT_PHENT: u64 = 4;
    /// Number of program headers
    pub const AT_PHNUM: u64 = 5;
    /// Page size
    pub const AT_PAGESZ: u64 = 6;
    /// Base address of interpreter
    pub const AT_BASE: u64 = 7;
    /// Flags
    pub const AT_FLAGS: u64 = 8;
    /// Program entry point
    pub const AT_ENTRY: u64 = 9;
    /// Real user ID
    pub const AT_UID: u64 = 11;
    /// Effective user ID
    pub const AT_EUID: u64 = 12;
    /// Real group ID
    pub const AT_GID: u64 = 13;
    /// Effective group ID
    pub const AT_EGID: u64 = 14;
    /// Platform string
    pub const AT_PLATFORM: u64 = 15;
    /// Hardware capabilities
    pub const AT_HWCAP: u64 = 16;
    /// Clock ticks per second
    pub const AT_CLKTCK: u64 = 17;
    /// Random bytes
    pub const AT_RANDOM: u64 = 25;
    /// Filename of executed program
    pub const AT_EXECFN: u64 = 31;
}

impl UserProgram {
    /// Create a new user program from loaded ELF
    ///
    /// # Arguments
    ///
    /// * `name` - Program name
    /// * `loaded` - Parsed ELF program info
    /// * `args` - Command line arguments
    /// * `envp` - Environment variables
    pub fn new(
        name: String,
        loaded: LoadedProgram,
        args: Vec<String>,
        envp: Vec<String>,
    ) -> Self {
        // Calculate base address for PIE
        let base_address = if loaded.is_pie {
            layout::PIE_BASE
        } else {
            0
        };

        // Calculate actual entry point
        let entry_point = if loaded.is_pie {
            base_address + loaded.entry_point
        } else {
            loaded.entry_point
        };

        // Create memory regions from ELF segments
        let mut regions: Vec<MemoryRegion> = loaded
            .segments
            .iter()
            .map(|seg| {
                let start = if loaded.is_pie {
                    base_address + seg.vaddr
                } else {
                    seg.vaddr
                };

                let region_type = if seg.is_executable() {
                    RegionType::Code
                } else if seg.is_writable() {
                    RegionType::Data
                } else {
                    RegionType::Rodata
                };

                MemoryRegion {
                    start,
                    size: seg.mem_size,
                    readable: seg.is_readable(),
                    writable: seg.is_writable(),
                    executable: seg.is_executable(),
                    region_type,
                }
            })
            .collect();

        // Calculate program end (for heap start)
        let program_end = regions
            .iter()
            .map(|r| r.start + r.size)
            .max()
            .unwrap_or(layout::USER_HEAP_START);
        
        // Align heap start to page boundary
        let heap_start = (program_end + 0xFFF) & !0xFFF;
        
        // Add stack region
        let stack_bottom = layout::USER_STACK_TOP - layout::USER_STACK_SIZE;
        regions.push(MemoryRegion {
            start: stack_bottom,
            size: layout::USER_STACK_SIZE,
            readable: true,
            writable: true,
            executable: false,
            region_type: RegionType::Stack,
        });

        // Build auxiliary vector
        let mut auxv = Vec::new();
        
        if let Some(phdr_vaddr) = loaded.phdr_vaddr {
            let phdr_addr = if loaded.is_pie {
                base_address + phdr_vaddr
            } else {
                phdr_vaddr
            };
            auxv.push((auxv::AT_PHDR, phdr_addr));
        }
        
        auxv.push((auxv::AT_PHENT, 56)); // sizeof(Elf64ProgramHeader)
        auxv.push((auxv::AT_PHNUM, loaded.phdr_count as u64));
        auxv.push((auxv::AT_PAGESZ, 4096));
        auxv.push((auxv::AT_BASE, base_address));
        auxv.push((auxv::AT_ENTRY, entry_point));
        auxv.push((auxv::AT_UID, 0));
        auxv.push((auxv::AT_EUID, 0));
        auxv.push((auxv::AT_GID, 0));
        auxv.push((auxv::AT_EGID, 0));
        auxv.push((auxv::AT_NULL, 0));

        UserProgram {
            name,
            entry_point,
            base_address,
            initial_sp: layout::USER_STACK_TOP,
            brk: heap_start,
            regions,
            state: ProgramState::Ready,
            exit_code: None,
            args,
            envp,
            auxv,
        }
    }

    /// Load program from ELF binary
    ///
    /// # Arguments
    ///
    /// * `name` - Program name
    /// * `binary` - Raw ELF binary data
    /// * `args` - Command line arguments
    /// * `envp` - Environment variables
    pub fn from_elf(
        name: String,
        binary: &[u8],
        args: Vec<String>,
        envp: Vec<String>,
    ) -> Result<Self, ElfError> {
        let loaded = Elf64Loader::parse(binary)?;
        Ok(Self::new(name, loaded, args, envp))
    }

    /// Calculate initial stack layout and return stack pointer
    ///
    /// Stack layout (top to bottom):
    /// - NULL (end marker)
    /// - auxv entries
    /// - NULL (end of envp)
    /// - environment pointers
    /// - NULL (end of argv)
    /// - argument pointers
    /// - argc
    ///
    /// Returns the adjusted stack pointer
    pub fn calculate_stack_layout(&self) -> u64 {
        // Calculate total size needed on stack
        let mut size: u64 = 0;
        
        // Auxiliary vector (each entry is 16 bytes: type + value)
        size += (self.auxv.len() as u64) * 16;
        
        // NULL terminator for envp
        size += 8;
        
        // Environment pointers
        size += (self.envp.len() as u64) * 8;
        
        // NULL terminator for argv
        size += 8;
        
        // Argument pointers
        size += (self.args.len() as u64) * 8;
        
        // argc
        size += 8;
        
        // Strings (arguments + environment + padding)
        for arg in &self.args {
            size += (arg.len() as u64) + 1; // +1 for null terminator
        }
        for env in &self.envp {
            size += (env.len() as u64) + 1;
        }
        
        // Align to 16 bytes (x86_64 ABI requirement)
        size = (size + 15) & !15;
        
        // Return adjusted stack pointer
        self.initial_sp - size
    }

    /// Mark program as running
    pub fn start(&mut self) {
        self.state = ProgramState::Running;
    }

    /// Mark program as blocked
    pub fn block(&mut self) {
        self.state = ProgramState::Blocked;
    }

    /// Mark program as ready (unblock)
    pub fn unblock(&mut self) {
        if self.state == ProgramState::Blocked {
            self.state = ProgramState::Ready;
        }
    }

    /// Mark program as exited
    pub fn exit(&mut self, code: i32) {
        self.state = ProgramState::Exited(code);
        self.exit_code = Some(code);
    }

    /// Mark program as killed
    pub fn kill(&mut self) {
        self.state = ProgramState::Killed;
    }

    /// Check if program has finished
    pub fn is_finished(&self) -> bool {
        matches!(self.state, ProgramState::Exited(_) | ProgramState::Killed)
    }

    /// Find memory region containing address
    pub fn find_region(&self, addr: u64) -> Option<&MemoryRegion> {
        self.regions.iter().find(|r| {
            addr >= r.start && addr < r.start + r.size
        })
    }

    /// Check if address is valid for read
    pub fn can_read(&self, addr: u64) -> bool {
        self.find_region(addr).map_or(false, |r| r.readable)
    }

    /// Check if address is valid for write
    pub fn can_write(&self, addr: u64) -> bool {
        self.find_region(addr).map_or(false, |r| r.writable)
    }

    /// Check if address is valid for execute
    pub fn can_execute(&self, addr: u64) -> bool {
        self.find_region(addr).map_or(false, |r| r.executable)
    }

    /// Extend heap (brk syscall)
    pub fn extend_brk(&mut self, new_brk: u64) -> Result<u64, &'static str> {
        // Find current heap region or create one
        let heap_region = self.regions.iter_mut().find(|r| r.region_type == RegionType::Heap);
        
        if let Some(heap) = heap_region {
            // Check bounds
            if new_brk < heap.start {
                return Err("Cannot shrink heap below start");
            }
            
            let max_brk = heap.start + layout::USER_HEAP_MAX_SIZE;
            if new_brk > max_brk {
                return Err("Heap size limit exceeded");
            }
            
            heap.size = new_brk - heap.start;
            self.brk = new_brk;
        } else {
            // Create new heap region
            if new_brk < self.brk {
                return Err("Cannot shrink brk below current value");
            }
            
            let heap_size = new_brk - self.brk;
            if heap_size > layout::USER_HEAP_MAX_SIZE {
                return Err("Heap size limit exceeded");
            }
            
            self.regions.push(MemoryRegion {
                start: self.brk,
                size: heap_size,
                readable: true,
                writable: true,
                executable: false,
                region_type: RegionType::Heap,
            });
            
            self.brk = new_brk;
        }
        
        Ok(self.brk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::elf::*;

    #[test]
    fn test_user_program_layout() {
        assert!(layout::USER_TEXT_START < layout::USER_SPACE_END);
        assert!(layout::USER_STACK_TOP <= layout::USER_SPACE_END);
        assert!(layout::USER_HEAP_START < layout::USER_STACK_TOP - layout::USER_STACK_SIZE);
    }

    #[test]
    fn test_program_states() {
        let loaded = LoadedProgram {
            entry_point: 0x400000,
            base_address: 0,
            phdr_vaddr: None,
            phdr_count: 0,
            segments: vec![LoadSegment {
                vaddr: 0x400000,
                mem_size: 0x1000,
                file_offset: 0,
                file_size: 0x1000,
                flags: PF_R | PF_X,
                align: 0x1000,
            }],
            min_vaddr: 0x400000,
            max_vaddr: 0x401000,
            total_size: 0x1000,
            is_pie: false,
            interpreter: None,
        };

        let mut program = UserProgram::new(
            String::from("test"),
            loaded,
            vec![String::from("test")],
            vec![],
        );

        assert_eq!(program.state, ProgramState::Ready);
        
        program.start();
        assert_eq!(program.state, ProgramState::Running);
        
        program.block();
        assert_eq!(program.state, ProgramState::Blocked);
        
        program.unblock();
        assert_eq!(program.state, ProgramState::Ready);
        
        program.exit(42);
        assert_eq!(program.state, ProgramState::Exited(42));
        assert!(program.is_finished());
    }
}
