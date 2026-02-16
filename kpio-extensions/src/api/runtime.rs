//! chrome.runtime API
//!
//! Provides extension lifecycle and messaging APIs.

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

use super::{ApiContext, ApiError, ApiResult, EventEmitter};
use crate::api::tabs::TabId;
use crate::ExtensionId;

/// Port name.
pub type PortName = String;

/// Port for long-lived connections.
#[derive(Debug, Clone)]
pub struct Port {
    /// Port name.
    pub name: String,
    /// Sender info.
    pub sender: Option<MessageSender>,
    /// Disconnect callback ID.
    disconnect_id: Option<u64>,
    /// Message callback ID.
    message_id: Option<u64>,
}

impl Port {
    /// Create a new port.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sender: None,
            disconnect_id: None,
            message_id: None,
        }
    }

    /// Post a message.
    pub fn post_message(&self, _message: &str) -> ApiResult<()> {
        // Would send message through port
        Ok(())
    }

    /// Disconnect the port.
    pub fn disconnect(&self) {
        // Would close the port
    }
}

/// Message sender information.
#[derive(Debug, Clone)]
pub struct MessageSender {
    /// Tab that opened the connection.
    pub tab: Option<TabId>,
    /// Frame ID.
    pub frame_id: Option<i32>,
    /// Document ID.
    pub document_id: Option<String>,
    /// URL of the sender.
    pub url: Option<String>,
    /// Origin of the sender.
    pub origin: Option<String>,
    /// Extension ID.
    pub id: Option<ExtensionId>,
    /// Whether from native messaging.
    pub native_messaging_host: Option<String>,
    /// TLS channel ID.
    pub tls_channel_id: Option<String>,
}

impl Default for MessageSender {
    fn default() -> Self {
        Self {
            tab: None,
            frame_id: None,
            document_id: None,
            url: None,
            origin: None,
            id: None,
            native_messaging_host: None,
            tls_channel_id: None,
        }
    }
}

/// Connect info.
#[derive(Debug, Clone, Default)]
pub struct ConnectInfo {
    /// Port name.
    pub name: Option<String>,
    /// Include TLS channel ID.
    pub include_tls_channel_id: Option<bool>,
}

/// Extension info.
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    /// Extension ID.
    pub id: ExtensionId,
    /// Extension name.
    pub name: String,
    /// Short name.
    pub short_name: String,
    /// Description.
    pub description: String,
    /// Version.
    pub version: String,
    /// Version name.
    pub version_name: Option<String>,
    /// Manifest version.
    pub manifest_version: u8,
    /// Install type.
    pub install_type: InstallType,
    /// Whether enabled.
    pub enabled: bool,
    /// May enable.
    pub may_enable: bool,
    /// May disable.
    pub may_disable: bool,
    /// Disabled reason.
    pub disabled_reason: Option<DisabledReason>,
    /// Host permissions.
    pub host_permissions: Vec<String>,
    /// Permissions.
    pub permissions: Vec<String>,
    /// Optional permissions.
    pub optional_permissions: Vec<String>,
    /// Homepage URL.
    pub homepage_url: Option<String>,
    /// Update URL.
    pub update_url: Option<String>,
    /// Options URL.
    pub options_url: Option<String>,
    /// Icon URLs.
    pub icons: BTreeMap<String, String>,
    /// Is app.
    pub is_app: bool,
    /// Launch type for apps.
    pub app_launch_url: Option<String>,
    /// Offline enabled.
    pub offline_enabled: bool,
    /// Type.
    pub extension_type: ExtensionType,
}

/// Install type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallType {
    Admin,
    Development,
    Normal,
    Sideload,
    Other,
}

/// Disabled reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledReason {
    Unknown,
    PermissionsIncrease,
}

/// Extension type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionType {
    Extension,
    HostedApp,
    PackagedApp,
    LegacyPackagedApp,
    Theme,
    LoginScreenExtension,
}

/// Platform info.
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// OS.
    pub os: PlatformOs,
    /// Architecture.
    pub arch: PlatformArch,
    /// Native client architecture.
    pub nacl_arch: PlatformNaclArch,
}

/// Platform OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformOs {
    Mac,
    Win,
    Android,
    Cros,
    Linux,
    OpenBsd,
    Fuchsia,
}

/// Platform architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformArch {
    Arm,
    Arm64,
    X86_32,
    X86_64,
    Mips,
    Mips64,
}

/// Platform NaCl architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformNaclArch {
    Arm,
    X86_32,
    X86_64,
    Mips,
    Mips64,
}

/// Installed reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnInstalledReason {
    Install,
    Update,
    ChromeUpdate,
    SharedModuleUpdate,
}

/// Install details.
#[derive(Debug, Clone)]
pub struct InstalledDetails {
    /// Install reason.
    pub reason: OnInstalledReason,
    /// Previous version (on update).
    pub previous_version: Option<String>,
    /// ID of shared module.
    pub id: Option<String>,
}

/// Update check status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCheckStatus {
    Throttled,
    NoUpdate,
    UpdateAvailable,
}

/// Update check result.
#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    /// Status.
    pub status: UpdateCheckStatus,
    /// Version if available.
    pub version: Option<String>,
}

/// Context type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextType {
    Tab,
    Popup,
    Background,
    OffscreenDocument,
    SidePanel,
}

/// Runtime API.
pub struct RuntimeApi {
    /// Extension ID.
    extension_id: RwLock<Option<ExtensionId>>,
    /// Extension info.
    extension_info: RwLock<Option<ExtensionInfo>>,
    /// Last error.
    last_error: RwLock<Option<String>>,
    /// Message handlers.
    message_handlers: RwLock<Vec<MessageHandler>>,
    /// Ports.
    ports: RwLock<BTreeMap<u64, Port>>,
    /// Next port ID.
    next_port_id: RwLock<u64>,
    /// On install event.
    pub on_installed: RwLock<EventEmitter<InstalledDetails>>,
    /// On startup event.
    pub on_startup: RwLock<EventEmitter<()>>,
    /// On suspend event.
    pub on_suspend: RwLock<EventEmitter<()>>,
    /// On suspend canceled event.
    pub on_suspend_canceled: RwLock<EventEmitter<()>>,
    /// On update available event.
    pub on_update_available: RwLock<EventEmitter<UpdateCheckResult>>,
    /// On connect event.
    pub on_connect: RwLock<EventEmitter<Port>>,
    /// On connect external event.
    pub on_connect_external: RwLock<EventEmitter<Port>>,
    /// On message event.
    pub on_message: RwLock<EventEmitter<(String, MessageSender)>>,
    /// On message external event.
    pub on_message_external: RwLock<EventEmitter<(String, MessageSender)>>,
}

/// Message handler type.
type MessageHandler = Box<dyn Fn(&str, &MessageSender) -> Option<String> + Send + Sync>;

impl RuntimeApi {
    /// Create a new Runtime API.
    pub fn new() -> Self {
        Self {
            extension_id: RwLock::new(None),
            extension_info: RwLock::new(None),
            last_error: RwLock::new(None),
            message_handlers: RwLock::new(Vec::new()),
            ports: RwLock::new(BTreeMap::new()),
            next_port_id: RwLock::new(1),
            on_installed: RwLock::new(EventEmitter::new()),
            on_startup: RwLock::new(EventEmitter::new()),
            on_suspend: RwLock::new(EventEmitter::new()),
            on_suspend_canceled: RwLock::new(EventEmitter::new()),
            on_update_available: RwLock::new(EventEmitter::new()),
            on_connect: RwLock::new(EventEmitter::new()),
            on_connect_external: RwLock::new(EventEmitter::new()),
            on_message: RwLock::new(EventEmitter::new()),
            on_message_external: RwLock::new(EventEmitter::new()),
        }
    }

    /// Get extension ID.
    pub fn id(&self, ctx: &ApiContext) -> ExtensionId {
        ctx.extension_id.clone()
    }

    /// Get URL of a resource.
    pub fn get_url(&self, ctx: &ApiContext, path: &str) -> String {
        alloc::format!(
            "chrome-extension://{}/{}",
            ctx.extension_id.as_str(),
            path.trim_start_matches('/')
        )
    }

    /// Get manifest.
    pub fn get_manifest(&self, _ctx: &ApiContext) -> ApiResult<String> {
        // Would return parsed manifest
        Ok("{}".to_string())
    }

    /// Get platform info.
    pub fn get_platform_info(&self, _ctx: &ApiContext) -> ApiResult<PlatformInfo> {
        Ok(PlatformInfo {
            os: PlatformOs::Linux, // KPIO custom OS
            arch: PlatformArch::X86_64,
            nacl_arch: PlatformNaclArch::X86_64,
        })
    }

    /// Get background page.
    pub fn get_background_page(&self, _ctx: &ApiContext) -> ApiResult<Option<String>> {
        // Would return background page window
        Ok(None)
    }

    /// Open options page.
    pub fn open_options_page(&self, _ctx: &ApiContext) -> ApiResult<()> {
        // Would open extension options page
        Ok(())
    }

    /// Set uninstall URL.
    pub fn set_uninstall_url(&self, _ctx: &ApiContext, url: &str) -> ApiResult<()> {
        if !url.starts_with("http://") && !url.starts_with("https://") && !url.is_empty() {
            return Err(ApiError::invalid_argument("URL must be HTTP(S) or empty"));
        }
        // Would set uninstall URL
        Ok(())
    }

    /// Reload extension.
    pub fn reload(&self, _ctx: &ApiContext) {
        // Would reload extension
    }

    /// Request update check.
    pub fn request_update_check(&self, _ctx: &ApiContext) -> ApiResult<UpdateCheckResult> {
        Ok(UpdateCheckResult {
            status: UpdateCheckStatus::NoUpdate,
            version: None,
        })
    }

    /// Restart device (Chrome OS only).
    pub fn restart(&self, _ctx: &ApiContext) -> ApiResult<()> {
        Err(ApiError::permission_denied("restart"))
    }

    /// Restart after delay (Chrome OS only).
    pub fn restart_after_delay(&self, _ctx: &ApiContext, _seconds: i32) -> ApiResult<()> {
        Err(ApiError::permission_denied("restart"))
    }

    /// Connect to extension.
    pub fn connect(
        &self,
        ctx: &ApiContext,
        extension_id: Option<ExtensionId>,
        info: ConnectInfo,
    ) -> ApiResult<Port> {
        let target_id = extension_id.unwrap_or_else(|| ctx.extension_id.clone());

        let mut next_id = self.next_port_id.write();
        let port_id = *next_id;
        *next_id += 1;

        let port = Port {
            name: info.name.unwrap_or_default(),
            sender: Some(MessageSender {
                id: Some(ctx.extension_id.clone()),
                ..Default::default()
            }),
            disconnect_id: None,
            message_id: None,
        };

        self.ports.write().insert(port_id, port.clone());

        // Emit connect event on target
        if target_id == ctx.extension_id {
            self.on_connect.read().emit(&port);
        } else {
            self.on_connect_external.read().emit(&port);
        }

        Ok(port)
    }

    /// Connect to native application.
    pub fn connect_native(&self, _ctx: &ApiContext, application: &str) -> ApiResult<Port> {
        let port = Port {
            name: application.to_string(),
            sender: None,
            disconnect_id: None,
            message_id: None,
        };

        Ok(port)
    }

    /// Send one-time message.
    pub fn send_message(
        &self,
        ctx: &ApiContext,
        extension_id: Option<ExtensionId>,
        message: &str,
    ) -> ApiResult<Option<String>> {
        let target_id = extension_id.unwrap_or_else(|| ctx.extension_id.clone());

        let sender = MessageSender {
            id: Some(ctx.extension_id.clone()),
            tab: ctx.tab_id,
            frame_id: ctx.frame_id.map(|id| id as i32),
            ..Default::default()
        };

        // Emit message event
        if target_id == ctx.extension_id {
            self.on_message
                .read()
                .emit(&(message.to_string(), sender.clone()));
        } else {
            self.on_message_external
                .read()
                .emit(&(message.to_string(), sender.clone()));
        }

        // Check message handlers
        let handlers = self.message_handlers.read();
        for handler in handlers.iter() {
            if let Some(response) = handler(message, &sender) {
                return Ok(Some(response));
            }
        }

        Ok(None)
    }

    /// Send native message.
    pub fn send_native_message(
        &self,
        _ctx: &ApiContext,
        _application: &str,
        _message: &str,
    ) -> ApiResult<Option<String>> {
        // Would send to native messaging host
        Err(ApiError::not_found("Native host"))
    }

    /// Get last error.
    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.read().clone()
    }

    /// Set last error.
    pub fn set_last_error(&self, error: Option<String>) {
        *self.last_error.write() = error;
    }

    /// Get context type.
    pub fn get_context_type(&self, ctx: &ApiContext) -> ContextType {
        if ctx.tab_id.is_some() {
            ContextType::Tab
        } else {
            ContextType::Background
        }
    }

    /// Add message handler.
    pub fn add_message_handler<F>(&self, handler: F)
    where
        F: Fn(&str, &MessageSender) -> Option<String> + Send + Sync + 'static,
    {
        self.message_handlers.write().push(Box::new(handler));
    }
}

impl Default for RuntimeApi {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_api() {
        let api = RuntimeApi::new();
        let ctx = ApiContext::new(ExtensionId::new("test-extension"));

        // Get URL
        let url = api.get_url(&ctx, "/popup.html");
        assert_eq!(url, "chrome-extension://test-extension/popup.html");

        // Get platform info
        let info = api.get_platform_info(&ctx).unwrap();
        assert_eq!(info.os, PlatformOs::Linux);
    }

    #[test]
    fn test_messaging() {
        let api = RuntimeApi::new();
        let ctx = ApiContext::new(ExtensionId::new("test"));

        // Connect
        let port = api.connect(&ctx, None, ConnectInfo::default()).unwrap();
        assert!(port.sender.is_some());

        // Send message
        let response = api.send_message(&ctx, None, "hello").unwrap();
        assert!(response.is_none()); // No handler
    }
}
