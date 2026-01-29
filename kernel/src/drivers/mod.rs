//! Device Drivers
//!
//! Hardware device driver implementations.

pub mod display;
pub mod input;
pub mod net;
pub mod storage;

use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;

/// Driver error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverError {
    /// Device not found
    NotFound,
    /// Device already initialized
    AlreadyInitialized,
    /// Device not initialized
    NotInitialized,
    /// Operation not supported
    NotSupported,
    /// Hardware error
    HardwareError,
    /// Invalid parameters
    InvalidParameters,
    /// Timeout
    Timeout,
    /// Resource busy
    Busy,
    /// Out of memory
    OutOfMemory,
}

/// Driver status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverStatus {
    /// Not initialized
    Uninitialized,
    /// Initializing
    Initializing,
    /// Ready and operational
    Ready,
    /// Suspended
    Suspended,
    /// Error state
    Error,
    /// Stopped
    Stopped,
}

/// Base driver trait
pub trait Driver: Send + Sync {
    /// Get driver name
    fn name(&self) -> &str;

    /// Get driver version
    fn version(&self) -> (u32, u32, u32);

    /// Get driver status
    fn status(&self) -> DriverStatus;

    /// Initialize driver
    fn init(&mut self) -> Result<(), DriverError>;

    /// Shutdown driver
    fn shutdown(&mut self) -> Result<(), DriverError>;

    /// Suspend driver (power management)
    fn suspend(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    /// Resume driver (power management)
    fn resume(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

/// Driver manager for registering and managing drivers
pub struct DriverManager {
    /// Registered drivers
    drivers: Vec<Box<dyn Driver>>,
}

impl DriverManager {
    /// Create a new driver manager
    pub fn new() -> Self {
        Self {
            drivers: Vec::new(),
        }
    }

    /// Register a driver
    pub fn register(&mut self, driver: Box<dyn Driver>) {
        self.drivers.push(driver);
    }

    /// Find driver by name
    pub fn find(&self, name: &str) -> Option<&dyn Driver> {
        self.drivers.iter().find(|d| d.name() == name).map(|d| d.as_ref())
    }

    /// Initialize all drivers
    pub fn init_all(&mut self) -> Vec<(usize, Result<(), DriverError>)> {
        let mut results = Vec::new();
        for (idx, driver) in self.drivers.iter_mut().enumerate() {
            let result = driver.init();
            results.push((idx, result));
        }
        results
    }

    /// Shutdown all drivers
    pub fn shutdown_all(&mut self) {
        for driver in self.drivers.iter_mut().rev() {
            let _ = driver.shutdown();
        }
    }

    /// Suspend all drivers
    pub fn suspend_all(&mut self) -> Result<(), DriverError> {
        for driver in &mut self.drivers {
            driver.suspend()?;
        }
        Ok(())
    }

    /// Resume all drivers
    pub fn resume_all(&mut self) -> Result<(), DriverError> {
        for driver in &mut self.drivers {
            driver.resume()?;
        }
        Ok(())
    }

    /// Get driver count
    pub fn count(&self) -> usize {
        self.drivers.len()
    }

    /// List all driver names
    pub fn list(&self) -> Vec<&str> {
        self.drivers.iter().map(|d| d.name()).collect()
    }
}

impl Default for DriverManager {
    fn default() -> Self {
        Self::new()
    }
}
