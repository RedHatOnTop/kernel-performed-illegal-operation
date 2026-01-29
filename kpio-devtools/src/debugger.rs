//! JavaScript Debugger
//!
//! Provides breakpoint management, step debugging, and execution control.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

use crate::console::{RemoteObject, RemoteObjectId, StackTrace, CallFrame};

/// Debugger ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DebuggerId(pub String);

/// Script ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptId(pub String);

/// Breakpoint ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BreakpointId(pub String);

/// Call frame ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallFrameId(pub String);

/// Location in a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// Script ID.
    pub script_id: u32,
    /// Line number (0-based).
    pub line_number: i32,
    /// Column number (0-based).
    pub column_number: Option<i32>,
}

impl Location {
    /// Create a new location.
    pub fn new(script_id: u32, line: i32) -> Self {
        Self {
            script_id,
            line_number: line,
            column_number: None,
        }
    }
    
    /// Create a location with column.
    pub fn with_column(script_id: u32, line: i32, column: i32) -> Self {
        Self {
            script_id,
            line_number: line,
            column_number: Some(column),
        }
    }
}

/// Script source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptLanguage {
    JavaScript,
    WebAssembly,
}

/// Script parse event data.
#[derive(Debug, Clone)]
pub struct ScriptInfo {
    /// Script ID.
    pub script_id: ScriptId,
    /// URL.
    pub url: String,
    /// Start line.
    pub start_line: i32,
    /// Start column.
    pub start_column: i32,
    /// End line.
    pub end_line: i32,
    /// End column.
    pub end_column: i32,
    /// Execution context ID.
    pub execution_context_id: i32,
    /// Hash.
    pub hash: String,
    /// Is live edit.
    pub is_live_edit: bool,
    /// Source map URL.
    pub source_map_url: Option<String>,
    /// Has source URL comment.
    pub has_source_url: bool,
    /// Is module.
    pub is_module: bool,
    /// Length.
    pub length: i32,
    /// Stack trace (for eval).
    pub stack_trace: Option<StackTrace>,
    /// Script language.
    pub script_language: ScriptLanguage,
    /// Source.
    pub source: Option<String>,
}

impl ScriptInfo {
    /// Create a new script info.
    pub fn new(script_id: ScriptId, url: &str) -> Self {
        Self {
            script_id,
            url: url.to_string(),
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
            execution_context_id: 1,
            hash: String::new(),
            is_live_edit: false,
            source_map_url: None,
            has_source_url: false,
            is_module: false,
            length: 0,
            stack_trace: None,
            script_language: ScriptLanguage::JavaScript,
            source: None,
        }
    }
}

/// Breakpoint definition.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Breakpoint ID.
    pub id: BreakpointId,
    /// URL pattern (for URL breakpoints).
    pub url: Option<String>,
    /// URL regex pattern.
    pub url_regex: Option<String>,
    /// Script hash.
    pub script_hash: Option<String>,
    /// Line number.
    pub line_number: i32,
    /// Column number.
    pub column_number: Option<i32>,
    /// Condition.
    pub condition: Option<String>,
    /// Is enabled.
    pub enabled: bool,
    /// Actual locations where the breakpoint resolved.
    pub locations: Vec<Location>,
}

impl Breakpoint {
    /// Create a new breakpoint.
    pub fn new(id: BreakpointId, url: &str, line: i32) -> Self {
        Self {
            id,
            url: Some(url.to_string()),
            url_regex: None,
            script_hash: None,
            line_number: line,
            column_number: None,
            condition: None,
            enabled: true,
            locations: Vec::new(),
        }
    }
    
    /// Create a conditional breakpoint.
    pub fn conditional(id: BreakpointId, url: &str, line: i32, condition: &str) -> Self {
        let mut bp = Self::new(id, url, line);
        bp.condition = Some(condition.to_string());
        bp
    }
    
    /// Create a logpoint.
    pub fn logpoint(id: BreakpointId, url: &str, line: i32, log_message: &str) -> Self {
        // Logpoints are implemented as conditional breakpoints that always return false
        // but have a side effect of logging
        let condition = alloc::format!("console.log({}), false", log_message);
        Self::conditional(id, url, line, &condition)
    }
}

/// Pause reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseReason {
    /// Ambiguous breakpoint.
    Ambiguous,
    /// Assert.
    Assert,
    /// CSP Violation.
    CspViolation,
    /// Debugger statement.
    DebuggerStatement,
    /// DOM breakpoint.
    Dom,
    /// Event listener breakpoint.
    EventListener,
    /// Exception.
    Exception,
    /// Instrumentation breakpoint.
    Instrumentation,
    /// Out of memory.
    Oom,
    /// Other.
    Other,
    /// Promise rejection.
    PromiseRejection,
    /// Regular breakpoint.
    Breakpoint,
    /// XHR breakpoint.
    Xhr,
    /// Step.
    Step,
}

/// Debug call frame.
#[derive(Debug, Clone)]
pub struct DebugCallFrame {
    /// Call frame ID.
    pub call_frame_id: CallFrameId,
    /// Function name.
    pub function_name: String,
    /// Function location.
    pub function_location: Option<Location>,
    /// Current location.
    pub location: Location,
    /// URL.
    pub url: String,
    /// Scope chain.
    pub scope_chain: Vec<Scope>,
    /// This object.
    pub this: RemoteObject,
    /// Return value (if paused at return).
    pub return_value: Option<RemoteObject>,
    /// Can be restarted.
    pub can_be_restarted: bool,
}

impl DebugCallFrame {
    /// Create a new debug call frame.
    pub fn new(
        call_frame_id: CallFrameId,
        function_name: &str,
        location: Location,
        url: &str,
    ) -> Self {
        Self {
            call_frame_id,
            function_name: function_name.to_string(),
            function_location: None,
            location,
            url: url.to_string(),
            scope_chain: Vec::new(),
            this: RemoteObject::undefined(),
            return_value: None,
            can_be_restarted: false,
        }
    }
}

/// Scope.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Scope type.
    pub scope_type: ScopeType,
    /// Object representing the scope.
    pub object: RemoteObject,
    /// Name (for closures).
    pub name: Option<String>,
    /// Start location.
    pub start_location: Option<Location>,
    /// End location.
    pub end_location: Option<Location>,
}

impl Scope {
    /// Create a new scope.
    pub fn new(scope_type: ScopeType, object: RemoteObject) -> Self {
        Self {
            scope_type,
            object,
            name: None,
            start_location: None,
            end_location: None,
        }
    }
}

/// Scope type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeType {
    /// Global scope.
    Global,
    /// Local scope.
    Local,
    /// With scope.
    With,
    /// Closure scope.
    Closure,
    /// Catch scope.
    Catch,
    /// Block scope.
    Block,
    /// Script scope.
    Script,
    /// Eval scope.
    Eval,
    /// Module scope.
    Module,
    /// WASM expression stack.
    WasmExpressionStack,
}

/// Exception pause mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionPauseMode {
    /// Don't pause on exceptions.
    None,
    /// Pause on uncaught exceptions.
    Uncaught,
    /// Pause on all exceptions.
    All,
}

/// Step action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepAction {
    /// Step into (enter function calls).
    StepInto,
    /// Step over (skip function calls).
    StepOver,
    /// Step out (return from current function).
    StepOut,
}

/// Watch expression.
#[derive(Debug, Clone)]
pub struct WatchExpression {
    /// Expression.
    pub expression: String,
    /// Last evaluated value.
    pub value: Option<RemoteObject>,
    /// Error message (if evaluation failed).
    pub error: Option<String>,
}

impl WatchExpression {
    /// Create a new watch expression.
    pub fn new(expression: &str) -> Self {
        Self {
            expression: expression.to_string(),
            value: None,
            error: None,
        }
    }
}

/// Debugger state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebuggerState {
    /// Running.
    Running,
    /// Paused.
    Paused,
}

/// JavaScript Debugger.
pub struct Debugger {
    /// Is enabled.
    enabled: bool,
    /// Debugger state.
    state: DebuggerState,
    /// Scripts.
    scripts: BTreeMap<String, ScriptInfo>,
    /// Next script ID.
    next_script_id: u64,
    /// Breakpoints.
    breakpoints: BTreeMap<String, Breakpoint>,
    /// Next breakpoint ID.
    next_breakpoint_id: u64,
    /// Exception pause mode.
    exception_pause_mode: ExceptionPauseMode,
    /// Pause on async call.
    pause_on_async_call: bool,
    /// Call frames (when paused).
    call_frames: Vec<DebugCallFrame>,
    /// Pause reason.
    pause_reason: Option<PauseReason>,
    /// Pause data.
    pause_data: Option<RemoteObject>,
    /// Watch expressions.
    watch_expressions: Vec<WatchExpression>,
    /// Skip list (scripts to skip).
    skip_list: Vec<String>,
    /// Blackboxed scripts.
    blackboxed_scripts: BTreeMap<String, Vec<ScriptPosition>>,
}

impl Debugger {
    /// Create a new debugger.
    pub fn new() -> Self {
        Self {
            enabled: false,
            state: DebuggerState::Running,
            scripts: BTreeMap::new(),
            next_script_id: 1,
            breakpoints: BTreeMap::new(),
            next_breakpoint_id: 1,
            exception_pause_mode: ExceptionPauseMode::None,
            pause_on_async_call: false,
            call_frames: Vec::new(),
            pause_reason: None,
            pause_data: None,
            watch_expressions: Vec::new(),
            skip_list: Vec::new(),
            blackboxed_scripts: BTreeMap::new(),
        }
    }
    
    /// Enable the debugger.
    pub fn enable(&mut self) -> DebuggerId {
        self.enabled = true;
        DebuggerId(alloc::format!("debugger-{}", 1))
    }
    
    /// Disable the debugger.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.breakpoints.clear();
        self.call_frames.clear();
        self.state = DebuggerState::Running;
    }
    
    /// Is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Register a script.
    pub fn register_script(&mut self, url: &str, source: &str) -> ScriptId {
        let id = ScriptId(alloc::format!("{}", self.next_script_id));
        self.next_script_id += 1;
        
        let lines: Vec<&str> = source.lines().collect();
        let end_line = lines.len() as i32 - 1;
        let end_column = lines.last().map(|l| l.len() as i32).unwrap_or(0);
        
        let mut info = ScriptInfo::new(id.clone(), url);
        info.end_line = end_line;
        info.end_column = end_column;
        info.length = source.len() as i32;
        info.source = Some(source.to_string());
        
        self.scripts.insert(id.0.clone(), info);
        id
    }
    
    /// Get script info.
    pub fn get_script(&self, script_id: &ScriptId) -> Option<&ScriptInfo> {
        self.scripts.get(&script_id.0)
    }
    
    /// Get script source.
    pub fn get_script_source(&self, script_id: &ScriptId) -> Option<&str> {
        self.scripts.get(&script_id.0)
            .and_then(|s| s.source.as_deref())
    }
    
    /// Set a breakpoint.
    pub fn set_breakpoint(&mut self, url: &str, line: i32) -> BreakpointId {
        let id = BreakpointId(alloc::format!("{}:{}", url, line));
        let breakpoint = Breakpoint::new(id.clone(), url, line);
        self.breakpoints.insert(id.0.clone(), breakpoint);
        id
    }
    
    /// Set a breakpoint with condition.
    pub fn set_breakpoint_conditional(
        &mut self,
        url: &str,
        line: i32,
        condition: &str,
    ) -> BreakpointId {
        let id = BreakpointId(alloc::format!("{}:{}", url, line));
        let breakpoint = Breakpoint::conditional(id.clone(), url, line, condition);
        self.breakpoints.insert(id.0.clone(), breakpoint);
        id
    }
    
    /// Remove a breakpoint.
    pub fn remove_breakpoint(&mut self, breakpoint_id: &BreakpointId) -> bool {
        self.breakpoints.remove(&breakpoint_id.0).is_some()
    }
    
    /// Get all breakpoints.
    pub fn breakpoints(&self) -> impl Iterator<Item = &Breakpoint> {
        self.breakpoints.values()
    }
    
    /// Enable/disable a breakpoint.
    pub fn set_breakpoint_enabled(&mut self, breakpoint_id: &BreakpointId, enabled: bool) {
        if let Some(bp) = self.breakpoints.get_mut(&breakpoint_id.0) {
            bp.enabled = enabled;
        }
    }
    
    /// Set exception pause mode.
    pub fn set_pause_on_exceptions(&mut self, mode: ExceptionPauseMode) {
        self.exception_pause_mode = mode;
    }
    
    /// Get exception pause mode.
    pub fn pause_on_exceptions(&self) -> ExceptionPauseMode {
        self.exception_pause_mode
    }
    
    /// Check if should pause at location.
    pub fn should_pause_at(&self, script_id: &ScriptId, line: i32, _column: i32) -> bool {
        if !self.enabled {
            return false;
        }
        
        if let Some(script) = self.scripts.get(&script_id.0) {
            for bp in self.breakpoints.values() {
                if !bp.enabled {
                    continue;
                }
                
                if bp.line_number == line {
                    if let Some(ref url) = bp.url {
                        if script.url == *url {
                            return true;
                        }
                    }
                }
            }
        }
        
        false
    }
    
    /// Pause execution.
    pub fn pause(&mut self, reason: PauseReason, call_frames: Vec<DebugCallFrame>) {
        self.state = DebuggerState::Paused;
        self.pause_reason = Some(reason);
        self.call_frames = call_frames;
        
        // Evaluate watch expressions
        self.evaluate_watches();
    }
    
    /// Resume execution.
    pub fn resume(&mut self) {
        self.state = DebuggerState::Running;
        self.pause_reason = None;
        self.pause_data = None;
        self.call_frames.clear();
    }
    
    /// Step.
    pub fn step(&mut self, _action: StepAction) {
        // Would set step mode in the JS engine
        self.resume();
    }
    
    /// Step into.
    pub fn step_into(&mut self) {
        self.step(StepAction::StepInto);
    }
    
    /// Step over.
    pub fn step_over(&mut self) {
        self.step(StepAction::StepOver);
    }
    
    /// Step out.
    pub fn step_out(&mut self) {
        self.step(StepAction::StepOut);
    }
    
    /// Get current state.
    pub fn state(&self) -> DebuggerState {
        self.state
    }
    
    /// Is paused.
    pub fn is_paused(&self) -> bool {
        self.state == DebuggerState::Paused
    }
    
    /// Get call frames.
    pub fn call_frames(&self) -> &[DebugCallFrame] {
        &self.call_frames
    }
    
    /// Get pause reason.
    pub fn pause_reason(&self) -> Option<&PauseReason> {
        self.pause_reason.as_ref()
    }
    
    /// Add watch expression.
    pub fn add_watch(&mut self, expression: &str) {
        self.watch_expressions.push(WatchExpression::new(expression));
        if self.is_paused() {
            self.evaluate_watches();
        }
    }
    
    /// Remove watch expression.
    pub fn remove_watch(&mut self, index: usize) {
        if index < self.watch_expressions.len() {
            self.watch_expressions.remove(index);
        }
    }
    
    /// Get watch expressions.
    pub fn watch_expressions(&self) -> &[WatchExpression] {
        &self.watch_expressions
    }
    
    /// Evaluate watch expressions.
    fn evaluate_watches(&mut self) {
        // Would evaluate each expression in the current execution context
        for watch in &mut self.watch_expressions {
            // Simplified - would use the JS engine
            watch.value = Some(RemoteObject::undefined());
        }
    }
    
    /// Evaluate expression.
    pub fn evaluate_on_call_frame(
        &self,
        _call_frame_id: &CallFrameId,
        _expression: &str,
    ) -> Result<RemoteObject, String> {
        // Would evaluate in the context of the call frame
        Ok(RemoteObject::undefined())
    }
    
    /// Set variable value.
    pub fn set_variable_value(
        &self,
        _scope_number: i32,
        _variable_name: &str,
        _new_value: RemoteObject,
        _call_frame_id: &CallFrameId,
    ) -> Result<(), String> {
        // Would set the variable in the scope
        Ok(())
    }
    
    /// Restart frame.
    pub fn restart_frame(&mut self, _call_frame_id: &CallFrameId) -> Result<(), String> {
        // Would restart execution at the beginning of the frame
        Ok(())
    }
    
    /// Blackbox script.
    pub fn blackbox_script(&mut self, script_id: &ScriptId) {
        self.blackboxed_scripts.insert(script_id.0.clone(), Vec::new());
    }
    
    /// Is script blackboxed.
    pub fn is_blackboxed(&self, script_id: &ScriptId) -> bool {
        self.blackboxed_scripts.contains_key(&script_id.0)
    }
    
    /// Set skip list.
    pub fn set_skip_list(&mut self, patterns: Vec<String>) {
        self.skip_list = patterns;
    }
    
    /// Should skip URL.
    pub fn should_skip(&self, url: &str) -> bool {
        self.skip_list.iter().any(|pattern| {
            if pattern.contains('*') {
                // Simple glob matching
                let parts: Vec<&str> = pattern.split('*').collect();
                let mut pos = 0;
                for part in parts {
                    if part.is_empty() {
                        continue;
                    }
                    if let Some(found) = url[pos..].find(part) {
                        pos += found + part.len();
                    } else {
                        return false;
                    }
                }
                true
            } else {
                url == pattern
            }
        })
    }
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

/// Script position (for blackboxing ranges).
#[derive(Debug, Clone, Copy)]
pub struct ScriptPosition {
    /// Line number.
    pub line_number: i32,
    /// Column number.
    pub column_number: i32,
}

/// Event listener breakpoint.
#[derive(Debug, Clone)]
pub struct EventListenerBreakpoint {
    /// Event name.
    pub event_name: String,
    /// Target name (optional).
    pub target_name: Option<String>,
}

/// DOM breakpoint.
#[derive(Debug, Clone)]
pub struct DomBreakpoint {
    /// Node ID.
    pub node_id: i32,
    /// Breakpoint type.
    pub breakpoint_type: DomBreakpointType,
}

/// DOM breakpoint type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomBreakpointType {
    /// Subtree modified.
    SubtreeModified,
    /// Attribute modified.
    AttributeModified,
    /// Node removed.
    NodeRemoved,
}

/// XHR breakpoint.
#[derive(Debug, Clone)]
pub struct XhrBreakpoint {
    /// URL pattern.
    pub url: String,
}

/// Source map.
#[derive(Debug, Clone)]
pub struct SourceMap {
    /// Version.
    pub version: i32,
    /// Source root.
    pub source_root: Option<String>,
    /// Sources.
    pub sources: Vec<String>,
    /// Sources content.
    pub sources_content: Option<Vec<Option<String>>>,
    /// Names.
    pub names: Vec<String>,
    /// Mappings.
    pub mappings: String,
}

impl SourceMap {
    /// Create a new source map.
    pub fn new() -> Self {
        Self {
            version: 3,
            source_root: None,
            sources: Vec::new(),
            sources_content: None,
            names: Vec::new(),
            mappings: String::new(),
        }
    }
    
    /// Parse VLQ-encoded mappings.
    pub fn parse_mappings(&self) -> Vec<SourceMapping> {
        let mut mappings = Vec::new();
        
        let mut generated_line = 0i32;
        let mut generated_column = 0i32;
        let mut source_index = 0i32;
        let mut source_line = 0i32;
        let mut source_column = 0i32;
        let mut name_index = 0i32;
        
        for group in self.mappings.split(';') {
            generated_column = 0;
            
            for segment in group.split(',') {
                if segment.is_empty() {
                    continue;
                }
                
                let values = decode_vlq(segment);
                if values.is_empty() {
                    continue;
                }
                
                generated_column += values[0];
                
                let mut mapping = SourceMapping {
                    generated_line,
                    generated_column,
                    source: None,
                    source_line: None,
                    source_column: None,
                    name: None,
                };
                
                if values.len() >= 4 {
                    source_index += values[1];
                    source_line += values[2];
                    source_column += values[3];
                    
                    mapping.source = self.sources.get(source_index as usize).cloned();
                    mapping.source_line = Some(source_line);
                    mapping.source_column = Some(source_column);
                    
                    if values.len() >= 5 {
                        name_index += values[4];
                        mapping.name = self.names.get(name_index as usize).cloned();
                    }
                }
                
                mappings.push(mapping);
            }
            
            generated_line += 1;
        }
        
        mappings
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Source mapping entry.
#[derive(Debug, Clone)]
pub struct SourceMapping {
    /// Generated line.
    pub generated_line: i32,
    /// Generated column.
    pub generated_column: i32,
    /// Source file.
    pub source: Option<String>,
    /// Source line.
    pub source_line: Option<i32>,
    /// Source column.
    pub source_column: Option<i32>,
    /// Name.
    pub name: Option<String>,
}

/// Decode VLQ-encoded segment.
fn decode_vlq(segment: &str) -> Vec<i32> {
    const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    let mut values = Vec::new();
    let mut value = 0i32;
    let mut shift = 0;
    
    for ch in segment.chars() {
        let digit = match BASE64_CHARS.iter().position(|&c| c == ch as u8) {
            Some(pos) => pos as i32,
            None => continue,
        };
        
        let continuation = digit & 32;
        value += (digit & 31) << shift;
        
        if continuation != 0 {
            shift += 5;
        } else {
            // Convert from unsigned to signed
            let signed = if value & 1 != 0 {
                -(value >> 1)
            } else {
                value >> 1
            };
            values.push(signed);
            value = 0;
            shift = 0;
        }
    }
    
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_debugger() {
        let mut debugger = Debugger::new();
        let id = debugger.enable();
        assert!(debugger.is_enabled());
        
        let script_id = debugger.register_script("test.js", "function test() { return 1; }");
        assert!(debugger.get_script(&script_id).is_some());
    }
    
    #[test]
    fn test_breakpoints() {
        let mut debugger = Debugger::new();
        debugger.enable();
        
        let bp_id = debugger.set_breakpoint("test.js", 10);
        assert!(debugger.breakpoints.contains_key(&bp_id.0));
        
        debugger.remove_breakpoint(&bp_id);
        assert!(!debugger.breakpoints.contains_key(&bp_id.0));
    }
    
    #[test]
    fn test_vlq_decode() {
        let values = decode_vlq("AAAA");
        assert_eq!(values, vec![0, 0, 0, 0]);
        
        let values = decode_vlq("AACA");
        assert_eq!(values, vec![0, 0, 1, 0]);
    }
    
    #[test]
    fn test_exception_pause() {
        let mut debugger = Debugger::new();
        debugger.enable();
        
        debugger.set_pause_on_exceptions(ExceptionPauseMode::All);
        assert_eq!(debugger.pause_on_exceptions(), ExceptionPauseMode::All);
    }
}
