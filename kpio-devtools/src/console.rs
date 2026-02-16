//! JavaScript Console Panel
//!
//! Provides console logging, REPL, and object inspection for DevTools.

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Console message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// console.log
    Log,
    /// console.debug
    Debug,
    /// console.info
    Info,
    /// console.warn
    Warning,
    /// console.error
    Error,
    /// console.dir
    Dir,
    /// console.dirxml
    DirXml,
    /// console.table
    Table,
    /// console.trace
    Trace,
    /// console.clear
    Clear,
    /// console.time
    StartGroup,
    /// console.groupCollapsed
    StartGroupCollapsed,
    /// console.groupEnd
    EndGroup,
    /// console.assert
    Assert,
    /// console.profile
    Profile,
    /// console.profileEnd
    ProfileEnd,
    /// console.count
    Count,
    /// console.timeEnd
    TimeEnd,
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Log => "log",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Dir => "dir",
            Self::DirXml => "dirXml",
            Self::Table => "table",
            Self::Trace => "trace",
            Self::Clear => "clear",
            Self::StartGroup => "startGroup",
            Self::StartGroupCollapsed => "startGroupCollapsed",
            Self::EndGroup => "endGroup",
            Self::Assert => "assert",
            Self::Profile => "profile",
            Self::ProfileEnd => "profileEnd",
            Self::Count => "count",
            Self::TimeEnd => "timeEnd",
        };
        write!(f, "{}", s)
    }
}

/// Console message source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSource {
    /// From XML parsing.
    Xml,
    /// From JavaScript.
    JavaScript,
    /// From network.
    Network,
    /// From console API.
    ConsoleApi,
    /// From storage.
    Storage,
    /// From App Cache.
    Appcache,
    /// From rendering.
    Rendering,
    /// Security messages.
    Security,
    /// Other sources.
    Other,
    /// Deprecation warnings.
    Deprecation,
    /// Worker messages.
    Worker,
    /// Violation messages.
    Violation,
    /// Intervention messages.
    Intervention,
    /// Recommendation messages.
    Recommendation,
}

impl fmt::Display for MessageSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Xml => "xml",
            Self::JavaScript => "javascript",
            Self::Network => "network",
            Self::ConsoleApi => "console-api",
            Self::Storage => "storage",
            Self::Appcache => "appcache",
            Self::Rendering => "rendering",
            Self::Security => "security",
            Self::Other => "other",
            Self::Deprecation => "deprecation",
            Self::Worker => "worker",
            Self::Violation => "violation",
            Self::Intervention => "intervention",
            Self::Recommendation => "recommendation",
        };
        write!(f, "{}", s)
    }
}

/// Console message level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageLevel {
    /// Verbose level.
    Verbose,
    /// Info level.
    Info,
    /// Warning level.
    Warning,
    /// Error level.
    Error,
}

impl fmt::Display for MessageLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Verbose => "verbose",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        };
        write!(f, "{}", s)
    }
}

/// Remote object ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteObjectId(pub String);

/// Remote object representing a JavaScript value.
#[derive(Debug, Clone)]
pub struct RemoteObject {
    /// Object type.
    pub object_type: ObjectType,
    /// Object subtype.
    pub subtype: Option<ObjectSubtype>,
    /// Object class name.
    pub class_name: Option<String>,
    /// String representation.
    pub value: Option<String>,
    /// Unserializable value (like Infinity, NaN).
    pub unserializable_value: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Remote object ID for further inspection.
    pub object_id: Option<RemoteObjectId>,
    /// Preview of the object.
    pub preview: Option<ObjectPreview>,
}

impl RemoteObject {
    /// Create a primitive value.
    pub fn primitive(object_type: ObjectType, value: Option<String>) -> Self {
        Self {
            object_type,
            subtype: None,
            class_name: None,
            value,
            unserializable_value: None,
            description: None,
            object_id: None,
            preview: None,
        }
    }

    /// Create an undefined value.
    pub fn undefined() -> Self {
        Self::primitive(ObjectType::Undefined, None)
    }

    /// Create a null value.
    pub fn null() -> Self {
        Self {
            object_type: ObjectType::Object,
            subtype: Some(ObjectSubtype::Null),
            class_name: None,
            value: None,
            unserializable_value: None,
            description: Some("null".to_string()),
            object_id: None,
            preview: None,
        }
    }

    /// Create a boolean value.
    pub fn boolean(value: bool) -> Self {
        Self::primitive(ObjectType::Boolean, Some(value.to_string()))
    }

    /// Create a number value.
    pub fn number(value: f64) -> Self {
        if value.is_nan() {
            Self {
                object_type: ObjectType::Number,
                subtype: None,
                class_name: None,
                value: None,
                unserializable_value: Some("NaN".to_string()),
                description: Some("NaN".to_string()),
                object_id: None,
                preview: None,
            }
        } else if value.is_infinite() {
            let desc = if value.is_sign_positive() {
                "Infinity"
            } else {
                "-Infinity"
            };
            Self {
                object_type: ObjectType::Number,
                subtype: None,
                class_name: None,
                value: None,
                unserializable_value: Some(desc.to_string()),
                description: Some(desc.to_string()),
                object_id: None,
                preview: None,
            }
        } else {
            // Use a simple integer check since no_std doesn't have full float formatting
            // Note: fract() is not available in no_std, so we use a different approach
            let desc = if value == (value as i64) as f64 {
                alloc::format!("{}", value as i64)
            } else {
                alloc::format!("{}", value)
            };
            Self::primitive(ObjectType::Number, Some(desc))
        }
    }

    /// Create a string value.
    pub fn string(value: &str) -> Self {
        Self::primitive(ObjectType::String, Some(value.to_string()))
    }

    /// Create a symbol value.
    pub fn symbol(description: &str) -> Self {
        Self {
            object_type: ObjectType::Symbol,
            subtype: None,
            class_name: None,
            value: None,
            unserializable_value: None,
            description: Some(alloc::format!("Symbol({})", description)),
            object_id: None,
            preview: None,
        }
    }

    /// Create a BigInt value.
    pub fn bigint(value: &str) -> Self {
        Self {
            object_type: ObjectType::Bigint,
            subtype: None,
            class_name: None,
            value: None,
            unserializable_value: Some(alloc::format!("{}n", value)),
            description: Some(alloc::format!("{}n", value)),
            object_id: None,
            preview: None,
        }
    }

    /// Create an object value.
    pub fn object(class_name: &str, object_id: RemoteObjectId) -> Self {
        Self {
            object_type: ObjectType::Object,
            subtype: None,
            class_name: Some(class_name.to_string()),
            value: None,
            unserializable_value: None,
            description: Some(class_name.to_string()),
            object_id: Some(object_id),
            preview: None,
        }
    }

    /// Create an array value.
    pub fn array(length: usize, object_id: RemoteObjectId) -> Self {
        Self {
            object_type: ObjectType::Object,
            subtype: Some(ObjectSubtype::Array),
            class_name: Some("Array".to_string()),
            value: None,
            unserializable_value: None,
            description: Some(alloc::format!("Array({})", length)),
            object_id: Some(object_id),
            preview: None,
        }
    }

    /// Create a function value.
    pub fn function(name: &str, object_id: RemoteObjectId) -> Self {
        Self {
            object_type: ObjectType::Function,
            subtype: None,
            class_name: Some("Function".to_string()),
            value: None,
            unserializable_value: None,
            description: Some(alloc::format!("function {}() {{ [native code] }}", name)),
            object_id: Some(object_id),
            preview: None,
        }
    }

    /// Create an error value.
    pub fn error(message: &str, object_id: RemoteObjectId) -> Self {
        Self {
            object_type: ObjectType::Object,
            subtype: Some(ObjectSubtype::Error),
            class_name: Some("Error".to_string()),
            value: None,
            unserializable_value: None,
            description: Some(message.to_string()),
            object_id: Some(object_id),
            preview: None,
        }
    }
}

/// Object type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    /// Object.
    Object,
    /// Function.
    Function,
    /// Undefined.
    Undefined,
    /// String.
    String,
    /// Number.
    Number,
    /// Boolean.
    Boolean,
    /// Symbol.
    Symbol,
    /// BigInt.
    Bigint,
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Object => "object",
            Self::Function => "function",
            Self::Undefined => "undefined",
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Symbol => "symbol",
            Self::Bigint => "bigint",
        };
        write!(f, "{}", s)
    }
}

/// Object subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSubtype {
    /// Array.
    Array,
    /// Null.
    Null,
    /// DOM node.
    Node,
    /// RegExp.
    Regexp,
    /// Date.
    Date,
    /// Map.
    Map,
    /// Set.
    Set,
    /// WeakMap.
    Weakmap,
    /// WeakSet.
    Weakset,
    /// Iterator.
    Iterator,
    /// Generator.
    Generator,
    /// Error.
    Error,
    /// Proxy.
    Proxy,
    /// Promise.
    Promise,
    /// TypedArray.
    Typedarray,
    /// ArrayBuffer.
    Arraybuffer,
    /// DataView.
    Dataview,
}

/// Object preview.
#[derive(Debug, Clone)]
pub struct ObjectPreview {
    /// Object type.
    pub object_type: ObjectType,
    /// Subtype.
    pub subtype: Option<ObjectSubtype>,
    /// Description.
    pub description: Option<String>,
    /// Whether the preview is truncated.
    pub overflow: bool,
    /// Properties.
    pub properties: Vec<PropertyPreview>,
    /// Entries (for Map/Set).
    pub entries: Option<Vec<EntryPreview>>,
}

/// Property preview.
#[derive(Debug, Clone)]
pub struct PropertyPreview {
    /// Property name.
    pub name: String,
    /// Property type.
    pub property_type: ObjectType,
    /// Value.
    pub value: Option<String>,
    /// Value preview (for nested objects).
    pub value_preview: Option<Box<ObjectPreview>>,
    /// Subtype.
    pub subtype: Option<ObjectSubtype>,
}

/// Entry preview (for Map/Set).
#[derive(Debug, Clone)]
pub struct EntryPreview {
    /// Key (for Map).
    pub key: Option<Box<ObjectPreview>>,
    /// Value.
    pub value: ObjectPreview,
}

/// Stack trace.
#[derive(Debug, Clone)]
pub struct StackTrace {
    /// Call frames.
    pub call_frames: Vec<CallFrame>,
    /// Description.
    pub description: Option<String>,
    /// Parent stack trace.
    pub parent: Option<Box<StackTrace>>,
    /// Parent stack trace ID.
    pub parent_id: Option<StackTraceId>,
}

impl StackTrace {
    /// Create an empty stack trace.
    pub fn empty() -> Self {
        Self {
            call_frames: Vec::new(),
            description: None,
            parent: None,
            parent_id: None,
        }
    }

    /// Add a call frame.
    pub fn push_frame(&mut self, frame: CallFrame) {
        self.call_frames.push(frame);
    }

    /// Format as string.
    pub fn format(&self) -> String {
        let mut result = String::new();
        for frame in &self.call_frames {
            result.push_str("    at ");
            result.push_str(&frame.function_name);
            result.push_str(" (");
            result.push_str(&frame.url);
            result.push(':');
            result.push_str(&frame.line_number.to_string());
            result.push(':');
            result.push_str(&frame.column_number.to_string());
            result.push_str(")\n");
        }
        result
    }
}

/// Stack trace ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StackTraceId {
    /// ID.
    pub id: String,
    /// Debugger ID.
    pub debugger_id: Option<String>,
}

/// Call frame.
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Call frame ID.
    pub call_frame_id: String,
    /// Function name.
    pub function_name: String,
    /// Script ID.
    pub script_id: String,
    /// URL.
    pub url: String,
    /// Line number (0-based).
    pub line_number: i32,
    /// Column number (0-based).
    pub column_number: i32,
}

impl CallFrame {
    /// Create a new call frame.
    pub fn new(function_name: &str, url: &str, line_number: i32, column_number: i32) -> Self {
        static FRAME_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        let id = FRAME_ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

        Self {
            call_frame_id: alloc::format!("frame-{}", id),
            function_name: function_name.to_string(),
            script_id: String::new(),
            url: url.to_string(),
            line_number,
            column_number,
        }
    }
}

/// Console message.
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    /// Message source.
    pub source: MessageSource,
    /// Message level.
    pub level: MessageLevel,
    /// Message text.
    pub text: String,
    /// Type.
    pub message_type: MessageType,
    /// URL.
    pub url: Option<String>,
    /// Line number.
    pub line: Option<i32>,
    /// Column number.
    pub column: Option<i32>,
    /// Stack trace.
    pub stack_trace: Option<StackTrace>,
    /// Arguments.
    pub args: Vec<RemoteObject>,
    /// Timestamp.
    pub timestamp: f64,
}

impl ConsoleMessage {
    /// Create a new console message.
    pub fn new(level: MessageLevel, text: &str) -> Self {
        Self {
            source: MessageSource::ConsoleApi,
            level,
            text: text.to_string(),
            message_type: match level {
                MessageLevel::Verbose => MessageType::Debug,
                MessageLevel::Info => MessageType::Info,
                MessageLevel::Warning => MessageType::Warning,
                MessageLevel::Error => MessageType::Error,
            },
            url: None,
            line: None,
            column: None,
            stack_trace: None,
            args: Vec::new(),
            timestamp: 0.0,
        }
    }

    /// Set location.
    pub fn with_location(mut self, url: &str, line: i32, column: i32) -> Self {
        self.url = Some(url.to_string());
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Set stack trace.
    pub fn with_stack_trace(mut self, stack_trace: StackTrace) -> Self {
        self.stack_trace = Some(stack_trace);
        self
    }

    /// Add argument.
    pub fn with_arg(mut self, arg: RemoteObject) -> Self {
        self.args.push(arg);
        self
    }
}

/// Console API.
pub struct Console {
    /// Messages.
    messages: Vec<ConsoleMessage>,
    /// Message limit.
    message_limit: usize,
    /// Message ID counter.
    next_message_id: u64,
    /// Timers.
    timers: BTreeMap<String, f64>,
    /// Counters.
    counters: BTreeMap<String, u32>,
    /// Group depth.
    group_depth: usize,
}

impl Console {
    /// Create a new console.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            message_limit: 1000,
            next_message_id: 1,
            timers: BTreeMap::new(),
            counters: BTreeMap::new(),
            group_depth: 0,
        }
    }

    /// Set message limit.
    pub fn set_message_limit(&mut self, limit: usize) {
        self.message_limit = limit;
        self.trim_messages();
    }

    /// Add a message.
    pub fn add_message(&mut self, message: ConsoleMessage) -> u64 {
        let id = self.next_message_id;
        self.next_message_id += 1;
        self.messages.push(message);
        self.trim_messages();
        id
    }

    /// Trim old messages.
    fn trim_messages(&mut self) {
        while self.messages.len() > self.message_limit {
            self.messages.remove(0);
        }
    }

    /// Log a message.
    pub fn log(&mut self, args: Vec<RemoteObject>) -> u64 {
        let text = self.format_args(&args);
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::Log;
        message.args = args;
        self.add_message(message)
    }

    /// Log a debug message.
    pub fn debug(&mut self, args: Vec<RemoteObject>) -> u64 {
        let text = self.format_args(&args);
        let mut message = ConsoleMessage::new(MessageLevel::Verbose, &text);
        message.message_type = MessageType::Debug;
        message.args = args;
        self.add_message(message)
    }

    /// Log an info message.
    pub fn info(&mut self, args: Vec<RemoteObject>) -> u64 {
        let text = self.format_args(&args);
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::Info;
        message.args = args;
        self.add_message(message)
    }

    /// Log a warning.
    pub fn warn(&mut self, args: Vec<RemoteObject>) -> u64 {
        let text = self.format_args(&args);
        let mut message = ConsoleMessage::new(MessageLevel::Warning, &text);
        message.message_type = MessageType::Warning;
        message.args = args;
        self.add_message(message)
    }

    /// Log an error.
    pub fn error(&mut self, args: Vec<RemoteObject>) -> u64 {
        let text = self.format_args(&args);
        let mut message = ConsoleMessage::new(MessageLevel::Error, &text);
        message.message_type = MessageType::Error;
        message.args = args;
        self.add_message(message)
    }

    /// Log a trace.
    pub fn trace(&mut self, args: Vec<RemoteObject>, stack_trace: StackTrace) -> u64 {
        let text = self.format_args(&args);
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::Trace;
        message.args = args;
        message.stack_trace = Some(stack_trace);
        self.add_message(message)
    }

    /// Assert.
    pub fn assert(&mut self, condition: bool, args: Vec<RemoteObject>) -> Option<u64> {
        if !condition {
            let text = self.format_args(&args);
            let mut message = ConsoleMessage::new(
                MessageLevel::Error,
                &alloc::format!("Assertion failed: {}", text),
            );
            message.message_type = MessageType::Assert;
            message.args = args;
            Some(self.add_message(message))
        } else {
            None
        }
    }

    /// Clear the console.
    pub fn clear(&mut self) -> u64 {
        self.messages.clear();
        self.group_depth = 0;
        let message = ConsoleMessage::new(MessageLevel::Info, "Console was cleared");
        self.add_message(message)
    }

    /// Start a group.
    pub fn group(&mut self, label: Option<&str>) -> u64 {
        let text = label.unwrap_or("console.group").to_string();
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::StartGroup;
        self.group_depth += 1;
        self.add_message(message)
    }

    /// Start a collapsed group.
    pub fn group_collapsed(&mut self, label: Option<&str>) -> u64 {
        let text = label.unwrap_or("console.groupCollapsed").to_string();
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::StartGroupCollapsed;
        self.group_depth += 1;
        self.add_message(message)
    }

    /// End a group.
    pub fn group_end(&mut self) -> u64 {
        if self.group_depth > 0 {
            self.group_depth -= 1;
        }
        let mut message = ConsoleMessage::new(MessageLevel::Info, "");
        message.message_type = MessageType::EndGroup;
        self.add_message(message)
    }

    /// Start a timer.
    pub fn time(&mut self, label: &str, timestamp: f64) {
        self.timers.insert(label.to_string(), timestamp);
    }

    /// Log timer value.
    pub fn time_log(&mut self, label: &str, timestamp: f64) -> Option<u64> {
        if let Some(&start) = self.timers.get(label) {
            let elapsed = timestamp - start;
            let text = alloc::format!("{}: {} ms", label, elapsed);
            let message = ConsoleMessage::new(MessageLevel::Info, &text);
            Some(self.add_message(message))
        } else {
            None
        }
    }

    /// End a timer.
    pub fn time_end(&mut self, label: &str, timestamp: f64) -> Option<u64> {
        if let Some(start) = self.timers.remove(label) {
            let elapsed = timestamp - start;
            let text = alloc::format!("{}: {} ms", label, elapsed);
            let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
            message.message_type = MessageType::TimeEnd;
            Some(self.add_message(message))
        } else {
            None
        }
    }

    /// Count.
    pub fn count(&mut self, label: &str) -> u64 {
        let count = self.counters.entry(label.to_string()).or_insert(0);
        *count += 1;
        let text = alloc::format!("{}: {}", label, count);
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::Count;
        self.add_message(message)
    }

    /// Reset counter.
    pub fn count_reset(&mut self, label: &str) {
        self.counters.insert(label.to_string(), 0);
    }

    /// Dir (object inspection).
    pub fn dir(&mut self, object: RemoteObject) -> u64 {
        let text = object.description.clone().unwrap_or_default();
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::Dir;
        message.args = alloc::vec![object];
        self.add_message(message)
    }

    /// Table.
    pub fn table(&mut self, data: RemoteObject, columns: Option<Vec<String>>) -> u64 {
        let text = data
            .description
            .clone()
            .unwrap_or_else(|| "table".to_string());
        let mut message = ConsoleMessage::new(MessageLevel::Info, &text);
        message.message_type = MessageType::Table;
        message.args = alloc::vec![data];
        if let Some(_cols) = columns {
            // Would store column info
        }
        self.add_message(message)
    }

    /// Get all messages.
    pub fn messages(&self) -> &[ConsoleMessage] {
        &self.messages
    }

    /// Get message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Format arguments to string.
    fn format_args(&self, args: &[RemoteObject]) -> String {
        let mut parts = Vec::new();
        for arg in args {
            let s = if let Some(ref value) = arg.value {
                value.clone()
            } else if let Some(ref desc) = arg.description {
                desc.clone()
            } else if let Some(ref unser) = arg.unserializable_value {
                unser.clone()
            } else {
                match arg.object_type {
                    ObjectType::Undefined => "undefined".to_string(),
                    ObjectType::Object => "[object]".to_string(),
                    ObjectType::Function => "[function]".to_string(),
                    _ => String::new(),
                }
            };
            parts.push(s);
        }
        parts.join(" ")
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

/// REPL context for evaluating JavaScript.
pub struct ReplContext {
    /// Variable storage (name -> object ID).
    variables: BTreeMap<String, RemoteObjectId>,
    /// Next object ID.
    next_object_id: u64,
    /// History.
    history: Vec<String>,
    /// History position.
    history_position: usize,
}

impl ReplContext {
    /// Create a new REPL context.
    pub fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
            next_object_id: 1,
            history: Vec::new(),
            history_position: 0,
        }
    }

    /// Generate a new object ID.
    pub fn new_object_id(&mut self) -> RemoteObjectId {
        let id = RemoteObjectId(alloc::format!(
            "{{\"injectedScriptId\":{},\"id\":{}}}",
            1,
            self.next_object_id
        ));
        self.next_object_id += 1;
        id
    }

    /// Set a variable.
    pub fn set_variable(&mut self, name: &str, object_id: RemoteObjectId) {
        self.variables.insert(name.to_string(), object_id);
    }

    /// Get a variable.
    pub fn get_variable(&self, name: &str) -> Option<&RemoteObjectId> {
        self.variables.get(name)
    }

    /// Add to history.
    pub fn add_to_history(&mut self, expression: &str) {
        if !expression.is_empty() {
            self.history.push(expression.to_string());
            self.history_position = self.history.len();
        }
    }

    /// Get previous history entry.
    pub fn history_previous(&mut self) -> Option<&str> {
        if self.history_position > 0 {
            self.history_position -= 1;
            self.history.get(self.history_position).map(|s| s.as_str())
        } else {
            None
        }
    }

    /// Get next history entry.
    pub fn history_next(&mut self) -> Option<&str> {
        if self.history_position < self.history.len() {
            self.history_position += 1;
            if self.history_position == self.history.len() {
                None
            } else {
                self.history.get(self.history_position).map(|s| s.as_str())
            }
        } else {
            None
        }
    }

    /// Evaluate an expression (simplified - would delegate to JS engine).
    pub fn evaluate(&mut self, expression: &str) -> EvaluationResult {
        self.add_to_history(expression);

        // Very simplified evaluation for demo
        if expression == "undefined" {
            return EvaluationResult::success(RemoteObject::undefined());
        }
        if expression == "null" {
            return EvaluationResult::success(RemoteObject::null());
        }
        if expression == "true" {
            return EvaluationResult::success(RemoteObject::boolean(true));
        }
        if expression == "false" {
            return EvaluationResult::success(RemoteObject::boolean(false));
        }
        if let Ok(num) = expression.parse::<f64>() {
            return EvaluationResult::success(RemoteObject::number(num));
        }
        if expression.starts_with('"') && expression.ends_with('"') && expression.len() >= 2 {
            let s = &expression[1..expression.len() - 1];
            return EvaluationResult::success(RemoteObject::string(s));
        }

        // Unknown expression - would need real JS engine
        EvaluationResult::exception(
            RemoteObject::error(
                "ReferenceError: expression evaluation not implemented",
                self.new_object_id(),
            ),
            None,
        )
    }
}

impl Default for ReplContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluation result.
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    /// Result value.
    pub result: RemoteObject,
    /// Exception details (if any).
    pub exception_details: Option<ExceptionDetails>,
}

impl EvaluationResult {
    /// Create a successful result.
    pub fn success(result: RemoteObject) -> Self {
        Self {
            result,
            exception_details: None,
        }
    }

    /// Create an exception result.
    pub fn exception(error: RemoteObject, stack_trace: Option<StackTrace>) -> Self {
        let text = error
            .description
            .clone()
            .unwrap_or_else(|| "Unknown error".to_string());
        Self {
            result: error.clone(),
            exception_details: Some(ExceptionDetails {
                exception_id: 1,
                text,
                line_number: 0,
                column_number: 0,
                script_id: None,
                url: None,
                stack_trace,
                exception: Some(error),
            }),
        }
    }
}

/// Exception details.
#[derive(Debug, Clone)]
pub struct ExceptionDetails {
    /// Exception ID.
    pub exception_id: i32,
    /// Exception text.
    pub text: String,
    /// Line number.
    pub line_number: i32,
    /// Column number.
    pub column_number: i32,
    /// Script ID.
    pub script_id: Option<String>,
    /// URL.
    pub url: Option<String>,
    /// Stack trace.
    pub stack_trace: Option<StackTrace>,
    /// The exception object.
    pub exception: Option<RemoteObject>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_log() {
        let mut console = Console::new();
        let id = console.log(alloc::vec![RemoteObject::string("Hello, World!")]);
        assert!(id > 0);
        assert_eq!(console.message_count(), 1);
    }

    #[test]
    fn test_console_timer() {
        let mut console = Console::new();
        console.time("test", 0.0);
        let id = console.time_end("test", 100.0);
        assert!(id.is_some());
    }

    #[test]
    fn test_console_counter() {
        let mut console = Console::new();
        console.count("test");
        console.count("test");
        console.count("test");
        assert_eq!(console.message_count(), 3);
    }

    #[test]
    fn test_remote_object() {
        let num = RemoteObject::number(42.0);
        assert_eq!(num.object_type, ObjectType::Number);
        assert_eq!(num.value, Some("42".to_string()));

        let nan = RemoteObject::number(f64::NAN);
        assert_eq!(nan.unserializable_value, Some("NaN".to_string()));
    }

    #[test]
    fn test_repl_evaluate() {
        let mut repl = ReplContext::new();

        let result = repl.evaluate("42");
        assert!(result.exception_details.is_none());
        assert_eq!(result.result.object_type, ObjectType::Number);

        let result = repl.evaluate("\"hello\"");
        assert_eq!(result.result.object_type, ObjectType::String);
    }
}
