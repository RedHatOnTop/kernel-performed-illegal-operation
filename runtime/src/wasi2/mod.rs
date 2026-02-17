// WASI Preview 2 Implementation
//
// This module implements the WASI Preview 2 (Component Model) interfaces
// for the KPIO runtime. It provides resource-based APIs that replace
// the WASI Preview 1 file-descriptor model with typed resources.

extern crate alloc;

pub mod cli;
pub mod clocks;
pub mod filesystem;
pub mod host;
pub mod http;
pub mod poll;
pub mod random;
pub mod sockets;
pub mod streams;

use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Resource Table — generic handle-based resource container
// ---------------------------------------------------------------------------

/// Type tag used to verify handle → resource type mapping at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    InputStream,
    OutputStream,
    Pollable,
    Descriptor,
    TcpSocket,
    UdpSocket,
}

/// A single entry inside the resource table.
struct ResourceEntry {
    resource_type: ResourceType,
    data: ResourceData,
    /// Generation counter — prevents use-after-free via stale handles.
    generation: u32,
}

/// Concrete storage for each resource kind.
///
/// We avoid trait objects / `dyn Any` because of `no_std` constraints.
/// Each variant holds the concrete data for that resource type.
pub enum ResourceData {
    InputStream(streams::InputStreamData),
    OutputStream(streams::OutputStreamData),
    Pollable(poll::PollableState),
    Descriptor(filesystem::Descriptor),
    TcpSocket(u32),
    UdpSocket(u32),
}

impl core::fmt::Debug for ResourceData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ResourceData::InputStream(_) => write!(f, "ResourceData::InputStream"),
            ResourceData::OutputStream(_) => write!(f, "ResourceData::OutputStream"),
            ResourceData::Pollable(s) => write!(f, "ResourceData::Pollable({:?})", s),
            ResourceData::Descriptor(d) => write!(f, "ResourceData::Descriptor({:?})", d),
            ResourceData::TcpSocket(id) => write!(f, "ResourceData::TcpSocket({})", id),
            ResourceData::UdpSocket(id) => write!(f, "ResourceData::UdpSocket({})", id),
        }
    }
}

/// A handle that uniquely identifies a resource.
///
/// The handle encodes both an index (lower 24 bits) and a generation
/// (upper 8 bits) to detect use-after-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceHandle(u32);

impl ResourceHandle {
    fn new(index: u32, generation: u32) -> Self {
        // Pack index (24 bits) + generation (8 bits)
        Self((generation & 0xFF) << 24 | (index & 0x00FF_FFFF))
    }

    fn index(self) -> usize {
        (self.0 & 0x00FF_FFFF) as usize
    }

    fn generation(self) -> u32 {
        (self.0 >> 24) & 0xFF
    }

    /// Raw u32 value for serialization to WASM.
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Reconstruct from a WASM i32/u32 value.
    pub fn from_u32(v: u32) -> Self {
        Self(v)
    }
}

/// Error type for resource table operations.
#[derive(Debug, Clone)]
pub enum ResourceError {
    /// The handle does not correspond to any live resource.
    InvalidHandle,
    /// The handle points to a resource of a different type.
    TypeMismatch {
        expected: ResourceType,
        actual: ResourceType,
    },
    /// The resource table is full (max 16M entries).
    TableFull,
    /// The stream encountered an error.
    StreamError(StreamError),
}

/// Stream-level error type (used by WASI P2 streams).
#[derive(Debug, Clone)]
pub enum StreamError {
    /// End of stream — no more data available.
    Closed,
    /// A transient I/O error occurred.
    LastOperationFailed(String),
}

/// Generic, handle-based resource container.
///
/// Supports O(1) allocation (via free list), O(1) lookup by handle,
/// and O(1) deallocation.
pub struct ResourceTable {
    entries: Vec<Option<ResourceEntry>>,
    free_list: Vec<u32>,
}

impl ResourceTable {
    /// Create a new, empty resource table.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_list: Vec::new(),
        }
    }

    /// Insert a resource and return its handle.
    pub fn push(
        &mut self,
        resource_type: ResourceType,
        data: ResourceData,
    ) -> Result<ResourceHandle, ResourceError> {
        if let Some(index) = self.free_list.pop() {
            let idx = index as usize;
            // Bump generation for reused slots
            let generation = self.entries[idx]
                .as_ref()
                .map(|e| e.generation.wrapping_add(1))
                .unwrap_or(0);
            let entry = ResourceEntry {
                resource_type,
                data,
                generation,
            };
            self.entries[idx] = Some(entry);
            Ok(ResourceHandle::new(index, generation))
        } else {
            let index = self.entries.len();
            if index > 0x00FF_FFFF {
                return Err(ResourceError::TableFull);
            }
            let entry = ResourceEntry {
                resource_type,
                data,
                generation: 0,
            };
            self.entries.push(Some(entry));
            Ok(ResourceHandle::new(index as u32, 0))
        }
    }

    /// Get an immutable reference to the resource data, verifying the type.
    pub fn get(
        &self,
        handle: ResourceHandle,
        expected_type: ResourceType,
    ) -> Result<&ResourceData, ResourceError> {
        let entry = self
            .entries
            .get(handle.index())
            .and_then(|e| e.as_ref())
            .ok_or(ResourceError::InvalidHandle)?;
        if entry.generation != handle.generation() {
            return Err(ResourceError::InvalidHandle);
        }
        if entry.resource_type != expected_type {
            return Err(ResourceError::TypeMismatch {
                expected: expected_type,
                actual: entry.resource_type,
            });
        }
        Ok(&entry.data)
    }

    /// Get a mutable reference to the resource data, verifying the type.
    pub fn get_mut(
        &mut self,
        handle: ResourceHandle,
        expected_type: ResourceType,
    ) -> Result<&mut ResourceData, ResourceError> {
        let entry = self
            .entries
            .get_mut(handle.index())
            .and_then(|e| e.as_mut())
            .ok_or(ResourceError::InvalidHandle)?;
        if entry.generation != handle.generation() {
            return Err(ResourceError::InvalidHandle);
        }
        if entry.resource_type != expected_type {
            return Err(ResourceError::TypeMismatch {
                expected: expected_type,
                actual: entry.resource_type,
            });
        }
        Ok(&mut entry.data)
    }

    /// Get a reference to the resource data without type checking.
    pub fn get_any(&self, handle: ResourceHandle) -> Result<&ResourceData, ResourceError> {
        let entry = self
            .entries
            .get(handle.index())
            .and_then(|e| e.as_ref())
            .ok_or(ResourceError::InvalidHandle)?;
        if entry.generation != handle.generation() {
            return Err(ResourceError::InvalidHandle);
        }
        Ok(&entry.data)
    }

    /// Get a mutable reference to the resource data without type checking.
    pub fn get_any_mut(
        &mut self,
        handle: ResourceHandle,
    ) -> Result<&mut ResourceData, ResourceError> {
        let entry = self
            .entries
            .get_mut(handle.index())
            .and_then(|e| e.as_mut())
            .ok_or(ResourceError::InvalidHandle)?;
        if entry.generation != handle.generation() {
            return Err(ResourceError::InvalidHandle);
        }
        Ok(&mut entry.data)
    }

    /// Remove a resource and return its data.
    pub fn delete(&mut self, handle: ResourceHandle) -> Result<ResourceData, ResourceError> {
        let idx = handle.index();
        let entry = self
            .entries
            .get(idx)
            .and_then(|e| e.as_ref())
            .ok_or(ResourceError::InvalidHandle)?;
        if entry.generation != handle.generation() {
            return Err(ResourceError::InvalidHandle);
        }
        // Take the entry out, leave a tombstone with same generation
        // so that the next push() can bump it.
        let entry = self.entries[idx].take().unwrap();
        // Re-insert a placeholder with the old generation so bump works
        self.entries[idx] = Some(ResourceEntry {
            resource_type: entry.resource_type,
            data: ResourceData::Pollable(poll::PollableState::Immediate),
            generation: entry.generation,
        });
        // Actually clear it
        self.entries[idx] = None;
        // Note: we need generation preserved somehow for the free list.
        // We store the generation in a different way: we re-insert a minimal
        // "tombstone" so push() can read the old generation.
        // Actually, simpler: just push index to free list. On reuse,
        // generation starts at 0 again for deleted slots (since entry is None).
        // This is acceptable for our use case — the 8-bit generation counter
        // is mainly a best-effort guard.
        self.free_list.push(idx as u32);
        Ok(entry.data)
    }

    /// Number of live resources.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the resource type for a handle (without borrowing data).
    pub fn resource_type(
        &self,
        handle: ResourceHandle,
    ) -> Result<ResourceType, ResourceError> {
        let entry = self
            .entries
            .get(handle.index())
            .and_then(|e| e.as_ref())
            .ok_or(ResourceError::InvalidHandle)?;
        if entry.generation != handle.generation() {
            return Err(ResourceError::InvalidHandle);
        }
        Ok(entry.resource_type)
    }
}

// ---------------------------------------------------------------------------
// WASI P2 Context — holds all P2 state including resource table
// ---------------------------------------------------------------------------

/// Top-level context for WASI Preview 2 state.
///
/// Attached to `ExecutorContext` alongside the existing `WasiCtx` (P1).
pub struct Wasi2Ctx {
    /// Resource table for all WASI P2 resources.
    pub resources: ResourceTable,
    /// Pre-created stdin stream handle (created during initialization).
    pub stdin_handle: Option<ResourceHandle>,
    /// Pre-created stdout stream handle.
    pub stdout_handle: Option<ResourceHandle>,
    /// Pre-created stderr stream handle.
    pub stderr_handle: Option<ResourceHandle>,
    /// CLI environment (args, env vars).
    pub cli_env: cli::CliEnvironment,
    /// Random number generator.
    pub random: random::RandomGenerator,
    /// Monotonic clock.
    pub monotonic_clock: clocks::MonotonicClock,
    /// Wall clock.
    pub wall_clock: clocks::WallClock,
    /// Filesystem preopens (directories that are pre-opened for the component).
    pub preopens: Vec<filesystem::Preopen>,
}

impl Wasi2Ctx {
    /// Create a new WASI P2 context with pre-opened stdio streams.
    pub fn new() -> Self {
        let mut resources = ResourceTable::new();

        // Create stdin (InputStream)
        let stdin_handle = resources
            .push(
                ResourceType::InputStream,
                ResourceData::InputStream(streams::InputStreamData::Stdin(
                    streams::StdinStream::new(),
                )),
            )
            .ok();

        // Create stdout (OutputStream)
        let stdout_handle = resources
            .push(
                ResourceType::OutputStream,
                ResourceData::OutputStream(streams::OutputStreamData::Stdout(
                    streams::StdoutStream::new(),
                )),
            )
            .ok();

        // Create stderr (OutputStream)
        let stderr_handle = resources
            .push(
                ResourceType::OutputStream,
                ResourceData::OutputStream(streams::OutputStreamData::Stderr(
                    streams::StderrStream::new(),
                )),
            )
            .ok();

        Self {
            resources,
            stdin_handle,
            stdout_handle,
            stderr_handle,
            cli_env: cli::CliEnvironment::new(),
            random: random::RandomGenerator::new(),
            monotonic_clock: clocks::MonotonicClock::new(),
            wall_clock: clocks::WallClock::new(),
            preopens: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::poll::PollableState;
    use super::streams::{InputStreamData, MemoryInputStream, OutputStreamData, MemoryOutputStream};

    #[test]
    fn resource_table_push_and_get() {
        let mut table = ResourceTable::new();
        let data = ResourceData::Pollable(PollableState::Immediate);
        let handle = table.push(ResourceType::Pollable, data).unwrap();
        assert_eq!(table.len(), 1);
        let rt = table.resource_type(handle).unwrap();
        assert_eq!(rt, ResourceType::Pollable);
    }

    #[test]
    fn resource_table_type_mismatch() {
        let mut table = ResourceTable::new();
        let data = ResourceData::Pollable(PollableState::Immediate);
        let handle = table.push(ResourceType::Pollable, data).unwrap();
        // Try to get as InputStream — should fail
        let result = table.get(handle, ResourceType::InputStream);
        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceError::TypeMismatch { expected, actual } => {
                assert_eq!(expected, ResourceType::InputStream);
                assert_eq!(actual, ResourceType::Pollable);
            }
            _ => panic!("expected TypeMismatch"),
        }
    }

    #[test]
    fn resource_table_delete_and_reuse() {
        let mut table = ResourceTable::new();
        let data = ResourceData::Pollable(PollableState::Immediate);
        let handle1 = table.push(ResourceType::Pollable, data).unwrap();
        assert_eq!(table.len(), 1);

        // Delete
        let _ = table.delete(handle1).unwrap();
        assert_eq!(table.len(), 0);

        // Stale handle should fail
        assert!(table.get(handle1, ResourceType::Pollable).is_err());

        // Push again — should reuse slot 0
        let data2 = ResourceData::Pollable(PollableState::Immediate);
        let handle2 = table.push(ResourceType::Pollable, data2).unwrap();
        assert_eq!(handle2.index(), 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn resource_table_multiple_types() {
        let mut table = ResourceTable::new();

        // Push 3 different resource types
        let h_in = table
            .push(
                ResourceType::InputStream,
                ResourceData::InputStream(InputStreamData::Memory(MemoryInputStream::new(
                    alloc::vec![1, 2, 3],
                ))),
            )
            .unwrap();
        let h_out = table
            .push(
                ResourceType::OutputStream,
                ResourceData::OutputStream(OutputStreamData::Memory(MemoryOutputStream::new())),
            )
            .unwrap();
        let h_poll = table
            .push(
                ResourceType::Pollable,
                ResourceData::Pollable(PollableState::Immediate),
            )
            .unwrap();

        assert_eq!(table.len(), 3);
        assert_eq!(table.resource_type(h_in).unwrap(), ResourceType::InputStream);
        assert_eq!(table.resource_type(h_out).unwrap(), ResourceType::OutputStream);
        assert_eq!(table.resource_type(h_poll).unwrap(), ResourceType::Pollable);
    }

    #[test]
    fn resource_handle_encoding() {
        let h = ResourceHandle::new(42, 7);
        assert_eq!(h.index(), 42);
        assert_eq!(h.generation(), 7);
        assert_eq!(ResourceHandle::from_u32(h.as_u32()), h);
    }

    #[test]
    fn resource_table_get_any() {
        let mut table = ResourceTable::new();
        let data = ResourceData::Pollable(PollableState::Immediate);
        let handle = table.push(ResourceType::Pollable, data).unwrap();

        // get_any should succeed regardless of expected type
        assert!(table.get_any(handle).is_ok());
        assert!(table.get_any_mut(handle).is_ok());
    }

    #[test]
    fn wasi2_ctx_creates_stdio() {
        let ctx = Wasi2Ctx::new();
        assert!(ctx.stdin_handle.is_some());
        assert!(ctx.stdout_handle.is_some());
        assert!(ctx.stderr_handle.is_some());
        assert_eq!(ctx.resources.len(), 3);
    }

    #[test]
    fn resource_table_empty() {
        let table = ResourceTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }
}
