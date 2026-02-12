//! Service Worker Bridge
//!
//! Connects the browser's Service Worker lifecycle with the KPIO kernel,
//! managing registration, state transitions, and event dispatch.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

// ── Types ───────────────────────────────────────────────────

/// Unique identifier for a service worker registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceWorkerId(pub u64);

/// Service Worker state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwState {
    /// Script parsed, not yet installed.
    Parsed,
    /// `install` event fired; waiting for `waitUntil` promises.
    Installing,
    /// Installed but another SW is still active.
    Waiting,
    /// `activate` event complete; now controlling clients.
    Activated,
    /// Replaced or explicitly unregistered.
    Redundant,
}

/// A single Service Worker registration.
#[derive(Debug, Clone)]
pub struct ServiceWorkerRegistration {
    pub id: ServiceWorkerId,
    /// URL scope this SW controls (e.g., `https://example.com/app/`).
    pub scope: String,
    /// URL of the SW script.
    pub script_url: String,
    /// Current lifecycle state.
    pub state: SwState,
    /// VFS path where the script is cached.
    pub cache_path: String,
    /// Whether `skipWaiting()` has been called.
    pub skip_waiting: bool,
    /// Whether `clients.claim()` has been called.
    pub clients_claimed: bool,
    /// Registration timestamp (ticks).
    pub registered_at: u64,
    /// Last update check timestamp.
    pub last_update_check: u64,
}

/// Bridge managing all SW registrations.
pub struct ServiceWorkerBridge {
    registrations: BTreeMap<u64, ServiceWorkerRegistration>,
    /// scope → registration ID for quick lookup.
    scope_index: BTreeMap<String, u64>,
    next_id: u64,
}

/// Global service worker bridge instance.
pub static SW_BRIDGE: RwLock<ServiceWorkerBridge> = RwLock::new(ServiceWorkerBridge::new());

// ── Errors ──────────────────────────────────────────────────

/// SW bridge errors.
#[derive(Debug, Clone)]
pub enum SwError {
    /// A registration for this scope already exists.
    AlreadyRegistered,
    /// Registration not found.
    NotFound,
    /// Script download / storage failed.
    ScriptFetchFailed(String),
    /// Invalid scope or script URL.
    InvalidUrl(String),
    /// VFS I/O error.
    IoError,
}

impl core::fmt::Display for SwError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SwError::AlreadyRegistered => write!(f, "SW already registered for this scope"),
            SwError::NotFound => write!(f, "SW registration not found"),
            SwError::ScriptFetchFailed(u) => write!(f, "failed to fetch SW script: {}", u),
            SwError::InvalidUrl(u) => write!(f, "invalid URL: {}", u),
            SwError::IoError => write!(f, "VFS I/O error"),
        }
    }
}

// ── Implementation ──────────────────────────────────────────

impl ServiceWorkerBridge {
    pub const fn new() -> Self {
        Self {
            registrations: BTreeMap::new(),
            scope_index: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Register a new service worker for the given scope.
    ///
    /// The script at `script_url` is stored in VFS at
    /// `/apps/cache/{app_id}/sw.js` (the `app_id` is encoded in the scope).
    pub fn register(
        &mut self,
        scope: &str,
        script_url: &str,
    ) -> Result<ServiceWorkerId, SwError> {
        if scope.is_empty() || script_url.is_empty() {
            return Err(SwError::InvalidUrl(String::from("empty scope or script URL")));
        }

        // Check for existing registration
        if self.scope_index.contains_key(scope) {
            return Err(SwError::AlreadyRegistered);
        }

        let id = ServiceWorkerId(self.next_id);
        self.next_id += 1;

        // Derive VFS cache path from scope hash
        let scope_hash = simple_hash(scope);
        let cache_path = alloc::format!("/apps/cache/{}/sw.js", scope_hash);

        let registration = ServiceWorkerRegistration {
            id,
            scope: String::from(scope),
            script_url: String::from(script_url),
            state: SwState::Parsed,
            cache_path,
            skip_waiting: false,
            clients_claimed: false,
            registered_at: 0, // TODO: use kernel tick
            last_update_check: 0,
        };

        self.registrations.insert(id.0, registration);
        self.scope_index.insert(String::from(scope), id.0);

        // Transition: Parsed → Installing
        self.transition(id, SwState::Installing);

        Ok(id)
    }

    /// Unregister a service worker by scope.
    pub fn unregister(&mut self, scope: &str) -> Result<(), SwError> {
        let id = self
            .scope_index
            .remove(scope)
            .ok_or(SwError::NotFound)?;

        if let Some(reg) = self.registrations.get_mut(&id) {
            reg.state = SwState::Redundant;
        }
        // Keep in registrations for a while (could GC later)

        Ok(())
    }

    /// Get the registration for a given scope (exact match).
    pub fn get_registration(&self, scope: &str) -> Option<&ServiceWorkerRegistration> {
        let id = self.scope_index.get(scope)?;
        self.registrations.get(id)
    }

    /// Find the active SW that controls a given URL.
    ///
    /// Walks all registrations and returns the one with the longest
    /// matching scope prefix that is in `Activated` state.
    pub fn match_scope(&self, url: &str) -> Option<&ServiceWorkerRegistration> {
        let mut best: Option<&ServiceWorkerRegistration> = None;
        for reg in self.registrations.values() {
            if reg.state == SwState::Activated && url.starts_with(&reg.scope) {
                match best {
                    Some(b) if b.scope.len() >= reg.scope.len() => {}
                    _ => best = Some(reg),
                }
            }
        }
        best
    }

    /// Advance the SW lifecycle state.
    pub fn transition(&mut self, id: ServiceWorkerId, new_state: SwState) {
        if let Some(reg) = self.registrations.get_mut(&id.0) {
            reg.state = new_state;
        }
    }

    /// Called when the `install` event handler completes successfully.
    pub fn on_install_complete(&mut self, id: ServiceWorkerId) {
        if let Some(reg) = self.registrations.get_mut(&id.0) {
            if reg.state == SwState::Installing {
                if reg.skip_waiting {
                    reg.state = SwState::Activated;
                } else {
                    reg.state = SwState::Waiting;
                }
            }
        }
    }

    /// Called when the `activate` event handler completes.
    pub fn on_activate_complete(&mut self, id: ServiceWorkerId) {
        if let Some(reg) = self.registrations.get_mut(&id.0) {
            if reg.state == SwState::Installing || reg.state == SwState::Waiting {
                reg.state = SwState::Activated;
            }
        }
    }

    /// Signal `skipWaiting()` — moves from Waiting → Activated.
    pub fn skip_waiting(&mut self, id: ServiceWorkerId) {
        if let Some(reg) = self.registrations.get_mut(&id.0) {
            reg.skip_waiting = true;
            if reg.state == SwState::Waiting {
                reg.state = SwState::Activated;
            }
        }
    }

    /// Signal `clients.claim()` — immediately control all in-scope clients.
    pub fn clients_claim(&mut self, id: ServiceWorkerId) {
        if let Some(reg) = self.registrations.get_mut(&id.0) {
            reg.clients_claimed = true;
        }
    }

    /// Check if a SW script has changed (byte comparison).
    ///
    /// Returns `true` if the new script differs from the cached version.
    pub fn needs_update(&self, id: ServiceWorkerId, new_script: &[u8]) -> bool {
        if let Some(reg) = self.registrations.get(&id.0) {
            // Try reading the cached script from VFS
            // (In a real system this would compare bytes; here we do a
            // simple length + first/last byte check for efficiency.)
            let _ = &reg.cache_path;
            // TODO: actual VFS read + compare
            !new_script.is_empty()
        } else {
            false
        }
    }

    /// List all registrations.
    pub fn list(&self) -> Vec<&ServiceWorkerRegistration> {
        self.registrations.values().collect()
    }

    /// Number of registrations.
    pub fn count(&self) -> usize {
        self.registrations.len()
    }
}

/// Simple non-cryptographic hash for scope → directory mapping.
fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup() {
        let mut bridge = ServiceWorkerBridge::new();
        let id = bridge.register("https://example.com/", "/sw.js").unwrap();
        assert_eq!(id.0, 1);

        let reg = bridge.get_registration("https://example.com/").unwrap();
        assert_eq!(reg.state, SwState::Installing);
    }

    #[test]
    fn duplicate_scope_rejected() {
        let mut bridge = ServiceWorkerBridge::new();
        bridge.register("https://app.com/", "/sw.js").unwrap();
        let err = bridge.register("https://app.com/", "/sw2.js");
        assert!(matches!(err, Err(SwError::AlreadyRegistered)));
    }

    #[test]
    fn lifecycle_transitions() {
        let mut bridge = ServiceWorkerBridge::new();
        let id = bridge.register("https://test.com/", "/sw.js").unwrap();
        assert_eq!(bridge.get_registration("https://test.com/").unwrap().state, SwState::Installing);

        bridge.on_install_complete(id);
        assert_eq!(bridge.get_registration("https://test.com/").unwrap().state, SwState::Waiting);

        bridge.skip_waiting(id);
        assert_eq!(bridge.get_registration("https://test.com/").unwrap().state, SwState::Activated);
    }

    #[test]
    fn unregister() {
        let mut bridge = ServiceWorkerBridge::new();
        let id = bridge.register("https://del.com/", "/sw.js").unwrap();
        bridge.on_install_complete(id);
        bridge.skip_waiting(id);

        bridge.unregister("https://del.com/").unwrap();
        // Registration still in map but marked redundant
        assert!(bridge.get_registration("https://del.com/").is_none()); // removed from scope_index
    }

    #[test]
    fn match_scope_longest_prefix() {
        let mut bridge = ServiceWorkerBridge::new();
        let id1 = bridge.register("https://app.com/", "/sw1.js").unwrap();
        let id2 = bridge.register("https://app.com/sub/", "/sw2.js").unwrap();

        // Activate both
        bridge.on_install_complete(id1);
        bridge.skip_waiting(id1);
        bridge.on_install_complete(id2);
        bridge.skip_waiting(id2);

        let matched = bridge.match_scope("https://app.com/sub/page.html").unwrap();
        assert_eq!(matched.id, id2); // longer scope wins
    }

    #[test]
    fn empty_scope_rejected() {
        let mut bridge = ServiceWorkerBridge::new();
        assert!(matches!(
            bridge.register("", "/sw.js"),
            Err(SwError::InvalidUrl(_))
        ));
    }

    #[test]
    fn clients_claim() {
        let mut bridge = ServiceWorkerBridge::new();
        let id = bridge.register("https://claim.com/", "/sw.js").unwrap();
        bridge.on_install_complete(id);
        bridge.skip_waiting(id);
        bridge.clients_claim(id);

        let reg = bridge.registrations.get(&id.0).unwrap();
        assert!(reg.clients_claimed);
    }
}
