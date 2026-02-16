//! W^X (Write XOR Execute) Executable Memory Region
//!
//! Provides an abstraction for managing code memory regions that enforces
//! the W^X invariant: memory is either writable or executable, never both.

use alloc::vec::Vec;

/// Permission state for an executable region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionState {
    /// Region is writable but NOT executable.
    Writable,
    /// Region is executable but NOT writable.
    Executable,
    /// Region has been freed.
    Freed,
}

/// An executable memory region with W^X enforcement.
///
/// In the kernel, this would use page table manipulation to
/// toggle NX bits. For the runtime abstraction, we track state
/// and enforce invariants in software.
#[derive(Debug)]
pub struct ExecutableRegion {
    /// Code buffer.
    buffer: Vec<u8>,
    /// Current permission state.
    state: RegionState,
    /// Maximum allowed size.
    max_size: usize,
    /// Whether the code has been finalized.
    finalized: bool,
}

/// Errors from executable region operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutableError {
    /// Attempted to write while in Executable state.
    WriteWhileExecutable,
    /// Attempted to execute while in Writable state.
    ExecuteWhileWritable,
    /// Region has been freed.
    RegionFreed,
    /// Exceeds maximum size.
    ExceedsMaxSize,
    /// Region not finalized for execution.
    NotFinalized,
}

impl ExecutableRegion {
    /// Default maximum code region size: 16 MB.
    pub const DEFAULT_MAX_SIZE: usize = 16 * 1024 * 1024;

    /// Create a new writable region.
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: Vec::new(),
            state: RegionState::Writable,
            max_size,
            finalized: false,
        }
    }

    /// Create with default max size.
    pub fn with_default_max() -> Self {
        Self::new(Self::DEFAULT_MAX_SIZE)
    }

    /// Get current state.
    pub fn state(&self) -> RegionState {
        self.state
    }

    /// Write code into the region. Only allowed in Writable state.
    pub fn write(&mut self, code: &[u8]) -> Result<(), ExecutableError> {
        if self.state == RegionState::Freed {
            return Err(ExecutableError::RegionFreed);
        }
        if self.state == RegionState::Executable {
            return Err(ExecutableError::WriteWhileExecutable);
        }
        if self.buffer.len() + code.len() > self.max_size {
            return Err(ExecutableError::ExceedsMaxSize);
        }
        self.buffer.extend_from_slice(code);
        Ok(())
    }

    /// Finalize: transition from Writable → Executable.
    ///
    /// In a real kernel, this would call `mprotect(RX)` or flip page table
    /// NX bits. After this call, `write()` will fail and `execute()` will
    /// succeed.
    pub fn make_executable(&mut self) -> Result<(), ExecutableError> {
        if self.state == RegionState::Freed {
            return Err(ExecutableError::RegionFreed);
        }
        if self.state == RegionState::Executable {
            return Ok(()); // already executable
        }
        self.state = RegionState::Executable;
        self.finalized = true;
        Ok(())
    }

    /// Transition back to Writable (for patching/recompilation).
    ///
    /// In a real kernel, this would call `mprotect(RW)`.
    pub fn make_writable(&mut self) -> Result<(), ExecutableError> {
        if self.state == RegionState::Freed {
            return Err(ExecutableError::RegionFreed);
        }
        self.state = RegionState::Writable;
        Ok(())
    }

    /// Check if the region can be executed.
    pub fn is_executable(&self) -> bool {
        self.state == RegionState::Executable && self.finalized
    }

    /// Check if the region can be written.
    pub fn is_writable(&self) -> bool {
        self.state == RegionState::Writable
    }

    /// Get the code bytes (read-only, always allowed unless freed).
    pub fn code(&self) -> Result<&[u8], ExecutableError> {
        if self.state == RegionState::Freed {
            return Err(ExecutableError::RegionFreed);
        }
        Ok(&self.buffer)
    }

    /// Get code size.
    pub fn size(&self) -> usize {
        self.buffer.len()
    }

    /// Free the region.
    pub fn free(&mut self) {
        self.buffer.clear();
        self.state = RegionState::Freed;
    }
}

/// Manager for multiple executable regions.
pub struct ExecutableMemoryManager {
    /// Active regions.
    regions: Vec<ExecutableRegion>,
    /// Total code size across all regions.
    total_size: usize,
    /// Global maximum code size.
    global_max: usize,
}

impl ExecutableMemoryManager {
    /// Create a new manager.
    pub fn new(global_max: usize) -> Self {
        Self {
            regions: Vec::new(),
            total_size: 0,
            global_max,
        }
    }

    /// Allocate a new code region.
    pub fn allocate(&mut self, code: &[u8]) -> Result<usize, ExecutableError> {
        if self.total_size + code.len() > self.global_max {
            return Err(ExecutableError::ExceedsMaxSize);
        }

        let mut region = ExecutableRegion::new(code.len() + 64);
        region.write(code)?;
        region.make_executable()?;

        let id = self.regions.len();
        self.total_size += code.len();
        self.regions.push(region);
        Ok(id)
    }

    /// Get a region by ID.
    pub fn get(&self, id: usize) -> Option<&ExecutableRegion> {
        self.regions.get(id)
    }

    /// Free a region by ID.
    pub fn free(&mut self, id: usize) {
        if let Some(region) = self.regions.get_mut(id) {
            self.total_size -= region.size();
            region.free();
        }
    }

    /// Total code size.
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Number of active regions.
    pub fn active_count(&self) -> usize {
        self.regions
            .iter()
            .filter(|r| r.state() != RegionState::Freed)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wxe_write_then_execute() {
        let mut region = ExecutableRegion::new(1024);

        assert_eq!(region.state(), RegionState::Writable);
        assert!(region.is_writable());
        assert!(!region.is_executable());

        // Write code
        region.write(&[0x90, 0x90, 0xC3]).unwrap(); // nop, nop, ret
        assert_eq!(region.size(), 3);

        // Cannot execute while writable
        assert!(!region.is_executable());

        // Make executable
        region.make_executable().unwrap();
        assert_eq!(region.state(), RegionState::Executable);
        assert!(region.is_executable());
        assert!(!region.is_writable());

        // Cannot write while executable
        let err = region.write(&[0x90]);
        assert_eq!(err, Err(ExecutableError::WriteWhileExecutable));
    }

    #[test]
    fn test_wxe_make_writable_again() {
        let mut region = ExecutableRegion::new(1024);
        region.write(&[0xCC]).unwrap();
        region.make_executable().unwrap();

        // Transition back to writable
        region.make_writable().unwrap();
        assert!(region.is_writable());
        assert!(!region.is_executable());

        // Can write again
        region.write(&[0x90]).unwrap();
        assert_eq!(region.size(), 2); // original + new
    }

    #[test]
    fn test_wxe_freed_region() {
        let mut region = ExecutableRegion::new(1024);
        region.write(&[0x90]).unwrap();
        region.free();

        assert_eq!(region.state(), RegionState::Freed);
        assert_eq!(region.write(&[0x90]), Err(ExecutableError::RegionFreed));
        assert_eq!(region.make_executable(), Err(ExecutableError::RegionFreed));
        assert_eq!(region.code(), Err(ExecutableError::RegionFreed));
    }

    #[test]
    fn test_wxe_max_size_enforcement() {
        let mut region = ExecutableRegion::new(4);
        region.write(&[0x90, 0x90, 0x90, 0x90]).unwrap();

        let err = region.write(&[0x90]);
        assert_eq!(err, Err(ExecutableError::ExceedsMaxSize));
    }

    #[test]
    fn test_memory_manager() {
        let mut mgr = ExecutableMemoryManager::new(4096);

        let id1 = mgr.allocate(&[0x90, 0xC3]).unwrap();
        let id2 = mgr.allocate(&[0x55, 0x48, 0x89, 0xE5, 0xC3]).unwrap();

        assert_eq!(mgr.active_count(), 2);
        assert_eq!(mgr.total_size(), 7);

        assert!(mgr.get(id1).unwrap().is_executable());
        assert!(mgr.get(id2).unwrap().is_executable());

        mgr.free(id1);
        assert_eq!(mgr.active_count(), 1);
    }
}
