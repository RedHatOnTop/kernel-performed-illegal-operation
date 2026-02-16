//! Parallel Layout Engine
//!
//! Implements parallel layout computation using work stealing and
//! divide-and-conquer algorithms for improved performance on multi-core systems.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    Parallel Layout Engine                                │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  ┌──────────────────┐     ┌──────────────────────────────────┐         │
//! │  │  Task Scheduler  │     │      Work Stealing Queues        │         │
//! │  │  (coordinates)   │────>│  [Queue 0] [Queue 1] [Queue 2]   │         │
//! │  └──────────────────┘     └──────────────────────────────────┘         │
//! │           │                            │                               │
//! │           ▼                            ▼                               │
//! │  ┌──────────────────────────────────────────────────────────────┐     │
//! │  │                    Layout Workers                            │     │
//! │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐         │     │
//! │  │  │Worker 0 │  │Worker 1 │  │Worker 2 │  │Worker 3 │  ...    │     │
//! │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘         │     │
//! │  └──────────────────────────────────────────────────────────────┘     │
//! │           │                                                           │
//! │           ▼                                                           │
//! │  ┌──────────────────────────────────────────────────────────────┐    │
//! │  │                Layout Tasks (subtrees)                       │    │
//! │  │  [Block Layout] [Inline Layout] [Flex Layout] [Grid Layout]  │    │
//! │  └──────────────────────────────────────────────────────────────┘    │
//! │                                                                       │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::box_model::{BoxDimensions, Rect};
use crate::layout_box::LayoutBox;

/// Maximum number of worker threads.
pub const MAX_WORKERS: usize = 16;

/// Minimum subtree size to parallelize.
pub const MIN_PARALLEL_THRESHOLD: usize = 8;

/// Parallel layout scheduler.
pub struct ParallelScheduler {
    /// Work queues for each worker.
    work_queues: Vec<WorkQueue>,
    /// Number of active workers.
    worker_count: AtomicUsize,
    /// Global task counter.
    task_counter: AtomicU64,
    /// Scheduler statistics.
    stats: SchedulerStats,
    /// Whether scheduler is running.
    running: AtomicBool,
}

/// Work queue for a single worker.
pub struct WorkQueue {
    /// Local tasks (LIFO for cache locality).
    local: Mutex<VecDeque<LayoutTask>>,
    /// Shared tasks (for stealing).
    shared: Mutex<VecDeque<LayoutTask>>,
    /// Number of pending tasks.
    pending: AtomicUsize,
}

impl WorkQueue {
    /// Create a new work queue.
    pub fn new() -> Self {
        Self {
            local: Mutex::new(VecDeque::new()),
            shared: Mutex::new(VecDeque::new()),
            pending: AtomicUsize::new(0),
        }
    }

    /// Push a task to the local queue.
    pub fn push_local(&self, task: LayoutTask) {
        self.local.lock().push_back(task);
        self.pending.fetch_add(1, Ordering::Relaxed);
    }

    /// Push a task to the shared queue (for stealing).
    pub fn push_shared(&self, task: LayoutTask) {
        self.shared.lock().push_back(task);
        self.pending.fetch_add(1, Ordering::Relaxed);
    }

    /// Pop from local queue (LIFO).
    pub fn pop_local(&self) -> Option<LayoutTask> {
        let task = self.local.lock().pop_back();
        if task.is_some() {
            self.pending.fetch_sub(1, Ordering::Relaxed);
        }
        task
    }

    /// Steal from shared queue (FIFO for fairness).
    pub fn steal(&self) -> Option<LayoutTask> {
        let task = self.shared.lock().pop_front();
        if task.is_some() {
            self.pending.fetch_sub(1, Ordering::Relaxed);
        }
        task
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.pending.load(Ordering::Relaxed) == 0
    }

    /// Get pending count.
    pub fn pending_count(&self) -> usize {
        self.pending.load(Ordering::Relaxed)
    }
}

impl Default for WorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Layout task representing a subtree to layout.
pub struct LayoutTask {
    /// Task ID.
    pub id: u64,
    /// Task type.
    pub task_type: LayoutTaskType,
    /// Containing block dimensions.
    pub containing_block: Rect,
    /// Task priority (higher = more important).
    pub priority: u32,
    /// Parent task ID (for dependency tracking).
    pub parent_id: Option<u64>,
    /// Completion callback data.
    pub user_data: u64,
}

impl LayoutTask {
    /// Create a new layout task.
    pub fn new(id: u64, task_type: LayoutTaskType, containing_block: Rect, priority: u32) -> Self {
        Self {
            id,
            task_type,
            containing_block,
            priority,
            parent_id: None,
            user_data: 0,
        }
    }

    /// Set parent task.
    pub fn with_parent(mut self, parent_id: u64) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

/// Types of layout tasks.
#[derive(Debug, Clone)]
pub enum LayoutTaskType {
    /// Block formatting context.
    Block {
        /// Box ID in layout tree.
        box_id: usize,
        /// Number of children.
        child_count: usize,
    },
    /// Inline formatting context.
    Inline {
        /// Starting box ID.
        start_id: usize,
        /// Ending box ID.
        end_id: usize,
    },
    /// Flexbox layout.
    Flex {
        /// Container box ID.
        container_id: usize,
        /// Flex items.
        item_count: usize,
    },
    /// Grid layout.
    Grid {
        /// Container box ID.
        container_id: usize,
        /// Row count.
        rows: usize,
        /// Column count.
        cols: usize,
    },
    /// Text measurement.
    TextMeasure {
        /// Text run ID.
        run_id: usize,
    },
    /// Image sizing.
    ImageSize {
        /// Image ID.
        image_id: usize,
    },
    /// Merge child results.
    MergeResults {
        /// Parent box ID.
        parent_id: usize,
        /// Child result IDs.
        child_ids: Vec<usize>,
    },
}

/// Layout task result.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Task ID.
    pub task_id: u64,
    /// Computed dimensions.
    pub dimensions: BoxDimensions,
    /// Child results (for merging).
    pub children: Vec<LayoutResult>,
    /// Whether layout succeeded.
    pub success: bool,
    /// Execution time (nanoseconds).
    pub execution_ns: u64,
}

impl LayoutResult {
    /// Create a successful result.
    pub fn success(task_id: u64, dimensions: BoxDimensions) -> Self {
        Self {
            task_id,
            dimensions,
            children: Vec::new(),
            success: true,
            execution_ns: 0,
        }
    }

    /// Create a failed result.
    pub fn failure(task_id: u64) -> Self {
        Self {
            task_id,
            dimensions: BoxDimensions::default(),
            children: Vec::new(),
            success: false,
            execution_ns: 0,
        }
    }
}

/// Scheduler statistics.
#[derive(Debug, Default)]
pub struct SchedulerStats {
    /// Total tasks scheduled.
    pub tasks_scheduled: AtomicU64,
    /// Total tasks completed.
    pub tasks_completed: AtomicU64,
    /// Total tasks stolen.
    pub tasks_stolen: AtomicU64,
    /// Total execution time (nanoseconds).
    pub total_execution_ns: AtomicU64,
    /// Maximum queue depth.
    pub max_queue_depth: AtomicU32,
}

impl SchedulerStats {
    pub const fn new() -> Self {
        Self {
            tasks_scheduled: AtomicU64::new(0),
            tasks_completed: AtomicU64::new(0),
            tasks_stolen: AtomicU64::new(0),
            total_execution_ns: AtomicU64::new(0),
            max_queue_depth: AtomicU32::new(0),
        }
    }
}

impl ParallelScheduler {
    /// Create a new scheduler.
    pub fn new(worker_count: usize) -> Self {
        let count = worker_count.min(MAX_WORKERS);
        let mut work_queues = Vec::with_capacity(count);
        for _ in 0..count {
            work_queues.push(WorkQueue::new());
        }

        Self {
            work_queues,
            worker_count: AtomicUsize::new(count),
            task_counter: AtomicU64::new(0),
            stats: SchedulerStats::new(),
            running: AtomicBool::new(false),
        }
    }

    /// Start the scheduler.
    pub fn start(&self) {
        self.running.store(true, Ordering::Release);
    }

    /// Stop the scheduler.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Check if running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Schedule a task.
    pub fn schedule(&self, task: LayoutTask, worker_hint: Option<usize>) -> u64 {
        let worker = worker_hint.unwrap_or_else(|| {
            // Simple load balancing: choose worker with fewest tasks
            let mut min_worker = 0;
            let mut min_count = usize::MAX;
            for (i, queue) in self.work_queues.iter().enumerate() {
                let count = queue.pending_count();
                if count < min_count {
                    min_count = count;
                    min_worker = i;
                }
            }
            min_worker
        }) % self.work_queues.len();

        let task_id = task.id;
        self.work_queues[worker].push_shared(task);
        self.stats.tasks_scheduled.fetch_add(1, Ordering::Relaxed);

        task_id
    }

    /// Generate a new task ID.
    pub fn next_task_id(&self) -> u64 {
        self.task_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Process tasks on a worker.
    pub fn process_worker(&self, worker_id: usize) -> Vec<LayoutResult> {
        let mut results = Vec::new();

        // Try local queue first
        while let Some(task) = self.work_queues[worker_id].pop_local() {
            if let Some(result) = self.execute_task(task) {
                results.push(result);
            }
        }

        // Try shared queue
        while let Some(task) = self.work_queues[worker_id].steal() {
            if let Some(result) = self.execute_task(task) {
                results.push(result);
            }
        }

        // Try stealing from other workers
        for i in 0..self.work_queues.len() {
            if i != worker_id {
                while let Some(task) = self.work_queues[i].steal() {
                    self.stats.tasks_stolen.fetch_add(1, Ordering::Relaxed);
                    if let Some(result) = self.execute_task(task) {
                        results.push(result);
                    }
                }
            }
        }

        results
    }

    /// Execute a single task.
    fn execute_task(&self, task: LayoutTask) -> Option<LayoutResult> {
        let result = match task.task_type {
            LayoutTaskType::Block {
                box_id,
                child_count,
            } => self.execute_block_layout(task.id, box_id, child_count, &task.containing_block),
            LayoutTaskType::Inline { start_id, end_id } => {
                self.execute_inline_layout(task.id, start_id, end_id, &task.containing_block)
            }
            LayoutTaskType::Flex {
                container_id,
                item_count,
            } => {
                self.execute_flex_layout(task.id, container_id, item_count, &task.containing_block)
            }
            LayoutTaskType::Grid {
                container_id,
                rows,
                cols,
            } => {
                self.execute_grid_layout(task.id, container_id, rows, cols, &task.containing_block)
            }
            LayoutTaskType::TextMeasure { run_id } => self.execute_text_measure(task.id, run_id),
            LayoutTaskType::ImageSize { image_id } => self.execute_image_size(task.id, image_id),
            LayoutTaskType::MergeResults {
                parent_id,
                ref child_ids,
            } => self.execute_merge_results(task.id, parent_id, child_ids),
        };

        self.stats.tasks_completed.fetch_add(1, Ordering::Relaxed);
        Some(result)
    }

    /// Execute block layout.
    fn execute_block_layout(
        &self,
        task_id: u64,
        _box_id: usize,
        _child_count: usize,
        containing_block: &Rect,
    ) -> LayoutResult {
        // Simulated block layout
        let dimensions = BoxDimensions {
            content: Rect {
                x: containing_block.x,
                y: containing_block.y,
                width: containing_block.width,
                height: 0.0, // Will be computed from children
            },
            ..Default::default()
        };

        LayoutResult::success(task_id, dimensions)
    }

    /// Execute inline layout.
    fn execute_inline_layout(
        &self,
        task_id: u64,
        _start_id: usize,
        _end_id: usize,
        containing_block: &Rect,
    ) -> LayoutResult {
        let dimensions = BoxDimensions {
            content: Rect {
                x: containing_block.x,
                y: containing_block.y,
                width: containing_block.width,
                height: 20.0, // Line height
            },
            ..Default::default()
        };

        LayoutResult::success(task_id, dimensions)
    }

    /// Execute flexbox layout.
    fn execute_flex_layout(
        &self,
        task_id: u64,
        _container_id: usize,
        _item_count: usize,
        containing_block: &Rect,
    ) -> LayoutResult {
        let dimensions = BoxDimensions {
            content: *containing_block,
            ..Default::default()
        };

        LayoutResult::success(task_id, dimensions)
    }

    /// Execute grid layout.
    fn execute_grid_layout(
        &self,
        task_id: u64,
        _container_id: usize,
        _rows: usize,
        _cols: usize,
        containing_block: &Rect,
    ) -> LayoutResult {
        let dimensions = BoxDimensions {
            content: *containing_block,
            ..Default::default()
        };

        LayoutResult::success(task_id, dimensions)
    }

    /// Execute text measurement.
    fn execute_text_measure(&self, task_id: u64, _run_id: usize) -> LayoutResult {
        let dimensions = BoxDimensions {
            content: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0, // Measured width
                height: 16.0, // Font size
            },
            ..Default::default()
        };

        LayoutResult::success(task_id, dimensions)
    }

    /// Execute image size computation.
    fn execute_image_size(&self, task_id: u64, _image_id: usize) -> LayoutResult {
        let dimensions = BoxDimensions {
            content: Rect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 150.0,
            },
            ..Default::default()
        };

        LayoutResult::success(task_id, dimensions)
    }

    /// Merge child results.
    fn execute_merge_results(
        &self,
        task_id: u64,
        _parent_id: usize,
        _child_ids: &[usize],
    ) -> LayoutResult {
        LayoutResult::success(task_id, BoxDimensions::default())
    }

    /// Get scheduler statistics.
    pub fn stats(&self) -> &SchedulerStats {
        &self.stats
    }

    /// Get worker count.
    pub fn worker_count(&self) -> usize {
        self.worker_count.load(Ordering::Relaxed)
    }
}

/// Parallel layout context.
pub struct ParallelLayoutContext {
    /// Scheduler.
    scheduler: Arc<ParallelScheduler>,
    /// Layout tree reference.
    // In a real implementation, this would hold references to the layout tree
    /// Results cache.
    results: Mutex<Vec<LayoutResult>>,
    /// Pending dependencies.
    pending_deps: Mutex<Vec<(u64, Vec<u64>)>>,
}

impl ParallelLayoutContext {
    /// Create a new parallel layout context.
    pub fn new(worker_count: usize) -> Self {
        Self {
            scheduler: Arc::new(ParallelScheduler::new(worker_count)),
            results: Mutex::new(Vec::new()),
            pending_deps: Mutex::new(Vec::new()),
        }
    }

    /// Start parallel layout.
    pub fn layout(&self, containing_block: Rect) {
        self.scheduler.start();

        // Create root task
        let root_task = LayoutTask::new(
            self.scheduler.next_task_id(),
            LayoutTaskType::Block {
                box_id: 0,
                child_count: 0,
            },
            containing_block,
            100, // Highest priority
        );

        self.scheduler.schedule(root_task, Some(0));
    }

    /// Process layout on a worker thread.
    pub fn process(&self, worker_id: usize) {
        let results = self.scheduler.process_worker(worker_id);
        self.results.lock().extend(results);
    }

    /// Wait for layout to complete.
    pub fn wait(&self) -> Vec<LayoutResult> {
        // In a real implementation, this would wait for all tasks
        // For now, just return collected results
        let mut results = self.results.lock();
        core::mem::take(&mut *results)
    }

    /// Get scheduler reference.
    pub fn scheduler(&self) -> &ParallelScheduler {
        &self.scheduler
    }
}

/// Subtree partitioner for parallel layout.
pub struct SubtreePartitioner {
    /// Minimum subtree size for parallelization.
    min_size: usize,
    /// Maximum depth for parallelization.
    max_depth: usize,
}

impl SubtreePartitioner {
    /// Create a new partitioner.
    pub fn new(min_size: usize, max_depth: usize) -> Self {
        Self {
            min_size,
            max_depth,
        }
    }

    /// Check if subtree should be parallelized.
    pub fn should_parallelize(&self, subtree_size: usize, depth: usize) -> bool {
        subtree_size >= self.min_size && depth < self.max_depth
    }

    /// Partition a subtree into parallel tasks.
    pub fn partition(&self, _root: &LayoutBox, depth: usize) -> Vec<LayoutTaskType> {
        let mut tasks = Vec::new();

        if depth >= self.max_depth {
            return tasks;
        }

        // In a real implementation, this would analyze the subtree
        // and create appropriate tasks for parallel execution
        tasks
    }
}

impl Default for SubtreePartitioner {
    fn default() -> Self {
        Self::new(MIN_PARALLEL_THRESHOLD, 4)
    }
}

/// Dependency tracker for layout tasks.
pub struct DependencyTracker {
    /// Task dependencies: (task_id, depends_on).
    dependencies: Mutex<Vec<(u64, Vec<u64>)>>,
    /// Completed tasks.
    completed: Mutex<Vec<u64>>,
}

impl DependencyTracker {
    /// Create a new dependency tracker.
    pub fn new() -> Self {
        Self {
            dependencies: Mutex::new(Vec::new()),
            completed: Mutex::new(Vec::new()),
        }
    }

    /// Add a dependency.
    pub fn add_dependency(&self, task_id: u64, depends_on: u64) {
        let mut deps = self.dependencies.lock();

        // Find or create entry for task
        if let Some(entry) = deps.iter_mut().find(|(id, _)| *id == task_id) {
            entry.1.push(depends_on);
        } else {
            deps.push((task_id, alloc::vec![depends_on]));
        }
    }

    /// Mark task as completed.
    pub fn complete(&self, task_id: u64) {
        self.completed.lock().push(task_id);
    }

    /// Check if task is ready to execute.
    pub fn is_ready(&self, task_id: u64) -> bool {
        let deps = self.dependencies.lock();
        let completed = self.completed.lock();

        if let Some((_, required)) = deps
            .iter()
            .find(|(id, _): &&(u64, Vec<u64>)| *id == task_id)
        {
            required.iter().all(|dep: &u64| completed.contains(dep))
        } else {
            true // No dependencies
        }
    }

    /// Get ready tasks.
    pub fn get_ready_tasks(&self) -> Vec<u64> {
        let deps = self.dependencies.lock();
        let completed = self.completed.lock();

        deps.iter()
            .filter(|(_, required): &&(u64, Vec<u64>)| {
                required.iter().all(|dep: &u64| completed.contains(dep))
            })
            .map(|(id, _)| *id)
            .collect()
    }
}

impl Default for DependencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_queue() {
        let queue = WorkQueue::new();

        let task = LayoutTask::new(
            1,
            LayoutTaskType::Block {
                box_id: 0,
                child_count: 0,
            },
            Rect::default(),
            10,
        );

        queue.push_local(task);
        assert!(!queue.is_empty());
        assert_eq!(queue.pending_count(), 1);

        let popped = queue.pop_local();
        assert!(popped.is_some());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_scheduler() {
        let scheduler = ParallelScheduler::new(4);

        let task = LayoutTask::new(
            scheduler.next_task_id(),
            LayoutTaskType::Block {
                box_id: 0,
                child_count: 0,
            },
            Rect::default(),
            10,
        );

        scheduler.schedule(task, None);
        assert_eq!(scheduler.stats.tasks_scheduled.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_dependency_tracker() {
        let tracker = DependencyTracker::new();

        tracker.add_dependency(2, 1);
        tracker.add_dependency(3, 2);

        assert!(!tracker.is_ready(2));
        assert!(tracker.is_ready(1));

        tracker.complete(1);
        assert!(tracker.is_ready(2));
        assert!(!tracker.is_ready(3));

        tracker.complete(2);
        assert!(tracker.is_ready(3));
    }
}
