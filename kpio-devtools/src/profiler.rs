//! Performance Profiler
//!
//! Provides CPU profiling, memory tracking, and performance analysis.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::boxed::Box;

/// Profile ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileId(pub String);

/// Script ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptId(pub String);

/// Call frame ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallFrameId(pub i32);

/// CPU profile.
#[derive(Debug, Clone)]
pub struct Profile {
    /// Profile ID.
    pub id: ProfileId,
    /// Nodes (call tree).
    pub nodes: Vec<ProfileNode>,
    /// Start time (microseconds).
    pub start_time: f64,
    /// End time (microseconds).
    pub end_time: f64,
    /// Samples (node IDs).
    pub samples: Vec<i32>,
    /// Time deltas between samples (microseconds).
    pub time_deltas: Vec<i64>,
}

impl Profile {
    /// Create a new profile.
    pub fn new(id: ProfileId) -> Self {
        Self {
            id,
            nodes: Vec::new(),
            start_time: 0.0,
            end_time: 0.0,
            samples: Vec::new(),
            time_deltas: Vec::new(),
        }
    }
    
    /// Duration in microseconds.
    pub fn duration(&self) -> f64 {
        self.end_time - self.start_time
    }
    
    /// Total samples.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
    
    /// Get node by ID.
    pub fn get_node(&self, id: i32) -> Option<&ProfileNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
    
    /// Calculate self time for each node.
    pub fn calculate_self_times(&self) -> BTreeMap<i32, f64> {
        let mut self_times: BTreeMap<i32, f64> = BTreeMap::new();
        
        for (&sample, &delta) in self.samples.iter().zip(self.time_deltas.iter()) {
            *self_times.entry(sample).or_insert(0.0) += delta as f64;
        }
        
        self_times
    }
    
    /// Calculate total time for each node (self + children).
    pub fn calculate_total_times(&self) -> BTreeMap<i32, f64> {
        let self_times = self.calculate_self_times();
        let mut total_times: BTreeMap<i32, f64> = self_times.clone();
        
        // Propagate times up the tree
        for node in self.nodes.iter().rev() {
            let node_total = *total_times.get(&node.id).unwrap_or(&0.0);
            if let Some(parent) = node.parent {
                *total_times.entry(parent).or_insert(0.0) += node_total;
            }
        }
        
        total_times
    }
}

/// Profile node (call tree node).
#[derive(Debug, Clone)]
pub struct ProfileNode {
    /// Node ID.
    pub id: i32,
    /// Call frame.
    pub call_frame: RuntimeCallFrame,
    /// Hit count.
    pub hit_count: i32,
    /// Children node IDs.
    pub children: Vec<i32>,
    /// Parent node ID.
    pub parent: Option<i32>,
    /// Deopt reason.
    pub deopt_reason: Option<String>,
    /// Position ticks.
    pub position_ticks: Vec<PositionTickInfo>,
}

impl ProfileNode {
    /// Create a new profile node.
    pub fn new(id: i32, call_frame: RuntimeCallFrame) -> Self {
        Self {
            id,
            call_frame,
            hit_count: 0,
            children: Vec::new(),
            parent: None,
            deopt_reason: None,
            position_ticks: Vec::new(),
        }
    }
}

/// Runtime call frame.
#[derive(Debug, Clone)]
pub struct RuntimeCallFrame {
    /// Function name.
    pub function_name: String,
    /// Script ID.
    pub script_id: ScriptId,
    /// URL.
    pub url: String,
    /// Line number (0-based).
    pub line_number: i32,
    /// Column number (0-based).
    pub column_number: i32,
}

impl RuntimeCallFrame {
    /// Create a new call frame.
    pub fn new(function_name: &str, url: &str, line: i32, column: i32) -> Self {
        Self {
            function_name: function_name.to_string(),
            script_id: ScriptId(String::new()),
            url: url.to_string(),
            line_number: line,
            column_number: column,
        }
    }
    
    /// Root frame.
    pub fn root() -> Self {
        Self {
            function_name: "(root)".to_string(),
            script_id: ScriptId("0".to_string()),
            url: String::new(),
            line_number: 0,
            column_number: 0,
        }
    }
    
    /// Program frame.
    pub fn program() -> Self {
        Self {
            function_name: "(program)".to_string(),
            script_id: ScriptId("0".to_string()),
            url: String::new(),
            line_number: 0,
            column_number: 0,
        }
    }
    
    /// Idle frame.
    pub fn idle() -> Self {
        Self {
            function_name: "(idle)".to_string(),
            script_id: ScriptId("0".to_string()),
            url: String::new(),
            line_number: 0,
            column_number: 0,
        }
    }
    
    /// GC frame.
    pub fn gc() -> Self {
        Self {
            function_name: "(garbage collector)".to_string(),
            script_id: ScriptId("0".to_string()),
            url: String::new(),
            line_number: 0,
            column_number: 0,
        }
    }
}

/// Position tick info.
#[derive(Debug, Clone)]
pub struct PositionTickInfo {
    /// Line number.
    pub line: i32,
    /// Tick count.
    pub ticks: i32,
}

/// Heap snapshot.
#[derive(Debug, Clone)]
pub struct HeapSnapshot {
    /// Snapshot ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Timestamp.
    pub timestamp: f64,
    /// Nodes.
    pub nodes: Vec<HeapNode>,
    /// Edges.
    pub edges: Vec<HeapEdge>,
    /// Strings table.
    pub strings: Vec<String>,
    /// Statistics.
    pub statistics: HeapStatistics,
}

impl HeapSnapshot {
    /// Create a new snapshot.
    pub fn new(id: &str, title: &str, timestamp: f64) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            timestamp,
            nodes: Vec::new(),
            edges: Vec::new(),
            strings: Vec::new(),
            statistics: HeapStatistics::default(),
        }
    }
}

/// Heap node.
#[derive(Debug, Clone)]
pub struct HeapNode {
    /// Node type.
    pub node_type: HeapNodeType,
    /// Name index in strings table.
    pub name: u32,
    /// ID.
    pub id: u32,
    /// Self size.
    pub self_size: u32,
    /// Edge count.
    pub edge_count: u32,
    /// Trace node ID.
    pub trace_node_id: u32,
    /// Detachedness.
    pub detachedness: u8,
}

/// Heap node type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapNodeType {
    Hidden,
    Array,
    String,
    Object,
    Code,
    Closure,
    Regexp,
    Number,
    Native,
    Synthetic,
    ConcatenatedString,
    SlicedString,
    Symbol,
    BigInt,
}

/// Heap edge.
#[derive(Debug, Clone)]
pub struct HeapEdge {
    /// Edge type.
    pub edge_type: HeapEdgeType,
    /// Name or index.
    pub name_or_index: u32,
    /// Target node index.
    pub to_node: u32,
}

/// Heap edge type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapEdgeType {
    Context,
    Element,
    Property,
    Internal,
    Hidden,
    Shortcut,
    Weak,
}

/// Heap statistics.
#[derive(Debug, Clone, Default)]
pub struct HeapStatistics {
    /// Total heap size.
    pub total_size: u64,
    /// Used heap size.
    pub used_size: u64,
    /// External size.
    pub external_size: u64,
    /// Object count.
    pub object_count: u64,
}

/// Sampling heap profile.
#[derive(Debug, Clone)]
pub struct SamplingHeapProfile {
    /// Root node.
    pub head: SamplingHeapProfileNode,
    /// Samples.
    pub samples: Vec<SamplingHeapProfileSample>,
}

/// Sampling heap profile node.
#[derive(Debug, Clone)]
pub struct SamplingHeapProfileNode {
    /// Call frame.
    pub call_frame: RuntimeCallFrame,
    /// Self size.
    pub self_size: u64,
    /// Node ID.
    pub id: u32,
    /// Children.
    pub children: Vec<SamplingHeapProfileNode>,
}

/// Sampling heap profile sample.
#[derive(Debug, Clone)]
pub struct SamplingHeapProfileSample {
    /// Size.
    pub size: u64,
    /// Node ID.
    pub node_id: u32,
    /// Ordinal.
    pub ordinal: f64,
}

/// Trace event.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Category.
    pub cat: String,
    /// Name.
    pub name: String,
    /// Phase.
    pub ph: TraceEventPhase,
    /// Timestamp (microseconds).
    pub ts: f64,
    /// Thread ID.
    pub tid: u32,
    /// Process ID.
    pub pid: u32,
    /// Duration (for complete events).
    pub dur: Option<f64>,
    /// Arguments.
    pub args: BTreeMap<String, TraceEventArg>,
}

impl TraceEvent {
    /// Create a begin event.
    pub fn begin(cat: &str, name: &str, ts: f64, tid: u32, pid: u32) -> Self {
        Self {
            cat: cat.to_string(),
            name: name.to_string(),
            ph: TraceEventPhase::Begin,
            ts,
            tid,
            pid,
            dur: None,
            args: BTreeMap::new(),
        }
    }
    
    /// Create an end event.
    pub fn end(cat: &str, name: &str, ts: f64, tid: u32, pid: u32) -> Self {
        Self {
            cat: cat.to_string(),
            name: name.to_string(),
            ph: TraceEventPhase::End,
            ts,
            tid,
            pid,
            dur: None,
            args: BTreeMap::new(),
        }
    }
    
    /// Create a complete event.
    pub fn complete(cat: &str, name: &str, ts: f64, dur: f64, tid: u32, pid: u32) -> Self {
        Self {
            cat: cat.to_string(),
            name: name.to_string(),
            ph: TraceEventPhase::Complete,
            ts,
            tid,
            pid,
            dur: Some(dur),
            args: BTreeMap::new(),
        }
    }
    
    /// Create an instant event.
    pub fn instant(cat: &str, name: &str, ts: f64, tid: u32, pid: u32) -> Self {
        Self {
            cat: cat.to_string(),
            name: name.to_string(),
            ph: TraceEventPhase::Instant,
            ts,
            tid,
            pid,
            dur: None,
            args: BTreeMap::new(),
        }
    }
    
    /// Add an argument.
    pub fn with_arg(mut self, name: &str, value: TraceEventArg) -> Self {
        self.args.insert(name.to_string(), value);
        self
    }
}

/// Trace event phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventPhase {
    /// Begin.
    Begin,
    /// End.
    End,
    /// Complete (has duration).
    Complete,
    /// Instant.
    Instant,
    /// Counter.
    Counter,
    /// Async begin.
    AsyncBegin,
    /// Async end.
    AsyncEnd,
    /// Async step.
    AsyncStep,
    /// Flow begin.
    FlowBegin,
    /// Flow step.
    FlowStep,
    /// Flow end.
    FlowEnd,
    /// Metadata.
    Metadata,
    /// Sample.
    Sample,
    /// Object created.
    ObjectCreated,
    /// Object snapshot.
    ObjectSnapshot,
    /// Object destroyed.
    ObjectDestroyed,
}

impl TraceEventPhase {
    /// To character representation.
    pub fn as_char(&self) -> char {
        match self {
            Self::Begin => 'B',
            Self::End => 'E',
            Self::Complete => 'X',
            Self::Instant => 'i',
            Self::Counter => 'C',
            Self::AsyncBegin => 'b',
            Self::AsyncEnd => 'e',
            Self::AsyncStep => 'n',
            Self::FlowBegin => 's',
            Self::FlowStep => 't',
            Self::FlowEnd => 'f',
            Self::Metadata => 'M',
            Self::Sample => 'P',
            Self::ObjectCreated => 'N',
            Self::ObjectSnapshot => 'O',
            Self::ObjectDestroyed => 'D',
        }
    }
}

/// Trace event argument.
#[derive(Debug, Clone)]
pub enum TraceEventArg {
    String(String),
    Number(f64),
    Bool(bool),
    Object(BTreeMap<String, Box<TraceEventArg>>),
}

/// Performance metrics.
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    /// Timestamp.
    pub timestamp: f64,
    /// Documents.
    pub documents: u64,
    /// Frames.
    pub frames: u64,
    /// JS event listeners.
    pub js_event_listeners: u64,
    /// Nodes.
    pub nodes: u64,
    /// Layout count.
    pub layout_count: u64,
    /// Recalc style count.
    pub recalc_style_count: u64,
    /// Layout duration (milliseconds).
    pub layout_duration: f64,
    /// Recalc style duration (milliseconds).
    pub recalc_style_duration: f64,
    /// Script duration (milliseconds).
    pub script_duration: f64,
    /// Task duration (milliseconds).
    pub task_duration: f64,
    /// JS heap used size.
    pub js_heap_used_size: u64,
    /// JS heap total size.
    pub js_heap_total_size: u64,
}

/// Performance timing (Navigation Timing API).
#[derive(Debug, Clone, Default)]
pub struct PerformanceTiming {
    /// Navigation start.
    pub navigation_start: f64,
    /// Unload event start.
    pub unload_event_start: f64,
    /// Unload event end.
    pub unload_event_end: f64,
    /// Redirect start.
    pub redirect_start: f64,
    /// Redirect end.
    pub redirect_end: f64,
    /// Fetch start.
    pub fetch_start: f64,
    /// Domain lookup start.
    pub domain_lookup_start: f64,
    /// Domain lookup end.
    pub domain_lookup_end: f64,
    /// Connect start.
    pub connect_start: f64,
    /// Connect end.
    pub connect_end: f64,
    /// Secure connection start.
    pub secure_connection_start: f64,
    /// Request start.
    pub request_start: f64,
    /// Response start.
    pub response_start: f64,
    /// Response end.
    pub response_end: f64,
    /// DOM loading.
    pub dom_loading: f64,
    /// DOM interactive.
    pub dom_interactive: f64,
    /// DOM content loaded event start.
    pub dom_content_loaded_event_start: f64,
    /// DOM content loaded event end.
    pub dom_content_loaded_event_end: f64,
    /// DOM complete.
    pub dom_complete: f64,
    /// Load event start.
    pub load_event_start: f64,
    /// Load event end.
    pub load_event_end: f64,
}

impl PerformanceTiming {
    /// Calculate DNS lookup time.
    pub fn dns_time(&self) -> f64 {
        self.domain_lookup_end - self.domain_lookup_start
    }
    
    /// Calculate TCP connect time.
    pub fn connect_time(&self) -> f64 {
        self.connect_end - self.connect_start
    }
    
    /// Calculate TLS time.
    pub fn tls_time(&self) -> f64 {
        if self.secure_connection_start > 0.0 {
            self.connect_end - self.secure_connection_start
        } else {
            0.0
        }
    }
    
    /// Calculate time to first byte.
    pub fn ttfb(&self) -> f64 {
        self.response_start - self.navigation_start
    }
    
    /// Calculate response time.
    pub fn response_time(&self) -> f64 {
        self.response_end - self.response_start
    }
    
    /// Calculate DOM processing time.
    pub fn dom_processing_time(&self) -> f64 {
        self.dom_complete - self.dom_loading
    }
    
    /// Calculate total page load time.
    pub fn page_load_time(&self) -> f64 {
        self.load_event_end - self.navigation_start
    }
}

/// Paint timing entry.
#[derive(Debug, Clone)]
pub struct PaintTimingEntry {
    /// Name (first-paint, first-contentful-paint).
    pub name: String,
    /// Entry type.
    pub entry_type: String,
    /// Start time.
    pub start_time: f64,
    /// Duration (always 0 for paint).
    pub duration: f64,
}

/// Long task entry.
#[derive(Debug, Clone)]
pub struct LongTaskEntry {
    /// Name.
    pub name: String,
    /// Entry type.
    pub entry_type: String,
    /// Start time.
    pub start_time: f64,
    /// Duration.
    pub duration: f64,
    /// Attribution.
    pub attribution: Vec<TaskAttribution>,
}

/// Task attribution.
#[derive(Debug, Clone)]
pub struct TaskAttribution {
    /// Name.
    pub name: String,
    /// Entry type.
    pub entry_type: String,
    /// Start time.
    pub start_time: f64,
    /// Duration.
    pub duration: f64,
    /// Container type.
    pub container_type: String,
    /// Container src.
    pub container_src: String,
    /// Container id.
    pub container_id: String,
    /// Container name.
    pub container_name: String,
}

/// Layout shift entry.
#[derive(Debug, Clone)]
pub struct LayoutShiftEntry {
    /// Name.
    pub name: String,
    /// Entry type.
    pub entry_type: String,
    /// Start time.
    pub start_time: f64,
    /// Duration.
    pub duration: f64,
    /// Value (layout shift score).
    pub value: f64,
    /// Had recent input.
    pub had_recent_input: bool,
    /// Last input time.
    pub last_input_time: f64,
    /// Sources.
    pub sources: Vec<LayoutShiftSource>,
}

/// Layout shift source.
#[derive(Debug, Clone)]
pub struct LayoutShiftSource {
    /// Node.
    pub node: Option<String>,
    /// Previous rect.
    pub previous_rect: DomRect,
    /// Current rect.
    pub current_rect: DomRect,
}

/// DOM rect.
#[derive(Debug, Clone, Copy, Default)]
pub struct DomRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Profiler.
pub struct Profiler {
    /// Active profiles.
    profiles: BTreeMap<String, Profile>,
    /// Next profile ID.
    next_profile_id: u64,
    /// Is profiling.
    is_profiling: bool,
    /// Current profile.
    current_profile: Option<ProfileId>,
    /// Trace events.
    trace_events: Vec<TraceEvent>,
    /// Is tracing.
    is_tracing: bool,
    /// Heap snapshots.
    heap_snapshots: Vec<HeapSnapshot>,
    /// Sampling interval (microseconds).
    sampling_interval: u64,
}

impl Profiler {
    /// Create a new profiler.
    pub fn new() -> Self {
        Self {
            profiles: BTreeMap::new(),
            next_profile_id: 1,
            is_profiling: false,
            current_profile: None,
            trace_events: Vec::new(),
            is_tracing: false,
            heap_snapshots: Vec::new(),
            sampling_interval: 100, // 100 microseconds
        }
    }
    
    /// Set sampling interval.
    pub fn set_sampling_interval(&mut self, interval: u64) {
        self.sampling_interval = interval;
    }
    
    /// Start profiling.
    pub fn start(&mut self) -> ProfileId {
        let id = ProfileId(alloc::format!("profile-{}", self.next_profile_id));
        self.next_profile_id += 1;
        
        let mut profile = Profile::new(id.clone());
        
        // Add root node
        let root = ProfileNode::new(1, RuntimeCallFrame::root());
        profile.nodes.push(root);
        
        self.profiles.insert(id.0.clone(), profile);
        self.current_profile = Some(id.clone());
        self.is_profiling = true;
        
        id
    }
    
    /// Stop profiling.
    pub fn stop(&mut self) -> Option<Profile> {
        if let Some(id) = self.current_profile.take() {
            self.is_profiling = false;
            self.profiles.remove(&id.0)
        } else {
            None
        }
    }
    
    /// Add a sample.
    pub fn add_sample(&mut self, node_id: i32, timestamp: f64) {
        if let Some(ref id) = self.current_profile {
            if let Some(profile) = self.profiles.get_mut(&id.0) {
                if profile.samples.is_empty() {
                    profile.start_time = timestamp;
                }
                
                let delta = if let Some(last) = profile.samples.last() {
                    let last_time = profile.start_time + 
                        profile.time_deltas.iter().sum::<i64>() as f64;
                    ((timestamp - last_time) * 1000.0) as i64
                } else {
                    0
                };
                
                profile.samples.push(node_id);
                profile.time_deltas.push(delta);
                profile.end_time = timestamp;
                
                // Increment hit count
                if let Some(node) = profile.nodes.iter_mut().find(|n| n.id == node_id) {
                    node.hit_count += 1;
                }
            }
        }
    }
    
    /// Get profile by ID.
    pub fn get_profile(&self, id: &ProfileId) -> Option<&Profile> {
        self.profiles.get(&id.0)
    }
    
    /// Start tracing.
    pub fn start_tracing(&mut self, categories: &[&str]) {
        self.trace_events.clear();
        self.is_tracing = true;
        
        // Add metadata event for enabled categories
        let mut event = TraceEvent {
            cat: "__metadata".to_string(),
            name: "trace_categories".to_string(),
            ph: TraceEventPhase::Metadata,
            ts: 0.0,
            tid: 0,
            pid: 1,
            dur: None,
            args: BTreeMap::new(),
        };
        event.args.insert(
            "categories".to_string(),
            TraceEventArg::String(categories.join(",")),
        );
        self.trace_events.push(event);
    }
    
    /// Stop tracing.
    pub fn stop_tracing(&mut self) -> Vec<TraceEvent> {
        self.is_tracing = false;
        core::mem::take(&mut self.trace_events)
    }
    
    /// Add trace event.
    pub fn add_trace_event(&mut self, event: TraceEvent) {
        if self.is_tracing {
            self.trace_events.push(event);
        }
    }
    
    /// Take heap snapshot.
    pub fn take_heap_snapshot(&mut self, title: &str, timestamp: f64) -> String {
        let id = alloc::format!("snapshot-{}", self.heap_snapshots.len() + 1);
        let snapshot = HeapSnapshot::new(&id, title, timestamp);
        self.heap_snapshots.push(snapshot);
        id
    }
    
    /// Get heap snapshot.
    pub fn get_heap_snapshot(&self, id: &str) -> Option<&HeapSnapshot> {
        self.heap_snapshots.iter().find(|s| s.id == id)
    }
    
    /// Collect garbage.
    pub fn collect_garbage(&self) {
        // Would trigger GC in the JS engine
    }
    
    /// Get performance metrics.
    pub fn get_metrics(&self) -> PerformanceMetrics {
        // Would collect from browser engine
        PerformanceMetrics::default()
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Flame graph builder.
pub struct FlameGraphBuilder {
    /// Stack samples.
    samples: Vec<Vec<RuntimeCallFrame>>,
    /// Sample values.
    values: Vec<u64>,
}

impl FlameGraphBuilder {
    /// Create a new flame graph builder.
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            values: Vec::new(),
        }
    }
    
    /// Add a sample.
    pub fn add_sample(&mut self, stack: Vec<RuntimeCallFrame>, value: u64) {
        self.samples.push(stack);
        self.values.push(value);
    }
    
    /// Build from profile.
    pub fn from_profile(profile: &Profile) -> Self {
        let mut builder = Self::new();
        
        // Build stack for each sample
        for &sample_id in &profile.samples {
            let mut stack = Vec::new();
            let mut current_id = Some(sample_id);
            
            while let Some(id) = current_id {
                if let Some(node) = profile.get_node(id) {
                    stack.push(node.call_frame.clone());
                    current_id = node.parent;
                } else {
                    break;
                }
            }
            
            stack.reverse();
            builder.add_sample(stack, 1);
        }
        
        builder
    }
    
    /// Generate folded stacks format.
    pub fn to_folded_stacks(&self) -> String {
        let mut lines = Vec::new();
        
        for (stack, &value) in self.samples.iter().zip(self.values.iter()) {
            let stack_str: Vec<String> = stack.iter()
                .map(|f| {
                    if f.url.is_empty() {
                        f.function_name.clone()
                    } else {
                        alloc::format!("{} ({}:{})", f.function_name, f.url, f.line_number + 1)
                    }
                })
                .collect();
            
            lines.push(alloc::format!("{} {}", stack_str.join(";"), value));
        }
        
        lines.join("\n")
    }
}

impl Default for FlameGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_profiler() {
        let mut profiler = Profiler::new();
        let id = profiler.start();
        
        assert!(profiler.is_profiling);
        
        let profile = profiler.stop();
        assert!(profile.is_some());
        assert!(!profiler.is_profiling);
    }
    
    #[test]
    fn test_trace_event() {
        let event = TraceEvent::complete("v8", "ParseFunction", 1000.0, 500.0, 1, 1)
            .with_arg("url", TraceEventArg::String("script.js".to_string()));
        
        assert_eq!(event.ph, TraceEventPhase::Complete);
        assert_eq!(event.dur, Some(500.0));
    }
    
    #[test]
    fn test_performance_timing() {
        let mut timing = PerformanceTiming::default();
        timing.navigation_start = 0.0;
        timing.response_start = 100.0;
        timing.load_event_end = 500.0;
        
        assert_eq!(timing.ttfb(), 100.0);
        assert_eq!(timing.page_load_time(), 500.0);
    }
}
