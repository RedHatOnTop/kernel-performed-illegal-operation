//! Fuzzing Harness
//!
//! Main fuzzing harness and orchestration.

use crate::{
    Corpus, CoverageTracker, CrashInfo, CrashType, FuzzResult, FuzzStats, FuzzTarget, FuzzerConfig,
    Mutator, StackFrame,
};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Main fuzzer harness
pub struct FuzzHarness {
    /// Configuration
    config: FuzzerConfig,
    /// Fuzz targets
    targets: Vec<Box<dyn FuzzTarget>>,
    /// Mutator
    mutator: Mutator,
    /// Corpus
    corpus: Corpus,
    /// Coverage tracker
    coverage: CoverageTracker,
    /// Statistics
    stats: FuzzStats,
    /// Found crashes
    crashes: Vec<CrashEntry>,
}

/// A crash entry
#[derive(Debug, Clone)]
pub struct CrashEntry {
    /// Input that caused crash
    pub input: Vec<u8>,
    /// Crash info
    pub info: CrashInfo,
    /// Target name
    pub target: String,
    /// Hash for deduplication
    pub hash: u64,
}

impl FuzzHarness {
    /// Create new harness
    pub fn new(config: FuzzerConfig) -> Self {
        let seed = 12345u64; // Would use random in real implementation

        Self {
            config,
            targets: Vec::new(),
            mutator: Mutator::new(seed),
            corpus: Corpus::new(10000),
            coverage: CoverageTracker::new(65536),
            stats: FuzzStats::default(),
            crashes: Vec::new(),
        }
    }

    /// Add fuzz target
    pub fn add_target(&mut self, target: Box<dyn FuzzTarget>) {
        self.targets.push(target);
    }

    /// Add corpus entry
    pub fn add_corpus(&mut self, data: Vec<u8>) {
        self.corpus.add(data, 0);
    }

    /// Add dictionary entries
    pub fn add_dictionary(&mut self, entries: Vec<Vec<u8>>) {
        for entry in entries {
            self.mutator.add_dictionary(entry);
        }
    }

    /// Run fuzzing loop
    pub fn run(&mut self, iterations: u64) -> FuzzReport {
        for _ in 0..iterations {
            self.fuzz_iteration();
        }

        FuzzReport {
            stats: self.stats.clone(),
            crashes: self.crashes.clone(),
            corpus_size: self.corpus.len(),
            coverage_edges: self.coverage.total_edges(),
        }
    }

    /// Run single fuzzing iteration
    fn fuzz_iteration(&mut self) {
        self.stats.iterations += 1;
        self.coverage.reset_iteration();

        // Get base input from corpus or generate new
        let mut input = if let Some(base) = self.corpus.random_entry(self.stats.iterations) {
            base.clone()
        } else {
            Vec::new()
        };

        // Mutate input
        self.mutator.mutate(&mut input);

        // Collect results first to avoid borrow issues
        let mut crash_info: Option<(CrashInfo, String)> = None;
        let mut has_timeout = false;
        let mut is_interesting = false;

        // Run against all targets
        for target in &mut self.targets {
            let result = target.fuzz(&input);

            match &result {
                FuzzResult::Crash(info) => {
                    crash_info = Some((info.clone(), String::from(target.name())));
                }
                FuzzResult::Timeout => {
                    has_timeout = true;
                }
                FuzzResult::Interesting(_) => {
                    is_interesting = true;
                }
                _ => {}
            }

            target.reset();
        }

        // Handle results after loop ends
        if let Some((info, name)) = crash_info {
            self.handle_crash(&input, &info, &name);
        }
        if has_timeout {
            self.stats.timeouts += 1;
        }
        if is_interesting {
            if self
                .corpus
                .add(input.clone(), self.coverage.total_edges() as u64)
            {
                self.stats.corpus_size = self.corpus.len();
            }
        }

        // Check for new coverage
        if self.coverage.has_new_coverage() {
            self.corpus
                .add(input.clone(), self.coverage.total_edges() as u64);
            self.stats.coverage_edges = self.coverage.total_edges();
        }
    }

    fn handle_crash(&mut self, input: &[u8], info: &CrashInfo, target_name: &str) {
        // Calculate crash hash for deduplication
        let hash = self.hash_crash(info);

        // Check if unique
        let is_unique = !self.crashes.iter().any(|c| c.hash == hash);

        self.stats.crashes += 1;

        if is_unique {
            self.stats.unique_crashes += 1;
            self.crashes.push(CrashEntry {
                input: input.to_vec(),
                info: info.clone(),
                target: String::from(target_name),
                hash,
            });
        }
    }

    fn hash_crash(&self, info: &CrashInfo) -> u64 {
        // Simple hash based on crash type and top stack frames
        let mut hash = match info.crash_type {
            CrashType::Segfault => 1,
            CrashType::StackOverflow => 2,
            CrashType::Abort => 3,
            CrashType::Panic => 4,
            CrashType::IntegerOverflow => 5,
            CrashType::OutOfMemory => 6,
            CrashType::Unknown => 0,
        };

        for (i, frame) in info.stack_trace.iter().take(5).enumerate() {
            hash ^= frame.address.wrapping_mul((i + 1) as u64);
        }

        hash
    }

    /// Get current statistics
    pub fn stats(&self) -> &FuzzStats {
        &self.stats
    }

    /// Get found crashes
    pub fn crashes(&self) -> &[CrashEntry] {
        &self.crashes
    }

    /// Get corpus size
    pub fn corpus_size(&self) -> usize {
        self.corpus.len()
    }
}

/// Fuzzing report
#[derive(Debug, Clone)]
pub struct FuzzReport {
    /// Statistics
    pub stats: FuzzStats,
    /// Crashes found
    pub crashes: Vec<CrashEntry>,
    /// Final corpus size
    pub corpus_size: usize,
    /// Coverage edges
    pub coverage_edges: usize,
}

impl FuzzReport {
    /// Format as text
    pub fn format(&self) -> String {
        let mut output = String::new();

        output.push_str("=== Fuzzing Report ===\n\n");

        output.push_str("Statistics:\n");
        output.push_str(&format!("  Iterations: {}\n", self.stats.iterations));
        output.push_str(&format!("  Exec/sec: {:.2}\n", self.stats.execs_per_sec));
        output.push_str(&format!("  Runtime: {:.2}s\n", self.stats.runtime_s));
        output.push_str(&format!(
            "  Crashes: {} ({} unique)\n",
            self.stats.crashes, self.stats.unique_crashes
        ));
        output.push_str(&format!("  Timeouts: {}\n", self.stats.timeouts));
        output.push_str(&format!("  Corpus: {}\n", self.corpus_size));
        output.push_str(&format!("  Coverage: {} edges\n", self.coverage_edges));

        if !self.crashes.is_empty() {
            output.push_str("\nCrashes:\n");
            for crash in &self.crashes {
                output.push_str(&format!(
                    "  - {} in {}: {:?}\n",
                    format!("{:?}", crash.info.crash_type),
                    crash.target,
                    crash.info.address
                ));
            }
        }

        output
    }
}

/// Crash minimizer
pub struct CrashMinimizer<'a> {
    /// Target to reproduce crash
    target: &'a mut dyn FuzzTarget,
    /// Maximum attempts
    max_attempts: usize,
}

impl<'a> CrashMinimizer<'a> {
    /// Create new minimizer
    pub fn new(target: &'a mut dyn FuzzTarget) -> Self {
        Self {
            target,
            max_attempts: 10000,
        }
    }

    /// Minimize crash input
    pub fn minimize(&mut self, input: Vec<u8>) -> Vec<u8> {
        let mut current = input;
        let mut improved = true;

        while improved {
            improved = false;

            // Try removing chunks
            for chunk_size in [32, 16, 8, 4, 2, 1] {
                let result = self.try_remove_chunks(&current, chunk_size);
                if result.len() < current.len() && self.still_crashes(&result) {
                    current = result;
                    improved = true;
                    break;
                }
            }

            // Try zeroing bytes
            let result = self.try_zero_bytes(&current);
            if result != current && self.still_crashes(&result) {
                current = result;
                improved = true;
            }
        }

        current
    }

    fn try_remove_chunks(&mut self, input: &[u8], chunk_size: usize) -> Vec<u8> {
        let mut best = input.to_vec();

        for start in (0..input.len()).step_by(chunk_size) {
            let end = (start + chunk_size).min(input.len());

            let mut candidate = Vec::new();
            candidate.extend_from_slice(&input[..start]);
            candidate.extend_from_slice(&input[end..]);

            if self.still_crashes(&candidate) && candidate.len() < best.len() {
                best = candidate;
            }
        }

        best
    }

    fn try_zero_bytes(&mut self, input: &[u8]) -> Vec<u8> {
        let mut result = input.to_vec();

        for i in 0..result.len() {
            if result[i] != 0 {
                let original = result[i];
                result[i] = 0;

                if !self.still_crashes(&result) {
                    result[i] = original;
                }
            }
        }

        result
    }

    fn still_crashes(&mut self, input: &[u8]) -> bool {
        self.target.reset();
        matches!(self.target.fuzz(input), FuzzResult::Crash(_))
    }
}

/// Create a crash info from a panic
pub fn crash_from_panic(message: &str) -> CrashInfo {
    CrashInfo {
        crash_type: CrashType::Panic,
        address: None,
        stack_trace: vec![StackFrame {
            function: Some(String::from(message)),
            address: 0,
            file: None,
            line: None,
        }],
        registers: Vec::new(),
    }
}

/// Run a quick fuzz test
pub fn quick_fuzz<T: FuzzTarget>(mut target: T, corpus: Vec<Vec<u8>>, iterations: u64) -> bool {
    let mut crashes = false;
    let mut mutator = Mutator::new(42);
    let mut current_corpus = corpus;

    for _ in 0..iterations {
        let base_idx: Option<usize> = if current_corpus.is_empty() {
            None
        } else {
            Some(mutator.mutate_seed() as usize % current_corpus.len())
        };

        let mut input = base_idx
            .map(|i: usize| current_corpus[i].clone())
            .unwrap_or_default();

        mutator.mutate(&mut input);

        let result = target.fuzz(&input);

        if result.is_crash() {
            crashes = true;
        } else if result.is_interesting() {
            current_corpus.push(input);
        }

        target.reset();
    }

    crashes
}

impl Mutator {
    /// Get random value (exposed for quick_fuzz)
    pub fn mutate_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.seed
    }
}
