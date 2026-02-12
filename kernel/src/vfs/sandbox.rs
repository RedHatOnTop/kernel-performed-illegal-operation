//! VFS App Sandbox
//!
//! Provides per-app filesystem isolation. Each app has a private home
//! directory under `/apps/data/{app_id}/` and may only read a set of
//! globally-allowed system paths.
//!
//! Path traversal attacks (`../`) are detected and rejected.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::app::registry::KernelAppId;
use crate::app::permissions::{AppPermissions, FsScope, PermissionChecker};
use crate::terminal::fs;
use super::VfsError;

/// Globally-readable system paths available to every app (read-only).
const GLOBAL_READ_PATHS: &[&str] = &[
    "/system/fonts",
    "/system/locale",
    "/system/theme",
];

/// Base directory under which all app data directories are created.
const APP_DATA_ROOT: &str = "/apps/data";

// ─── Path helpers ──────────────────────────────────────────────

/// Return the canonical home directory for an app: `/apps/data/{id}/`
pub fn app_home_dir(app_id: KernelAppId) -> String {
    format!("{}/{}", APP_DATA_ROOT, app_id.0)
}

/// Normalise a path by collapsing `.` and `..` components.
/// Returns `Err` if the result would escape the given `root`.
fn normalise_within(root: &str, raw: &str) -> Result<String, VfsError> {
    // Build the starting components from root (strip trailing '/')
    let root = root.trim_end_matches('/');
    let root_depth = root.matches('/').count();

    let mut parts: Vec<&str> = root.split('/').filter(|s| !s.is_empty()).collect();

    for component in raw.split('/') {
        match component {
            "" | "." => { /* skip */ }
            ".." => {
                // Never pop below root depth
                if parts.len() <= root_depth {
                    return Err(VfsError::PermissionDenied);
                }
                parts.pop();
            }
            other => parts.push(other),
        }
    }

    let mut result = String::from("/");
    for (i, p) in parts.iter().enumerate() {
        result.push_str(p);
        if i + 1 < parts.len() {
            result.push('/');
        }
    }

    // Verify the result still starts with root
    if !result.starts_with(root) {
        return Err(VfsError::PermissionDenied);
    }

    Ok(result)
}

// ─── Path resolution ───────────────────────────────────────────

/// Resolve a path requested by an app into an absolute VFS path.
///
/// - **Relative paths** are resolved relative to the app's home directory.
/// - **Absolute paths** are checked against the app's allowed paths plus
///   the globally-readable system paths.
/// - Path traversal (`../`) that escapes allowed roots is rejected.
pub fn resolve_path(
    app_id: KernelAppId,
    requested: &str,
    permissions: &AppPermissions,
) -> Result<String, VfsError> {
    let home = app_home_dir(app_id);

    if requested.starts_with('/') {
        // Absolute path — check if within home or allowed
        resolve_absolute(app_id, requested, &home, permissions)
    } else {
        // Relative path → resolve under home
        normalise_within(&home, requested)
    }
}

/// Check an absolute path against the permission model.
fn resolve_absolute(
    _app_id: KernelAppId,
    path: &str,
    home: &str,
    permissions: &AppPermissions,
) -> Result<String, VfsError> {
    // Normalise first to prevent `..` tricks
    let normalised = normalise_within("/", path)?;

    // Always allow access within own home directory
    if normalised.starts_with(home) {
        return Ok(normalised);
    }

    // Check global readable system paths
    for allowed in GLOBAL_READ_PATHS {
        if normalised.starts_with(allowed) {
            return Ok(normalised);
        }
    }

    // Check per-app filesystem scope
    match &permissions.filesystem {
        FsScope::None => Err(VfsError::PermissionDenied),
        FsScope::AppDataOnly => {
            // Only the home directory is allowed
            Err(VfsError::PermissionDenied)
        }
        FsScope::ReadOnly(paths) => {
            for p in paths {
                if normalised.starts_with(p.as_str()) {
                    return Ok(normalised);
                }
            }
            Err(VfsError::PermissionDenied)
        }
        FsScope::ReadWrite(paths) => {
            for p in paths {
                if normalised.starts_with(p.as_str()) {
                    return Ok(normalised);
                }
            }
            Err(VfsError::PermissionDenied)
        }
    }
}

// ─── Sandboxed VFS Operations ──────────────────────────────────

/// Read all bytes from a file, enforcing sandbox isolation.
pub fn read_all_sandboxed(
    app_id: KernelAppId,
    path: &str,
    permissions: &AppPermissions,
) -> Result<Vec<u8>, VfsError> {
    let resolved = resolve_path(app_id, path, permissions)?;
    crate::vfs::read_all(&resolved)
}

/// Write bytes to a file within the app's home directory.
///
/// Writing is only permitted inside the app's own data directory
/// or in explicitly writable paths.
pub fn write_all_sandboxed(
    app_id: KernelAppId,
    path: &str,
    data: &[u8],
    permissions: &AppPermissions,
) -> Result<(), VfsError> {
    let resolved = resolve_path(app_id, path, permissions)?;
    let home = app_home_dir(app_id);

    // Writing inside home is always OK
    if resolved.starts_with(&home) {
        return crate::vfs::write_all(&resolved, data);
    }

    // Check writable scope
    match &permissions.filesystem {
        FsScope::ReadWrite(paths) => {
            for p in paths {
                if resolved.starts_with(p.as_str()) {
                    return crate::vfs::write_all(&resolved, data);
                }
            }
            Err(VfsError::PermissionDenied)
        }
        _ => Err(VfsError::PermissionDenied),
    }
}

// ─── Directory lifecycle ───────────────────────────────────────

/// Create the data directory for a newly installed app.
///
/// Creates `/apps/data/{app_id}/` (and parent dirs if necessary).
pub fn create_app_directory(app_id: KernelAppId) {
    fs::with_fs(|f| {
        // Ensure /apps exists
        let root = f.resolve("/").unwrap_or(0);
        let apps_ino = f.resolve("/apps").unwrap_or_else(|| {
            f.mkdir(root, "apps").unwrap_or(0)
        });
        // Ensure /apps/data exists
        let data_ino = f.resolve("/apps/data").unwrap_or_else(|| {
            f.mkdir(apps_ino, "data").unwrap_or(0)
        });
        // Create /apps/data/{app_id}
        let dir_name = alloc::format!("{}", app_id.0);
        let _ = f.mkdir(data_ino, &dir_name);
    });

    crate::serial_println!("[app/sandbox] created directory for app {}", app_id.0);
}

/// Remove the data directory for an uninstalled app.
///
/// Removes `/apps/data/{app_id}/` and all its contents.
pub fn remove_app_directory(app_id: KernelAppId) {
    let dir_name = alloc::format!("{}", app_id.0);
    fs::with_fs(|f| {
        if let Some(data_ino) = f.resolve("/apps/data") {
            let _ = f.remove(data_ino, &dir_name);
        }
    });

    crate::serial_println!("[app/sandbox] removed directory for app {}", app_id.0);
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::permissions::{AppPermissions, FsScope, NetScope};
    use alloc::vec;
    use alloc::string::String;

    fn default_perms() -> AppPermissions {
        AppPermissions::default_web_app()
    }

    #[test]
    fn test_relative_path_resolves_to_home() {
        let app_id = KernelAppId(42);
        let perms = default_perms();
        let result = resolve_path(app_id, "data.json", &perms).unwrap();
        assert_eq!(result, "/apps/data/42/data.json");
    }

    #[test]
    fn test_nested_relative_path() {
        let app_id = KernelAppId(7);
        let perms = default_perms();
        let result = resolve_path(app_id, "subdir/file.txt", &perms).unwrap();
        assert_eq!(result, "/apps/data/7/subdir/file.txt");
    }

    #[test]
    fn test_traversal_attack_blocked() {
        let app_id = KernelAppId(1);
        let perms = default_perms();
        let result = resolve_path(app_id, "../../etc/passwd", &perms);
        assert!(result.is_err());
    }

    #[test]
    fn test_absolute_path_within_home_allowed() {
        let app_id = KernelAppId(10);
        let perms = default_perms();
        let result = resolve_path(app_id, "/apps/data/10/cache/index.html", &perms).unwrap();
        assert_eq!(result, "/apps/data/10/cache/index.html");
    }

    #[test]
    fn test_absolute_path_other_app_denied() {
        let app_id = KernelAppId(1);
        let perms = default_perms();
        let result = resolve_path(app_id, "/apps/data/2/secret.txt", &perms);
        assert!(result.is_err());
    }

    #[test]
    fn test_global_read_path_allowed() {
        let app_id = KernelAppId(1);
        let perms = default_perms();
        let result = resolve_path(app_id, "/system/fonts/default.ttf", &perms).unwrap();
        assert_eq!(result, "/system/fonts/default.ttf");
    }

    #[test]
    fn test_system_path_denied_if_not_global() {
        let app_id = KernelAppId(1);
        let perms = default_perms();
        let result = resolve_path(app_id, "/system/secret/key.pem", &perms);
        assert!(result.is_err());
    }

    #[test]
    fn test_readonly_scope_allows_listed_paths() {
        let app_id = KernelAppId(1);
        let mut perms = default_perms();
        perms.filesystem = FsScope::ReadOnly(vec![String::from("/shared/media")]);
        let result = resolve_path(app_id, "/shared/media/photo.png", &perms).unwrap();
        assert_eq!(result, "/shared/media/photo.png");
    }

    #[test]
    fn test_readwrite_scope_allows_write() {
        let app_id = KernelAppId(5);
        let mut perms = default_perms();
        perms.filesystem = FsScope::ReadWrite(vec![String::from("/shared/uploads")]);
        // resolve should succeed for the listed path
        let result = resolve_path(app_id, "/shared/uploads/file.bin", &perms).unwrap();
        assert_eq!(result, "/shared/uploads/file.bin");
    }

    #[test]
    fn test_none_scope_denies_everything() {
        let app_id = KernelAppId(1);
        let mut perms = default_perms();
        perms.filesystem = FsScope::None;
        // Even home should be accessible via relative path (home is always allowed)
        let result = resolve_path(app_id, "file.txt", &perms).unwrap();
        assert_eq!(result, "/apps/data/1/file.txt");
        // But absolute path outside home is denied
        let result = resolve_path(app_id, "/other/path", &perms);
        assert!(result.is_err());
    }

    #[test]
    fn test_dot_segments_normalised() {
        let app_id = KernelAppId(3);
        let perms = default_perms();
        let result = resolve_path(app_id, "./a/./b/../c.txt", &perms).unwrap();
        assert_eq!(result, "/apps/data/3/a/c.txt");
    }

    #[test]
    fn test_app_home_dir() {
        assert_eq!(app_home_dir(KernelAppId(99)), "/apps/data/99");
    }
}
