//! Resource Management (cgroups-like)
//!
//! This module implements resource limits and accounting for
//! browser processes, similar to Linux cgroups.

use alloc::collections::BTreeMap;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use crate::browser::coordinator::TabId;
use crate::process::ProcessId;

/// Resource group identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceGroupId(pub u32);

impl ResourceGroupId {
    /// Root resource group.
    pub const ROOT: ResourceGroupId = ResourceGroupId(0);
}

/// CPU resource limits.
#[derive(Debug, Clone, Copy)]
pub struct CpuLimits {
    /// CPU quota (microseconds per period, 0 = unlimited).
    pub quota_us: u64,
    /// CPU period (microseconds).
    pub period_us: u64,
    /// CPU shares (for proportional scheduling).
    pub shares: u32,
    /// Maximum CPU cores (0 = unlimited).
    pub max_cores: u8,
}

impl Default for CpuLimits {
    fn default() -> Self {
        CpuLimits {
            quota_us: 0,
            period_us: 100_000, // 100ms
            shares: 1024,
            max_cores: 0,
        }
    }
}

impl CpuLimits {
    /// Create restrictive CPU limits.
    pub fn restrictive() -> Self {
        CpuLimits {
            quota_us: 90_000, // 90ms per 100ms = 90%
            period_us: 100_000,
            shares: 512,
            max_cores: 2,
        }
    }
}

/// Memory resource limits.
#[derive(Debug, Clone, Copy)]
pub struct MemoryLimits {
    /// Maximum memory (bytes, 0 = unlimited).
    pub max_bytes: u64,
    /// High watermark (soft limit).
    pub high_bytes: u64,
    /// Low watermark (protected memory).
    pub low_bytes: u64,
    /// Minimum guaranteed memory.
    pub min_bytes: u64,
    /// Include kernel memory in limit.
    pub include_kernel: bool,
    /// OOM kill enabled.
    pub oom_kill: bool,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        MemoryLimits {
            max_bytes: 0,
            high_bytes: 0,
            low_bytes: 0,
            min_bytes: 0,
            include_kernel: true,
            oom_kill: true,
        }
    }
}

impl MemoryLimits {
    /// Create typical browser tab limits.
    pub fn browser_tab() -> Self {
        MemoryLimits {
            max_bytes: 512 * 1024 * 1024,  // 512MB hard limit
            high_bytes: 256 * 1024 * 1024, // 256MB soft limit
            low_bytes: 32 * 1024 * 1024,   // 32MB protected
            min_bytes: 16 * 1024 * 1024,   // 16MB guaranteed
            include_kernel: true,
            oom_kill: true,
        }
    }
}

/// I/O resource limits.
#[derive(Debug, Clone, Copy)]
pub struct IoLimits {
    /// Maximum read bytes per second.
    pub read_bps: u64,
    /// Maximum write bytes per second.
    pub write_bps: u64,
    /// Maximum read IOPS.
    pub read_iops: u32,
    /// Maximum write IOPS.
    pub write_iops: u32,
    /// I/O weight (1-10000).
    pub weight: u16,
}

impl Default for IoLimits {
    fn default() -> Self {
        IoLimits {
            read_bps: 0,
            write_bps: 0,
            read_iops: 0,
            write_iops: 0,
            weight: 100,
        }
    }
}

/// Network resource limits.
#[derive(Debug, Clone, Copy)]
pub struct NetworkLimits {
    /// Maximum egress bytes per second.
    pub egress_bps: u64,
    /// Maximum ingress bytes per second.
    pub ingress_bps: u64,
    /// Maximum connections.
    pub max_connections: u32,
    /// Maximum sockets.
    pub max_sockets: u32,
}

impl Default for NetworkLimits {
    fn default() -> Self {
        NetworkLimits {
            egress_bps: 0,
            ingress_bps: 0,
            max_connections: 256,
            max_sockets: 64,
        }
    }
}

/// Combined resource limits.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// CPU limits.
    pub cpu: CpuLimits,
    /// Memory limits.
    pub memory: MemoryLimits,
    /// I/O limits.
    pub io: IoLimits,
    /// Network limits.
    pub network: NetworkLimits,
    /// Maximum processes.
    pub max_processes: u32,
    /// Maximum threads per process.
    pub max_threads: u32,
    /// Maximum open file descriptors.
    pub max_fds: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        ResourceLimits {
            cpu: CpuLimits::default(),
            memory: MemoryLimits::default(),
            io: IoLimits::default(),
            network: NetworkLimits::default(),
            max_processes: 0,
            max_threads: 0,
            max_fds: 0,
        }
    }
}

impl ResourceLimits {
    /// Create browser tab resource limits.
    pub fn browser_tab() -> Self {
        ResourceLimits {
            cpu: CpuLimits::restrictive(),
            memory: MemoryLimits::browser_tab(),
            io: IoLimits::default(),
            network: NetworkLimits::default(),
            max_processes: 4,
            max_threads: 32,
            max_fds: 256,
        }
    }
}

/// CPU usage tracking.
#[derive(Debug, Default)]
pub struct CpuUsage {
    /// Total CPU time (nanoseconds).
    pub total_ns: AtomicU64,
    /// User CPU time.
    pub user_ns: AtomicU64,
    /// System CPU time.
    pub system_ns: AtomicU64,
    /// Throttled time.
    pub throttled_ns: AtomicU64,
    /// Number of throttle events.
    pub throttle_count: AtomicU64,
}

/// Memory usage tracking.
#[derive(Debug, Default)]
pub struct MemoryUsage {
    /// Current memory usage.
    pub current_bytes: AtomicU64,
    /// Peak memory usage.
    pub peak_bytes: AtomicU64,
    /// Kernel memory usage.
    pub kernel_bytes: AtomicU64,
    /// Cache/buffer memory.
    pub cache_bytes: AtomicU64,
    /// OOM kill count.
    pub oom_kills: AtomicU64,
}

/// I/O usage tracking.
#[derive(Debug, Default)]
pub struct IoUsage {
    /// Total bytes read.
    pub read_bytes: AtomicU64,
    /// Total bytes written.
    pub write_bytes: AtomicU64,
    /// Read operations.
    pub read_ops: AtomicU64,
    /// Write operations.
    pub write_ops: AtomicU64,
}

/// Network usage tracking.
#[derive(Debug, Default)]
pub struct NetworkUsage {
    /// Bytes received.
    pub rx_bytes: AtomicU64,
    /// Bytes transmitted.
    pub tx_bytes: AtomicU64,
    /// Packets received.
    pub rx_packets: AtomicU64,
    /// Packets transmitted.
    pub tx_packets: AtomicU64,
    /// Current connections.
    pub connections: AtomicU64,
}

/// Combined resource usage.
#[derive(Debug, Default)]
pub struct ResourceUsage {
    /// CPU usage.
    pub cpu: CpuUsage,
    /// Memory usage.
    pub memory: MemoryUsage,
    /// I/O usage.
    pub io: IoUsage,
    /// Network usage.
    pub network: NetworkUsage,
}

impl ResourceUsage {
    /// Get memory usage snapshot.
    pub fn memory_snapshot(&self) -> (u64, u64) {
        (
            self.memory.current_bytes.load(Ordering::Relaxed),
            self.memory.peak_bytes.load(Ordering::Relaxed),
        )
    }

    /// Add CPU time.
    pub fn add_cpu_time(&self, user_ns: u64, system_ns: u64) {
        self.cpu.user_ns.fetch_add(user_ns, Ordering::Relaxed);
        self.cpu.system_ns.fetch_add(system_ns, Ordering::Relaxed);
        self.cpu
            .total_ns
            .fetch_add(user_ns + system_ns, Ordering::Relaxed);
    }

    /// Update memory usage.
    pub fn update_memory(&self, current: u64) {
        self.memory.current_bytes.store(current, Ordering::Relaxed);

        // Update peak if needed
        let mut peak = self.memory.peak_bytes.load(Ordering::Relaxed);
        while current > peak {
            match self.memory.peak_bytes.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(p) => peak = p,
            }
        }
    }

    /// Add I/O.
    pub fn add_io(&self, read_bytes: u64, write_bytes: u64) {
        if read_bytes > 0 {
            self.io.read_bytes.fetch_add(read_bytes, Ordering::Relaxed);
            self.io.read_ops.fetch_add(1, Ordering::Relaxed);
        }
        if write_bytes > 0 {
            self.io
                .write_bytes
                .fetch_add(write_bytes, Ordering::Relaxed);
            self.io.write_ops.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Resource group (cgroup).
pub struct ResourceGroup {
    /// Group ID.
    pub id: ResourceGroupId,
    /// Group name.
    pub name: String,
    /// Resource limits.
    pub limits: ResourceLimits,
    /// Resource usage.
    pub usage: ResourceUsage,
    /// Processes in this group.
    pub processes: BTreeMap<ProcessId, ()>,
    /// Parent group.
    pub parent: Option<ResourceGroupId>,
    /// Child groups.
    pub children: alloc::vec::Vec<ResourceGroupId>,
    /// Is frozen (suspended).
    pub frozen: bool,
}

impl ResourceGroup {
    /// Create new resource group.
    pub fn new(id: ResourceGroupId, name: &str, limits: ResourceLimits) -> Self {
        ResourceGroup {
            id,
            name: String::from(name),
            limits,
            usage: ResourceUsage::default(),
            processes: BTreeMap::new(),
            parent: None,
            children: alloc::vec::Vec::new(),
            frozen: false,
        }
    }

    /// Add a process.
    pub fn add_process(&mut self, pid: ProcessId) -> Result<(), ResourceError> {
        if self.limits.max_processes > 0
            && self.processes.len() >= self.limits.max_processes as usize
        {
            return Err(ResourceError::ProcessLimit);
        }
        self.processes.insert(pid, ());
        Ok(())
    }

    /// Remove a process.
    pub fn remove_process(&mut self, pid: ProcessId) {
        self.processes.remove(&pid);
    }

    /// Check memory limit.
    pub fn check_memory(&self, additional: u64) -> Result<(), ResourceError> {
        if self.limits.memory.max_bytes == 0 {
            return Ok(());
        }

        let current = self.usage.memory.current_bytes.load(Ordering::Relaxed);
        if current + additional > self.limits.memory.max_bytes {
            return Err(ResourceError::MemoryLimit);
        }

        Ok(())
    }

    /// Check if throttled.
    pub fn is_cpu_throttled(&self) -> bool {
        if self.limits.cpu.quota_us == 0 {
            return false;
        }

        // Simplified check - in reality would track time windows
        let used = self.usage.cpu.total_ns.load(Ordering::Relaxed) / 1000;
        used >= self.limits.cpu.quota_us
    }

    /// Freeze the group.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Thaw the group.
    pub fn thaw(&mut self) {
        self.frozen = false;
    }
}

/// Resource error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceError {
    /// Memory limit exceeded.
    MemoryLimit,
    /// CPU quota exceeded.
    CpuQuota,
    /// Process limit exceeded.
    ProcessLimit,
    /// I/O limit exceeded.
    IoLimit,
    /// Group not found.
    NotFound,
    /// Invalid operation.
    InvalidOperation,
}

/// Resource manager.
pub struct ResourceManager {
    /// Resource groups.
    groups: BTreeMap<ResourceGroupId, Mutex<ResourceGroup>>,
    /// Process to group mapping.
    process_group: BTreeMap<ProcessId, ResourceGroupId>,
    /// Tab to group mapping.
    tab_group: BTreeMap<TabId, ResourceGroupId>,
    /// Next group ID.
    next_id: u32,
}

impl ResourceManager {
    /// Create new resource manager.
    pub fn new() -> Self {
        let mut mgr = ResourceManager {
            groups: BTreeMap::new(),
            process_group: BTreeMap::new(),
            tab_group: BTreeMap::new(),
            next_id: 1,
        };

        // Create root group
        let root = ResourceGroup::new(ResourceGroupId::ROOT, "root", ResourceLimits::default());
        mgr.groups.insert(ResourceGroupId::ROOT, Mutex::new(root));

        mgr
    }

    /// Create a resource group.
    pub fn create_group(
        &mut self,
        name: &str,
        limits: ResourceLimits,
        parent: Option<ResourceGroupId>,
    ) -> ResourceGroupId {
        let id = ResourceGroupId(self.next_id);
        self.next_id += 1;

        let mut group = ResourceGroup::new(id, name, limits);
        group.parent = parent;

        // Add to parent's children
        if let Some(parent_id) = parent {
            if let Some(parent_group) = self.groups.get(&parent_id) {
                parent_group.lock().children.push(id);
            }
        }

        crate::serial_println!("[Resource] Created group {} ({})", id.0, name);

        self.groups.insert(id, Mutex::new(group));

        id
    }

    /// Create group for browser tab.
    pub fn create_tab_group(&mut self, tab: TabId) -> ResourceGroupId {
        let name = alloc::format!("tab_{}", tab.0);
        let id = self.create_group(
            &name,
            ResourceLimits::browser_tab(),
            Some(ResourceGroupId::ROOT),
        );
        self.tab_group.insert(tab, id);
        id
    }

    /// Add process to group.
    pub fn add_process(
        &mut self,
        group_id: ResourceGroupId,
        pid: ProcessId,
    ) -> Result<(), ResourceError> {
        let group = self.groups.get(&group_id).ok_or(ResourceError::NotFound)?;
        group.lock().add_process(pid)?;
        self.process_group.insert(pid, group_id);
        Ok(())
    }

    /// Get group for process.
    pub fn process_group(&self, pid: ProcessId) -> Option<ResourceGroupId> {
        self.process_group.get(&pid).copied()
    }

    /// Check memory allocation.
    pub fn check_memory_alloc(&self, pid: ProcessId, size: u64) -> Result<(), ResourceError> {
        let group_id = self
            .process_group
            .get(&pid)
            .copied()
            .unwrap_or(ResourceGroupId::ROOT);

        if let Some(group) = self.groups.get(&group_id) {
            group.lock().check_memory(size)?;
        }

        Ok(())
    }

    /// Report memory usage.
    pub fn report_memory(&self, pid: ProcessId, current: u64) {
        let group_id = self
            .process_group
            .get(&pid)
            .copied()
            .unwrap_or(ResourceGroupId::ROOT);

        if let Some(group) = self.groups.get(&group_id) {
            group.lock().usage.update_memory(current);
        }
    }

    /// Get group usage.
    pub fn get_usage(&self, group_id: ResourceGroupId) -> Option<(u64, u64)> {
        self.groups
            .get(&group_id)
            .map(|g| g.lock().usage.memory_snapshot())
    }

    /// Clean up for tab.
    pub fn cleanup_tab(&mut self, tab: TabId) {
        if let Some(group_id) = self.tab_group.remove(&tab) {
            // Remove all processes in this group
            if let Some(group) = self.groups.remove(&group_id) {
                let pids: alloc::vec::Vec<_> = group.lock().processes.keys().copied().collect();
                for pid in pids {
                    self.process_group.remove(&pid);
                }
            }
        }
    }
}

/// Global resource manager.
static RESOURCE_MANAGER: RwLock<Option<ResourceManager>> = RwLock::new(None);

/// Initialize resource manager.
pub fn init() {
    let mut mgr = RESOURCE_MANAGER.write();
    *mgr = Some(ResourceManager::new());
    crate::serial_println!("[Resource] Manager initialized");
}

/// Create resource group for tab.
pub fn create_tab_group(tab: TabId) -> Option<ResourceGroupId> {
    Some(RESOURCE_MANAGER.write().as_mut()?.create_tab_group(tab))
}

/// Add process to group.
pub fn add_process(group_id: ResourceGroupId, pid: ProcessId) -> Result<(), ResourceError> {
    RESOURCE_MANAGER
        .write()
        .as_mut()
        .ok_or(ResourceError::InvalidOperation)?
        .add_process(group_id, pid)
}

/// Check memory allocation.
pub fn check_memory(pid: ProcessId, size: u64) -> Result<(), ResourceError> {
    RESOURCE_MANAGER
        .read()
        .as_ref()
        .ok_or(ResourceError::InvalidOperation)?
        .check_memory_alloc(pid, size)
}

/// Report memory usage.
pub fn report_memory(pid: ProcessId, current: u64) {
    if let Some(mgr) = RESOURCE_MANAGER.read().as_ref() {
        mgr.report_memory(pid, current);
    }
}
