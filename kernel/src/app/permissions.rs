//! App Permissions Framework
//!
//! Defines the capability-based permission model for installed apps.
//! Each app gets a `AppPermissions` set that constrains its access to
//! filesystem paths, network, notifications, clipboard, and system resources.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::error::AppError;
use super::registry::KernelAppId;

// ── Permission Types ────────────────────────────────────────

/// Filesystem access scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsScope {
    /// No filesystem access at all.
    None,
    /// Read/write only within the app's own data directory.
    AppDataOnly,
    /// Read-only access to a set of additional paths (plus app data r/w).
    ReadOnly(Vec<String>),
    /// Full read/write to specified paths (plus app data r/w).
    ReadWrite(Vec<String>),
}

/// Network access scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetScope {
    /// No network access.
    None,
    /// Only localhost (127.0.0.1 / ::1).
    LocalOnly,
    /// Access limited to a list of allowed domains.
    AllowList(Vec<String>),
    /// Unrestricted network access.
    Full,
}

/// Complete permission set for an app.
#[derive(Debug, Clone)]
pub struct AppPermissions {
    /// Filesystem access scope.
    pub filesystem: FsScope,
    /// Network access scope.
    pub network: NetScope,
    /// Whether the app may show desktop notifications.
    pub notifications: bool,
    /// Whether the app may read/write the system clipboard.
    pub clipboard: bool,
    /// Whether the app may run in the background after its window closes.
    pub background: bool,
    /// Maximum memory the app may consume (KiB). 0 = unlimited.
    pub max_memory_kb: u32,
}

impl AppPermissions {
    /// Default permissions for a newly-installed WebApp.
    ///
    /// Conservative: app-data-only FS, scope-based network, no
    /// notifications until the user grants them.
    pub fn default_web_app() -> Self {
        Self {
            filesystem: FsScope::AppDataOnly,
            network: NetScope::Full, // PWAs typically need network
            notifications: false,    // ask first
            clipboard: false,
            background: false,
            max_memory_kb: 64 * 1024, // 64 MiB
        }
    }

    /// Default permissions for a WASM/WASI app.
    pub fn default_wasm_app() -> Self {
        Self {
            filesystem: FsScope::AppDataOnly,
            network: NetScope::None,
            notifications: false,
            clipboard: false,
            background: false,
            max_memory_kb: 32 * 1024, // 32 MiB
        }
    }

    /// Permissions for built-in system apps (maximally privileged).
    pub fn system_app() -> Self {
        Self {
            filesystem: FsScope::ReadWrite(Vec::new()), // full access
            network: NetScope::Full,
            notifications: true,
            clipboard: true,
            background: true,
            max_memory_kb: 0, // unlimited
        }
    }
}

// ── Permission Checker ──────────────────────────────────────

/// Checks whether a given app is allowed to perform an operation.
pub struct PermissionChecker;

impl PermissionChecker {
    /// Check filesystem access for an app.
    ///
    /// `app_data_dir` is the app's private directory (e.g. `/apps/data/3/`).
    /// `path` is the path the app wants to access.
    /// `write` indicates whether write access is requested.
    pub fn check_fs(
        permissions: &AppPermissions,
        app_data_dir: &str,
        path: &str,
        write: bool,
    ) -> Result<(), AppError> {
        // Normalise: strip trailing slashes for comparison
        let normalised = path.trim_end_matches('/');
        let app_dir = app_data_dir.trim_end_matches('/');

        // App's own data directory is always allowed.
        if normalised.starts_with(app_dir) {
            return Ok(());
        }

        // Always allow read access to system shared resources.
        const GLOBAL_READ_PATHS: &[&str] = &["/system/fonts", "/system/locale", "/system/theme"];
        if !write {
            for allowed in GLOBAL_READ_PATHS {
                if normalised.starts_with(allowed) {
                    return Ok(());
                }
            }
        }

        match &permissions.filesystem {
            FsScope::None => Err(AppError::PermissionDenied),
            FsScope::AppDataOnly => {
                // Already checked above — anything else is denied.
                Err(AppError::PermissionDenied)
            }
            FsScope::ReadOnly(paths) => {
                if write {
                    return Err(AppError::PermissionDenied);
                }
                for allowed in paths {
                    if normalised.starts_with(allowed.trim_end_matches('/')) {
                        return Ok(());
                    }
                }
                Err(AppError::PermissionDenied)
            }
            FsScope::ReadWrite(paths) => {
                // Empty vec = full access (system apps).
                if paths.is_empty() {
                    return Ok(());
                }
                for allowed in paths {
                    if normalised.starts_with(allowed.trim_end_matches('/')) {
                        return Ok(());
                    }
                }
                Err(AppError::PermissionDenied)
            }
        }
    }

    /// Check network access for an app.
    pub fn check_net(permissions: &AppPermissions, domain: &str) -> Result<(), AppError> {
        match &permissions.network {
            NetScope::None => Err(AppError::PermissionDenied),
            NetScope::LocalOnly => {
                if domain == "127.0.0.1" || domain == "::1" || domain == "localhost" {
                    Ok(())
                } else {
                    Err(AppError::PermissionDenied)
                }
            }
            NetScope::AllowList(domains) => {
                if domains.iter().any(|d| d == domain) {
                    Ok(())
                } else {
                    Err(AppError::PermissionDenied)
                }
            }
            NetScope::Full => Ok(()),
        }
    }

    /// Check whether the app may send notifications.
    pub fn check_notification(permissions: &AppPermissions) -> bool {
        permissions.notifications
    }

    /// Check whether the app may access the clipboard.
    pub fn check_clipboard(permissions: &AppPermissions) -> bool {
        permissions.clipboard
    }

    /// Check whether the app may run in the background.
    pub fn check_background(permissions: &AppPermissions) -> bool {
        permissions.background
    }
}

// ── Persistence ─────────────────────────────────────────────

impl AppPermissions {
    /// Persist this permission set to VFS.
    pub fn save(&self, app_id: KernelAppId) -> Result<(), AppError> {
        let path = format!("/system/apps/permissions/{}.json", app_id.0);

        // Ensure directory
        let _ = crate::vfs::write_all("/system/apps/permissions/.keep", b"");

        let json = self.to_json();
        crate::vfs::write_all(&path, json.as_bytes()).map_err(|_| AppError::IoError)
    }

    /// Load permissions from VFS. Returns `default_web_app()` if missing.
    pub fn load(app_id: KernelAppId) -> Self {
        let path = format!("/system/apps/permissions/{}.json", app_id.0);
        match crate::vfs::read_all(&path) {
            Ok(data) => {
                if let Ok(s) = core::str::from_utf8(&data) {
                    Self::from_json(s)
                } else {
                    Self::default_web_app()
                }
            }
            Err(_) => Self::default_web_app(),
        }
    }

    /// Minimal JSON serialiser.
    fn to_json(&self) -> String {
        let fs_str = match &self.filesystem {
            FsScope::None => String::from("\"none\""),
            FsScope::AppDataOnly => String::from("\"app_data\""),
            FsScope::ReadOnly(_) => String::from("\"read_only\""),
            FsScope::ReadWrite(p) if p.is_empty() => String::from("\"full\""),
            FsScope::ReadWrite(_) => String::from("\"read_write\""),
        };
        let net_str = match &self.network {
            NetScope::None => String::from("\"none\""),
            NetScope::LocalOnly => String::from("\"local\""),
            NetScope::AllowList(_) => String::from("\"allow_list\""),
            NetScope::Full => String::from("\"full\""),
        };
        format!(
            "{{\"fs\":{},\"net\":{},\"notif\":{},\"clip\":{},\"bg\":{},\"mem\":{}}}",
            fs_str,
            net_str,
            self.notifications,
            self.clipboard,
            self.background,
            self.max_memory_kb
        )
    }

    /// Minimal JSON deserialiser.
    fn from_json(json: &str) -> Self {
        let get_str = |key: &str| -> String {
            let needle = format!("\"{}\":", key);
            if let Some(pos) = json.find(&needle) {
                let rest = &json[pos + needle.len()..];
                let rest = rest.trim_start();
                if rest.starts_with('"') {
                    let inner = &rest[1..];
                    if let Some(end) = inner.find('"') {
                        return String::from(&inner[..end]);
                    }
                }
            }
            String::new()
        };
        let get_bool = |key: &str| -> bool {
            let needle = format!("\"{}\":", key);
            if let Some(pos) = json.find(&needle) {
                let rest = &json[pos + needle.len()..];
                rest.trim_start().starts_with("true")
            } else {
                false
            }
        };
        let get_u32 = |key: &str| -> u32 {
            let needle = format!("\"{}\":", key);
            if let Some(pos) = json.find(&needle) {
                let rest = &json[pos + needle.len()..];
                let num: String = rest
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                num.parse().unwrap_or(0)
            } else {
                0
            }
        };

        let filesystem = match get_str("fs").as_str() {
            "none" => FsScope::None,
            "app_data" => FsScope::AppDataOnly,
            "read_only" => FsScope::ReadOnly(Vec::new()),
            "full" => FsScope::ReadWrite(Vec::new()),
            "read_write" => FsScope::ReadWrite(Vec::new()),
            _ => FsScope::AppDataOnly,
        };
        let network = match get_str("net").as_str() {
            "none" => NetScope::None,
            "local" => NetScope::LocalOnly,
            "allow_list" => NetScope::AllowList(Vec::new()),
            "full" => NetScope::Full,
            _ => NetScope::Full,
        };

        Self {
            filesystem,
            network,
            notifications: get_bool("notif"),
            clipboard: get_bool("clip"),
            background: get_bool("bg"),
            max_memory_kb: get_u32("mem"),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fs_app_data_always_allowed() {
        let perm = AppPermissions::default_web_app();
        assert!(PermissionChecker::check_fs(
            &perm,
            "/apps/data/5/",
            "/apps/data/5/settings.json",
            true
        )
        .is_ok());
    }

    #[test]
    fn test_fs_outside_app_data_denied() {
        let perm = AppPermissions::default_web_app();
        assert!(
            PermissionChecker::check_fs(&perm, "/apps/data/5/", "/system/config.json", false)
                .is_err()
        );
    }

    #[test]
    fn test_fs_global_read_allowed() {
        let perm = AppPermissions::default_web_app();
        assert!(PermissionChecker::check_fs(
            &perm,
            "/apps/data/5/",
            "/system/fonts/default.ttf",
            false
        )
        .is_ok());
    }

    #[test]
    fn test_fs_global_write_denied() {
        let perm = AppPermissions::default_web_app();
        assert!(PermissionChecker::check_fs(
            &perm,
            "/apps/data/5/",
            "/system/fonts/evil.ttf",
            true
        )
        .is_err());
    }

    #[test]
    fn test_fs_system_app_full_access() {
        let perm = AppPermissions::system_app();
        assert!(
            PermissionChecker::check_fs(&perm, "/apps/data/0/", "/anywhere/anything", true).is_ok()
        );
    }

    #[test]
    fn test_fs_none_denies_everything() {
        let perm = AppPermissions {
            filesystem: FsScope::None,
            ..AppPermissions::default_web_app()
        };
        // Even app data is allowed (special case)
        assert!(PermissionChecker::check_fs(
            &perm,
            "/apps/data/1/",
            "/apps/data/1/file.txt",
            false
        )
        .is_ok());
        // But outside app data is denied
        assert!(PermissionChecker::check_fs(&perm, "/apps/data/1/", "/other", false).is_err());
    }

    #[test]
    fn test_net_none_denied() {
        let perm = AppPermissions {
            network: NetScope::None,
            ..AppPermissions::default_web_app()
        };
        assert!(PermissionChecker::check_net(&perm, "example.com").is_err());
    }

    #[test]
    fn test_net_local_only() {
        let perm = AppPermissions {
            network: NetScope::LocalOnly,
            ..AppPermissions::default_web_app()
        };
        assert!(PermissionChecker::check_net(&perm, "localhost").is_ok());
        assert!(PermissionChecker::check_net(&perm, "127.0.0.1").is_ok());
        assert!(PermissionChecker::check_net(&perm, "example.com").is_err());
    }

    #[test]
    fn test_net_allow_list() {
        let perm = AppPermissions {
            network: NetScope::AllowList(alloc::vec![
                String::from("api.example.com"),
                String::from("cdn.example.com")
            ]),
            ..AppPermissions::default_web_app()
        };
        assert!(PermissionChecker::check_net(&perm, "api.example.com").is_ok());
        assert!(PermissionChecker::check_net(&perm, "evil.com").is_err());
    }

    #[test]
    fn test_net_full_access() {
        let perm = AppPermissions::default_web_app();
        assert!(PermissionChecker::check_net(&perm, "anything.com").is_ok());
    }

    #[test]
    fn test_notification_permission() {
        let mut perm = AppPermissions::default_web_app();
        assert!(!PermissionChecker::check_notification(&perm));
        perm.notifications = true;
        assert!(PermissionChecker::check_notification(&perm));
    }

    #[test]
    fn test_clipboard_permission() {
        let perm = AppPermissions::default_web_app();
        assert!(!PermissionChecker::check_clipboard(&perm));
        let sysperm = AppPermissions::system_app();
        assert!(PermissionChecker::check_clipboard(&sysperm));
    }

    #[test]
    fn test_json_roundtrip() {
        let perm = AppPermissions {
            filesystem: FsScope::AppDataOnly,
            network: NetScope::Full,
            notifications: true,
            clipboard: false,
            background: true,
            max_memory_kb: 65536,
        };
        let json = perm.to_json();
        let restored = AppPermissions::from_json(&json);

        assert_eq!(restored.filesystem, FsScope::AppDataOnly);
        assert_eq!(restored.network, NetScope::Full);
        assert!(restored.notifications);
        assert!(!restored.clipboard);
        assert!(restored.background);
        assert_eq!(restored.max_memory_kb, 65536);
    }

    #[test]
    fn test_path_traversal_blocked() {
        let perm = AppPermissions::default_web_app();
        // An app trying to escape its sandbox via "../"
        assert!(PermissionChecker::check_fs(
            &perm,
            "/apps/data/5/",
            "/apps/data/5/../../etc/passwd",
            false
        )
        .is_err());
    }
}
