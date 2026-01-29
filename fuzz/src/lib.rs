//! KPIO Fuzzing Infrastructure
//!
//! Provides fuzzing harnesses for parser and protocol testing.

#![no_std]
extern crate alloc;

pub mod html;
pub mod css;
pub mod js;
pub mod network;
pub mod harness;

use alloc::string::String;
use alloc::vec::Vec;

/// Fuzzing target trait
pub trait FuzzTarget {
    /// Name of the fuzz target
    fn name(&self) -> &str;
    
    /// Run fuzzing iteration with input
    fn fuzz(&mut self, input: &[u8]) -> FuzzResult;
    
    /// Reset state between iterations
    fn reset(&mut self);
}

/// Result of a fuzz iteration
#[derive(Debug, Clone)]
pub enum FuzzResult {
    /// Input processed successfully
    Ok,
    /// Parsing error (expected for malformed input)
    ParseError(String),
    /// Timeout
    Timeout,
    /// Crash detected
    Crash(CrashInfo),
    /// Memory issue detected
    MemoryError(MemoryErrorKind),
    /// Interesting input found
    Interesting(String),
}

impl FuzzResult {
    /// Check if this is a crash
    pub fn is_crash(&self) -> bool {
        matches!(self, FuzzResult::Crash(_))
    }

    /// Check if this is interesting
    pub fn is_interesting(&self) -> bool {
        matches!(self, FuzzResult::Interesting(_) | FuzzResult::Crash(_))
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self, FuzzResult::Crash(_) | FuzzResult::MemoryError(_))
    }
}

/// Crash information
#[derive(Debug, Clone)]
pub struct CrashInfo {
    /// Crash type
    pub crash_type: CrashType,
    /// Address (if applicable)
    pub address: Option<u64>,
    /// Stack trace
    pub stack_trace: Vec<StackFrame>,
    /// Register state
    pub registers: Vec<(String, u64)>,
}

/// Type of crash
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashType {
    /// Segmentation fault
    Segfault,
    /// Stack overflow
    StackOverflow,
    /// Abort
    Abort,
    /// Panic
    Panic,
    /// Integer overflow
    IntegerOverflow,
    /// Out of memory
    OutOfMemory,
    /// Unknown
    Unknown,
}

/// Stack frame
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// Function name (if known)
    pub function: Option<String>,
    /// Address
    pub address: u64,
    /// File (if known)
    pub file: Option<String>,
    /// Line number (if known)
    pub line: Option<u32>,
}

/// Memory error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryErrorKind {
    /// Use after free
    UseAfterFree,
    /// Buffer overflow
    BufferOverflow,
    /// Double free
    DoubleFree,
    /// Memory leak
    MemoryLeak,
    /// Invalid access
    InvalidAccess,
    /// Stack buffer overflow
    StackBufferOverflow,
    /// Heap buffer overflow
    HeapBufferOverflow,
}

/// Fuzzer configuration
#[derive(Debug, Clone)]
pub struct FuzzerConfig {
    /// Maximum input size
    pub max_input_size: usize,
    /// Timeout per iteration (ms)
    pub timeout_ms: u64,
    /// Maximum iterations
    pub max_iterations: u64,
    /// Seed corpus directory
    pub corpus_dir: Option<String>,
    /// Output directory for crashes
    pub crash_dir: String,
    /// Number of parallel workers
    pub workers: u32,
    /// Enable coverage tracking
    pub coverage: bool,
    /// Enable sanitizers
    pub sanitizers: Sanitizers,
}

impl Default for FuzzerConfig {
    fn default() -> Self {
        Self {
            max_input_size: 1024 * 1024, // 1MB
            timeout_ms: 1000,
            max_iterations: u64::MAX,
            corpus_dir: None,
            crash_dir: String::from("./crashes"),
            workers: 1,
            coverage: true,
            sanitizers: Sanitizers::default(),
        }
    }
}

/// Sanitizer configuration
#[derive(Debug, Clone, Default)]
pub struct Sanitizers {
    /// Address sanitizer
    pub asan: bool,
    /// Memory sanitizer
    pub msan: bool,
    /// Undefined behavior sanitizer
    pub ubsan: bool,
    /// Thread sanitizer
    pub tsan: bool,
}

/// Fuzzing statistics
#[derive(Debug, Clone, Default)]
pub struct FuzzStats {
    /// Total iterations
    pub iterations: u64,
    /// Crashes found
    pub crashes: u64,
    /// Unique crashes
    pub unique_crashes: u64,
    /// Timeouts
    pub timeouts: u64,
    /// Corpus size
    pub corpus_size: usize,
    /// Coverage (edges)
    pub coverage_edges: usize,
    /// Coverage (features)
    pub coverage_features: usize,
    /// Executions per second
    pub execs_per_sec: f64,
    /// Runtime in seconds
    pub runtime_s: f64,
}

impl FuzzStats {
    /// Update execution rate
    pub fn update_rate(&mut self, elapsed_s: f64) {
        self.runtime_s = elapsed_s;
        if elapsed_s > 0.0 {
            self.execs_per_sec = self.iterations as f64 / elapsed_s;
        }
    }
}

/// Mutator for input generation
pub struct Mutator {
    /// Current seed
    seed: u64,
    /// Dictionary of interesting values
    dictionary: Vec<Vec<u8>>,
}

impl Mutator {
    /// Create a new mutator
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            dictionary: Vec::new(),
        }
    }

    /// Add dictionary entry
    pub fn add_dictionary(&mut self, entry: Vec<u8>) {
        self.dictionary.push(entry);
    }

    /// Generate random bytes
    fn random(&mut self) -> u64 {
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.seed
    }

    /// Mutate input
    pub fn mutate(&mut self, input: &mut Vec<u8>) {
        let strategy = self.random() % 10;
        
        match strategy {
            0 => self.bit_flip(input),
            1 => self.byte_flip(input),
            2 => self.byte_insert(input),
            3 => self.byte_delete(input),
            4 => self.byte_replace(input),
            5 => self.splice(input),
            6 => self.interesting_value(input),
            7 => self.dictionary_insert(input),
            8 => self.havoc(input),
            _ => self.random_bytes(input),
        }
    }

    fn bit_flip(&mut self, input: &mut Vec<u8>) {
        if input.is_empty() {
            return;
        }
        let pos = (self.random() as usize) % input.len();
        let bit = (self.random() % 8) as u8;
        input[pos] ^= 1 << bit;
    }

    fn byte_flip(&mut self, input: &mut Vec<u8>) {
        if input.is_empty() {
            return;
        }
        let pos = (self.random() as usize) % input.len();
        input[pos] ^= 0xFF;
    }

    fn byte_insert(&mut self, input: &mut Vec<u8>) {
        let pos = if input.is_empty() {
            0
        } else {
            (self.random() as usize) % input.len()
        };
        let byte = (self.random() & 0xFF) as u8;
        input.insert(pos, byte);
    }

    fn byte_delete(&mut self, input: &mut Vec<u8>) {
        if input.is_empty() {
            return;
        }
        let pos = (self.random() as usize) % input.len();
        input.remove(pos);
    }

    fn byte_replace(&mut self, input: &mut Vec<u8>) {
        if input.is_empty() {
            return;
        }
        let pos = (self.random() as usize) % input.len();
        input[pos] = (self.random() & 0xFF) as u8;
    }

    fn splice(&mut self, input: &mut Vec<u8>) {
        // Would splice with corpus entries
        let _ = input;
    }

    fn interesting_value(&mut self, input: &mut Vec<u8>) {
        const INTERESTING: &[u8] = &[0, 1, 0x7F, 0x80, 0xFF];
        if input.is_empty() {
            return;
        }
        let pos = (self.random() as usize) % input.len();
        let val_idx = (self.random() as usize) % INTERESTING.len();
        input[pos] = INTERESTING[val_idx];
    }

    fn dictionary_insert(&mut self, input: &mut Vec<u8>) {
        if self.dictionary.is_empty() {
            return;
        }
        let dict_idx = (self.random() as usize) % self.dictionary.len();
        // Clone the entry to avoid borrowing conflict
        let entry: Vec<u8> = self.dictionary[dict_idx].clone();
        let pos = if input.is_empty() {
            0
        } else {
            (self.random() as usize) % input.len()
        };
        for (i, &byte) in entry.iter().enumerate() {
            if pos + i < input.len() {
                input[pos + i] = byte;
            } else {
                input.push(byte);
            }
        }
    }

    fn havoc(&mut self, input: &mut Vec<u8>) {
        let iterations = (self.random() % 16) + 1;
        for _ in 0..iterations {
            let strategy = self.random() % 7;
            match strategy {
                0 => self.bit_flip(input),
                1 => self.byte_flip(input),
                2 => self.byte_insert(input),
                3 => self.byte_delete(input),
                4 => self.byte_replace(input),
                5 => self.interesting_value(input),
                _ => {}
            }
        }
    }

    fn random_bytes(&mut self, input: &mut Vec<u8>) {
        let count = ((self.random() % 8) + 1) as usize;
        for _ in 0..count {
            let byte = (self.random() & 0xFF) as u8;
            input.push(byte);
        }
    }
}

/// Corpus manager
pub struct Corpus {
    /// Input entries
    entries: Vec<CorpusEntry>,
    /// Maximum size
    max_size: usize,
}

/// A corpus entry
#[derive(Debug, Clone)]
pub struct CorpusEntry {
    /// Input data
    pub data: Vec<u8>,
    /// Coverage this input provides
    pub coverage: u64,
    /// Execution count
    pub exec_count: u64,
    /// Found timestamp
    pub timestamp: u64,
}

impl Corpus {
    /// Create new corpus
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
        }
    }

    /// Add entry if interesting
    pub fn add(&mut self, data: Vec<u8>, coverage: u64) -> bool {
        // Check if this provides new coverage
        let dominated = self.entries.iter().any(|e| e.coverage >= coverage);
        
        if !dominated {
            self.entries.push(CorpusEntry {
                data,
                coverage,
                exec_count: 1,
                timestamp: 0,
            });
            
            // Trim if too large
            if self.entries.len() > self.max_size {
                self.entries.sort_by_key(|e| core::cmp::Reverse(e.coverage));
                self.entries.truncate(self.max_size);
            }
            
            true
        } else {
            false
        }
    }

    /// Get random entry
    pub fn random_entry(&mut self, seed: u64) -> Option<&Vec<u8>> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = (seed as usize) % self.entries.len();
        self.entries[idx].exec_count += 1;
        Some(&self.entries[idx].data)
    }

    /// Get corpus size
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Coverage tracker
pub struct CoverageTracker {
    /// Edge coverage bitmap
    edges: Vec<u8>,
    /// Total edges seen
    total_edges: usize,
    /// New edges found this run
    new_edges: usize,
}

impl CoverageTracker {
    /// Create new tracker
    pub fn new(size: usize) -> Self {
        Self {
            edges: alloc::vec![0u8; size],
            total_edges: 0,
            new_edges: 0,
        }
    }

    /// Record edge hit
    pub fn record_edge(&mut self, edge: usize) {
        let idx = edge % self.edges.len();
        if self.edges[idx] == 0 {
            self.new_edges += 1;
            self.total_edges += 1;
        }
        self.edges[idx] = self.edges[idx].saturating_add(1);
    }

    /// Reset for new iteration
    pub fn reset_iteration(&mut self) {
        self.new_edges = 0;
    }

    /// Check if new coverage found
    pub fn has_new_coverage(&self) -> bool {
        self.new_edges > 0
    }

    /// Get total edges
    pub fn total_edges(&self) -> usize {
        self.total_edges
    }

    /// Get coverage percentage
    pub fn coverage_percent(&self) -> f64 {
        (self.total_edges as f64 / self.edges.len() as f64) * 100.0
    }
}
