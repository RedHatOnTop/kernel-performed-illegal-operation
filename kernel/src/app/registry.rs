//! App Registry
//!
//! Manages the catalog of installed applications. Each app receives a unique
//! `KernelAppId` and is persisted to VFS as `/system/apps/registry.json`.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::error::AppError;

// ── Types ───────────────────────────────────────────────────

/// Unique kernel-level application identifier (auto-incrementing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KernelAppId(pub u64);

impl KernelAppId {
    /// Reserved ID meaning "no app" / invalid.
    pub const NONE: KernelAppId = KernelAppId(0);
}

impl core::fmt::Display for KernelAppId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Application type discriminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelAppType {
    /// Progressive Web App — entry_point is `start_url`.
    WebApp {
        /// The URL scope (e.g., `https://example.com/app/`).
        scope: String,
        /// Whether the app has a Service Worker for offline use.
        offline_capable: bool,
    },
    /// WASM/WASI application — entry_point is VFS path to `.wasm`.
    WasmApp {
        /// WASI version string (e.g., `"preview2"`).
        wasi_version: String,
    },
    /// Native ELF binary — entry_point is VFS path to ELF.
    NativeApp,
}

/// Descriptor for an installed application.
#[derive(Debug, Clone)]
pub struct KernelAppDescriptor {
    /// Unique identifier assigned at registration.
    pub id: KernelAppId,
    /// Application type.
    pub app_type: KernelAppType,
    /// Human-readable name.
    pub name: String,
    /// Optional icon data (PNG bytes).
    pub icon_data: Option<Vec<u8>>,
    /// VFS path for app-private data (`/apps/data/{id}/`).
    pub install_path: String,
    /// Entry point: URL for WebApp, VFS path for WASM/Native.
    pub entry_point: String,
    /// Timestamp (kernel ticks) when the app was installed.
    pub installed_at: u64,
    /// Timestamp of last launch.
    pub last_launched: u64,
}

// ── Registry ────────────────────────────────────────────────

/// The registry file path inside VFS.
const REGISTRY_PATH: &str = "/system/apps/registry.json";

/// Maximum number of installed apps.
const MAX_APPS: usize = 256;

/// In-memory catalog of installed applications.
pub struct AppRegistry {
    /// id → descriptor
    apps: BTreeMap<u64, KernelAppDescriptor>,
    /// Next ID to assign.
    next_id: u64,
}

impl AppRegistry {
    /// Create an empty registry.
    pub const fn new() -> Self {
        Self {
            apps: BTreeMap::new(),
            next_id: 1, // 0 is reserved for NONE
        }
    }

    /// Register a new application.
    ///
    /// The caller supplies everything except `id` (auto-assigned) and
    /// `install_path` (derived from the assigned id).
    pub fn register(
        &mut self,
        app_type: KernelAppType,
        name: String,
        entry_point: String,
        icon_data: Option<Vec<u8>>,
    ) -> Result<KernelAppId, AppError> {
        if self.apps.len() >= MAX_APPS {
            return Err(AppError::ResourceExhausted);
        }

        // Duplicate check — same name + same type scope
        if self.find_by_name(&name).is_some() {
            return Err(AppError::AlreadyRegistered);
        }

        // For WebApps, also check scope uniqueness
        if let KernelAppType::WebApp { ref scope, .. } = app_type {
            for desc in self.apps.values() {
                if let KernelAppType::WebApp { scope: ref s, .. } = desc.app_type {
                    if s == scope {
                        return Err(AppError::AlreadyRegistered);
                    }
                }
            }
        }

        let id = KernelAppId(self.next_id);
        self.next_id += 1;

        let install_path = format!("/apps/data/{}/", id.0);

        let descriptor = KernelAppDescriptor {
            id,
            app_type,
            name: name.clone(),
            icon_data,
            install_path,
            entry_point,
            installed_at: Self::now(),
            last_launched: 0,
        };

        self.apps.insert(id.0, descriptor);

        crate::serial_println!("[KPIO/App] Registered app '{}' (id={})", name, id);
        Ok(id)
    }

    /// Unregister an application.
    pub fn unregister(&mut self, id: KernelAppId) -> Result<KernelAppDescriptor, AppError> {
        let desc = self.apps.remove(&id.0).ok_or(AppError::NotFound)?;
        crate::serial_println!("[KPIO/App] Unregistered app '{}' (id={})", desc.name, id);
        Ok(desc)
    }

    /// Look up an app by ID.
    pub fn get(&self, id: KernelAppId) -> Option<&KernelAppDescriptor> {
        self.apps.get(&id.0)
    }

    /// Mutable look-up (e.g., to update `last_launched`).
    pub fn get_mut(&mut self, id: KernelAppId) -> Option<&mut KernelAppDescriptor> {
        self.apps.get_mut(&id.0)
    }

    /// List all registered apps.
    pub fn list(&self) -> Vec<&KernelAppDescriptor> {
        self.apps.values().collect()
    }

    /// Find an app by exact name.
    pub fn find_by_name(&self, name: &str) -> Option<&KernelAppDescriptor> {
        self.apps.values().find(|d| d.name == name)
    }

    /// Find apps by type.
    pub fn find_by_type(&self, is_match: fn(&KernelAppType) -> bool) -> Vec<&KernelAppDescriptor> {
        self.apps
            .values()
            .filter(|d| is_match(&d.app_type))
            .collect()
    }

    /// Return all WebApp-type descriptors.
    pub fn web_apps(&self) -> Vec<&KernelAppDescriptor> {
        self.find_by_type(|t| matches!(t, KernelAppType::WebApp { .. }))
    }

    /// Number of registered apps.
    pub fn count(&self) -> usize {
        self.apps.len()
    }

    // ── Persistence ─────────────────────────────────────────

    /// Persist the registry to VFS as JSON.
    pub fn save_to_vfs(&self) -> Result<(), AppError> {
        let json = self.serialize_json();
        // Ensure parent directory exists
        let _ = Self::ensure_dir("/system");
        let _ = Self::ensure_dir("/system/apps");
        crate::vfs::write_all(REGISTRY_PATH, json.as_bytes()).map_err(|_| AppError::IoError)?;
        crate::serial_println!("[KPIO/App] Registry saved ({} apps)", self.apps.len());
        Ok(())
    }

    /// Load the registry from VFS.  If the file doesn't exist the
    /// registry starts empty (first boot).
    pub fn load_from_vfs(&mut self) -> Result<(), AppError> {
        match crate::vfs::read_all(REGISTRY_PATH) {
            Ok(data) => {
                if let Ok(json_str) = core::str::from_utf8(&data) {
                    self.deserialize_json(json_str);
                    crate::serial_println!("[KPIO/App] Registry loaded ({} apps)", self.apps.len());
                }
                Ok(())
            }
            Err(crate::vfs::VfsError::NotFound) => {
                // First boot — registry doesn't exist yet.
                crate::serial_println!("[KPIO/App] No registry found, starting fresh");
                Ok(())
            }
            Err(_) => Err(AppError::IoError),
        }
    }

    // ── Internal helpers ────────────────────────────────────

    /// Very small JSON serialiser (no serde in no_std kernel).
    fn serialize_json(&self) -> String {
        use alloc::string::ToString;
        let mut s = String::from("{\"next_id\":");
        s.push_str(&self.next_id.to_string());
        s.push_str(",\"apps\":[");
        let mut first = true;
        for desc in self.apps.values() {
            if !first {
                s.push(',');
            }
            first = false;
            s.push('{');
            s.push_str(&format!("\"id\":{}", desc.id.0));
            s.push_str(&format!(",\"name\":\"{}\"", Self::escape_json(&desc.name)));
            s.push_str(&format!(
                ",\"entry_point\":\"{}\"",
                Self::escape_json(&desc.entry_point)
            ));
            s.push_str(&format!(",\"install_path\":\"{}\"", desc.install_path));
            s.push_str(&format!(",\"installed_at\":{}", desc.installed_at));
            s.push_str(&format!(",\"last_launched\":{}", desc.last_launched));

            // app_type
            match &desc.app_type {
                KernelAppType::WebApp {
                    scope,
                    offline_capable,
                } => {
                    s.push_str(",\"type\":\"web\"");
                    s.push_str(&format!(",\"scope\":\"{}\"", Self::escape_json(scope)));
                    s.push_str(&format!(",\"offline\":{}", offline_capable));
                }
                KernelAppType::WasmApp { wasi_version } => {
                    s.push_str(",\"type\":\"wasm\"");
                    s.push_str(&format!(
                        ",\"wasi_version\":\"{}\"",
                        Self::escape_json(wasi_version)
                    ));
                }
                KernelAppType::NativeApp => {
                    s.push_str(",\"type\":\"native\"");
                }
            }

            // icon_data is NOT serialised (too large); re-read from
            // /apps/data/{id}/icon.png on load.
            s.push('}');
        }
        s.push_str("]}");
        s
    }

    /// Minimal JSON deserialiser — hand-rolled for no_std.
    fn deserialize_json(&mut self, json: &str) {
        // Parse next_id
        if let Some(pos) = json.find("\"next_id\":") {
            let rest = &json[pos + 10..];
            if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                if let Ok(val) = rest[..end].parse::<u64>() {
                    self.next_id = val;
                }
            }
        }

        // Parse apps array — find each { ... } block
        if let Some(arr_start) = json.find("\"apps\":[") {
            let arr = &json[arr_start + 8..];
            let mut depth = 0i32;
            let mut obj_start = None;
            for (i, ch) in arr.char_indices() {
                match ch {
                    '{' => {
                        if depth == 0 {
                            obj_start = Some(i);
                        }
                        depth += 1;
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            if let Some(start) = obj_start {
                                let obj_str = &arr[start..=i];
                                self.parse_app_object(obj_str);
                            }
                            obj_start = None;
                        }
                    }
                    ']' if depth == 0 => break,
                    _ => {}
                }
            }
        }
    }

    /// Parse a single `{ ... }` JSON object into a `KernelAppDescriptor`.
    fn parse_app_object(&mut self, obj: &str) {
        let get_str = |key: &str| -> Option<String> {
            let needle = format!("\"{}\":\"", key);
            if let Some(pos) = obj.find(&needle) {
                let rest = &obj[pos + needle.len()..];
                if let Some(end) = rest.find('"') {
                    return Some(String::from(&rest[..end]));
                }
            }
            None
        };
        let get_u64 = |key: &str| -> u64 {
            let needle = format!("\"{}\":", key);
            if let Some(pos) = obj.find(&needle) {
                let rest = &obj[pos + needle.len()..];
                let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                num_str.parse::<u64>().unwrap_or(0)
            } else {
                0
            }
        };
        let get_bool = |key: &str| -> bool {
            let needle = format!("\"{}\":", key);
            if let Some(pos) = obj.find(&needle) {
                let rest = &obj[pos + needle.len()..];
                rest.trim_start().starts_with("true")
            } else {
                false
            }
        };

        let id = get_u64("id");
        let name = match get_str("name") {
            Some(n) => n,
            None => return, // malformed
        };
        let entry_point = get_str("entry_point").unwrap_or_default();
        let install_path = get_str("install_path").unwrap_or_else(|| format!("/apps/data/{}/", id));
        let installed_at = get_u64("installed_at");
        let last_launched = get_u64("last_launched");
        let type_str = get_str("type").unwrap_or_default();

        let app_type = match type_str.as_str() {
            "web" => KernelAppType::WebApp {
                scope: get_str("scope").unwrap_or_default(),
                offline_capable: get_bool("offline"),
            },
            "wasm" => KernelAppType::WasmApp {
                wasi_version: get_str("wasi_version").unwrap_or_else(|| String::from("preview2")),
            },
            _ => KernelAppType::NativeApp,
        };

        // Try to load icon from VFS
        let icon_path = format!("/apps/data/{}/icon.png", id);
        let icon_data = crate::vfs::read_all(&icon_path).ok();

        let descriptor = KernelAppDescriptor {
            id: KernelAppId(id),
            app_type,
            name,
            icon_data,
            install_path,
            entry_point,
            installed_at,
            last_launched,
        };

        self.apps.insert(id, descriptor);
    }

    /// Minimal JSON string escaping.
    fn escape_json(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(c),
            }
        }
        out
    }

    fn ensure_dir(path: &str) {
        use crate::terminal::fs;
        let _ = fs::with_fs(|f| {
            if f.resolve(path).is_none() {
                if let Some(parent_ino) = f.resolve("/") {
                    let dir_name = path.trim_start_matches('/');
                    // Try to create — ignore errors if already exists
                    let _ = f.mkdir(parent_ino, dir_name);
                }
            }
        });
    }

    /// Current kernel "time" — uses a simple counter for now.
    fn now() -> u64 {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
    }
}

// ── Global instance ─────────────────────────────────────────

/// Global app registry protected by a spin mutex.
pub static APP_REGISTRY: Mutex<AppRegistry> = Mutex::new(AppRegistry::new());

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_web_type() -> KernelAppType {
        KernelAppType::WebApp {
            scope: String::from("https://notes.kpio/"),
            offline_capable: true,
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = AppRegistry::new();
        let id = reg
            .register(
                make_web_type(),
                String::from("KPIO Notes"),
                String::from("https://notes.kpio/"),
                None,
            )
            .unwrap();

        let desc = reg.get(id).expect("should exist");
        assert_eq!(desc.name, "KPIO Notes");
        assert_eq!(desc.id, id);
    }

    #[test]
    fn test_register_duplicate_name_rejected() {
        let mut reg = AppRegistry::new();
        reg.register(
            make_web_type(),
            String::from("Notes"),
            String::from("/notes"),
            None,
        )
        .unwrap();

        let result = reg.register(
            KernelAppType::NativeApp,
            String::from("Notes"),
            String::from("/notes2"),
            None,
        );
        assert!(matches!(result, Err(AppError::AlreadyRegistered)));
    }

    #[test]
    fn test_register_duplicate_scope_rejected() {
        let mut reg = AppRegistry::new();
        reg.register(
            KernelAppType::WebApp {
                scope: String::from("https://app.com/"),
                offline_capable: false,
            },
            String::from("App1"),
            String::from("https://app.com/"),
            None,
        )
        .unwrap();

        let result = reg.register(
            KernelAppType::WebApp {
                scope: String::from("https://app.com/"),
                offline_capable: false,
            },
            String::from("App2"),
            String::from("https://app.com/index.html"),
            None,
        );
        assert!(matches!(result, Err(AppError::AlreadyRegistered)));
    }

    #[test]
    fn test_unregister() {
        let mut reg = AppRegistry::new();
        let id = reg
            .register(make_web_type(), String::from("X"), String::from("/x"), None)
            .unwrap();

        let removed = reg.unregister(id).unwrap();
        assert_eq!(removed.name, "X");
        assert!(reg.get(id).is_none());
    }

    #[test]
    fn test_unregister_not_found() {
        let mut reg = AppRegistry::new();
        assert!(matches!(
            reg.unregister(KernelAppId(999)),
            Err(AppError::NotFound)
        ));
    }

    #[test]
    fn test_list_and_count() {
        let mut reg = AppRegistry::new();
        assert_eq!(reg.count(), 0);

        reg.register(
            KernelAppType::NativeApp,
            String::from("A"),
            String::from("/a"),
            None,
        )
        .unwrap();
        reg.register(
            KernelAppType::NativeApp,
            String::from("B"),
            String::from("/b"),
            None,
        )
        .unwrap();

        assert_eq!(reg.count(), 2);
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn test_find_by_name() {
        let mut reg = AppRegistry::new();
        reg.register(
            KernelAppType::NativeApp,
            String::from("Calc"),
            String::from("/calc"),
            None,
        )
        .unwrap();

        assert!(reg.find_by_name("Calc").is_some());
        assert!(reg.find_by_name("calc").is_none()); // case-sensitive
        assert!(reg.find_by_name("NotExist").is_none());
    }

    #[test]
    fn test_web_apps_filter() {
        let mut reg = AppRegistry::new();
        reg.register(
            KernelAppType::WebApp {
                scope: String::from("https://a.com/"),
                offline_capable: false,
            },
            String::from("WebA"),
            String::from("https://a.com/"),
            None,
        )
        .unwrap();
        reg.register(
            KernelAppType::NativeApp,
            String::from("Native1"),
            String::from("/n1"),
            None,
        )
        .unwrap();

        let web = reg.web_apps();
        assert_eq!(web.len(), 1);
        assert_eq!(web[0].name, "WebA");
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let mut reg = AppRegistry::new();
        reg.register(
            KernelAppType::WebApp {
                scope: String::from("https://notes.kpio/"),
                offline_capable: true,
            },
            String::from("Notes"),
            String::from("https://notes.kpio/index.html"),
            None,
        )
        .unwrap();
        reg.register(
            KernelAppType::WasmApp {
                wasi_version: String::from("preview2"),
            },
            String::from("Calc"),
            String::from("/apps/calc/app.wasm"),
            None,
        )
        .unwrap();

        let json = reg.serialize_json();

        // Deserialize into a fresh registry
        let mut reg2 = AppRegistry::new();
        reg2.deserialize_json(&json);

        assert_eq!(reg2.count(), 2);
        assert!(reg2.find_by_name("Notes").is_some());
        assert!(reg2.find_by_name("Calc").is_some());

        let notes = reg2.find_by_name("Notes").unwrap();
        assert!(matches!(
            notes.app_type,
            KernelAppType::WebApp {
                offline_capable: true,
                ..
            }
        ));
    }

    #[test]
    fn test_auto_increment_ids() {
        let mut reg = AppRegistry::new();
        let id1 = reg
            .register(
                KernelAppType::NativeApp,
                String::from("A"),
                String::from("/a"),
                None,
            )
            .unwrap();
        let id2 = reg
            .register(
                KernelAppType::NativeApp,
                String::from("B"),
                String::from("/b"),
                None,
            )
            .unwrap();

        assert_eq!(id1.0 + 1, id2.0);
    }

    #[test]
    fn test_install_path_derived() {
        let mut reg = AppRegistry::new();
        let id = reg
            .register(
                KernelAppType::NativeApp,
                String::from("X"),
                String::from("/x"),
                None,
            )
            .unwrap();

        let desc = reg.get(id).unwrap();
        assert_eq!(desc.install_path, format!("/apps/data/{}/", id.0));
    }
}
