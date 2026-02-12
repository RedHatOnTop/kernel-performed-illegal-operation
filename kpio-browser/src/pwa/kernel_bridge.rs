//! Kernel Bridge for PWA Management
//!
//! This module provides the interface between the browser-level PWA
//! management (`PwaManager`, `InstallManager`) and the kernel-level
//! app management subsystem (syscalls 106-111).
//!
//! Since `kpio-browser` cannot directly depend on the kernel crate,
//! communication happens through **function pointer callbacks** that
//! the kernel registers at boot time via `set_bridge()`.
//!
//! # Flow
//!
//! ```text
//! PwaManager::install(manifest)
//!   → kernel_bridge::pwa_install_to_kernel(manifest)
//!     → BRIDGE.install_fn(type, name, entry, scope)
//!       → [kernel] AppInstall syscall handler
//!         → AppRegistry::register()
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::{InstalledApp, PwaError, WebAppManifest, DisplayMode, AppIcon, IconPurpose};

// ─── Kernel App ID ─────────────────────────────────────────────

/// Kernel-assigned app identifier.
/// Mirrors `KernelAppId` from the kernel crate without creating a
/// dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct KernelAppId(pub u64);

// ─── Bridge callback signatures ────────────────────────────────

/// Install callback: (app_type, name, entry_point, scope) → Result<app_id, error_msg>
///   app_type: 0=WebApp, 1=WasmApp, 2=NativeApp
pub type InstallFn = fn(u64, &str, &str, &str) -> Result<u64, &'static str>;

/// Uninstall callback: (app_id) → Result<(), error_msg>
pub type UninstallFn = fn(u64) -> Result<(), &'static str>;

/// Launch callback: (app_id) → Result<instance_id, error_msg>
pub type LaunchFn = fn(u64) -> Result<u64, &'static str>;

/// Terminate callback: (instance_id) → Result<(), error_msg>
pub type TerminateFn = fn(u64) -> Result<(), &'static str>;

/// Save manifest callback: (app_id, manifest_json) → Result<(), error_msg>
pub type SaveManifestFn = fn(u64, &[u8]) -> Result<(), &'static str>;

/// Load manifest callback: (app_id) → Result<manifest_json_bytes, error_msg>
pub type LoadManifestFn = fn(u64) -> Result<Vec<u8>, &'static str>;

/// List kernel app IDs callback: () → Vec<(app_id, app_name)>
pub type ListAppsFn = fn() -> Vec<(u64, String)>;

// ─── Bridge state ──────────────────────────────────────────────

/// Bridge holding all kernel callbacks.
struct KernelBridge {
    install: Option<InstallFn>,
    uninstall: Option<UninstallFn>,
    launch: Option<LaunchFn>,
    terminate: Option<TerminateFn>,
    save_manifest: Option<SaveManifestFn>,
    load_manifest: Option<LoadManifestFn>,
    list_apps: Option<ListAppsFn>,
}

impl KernelBridge {
    const fn new() -> Self {
        Self {
            install: None,
            uninstall: None,
            launch: None,
            terminate: None,
            save_manifest: None,
            load_manifest: None,
            list_apps: None,
        }
    }

    fn is_connected(&self) -> bool {
        self.install.is_some()
    }
}

/// Global bridge instance.
static BRIDGE: Mutex<KernelBridge> = Mutex::new(KernelBridge::new());

// ─── Bridge registration (called by kernel at boot) ───────────

/// Register all kernel callbacks. Called once by the kernel during
/// initialisation.
pub fn set_bridge(
    install: InstallFn,
    uninstall: UninstallFn,
    launch: LaunchFn,
    terminate: TerminateFn,
    save_manifest: SaveManifestFn,
    load_manifest: LoadManifestFn,
    list_apps: ListAppsFn,
) {
    let mut bridge = BRIDGE.lock();
    bridge.install = Some(install);
    bridge.uninstall = Some(uninstall);
    bridge.launch = Some(launch);
    bridge.terminate = Some(terminate);
    bridge.save_manifest = Some(save_manifest);
    bridge.load_manifest = Some(load_manifest);
    bridge.list_apps = Some(list_apps);
}

/// Check if the kernel bridge has been connected.
pub fn is_connected() -> bool {
    BRIDGE.lock().is_connected()
}

// ─── High-level operations ─────────────────────────────────────

/// Install a PWA into the kernel app registry.
///
/// 1. Extracts name, start_url, scope from the manifest
/// 2. Calls the kernel install callback → receives `KernelAppId`
/// 3. Serialises the full manifest JSON to `/apps/data/{id}/manifest.json`
///
/// Returns the kernel app ID on success.
pub fn pwa_install_to_kernel(manifest: &WebAppManifest) -> Result<KernelAppId, PwaError> {
    let bridge = BRIDGE.lock();
    let install_fn = bridge.install
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;
    let save_fn = bridge.save_manifest
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;

    let name = manifest.display_name();
    let entry = &manifest.start_url;
    let scope = manifest.scope.as_deref().unwrap_or("/");

    // app_type 0 = WebApp
    let app_id = install_fn(0, name, entry, scope)
        .map_err(|e| PwaError::InstallationFailed(String::from(e)))?;

    // Serialise manifest to JSON and persist
    let manifest_json = serialize_manifest_simple(manifest);
    let _ = save_fn(app_id, manifest_json.as_bytes());

    Ok(KernelAppId(app_id))
}

/// Uninstall a PWA from the kernel registry.
pub fn pwa_uninstall_from_kernel(app_id: KernelAppId) -> Result<(), PwaError> {
    let bridge = BRIDGE.lock();
    let uninstall_fn = bridge.uninstall
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;

    uninstall_fn(app_id.0)
        .map_err(|e| PwaError::InstallationFailed(String::from(e)))
}

/// Launch a PWA via the kernel, returning the start_url and instance_id.
pub fn pwa_launch_from_kernel(app_id: KernelAppId) -> Result<(u64, String), PwaError> {
    let bridge = BRIDGE.lock();
    let launch_fn = bridge.launch
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;
    let load_fn = bridge.load_manifest
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;

    // Launch in kernel
    let instance_id = launch_fn(app_id.0)
        .map_err(|e| PwaError::InstallationFailed(String::from(e)))?;

    // Load manifest to extract start_url
    let manifest_bytes = load_fn(app_id.0)
        .map_err(|e| PwaError::ManifestFetchFailed(String::from(e)))?;
    let manifest_str = core::str::from_utf8(&manifest_bytes)
        .map_err(|_| PwaError::InvalidManifest(String::from("UTF-8 decode failed")))?;
    let start_url = extract_field(manifest_str, "start_url")
        .unwrap_or_else(|| String::from("/"));

    Ok((instance_id, start_url))
}

/// Terminate a PWA instance in the kernel.
pub fn pwa_terminate_in_kernel(instance_id: u64) -> Result<(), PwaError> {
    let bridge = BRIDGE.lock();
    let terminate_fn = bridge.terminate
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;

    terminate_fn(instance_id)
        .map_err(|e| PwaError::InstallationFailed(String::from(e)))
}

/// Synchronise the PwaManager with the kernel app registry.
///
/// Loads all WebApp-type entries from the kernel registry and merges
/// them into the PwaManager, creating `InstalledApp` entries for any
/// kernel-registered apps not yet tracked by the browser.
///
/// Returns `(added, removed)` counts.
pub fn sync_pwa_registry() -> Result<(usize, usize), PwaError> {
    let bridge = BRIDGE.lock();
    let list_fn = bridge.list_apps
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;
    let load_fn = bridge.load_manifest
        .ok_or(PwaError::InstallationFailed(String::from("Kernel bridge not connected")))?;

    let kernel_apps = list_fn();
    drop(bridge); // Release lock before acquiring PWA_MANAGER

    let mut pwa_mgr = super::PWA_MANAGER.write();
    let mut added = 0usize;
    let mut removed = 0usize;

    // Collect existing PWA app IDs with kernel_app_id
    let existing_kernel_ids: Vec<u64> = pwa_mgr.installed_apps()
        .filter_map(|app| app.kernel_app_id.map(|kid| kid.0))
        .collect();

    // Add kernel apps not in PwaManager
    for (kid, _name) in &kernel_apps {
        if existing_kernel_ids.contains(kid) {
            continue;
        }

        // Try to load manifest from kernel
        let bridge = BRIDGE.lock();
        if let Some(load) = bridge.load_manifest {
            if let Ok(bytes) = load(*kid) {
                if let Ok(json_str) = core::str::from_utf8(&bytes) {
                    if let Ok(manifest) = WebAppManifest::parse(json_str) {
                        let app = installed_app_from_manifest(&manifest, KernelAppId(*kid));
                        let _ = pwa_mgr.install(app);
                        added += 1;
                    }
                }
            }
        }
    }

    // Remove PwaManager apps whose kernel ID no longer exists
    let kernel_id_set: Vec<u64> = kernel_apps.iter().map(|(id, _)| *id).collect();
    let to_remove: Vec<String> = pwa_mgr.installed_apps()
        .filter(|app| {
            if let Some(kid) = app.kernel_app_id {
                !kernel_id_set.contains(&kid.0)
            } else {
                false // Keep apps without kernel IDs (legacy)
            }
        })
        .map(|app| app.id.clone())
        .collect();

    for id in to_remove {
        let _ = pwa_mgr.uninstall(&id);
        removed += 1;
    }

    Ok((added, removed))
}

// ─── Installability detection ──────────────────────────────────

/// Check whether an HTML document contains a link to a web app manifest.
///
/// Looks for `<link rel="manifest" href="...">` and returns the href
/// if found.
pub fn detect_manifest_link(html: &str) -> Option<String> {
    // Simple substring search — fine for kernel context
    let lower = html.to_lowercase();

    // Find <link ... rel="manifest" ... href="...">
    let mut search_from = 0;
    while let Some(link_start) = lower[search_from..].find("<link") {
        let abs_start = search_from + link_start;
        let tag_end = match lower[abs_start..].find('>') {
            Some(pos) => abs_start + pos,
            None => break,
        };
        let tag = &html[abs_start..=tag_end];
        let tag_lower = &lower[abs_start..=tag_end];

        if tag_lower.contains("rel=\"manifest\"") || tag_lower.contains("rel='manifest'") {
            // Extract href
            if let Some(href) = extract_attr(tag, "href") {
                return Some(href);
            }
        }
        search_from = tag_end + 1;
    }

    None
}

/// Evaluate whether a page is installable as a PWA.
///
/// Checks:
/// - Manifest link present
/// - Valid manifest with name + start_url
/// - HTTPS or `kpio://` origin
pub fn evaluate_installability(
    manifest: &WebAppManifest,
    url: &str,
    has_service_worker: bool,
) -> InstallabilityResult {
    let mut errors = Vec::new();

    // Origin check
    if !url.starts_with("https://") && !url.starts_with("kpio://") {
        errors.push(InstallabilityError::NotSecureOrigin);
    }

    // Mandatory fields
    if manifest.name.is_none() && manifest.short_name.is_none() {
        errors.push(InstallabilityError::NoName);
    }
    if manifest.start_url.is_empty() {
        errors.push(InstallabilityError::NoStartUrl);
    }

    // Offline capability
    let offline = has_service_worker;

    InstallabilityResult {
        installable: errors.is_empty(),
        offline_capable: offline,
        errors,
    }
}

/// Result of installability evaluation.
#[derive(Debug, Clone)]
pub struct InstallabilityResult {
    pub installable: bool,
    pub offline_capable: bool,
    pub errors: Vec<InstallabilityError>,
}

/// Reasons why a page is not installable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallabilityError {
    NotSecureOrigin,
    NoName,
    NoStartUrl,
    NoManifest,
}

// ─── Helpers ───────────────────────────────────────────────────

/// Build an `InstalledApp` from a manifest and kernel app ID.
fn installed_app_from_manifest(manifest: &WebAppManifest, kid: KernelAppId) -> InstalledApp {
    InstalledApp {
        id: alloc::format!("kernel-{}", kid.0),
        name: manifest.display_name().into(),
        start_url: manifest.start_url.clone(),
        scope: manifest.scope.clone().unwrap_or_else(|| String::from("/")),
        display: manifest.display,
        theme_color: manifest.theme_color_u32(),
        background_color: manifest.background_color_u32(),
        icons: manifest.icons.iter().map(|i| i.to_app_icon()).collect(),
        installed_at: 0,
        last_launched: None,
        kernel_app_id: Some(kid),
    }
}

/// A minimal manifest serialiser (hand-rolled JSON, no serde).
fn serialize_manifest_simple(m: &WebAppManifest) -> String {
    let name = m.name.as_deref().unwrap_or("");
    let short = m.short_name.as_deref().unwrap_or("");
    let scope = m.scope.as_deref().unwrap_or("/");
    let display = m.display.as_str();
    let theme = m.theme_color.as_deref().unwrap_or("");
    let bg = m.background_color.as_deref().unwrap_or("");

    alloc::format!(
        "{{\"name\":\"{}\",\"short_name\":\"{}\",\"start_url\":\"{}\",\"scope\":\"{}\",\"display\":\"{}\",\"theme_color\":\"{}\",\"background_color\":\"{}\"}}",
        name, short, &m.start_url, scope, display, theme, bg
    )
}

/// Extract a JSON string field value (very simple, no nesting).
fn extract_field(json: &str, field: &str) -> Option<String> {
    let pattern = alloc::format!("\"{}\":\"", field);
    let start = json.find(&pattern)? + pattern.len();
    let end = json[start..].find('"')? + start;
    Some(String::from(&json[start..end]))
}

/// Extract an HTML attribute value from a tag string.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let patterns = [
        alloc::format!("{}=\"", attr),
        alloc::format!("{}='", attr),
    ];

    for pat in &patterns {
        if let Some(start) = tag.find(pat.as_str()) {
            let val_start = start + pat.len();
            let quote = tag.as_bytes()[start + pat.len() - 1] as char;
            if let Some(end) = tag[val_start..].find(quote) {
                return Some(String::from(&tag[val_start..val_start + end]));
            }
        }
    }
    None
}

// ─── Unit Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_manifest_link_basic() {
        let html = r#"<html><head><link rel="manifest" href="/app.json"></head></html>"#;
        assert_eq!(detect_manifest_link(html), Some(String::from("/app.json")));
    }

    #[test]
    fn test_detect_manifest_link_single_quotes() {
        let html = "<html><head><link rel='manifest' href='/manifest.webmanifest'></head></html>";
        assert_eq!(detect_manifest_link(html), Some(String::from("/manifest.webmanifest")));
    }

    #[test]
    fn test_detect_manifest_missing() {
        let html = "<html><head><link rel=\"stylesheet\" href=\"/style.css\"></head></html>";
        assert_eq!(detect_manifest_link(html), None);
    }

    #[test]
    fn test_extract_field() {
        let json = r#"{"name":"My App","start_url":"/index.html"}"#;
        assert_eq!(extract_field(json, "name"), Some(String::from("My App")));
        assert_eq!(extract_field(json, "start_url"), Some(String::from("/index.html")));
        assert_eq!(extract_field(json, "missing"), None);
    }

    #[test]
    fn test_serialize_roundtrip() {
        let mut m = WebAppManifest::new();
        m.name = Some(String::from("Test App"));
        m.start_url = String::from("/start");
        m.scope = Some(String::from("/"));
        m.display = DisplayMode::Standalone;

        let json = serialize_manifest_simple(&m);
        assert!(json.contains("\"name\":\"Test App\""));
        assert!(json.contains("\"start_url\":\"/start\""));
        assert!(json.contains("\"display\":\"standalone\""));

        let name = extract_field(&json, "name");
        assert_eq!(name, Some(String::from("Test App")));
    }

    #[test]
    fn test_installability_no_name() {
        let mut m = WebAppManifest::new();
        m.start_url = String::from("/");
        let result = evaluate_installability(&m, "https://example.com", false);
        assert!(!result.installable);
        assert!(result.errors.contains(&InstallabilityError::NoName));
    }

    #[test]
    fn test_installability_not_https() {
        let mut m = WebAppManifest::new();
        m.name = Some(String::from("App"));
        m.start_url = String::from("/");
        let result = evaluate_installability(&m, "http://example.com", false);
        assert!(!result.installable);
        assert!(result.errors.contains(&InstallabilityError::NotSecureOrigin));
    }

    #[test]
    fn test_installability_kpio_scheme_ok() {
        let mut m = WebAppManifest::new();
        m.name = Some(String::from("KPIO App"));
        m.start_url = String::from("/app");
        let result = evaluate_installability(&m, "kpio://my-app", true);
        assert!(result.installable);
        assert!(result.offline_capable);
    }

    #[test]
    fn test_bridge_not_connected() {
        let result = is_connected();
        // In test, bridge is not connected
        assert!(!result);
    }

    #[test]
    fn test_extract_attr() {
        let tag = r#"<link rel="manifest" href="/app.json" crossorigin="use-credentials">"#;
        assert_eq!(extract_attr(tag, "href"), Some(String::from("/app.json")));
        assert_eq!(extract_attr(tag, "rel"), Some(String::from("manifest")));
    }
}
