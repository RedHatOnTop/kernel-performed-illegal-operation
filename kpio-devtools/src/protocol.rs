//! Chrome DevTools Protocol (CDP) Implementation
//!
//! Provides the protocol layer for communication with DevTools frontend.

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Message ID.
pub type MessageId = i32;

/// Session ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

/// Target ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetId(pub String);

/// CDP Message (incoming).
#[derive(Debug, Clone)]
pub struct CdpRequest {
    /// Message ID.
    pub id: MessageId,
    /// Method name (e.g., "DOM.getDocument").
    pub method: String,
    /// Parameters.
    pub params: Option<JsonValue>,
    /// Session ID (for target-specific commands).
    pub session_id: Option<SessionId>,
}

impl CdpRequest {
    /// Create a new request.
    pub fn new(id: MessageId, method: &str) -> Self {
        Self {
            id,
            method: method.to_string(),
            params: None,
            session_id: None,
        }
    }

    /// With parameters.
    pub fn with_params(mut self, params: JsonValue) -> Self {
        self.params = Some(params);
        self
    }

    /// With session.
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Get domain name.
    pub fn domain(&self) -> &str {
        self.method.split('.').next().unwrap_or("")
    }

    /// Get method name (without domain).
    pub fn method_name(&self) -> &str {
        self.method.split('.').nth(1).unwrap_or("")
    }
}

/// CDP Response.
#[derive(Debug, Clone)]
pub struct CdpResponse {
    /// Message ID.
    pub id: MessageId,
    /// Result (on success).
    pub result: Option<JsonValue>,
    /// Error (on failure).
    pub error: Option<CdpError>,
    /// Session ID.
    pub session_id: Option<SessionId>,
}

impl CdpResponse {
    /// Create a success response.
    pub fn success(id: MessageId, result: JsonValue) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
            session_id: None,
        }
    }

    /// Create an error response.
    pub fn error(id: MessageId, code: i32, message: &str) -> Self {
        Self {
            id,
            result: None,
            error: Some(CdpError {
                code,
                message: message.to_string(),
                data: None,
            }),
            session_id: None,
        }
    }

    /// With session.
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// CDP Error.
#[derive(Debug, Clone)]
pub struct CdpError {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Additional data.
    pub data: Option<String>,
}

impl CdpError {
    /// Parse error (-32700).
    pub fn parse_error(message: &str) -> Self {
        Self {
            code: -32700,
            message: message.to_string(),
            data: None,
        }
    }

    /// Invalid request (-32600).
    pub fn invalid_request(message: &str) -> Self {
        Self {
            code: -32600,
            message: message.to_string(),
            data: None,
        }
    }

    /// Method not found (-32601).
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: alloc::format!("'{}' wasn't found", method),
            data: None,
        }
    }

    /// Invalid params (-32602).
    pub fn invalid_params(message: &str) -> Self {
        Self {
            code: -32602,
            message: message.to_string(),
            data: None,
        }
    }

    /// Internal error (-32603).
    pub fn internal_error(message: &str) -> Self {
        Self {
            code: -32603,
            message: message.to_string(),
            data: None,
        }
    }

    /// Server error (-32000 to -32099).
    pub fn server_error(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: None,
        }
    }
}

/// CDP Event.
#[derive(Debug, Clone)]
pub struct CdpEvent {
    /// Event method (e.g., "DOM.documentUpdated").
    pub method: String,
    /// Parameters.
    pub params: Option<JsonValue>,
    /// Session ID.
    pub session_id: Option<SessionId>,
}

impl CdpEvent {
    /// Create a new event.
    pub fn new(method: &str) -> Self {
        Self {
            method: method.to_string(),
            params: None,
            session_id: None,
        }
    }

    /// With parameters.
    pub fn with_params(mut self, params: JsonValue) -> Self {
        self.params = Some(params);
        self
    }

    /// With session.
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// Simple JSON value type.
#[derive(Debug, Clone)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    /// Create a null value.
    pub fn null() -> Self {
        Self::Null
    }

    /// Create an empty object.
    pub fn object() -> Self {
        Self::Object(BTreeMap::new())
    }

    /// Create an empty array.
    pub fn array() -> Self {
        Self::Array(Vec::new())
    }

    /// Check if null.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(n) => Some(*n as i64),
            _ => None,
        }
    }

    /// Get as string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as array.
    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            Self::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Get as object.
    pub fn as_object(&self) -> Option<&BTreeMap<String, JsonValue>> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Get field from object.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(obj) => obj.get(key),
            _ => None,
        }
    }

    /// Get field from array.
    pub fn get_index(&self, index: usize) -> Option<&JsonValue> {
        match self {
            Self::Array(arr) => arr.get(index),
            _ => None,
        }
    }

    /// Insert into object.
    pub fn insert(&mut self, key: &str, value: JsonValue) {
        if let Self::Object(obj) = self {
            obj.insert(key.to_string(), value);
        }
    }

    /// Push to array.
    pub fn push(&mut self, value: JsonValue) {
        if let Self::Array(arr) = self {
            arr.push(value);
        }
    }
}

impl From<bool> for JsonValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i32> for JsonValue {
    fn from(n: i32) -> Self {
        Self::Number(n as f64)
    }
}

impl From<i64> for JsonValue {
    fn from(n: i64) -> Self {
        Self::Number(n as f64)
    }
}

impl From<f64> for JsonValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<&str> for JsonValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<String> for JsonValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl<T: Into<JsonValue>> From<Vec<T>> for JsonValue {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(|x| x.into()).collect())
    }
}

impl<T: Into<JsonValue>> From<Option<T>> for JsonValue {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

/// CDP Domain.
pub trait CdpDomain {
    /// Domain name (e.g., "DOM", "CSS", "Runtime").
    fn name(&self) -> &'static str;

    /// Handle a method call.
    fn handle(&mut self, method: &str, params: Option<&JsonValue>) -> Result<JsonValue, CdpError>;

    /// Enable the domain.
    fn enable(&mut self) -> Result<JsonValue, CdpError> {
        Ok(JsonValue::object())
    }

    /// Disable the domain.
    fn disable(&mut self) -> Result<JsonValue, CdpError> {
        Ok(JsonValue::object())
    }
}

/// CDP Protocol handler.
pub struct ProtocolHandler {
    /// Domains.
    domains: BTreeMap<String, Box<dyn CdpDomain + Send>>,
    /// Event queue.
    event_queue: Vec<CdpEvent>,
}

impl ProtocolHandler {
    /// Create a new protocol handler.
    pub fn new() -> Self {
        Self {
            domains: BTreeMap::new(),
            event_queue: Vec::new(),
        }
    }

    /// Register a domain.
    pub fn register_domain<D: CdpDomain + Send + 'static>(&mut self, domain: D) {
        self.domains
            .insert(domain.name().to_string(), Box::new(domain));
    }

    /// Handle a request.
    pub fn handle_request(&mut self, request: CdpRequest) -> CdpResponse {
        let domain_name = request.domain();
        let method_name = request.method_name();

        // Special handling for common methods
        if method_name == "enable" {
            if let Some(domain) = self.domains.get_mut(domain_name) {
                return match domain.enable() {
                    Ok(result) => CdpResponse::success(request.id, result),
                    Err(error) => CdpResponse {
                        id: request.id,
                        result: None,
                        error: Some(error),
                        session_id: request.session_id,
                    },
                };
            }
        } else if method_name == "disable" {
            if let Some(domain) = self.domains.get_mut(domain_name) {
                return match domain.disable() {
                    Ok(result) => CdpResponse::success(request.id, result),
                    Err(error) => CdpResponse {
                        id: request.id,
                        result: None,
                        error: Some(error),
                        session_id: request.session_id,
                    },
                };
            }
        }

        // Handle method in domain
        if let Some(domain) = self.domains.get_mut(domain_name) {
            match domain.handle(method_name, request.params.as_ref()) {
                Ok(result) => CdpResponse::success(request.id, result),
                Err(error) => CdpResponse {
                    id: request.id,
                    result: None,
                    error: Some(error),
                    session_id: request.session_id,
                },
            }
        } else {
            CdpResponse::error(
                request.id,
                -32601,
                &alloc::format!("'{}' wasn't found", request.method),
            )
        }
    }

    /// Queue an event.
    pub fn queue_event(&mut self, event: CdpEvent) {
        self.event_queue.push(event);
    }

    /// Take queued events.
    pub fn take_events(&mut self) -> Vec<CdpEvent> {
        core::mem::take(&mut self.event_queue)
    }

    /// Has queued events.
    pub fn has_events(&self) -> bool {
        !self.event_queue.is_empty()
    }
}

impl Default for ProtocolHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Target info.
#[derive(Debug, Clone)]
pub struct TargetInfo {
    /// Target ID.
    pub target_id: TargetId,
    /// Target type.
    pub target_type: TargetType,
    /// Title.
    pub title: String,
    /// URL.
    pub url: String,
    /// Whether the target is attached.
    pub attached: bool,
    /// Opener ID.
    pub opener_id: Option<TargetId>,
    /// Browser context ID.
    pub browser_context_id: Option<String>,
}

impl TargetInfo {
    /// Create a new target info.
    pub fn new(target_id: TargetId, target_type: TargetType, title: &str, url: &str) -> Self {
        Self {
            target_id,
            target_type,
            title: title.to_string(),
            url: url.to_string(),
            attached: false,
            opener_id: None,
            browser_context_id: None,
        }
    }
}

/// Target type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetType {
    Page,
    BackgroundPage,
    ServiceWorker,
    SharedWorker,
    Browser,
    Webview,
    Other,
}

impl TargetType {
    /// To string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::BackgroundPage => "background_page",
            Self::ServiceWorker => "service_worker",
            Self::SharedWorker => "shared_worker",
            Self::Browser => "browser",
            Self::Webview => "webview",
            Self::Other => "other",
        }
    }
}

/// Frame info.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    /// Frame ID.
    pub id: String,
    /// Parent frame ID.
    pub parent_id: Option<String>,
    /// Loader ID.
    pub loader_id: String,
    /// Frame name.
    pub name: Option<String>,
    /// URL.
    pub url: String,
    /// Security origin.
    pub security_origin: String,
    /// MIME type.
    pub mime_type: String,
}

/// Execution context.
#[derive(Debug, Clone)]
pub struct ExecutionContextDescription {
    /// Execution context ID.
    pub id: i32,
    /// Origin.
    pub origin: String,
    /// Name.
    pub name: String,
    /// Unique ID.
    pub unique_id: String,
    /// Aux data.
    pub aux_data: Option<ExecutionContextAuxData>,
}

/// Execution context aux data.
#[derive(Debug, Clone)]
pub struct ExecutionContextAuxData {
    /// Frame ID.
    pub frame_id: String,
    /// Is default.
    pub is_default: bool,
    /// Type.
    pub context_type: String,
}

/// Common CDP domain implementations.
pub mod domains {
    use super::*;

    /// Target domain.
    pub struct TargetDomain {
        targets: BTreeMap<String, TargetInfo>,
        sessions: BTreeMap<String, SessionId>,
    }

    impl TargetDomain {
        pub fn new() -> Self {
            Self {
                targets: BTreeMap::new(),
                sessions: BTreeMap::new(),
            }
        }

        pub fn add_target(&mut self, target: TargetInfo) {
            self.targets.insert(target.target_id.0.clone(), target);
        }
    }

    impl Default for TargetDomain {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CdpDomain for TargetDomain {
        fn name(&self) -> &'static str {
            "Target"
        }

        fn handle(
            &mut self,
            method: &str,
            params: Option<&JsonValue>,
        ) -> Result<JsonValue, CdpError> {
            match method {
                "getTargets" => {
                    let mut targets = JsonValue::array();
                    for target in self.targets.values() {
                        let mut info = JsonValue::object();
                        info.insert("targetId", target.target_id.0.clone().into());
                        info.insert("type", target.target_type.as_str().into());
                        info.insert("title", target.title.clone().into());
                        info.insert("url", target.url.clone().into());
                        info.insert("attached", target.attached.into());
                        targets.push(info);
                    }
                    let mut result = JsonValue::object();
                    result.insert("targetInfos", targets);
                    Ok(result)
                }
                "attachToTarget" => {
                    if let Some(params) = params {
                        if let Some(target_id) = params.get("targetId").and_then(|v| v.as_str()) {
                            if let Some(target) = self.targets.get_mut(target_id) {
                                target.attached = true;
                                let session_id = SessionId(alloc::format!("session-{}", target_id));
                                self.sessions
                                    .insert(target_id.to_string(), session_id.clone());

                                let mut result = JsonValue::object();
                                result.insert("sessionId", session_id.0.into());
                                return Ok(result);
                            }
                        }
                    }
                    Err(CdpError::invalid_params("targetId required"))
                }
                "detachFromTarget" => {
                    if let Some(params) = params {
                        if let Some(session_id) = params.get("sessionId").and_then(|v| v.as_str()) {
                            // Find and detach target
                            for (target_id, session) in &self.sessions {
                                if session.0 == session_id {
                                    if let Some(target) = self.targets.get_mut(target_id) {
                                        target.attached = false;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    Ok(JsonValue::object())
                }
                _ => Err(CdpError::method_not_found(&alloc::format!(
                    "Target.{}",
                    method
                ))),
            }
        }
    }

    /// Page domain.
    pub struct PageDomain {
        enabled: bool,
        frame_tree: Option<FrameInfo>,
    }

    impl PageDomain {
        pub fn new() -> Self {
            Self {
                enabled: false,
                frame_tree: None,
            }
        }

        pub fn set_frame(&mut self, frame: FrameInfo) {
            self.frame_tree = Some(frame);
        }
    }

    impl Default for PageDomain {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CdpDomain for PageDomain {
        fn name(&self) -> &'static str {
            "Page"
        }

        fn enable(&mut self) -> Result<JsonValue, CdpError> {
            self.enabled = true;
            Ok(JsonValue::object())
        }

        fn disable(&mut self) -> Result<JsonValue, CdpError> {
            self.enabled = false;
            Ok(JsonValue::object())
        }

        fn handle(
            &mut self,
            method: &str,
            _params: Option<&JsonValue>,
        ) -> Result<JsonValue, CdpError> {
            match method {
                "getFrameTree" => {
                    if let Some(ref frame) = self.frame_tree {
                        let mut frame_obj = JsonValue::object();
                        frame_obj.insert("id", frame.id.clone().into());
                        frame_obj.insert("loaderId", frame.loader_id.clone().into());
                        frame_obj.insert("url", frame.url.clone().into());
                        frame_obj.insert("securityOrigin", frame.security_origin.clone().into());
                        frame_obj.insert("mimeType", frame.mime_type.clone().into());

                        let mut tree = JsonValue::object();
                        tree.insert("frame", frame_obj);
                        tree.insert("childFrames", JsonValue::array());

                        let mut result = JsonValue::object();
                        result.insert("frameTree", tree);
                        Ok(result)
                    } else {
                        Err(CdpError::internal_error("No frame tree"))
                    }
                }
                "navigate" => {
                    // Would trigger navigation
                    let mut result = JsonValue::object();
                    result.insert("frameId", "main".into());
                    result.insert("loaderId", "loader-1".into());
                    Ok(result)
                }
                "reload" => {
                    // Would reload the page
                    Ok(JsonValue::object())
                }
                "stopLoading" => {
                    // Would stop loading
                    Ok(JsonValue::object())
                }
                "bringToFront" => {
                    // Would bring page to front
                    Ok(JsonValue::object())
                }
                "captureScreenshot" => {
                    // Would capture screenshot
                    let mut result = JsonValue::object();
                    result.insert("data", "".into()); // Base64 encoded image
                    Ok(result)
                }
                _ => Err(CdpError::method_not_found(&alloc::format!(
                    "Page.{}", method
                ))),
            }
        }
    }

    /// Runtime domain.
    pub struct RuntimeDomain {
        enabled: bool,
        execution_contexts: Vec<ExecutionContextDescription>,
    }

    impl RuntimeDomain {
        pub fn new() -> Self {
            Self {
                enabled: false,
                execution_contexts: Vec::new(),
            }
        }

        pub fn add_execution_context(&mut self, context: ExecutionContextDescription) {
            self.execution_contexts.push(context);
        }
    }

    impl Default for RuntimeDomain {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CdpDomain for RuntimeDomain {
        fn name(&self) -> &'static str {
            "Runtime"
        }

        fn enable(&mut self) -> Result<JsonValue, CdpError> {
            self.enabled = true;
            Ok(JsonValue::object())
        }

        fn disable(&mut self) -> Result<JsonValue, CdpError> {
            self.enabled = false;
            Ok(JsonValue::object())
        }

        fn handle(
            &mut self,
            method: &str,
            params: Option<&JsonValue>,
        ) -> Result<JsonValue, CdpError> {
            match method {
                "evaluate" => {
                    if let Some(params) = params {
                        if let Some(_expression) = params.get("expression").and_then(|v| v.as_str())
                        {
                            // Would evaluate the expression
                            let mut remote_object = JsonValue::object();
                            remote_object.insert("type", "undefined".into());

                            let mut result = JsonValue::object();
                            result.insert("result", remote_object);
                            return Ok(result);
                        }
                    }
                    Err(CdpError::invalid_params("expression required"))
                }
                "callFunctionOn" => {
                    // Would call function on object
                    let mut remote_object = JsonValue::object();
                    remote_object.insert("type", "undefined".into());

                    let mut result = JsonValue::object();
                    result.insert("result", remote_object);
                    Ok(result)
                }
                "getProperties" => {
                    // Would get object properties
                    let mut result = JsonValue::object();
                    result.insert("result", JsonValue::array());
                    Ok(result)
                }
                "releaseObject" => {
                    // Would release remote object reference
                    Ok(JsonValue::object())
                }
                "releaseObjectGroup" => {
                    // Would release object group
                    Ok(JsonValue::object())
                }
                "runIfWaitingForDebugger" => {
                    // Would resume if waiting for debugger
                    Ok(JsonValue::object())
                }
                "compileScript" => {
                    // Would compile script
                    let mut result = JsonValue::object();
                    result.insert("scriptId", "compiled-1".into());
                    Ok(result)
                }
                "runScript" => {
                    // Would run compiled script
                    let mut remote_object = JsonValue::object();
                    remote_object.insert("type", "undefined".into());

                    let mut result = JsonValue::object();
                    result.insert("result", remote_object);
                    Ok(result)
                }
                _ => Err(CdpError::method_not_found(&alloc::format!(
                    "Runtime.{}",
                    method
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::domains::*;
    use super::*;

    #[test]
    fn test_cdp_request() {
        let request = CdpRequest::new(1, "DOM.getDocument");
        assert_eq!(request.domain(), "DOM");
        assert_eq!(request.method_name(), "getDocument");
    }

    #[test]
    fn test_json_value() {
        let mut obj = JsonValue::object();
        obj.insert("name", "test".into());
        obj.insert("value", 42.into());
        obj.insert("enabled", true.into());

        assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("test"));
        assert_eq!(obj.get("value").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(obj.get("enabled").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_protocol_handler() {
        let mut handler = ProtocolHandler::new();
        handler.register_domain(TargetDomain::new());
        handler.register_domain(PageDomain::new());
        handler.register_domain(RuntimeDomain::new());

        let request = CdpRequest::new(1, "Target.getTargets");
        let response = handler.handle_request(request);

        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[test]
    fn test_cdp_error() {
        let error = CdpError::method_not_found("Unknown.method");
        assert_eq!(error.code, -32601);
    }
}
