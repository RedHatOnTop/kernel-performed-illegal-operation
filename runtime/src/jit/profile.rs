//! Profiling Data for JIT Optimization
//!
//! This module collects and stores profiling information used
//! to guide optimization decisions.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Profile data for a function.
#[derive(Debug, Clone)]
pub struct ProfileData {
    /// Number of times the function was called.
    pub call_count: u32,
    /// Number of times loops executed.
    pub loop_iterations: u64,
    /// Branch taken/not-taken statistics.
    pub branch_stats: Vec<BranchStats>,
    /// Type feedback for polymorphic calls.
    pub type_feedback: Vec<TypeFeedback>,
    /// Allocation site statistics.
    pub allocation_sites: Vec<AllocationSite>,
    /// Estimated execution time (cycles).
    pub estimated_cycles: u64,
}

impl ProfileData {
    /// Create new empty profile data.
    pub fn new() -> Self {
        Self {
            call_count: 0,
            loop_iterations: 0,
            branch_stats: Vec::new(),
            type_feedback: Vec::new(),
            allocation_sites: Vec::new(),
            estimated_cycles: 0,
        }
    }

    /// Record a function call.
    pub fn record_call(&mut self) {
        self.call_count = self.call_count.saturating_add(1);
    }

    /// Record loop iterations.
    pub fn record_loop(&mut self, iterations: u64) {
        self.loop_iterations = self.loop_iterations.saturating_add(iterations);
    }

    /// Record branch taken.
    pub fn record_branch_taken(&mut self, branch_idx: usize) {
        self.ensure_branch(branch_idx);
        if let Some(stats) = self.branch_stats.get_mut(branch_idx) {
            stats.taken = stats.taken.saturating_add(1);
        }
    }

    /// Record branch not taken.
    pub fn record_branch_not_taken(&mut self, branch_idx: usize) {
        self.ensure_branch(branch_idx);
        if let Some(stats) = self.branch_stats.get_mut(branch_idx) {
            stats.not_taken = stats.not_taken.saturating_add(1);
        }
    }

    fn ensure_branch(&mut self, idx: usize) {
        while self.branch_stats.len() <= idx {
            self.branch_stats.push(BranchStats::default());
        }
    }

    /// Get branch prediction accuracy.
    pub fn branch_prediction_accuracy(&self, branch_idx: usize) -> Option<f32> {
        self.branch_stats.get(branch_idx).map(|s| s.accuracy())
    }

    /// Check if function is hot enough for optimization.
    pub fn is_hot(&self, threshold: u32) -> bool {
        self.call_count >= threshold
    }

    /// Merge with another profile.
    pub fn merge(&mut self, other: &ProfileData) {
        self.call_count = self.call_count.saturating_add(other.call_count);
        self.loop_iterations = self.loop_iterations.saturating_add(other.loop_iterations);

        for (i, other_stats) in other.branch_stats.iter().enumerate() {
            self.ensure_branch(i);
            if let Some(stats) = self.branch_stats.get_mut(i) {
                stats.taken = stats.taken.saturating_add(other_stats.taken);
                stats.not_taken = stats.not_taken.saturating_add(other_stats.not_taken);
            }
        }
    }
}

impl Default for ProfileData {
    fn default() -> Self {
        Self::new()
    }
}

/// Branch statistics.
#[derive(Debug, Clone, Default)]
pub struct BranchStats {
    /// Times branch was taken.
    pub taken: u32,
    /// Times branch was not taken.
    pub not_taken: u32,
}

impl BranchStats {
    /// Get prediction accuracy assuming we predict the more likely outcome.
    pub fn accuracy(&self) -> f32 {
        let total = self.taken + self.not_taken;
        if total == 0 {
            return 0.5;
        }

        let max = self.taken.max(self.not_taken);
        max as f32 / total as f32
    }

    /// Check if branch is biased (>80% one way).
    pub fn is_biased(&self) -> bool {
        self.accuracy() > 0.8
    }

    /// Get predicted outcome.
    pub fn predicted_taken(&self) -> bool {
        self.taken >= self.not_taken
    }
}

/// Type feedback for polymorphic sites.
#[derive(Debug, Clone)]
pub struct TypeFeedback {
    /// Call site offset.
    pub offset: u32,
    /// Observed types (function indices).
    pub observed_types: Vec<u32>,
    /// Counts for each type.
    pub counts: Vec<u32>,
}

impl TypeFeedback {
    /// Create new type feedback.
    pub fn new(offset: u32) -> Self {
        Self {
            offset,
            observed_types: Vec::new(),
            counts: Vec::new(),
        }
    }

    /// Record an observed type.
    pub fn record(&mut self, type_idx: u32) {
        if let Some(pos) = self.observed_types.iter().position(|&t| t == type_idx) {
            self.counts[pos] = self.counts[pos].saturating_add(1);
        } else {
            self.observed_types.push(type_idx);
            self.counts.push(1);
        }
    }

    /// Check if call site is monomorphic.
    pub fn is_monomorphic(&self) -> bool {
        self.observed_types.len() == 1
    }

    /// Check if call site is megamorphic (too many types).
    pub fn is_megamorphic(&self) -> bool {
        self.observed_types.len() > 4
    }

    /// Get most common type.
    pub fn dominant_type(&self) -> Option<u32> {
        self.counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(i, _)| self.observed_types[i])
    }
}

/// Allocation site statistics.
#[derive(Debug, Clone)]
pub struct AllocationSite {
    /// Bytecode offset of allocation.
    pub offset: u32,
    /// Number of allocations.
    pub count: u32,
    /// Total bytes allocated.
    pub total_bytes: u64,
    /// Average object size.
    pub avg_size: u32,
}

/// Hotness counter for tiered compilation.
pub struct HotnessCounter {
    /// Current count.
    count: AtomicU32,
    /// Baseline threshold.
    baseline_threshold: u32,
    /// Optimized threshold.
    optimized_threshold: u32,
}

impl HotnessCounter {
    /// Create a new hotness counter.
    pub fn new(baseline_threshold: u32, optimized_threshold: u32) -> Self {
        Self {
            count: AtomicU32::new(0),
            baseline_threshold,
            optimized_threshold,
        }
    }

    /// Increment the counter.
    pub fn increment(&self) -> HotnessLevel {
        let count = self.count.fetch_add(1, Ordering::Relaxed) + 1;

        if count >= self.optimized_threshold {
            HotnessLevel::Optimized
        } else if count >= self.baseline_threshold {
            HotnessLevel::Baseline
        } else {
            HotnessLevel::Cold
        }
    }

    /// Get current hotness level.
    pub fn level(&self) -> HotnessLevel {
        let count = self.count.load(Ordering::Relaxed);

        if count >= self.optimized_threshold {
            HotnessLevel::Optimized
        } else if count >= self.baseline_threshold {
            HotnessLevel::Baseline
        } else {
            HotnessLevel::Cold
        }
    }

    /// Reset the counter.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
    }

    /// Get current count.
    pub fn count(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }
}

/// Hotness level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HotnessLevel {
    /// Function is cold (use interpreter).
    Cold,
    /// Function is warm (use baseline JIT).
    Baseline,
    /// Function is hot (use optimizing JIT).
    Optimized,
}

/// Global profiler for collecting runtime data.
pub struct Profiler {
    /// Profile data per function.
    profiles: spin::RwLock<BTreeMap<u64, ProfileData>>,
    /// Sampling rate (1 = every call, 10 = every 10th call).
    sampling_rate: u32,
    /// Sample counter.
    sample_counter: AtomicU64,
}

impl Profiler {
    /// Create a new profiler.
    pub fn new(sampling_rate: u32) -> Self {
        Self {
            profiles: spin::RwLock::new(BTreeMap::new()),
            sampling_rate: sampling_rate.max(1),
            sample_counter: AtomicU64::new(0),
        }
    }

    /// Check if we should sample this call.
    pub fn should_sample(&self) -> bool {
        let count = self.sample_counter.fetch_add(1, Ordering::Relaxed);
        count % self.sampling_rate as u64 == 0
    }

    /// Record a function call.
    pub fn record_call(&self, func_id: u64) {
        if self.should_sample() {
            let mut profiles = self.profiles.write();
            profiles
                .entry(func_id)
                .or_insert_with(ProfileData::new)
                .record_call();
        }
    }

    /// Get profile data for a function.
    pub fn get_profile(&self, func_id: u64) -> Option<ProfileData> {
        self.profiles.read().get(&func_id).cloned()
    }

    /// Clear all profile data.
    pub fn clear(&self) {
        self.profiles.write().clear();
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new(1)
    }
}
