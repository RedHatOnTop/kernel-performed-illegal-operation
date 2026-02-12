//! PWA ↔ Kernel Bridge — Kernel Side
//!
//! Provides public static functions that can be registered as
//! callbacks with the browser crate's `kernel_bridge` module.
//!
//! Since the kernel cannot depend on `kpio-browser` (circular dep),
//! these functions are exported with compatible signatures so that
//! the top-level integration code (or the app `init()`) can wire
//! them up by passing function pointers to the browser bridge.

use alloc::string::String;
use alloc::vec::Vec;

use crate::app::registry::{KernelAppId, KernelAppType, APP_REGISTRY};
use crate::app::lifecycle::{AppInstanceId, APP_LIFECYCLE};
use crate::vfs;
use crate::vfs::sandbox;

// ─── Public bridge functions ───────────────────────────────────

/// Install: register a WebApp in the kernel registry.
pub fn bridge_install(
    _app_type: u64,
    name: &str,
    entry_point: &str,
    scope: &str,
) -> Result<u64, &'static str> {
    let app_type = KernelAppType::WebApp {
        scope: String::from(scope),
        offline_capable: false,
    };

    let mut registry = APP_REGISTRY.lock();
    match registry.register(
        app_type,
        String::from(name),
        String::from(entry_point),
        None,
    ) {
        Ok(id) => {
            sandbox::create_app_directory(id);
            let _ = registry.save_to_vfs();
            Ok(id.0)
        }
        Err(_) => Err("registration failed"),
    }
}

/// Uninstall: remove from registry and clean up sandbox.
pub fn bridge_uninstall(app_id: u64) -> Result<(), &'static str> {
    let kid = KernelAppId(app_id);

    // Terminate running instances
    {
        let lifecycle = APP_LIFECYCLE.lock();
        let instances = lifecycle.instances_of(kid);
        drop(lifecycle);
        for info in instances {
            let mut lc = APP_LIFECYCLE.lock();
            let _ = lc.terminate(info.instance_id);
        }
    }

    let mut registry = APP_REGISTRY.lock();
    match registry.unregister(kid) {
        Ok(_) => {
            sandbox::remove_app_directory(kid);
            let _ = registry.save_to_vfs();
            Ok(())
        }
        Err(_) => Err("app not found"),
    }
}

/// Launch: create a new instance in the lifecycle manager.
pub fn bridge_launch(app_id: u64) -> Result<u64, &'static str> {
    let kid = KernelAppId(app_id);

    // Verify existence
    {
        let registry = APP_REGISTRY.lock();
        if registry.get(kid).is_none() {
            return Err("app not found");
        }
    }

    let mut lifecycle = APP_LIFECYCLE.lock();
    match lifecycle.launch(kid) {
        Ok(instance_id) => Ok(instance_id.0),
        Err(_) => Err("launch failed"),
    }
}

/// Terminate a running instance.
pub fn bridge_terminate(instance_id: u64) -> Result<(), &'static str> {
    let mut lifecycle = APP_LIFECYCLE.lock();
    match lifecycle.terminate(AppInstanceId(instance_id)) {
        Ok(()) => Ok(()),
        Err(_) => Err("instance not found"),
    }
}

/// Save manifest JSON bytes to the app's sandbox directory.
pub fn bridge_save_manifest(app_id: u64, data: &[u8]) -> Result<(), &'static str> {
    let path = alloc::format!("/apps/data/{}/manifest.json", app_id);
    vfs::write_all(&path, data).map_err(|_| "write failed")
}

/// Load manifest JSON bytes from the app's sandbox directory.
pub fn bridge_load_manifest(app_id: u64) -> Result<Vec<u8>, &'static str> {
    let path = alloc::format!("/apps/data/{}/manifest.json", app_id);
    vfs::read_all(&path).map_err(|_| "read failed")
}

/// List all WebApp-type registered apps.
pub fn bridge_list_apps() -> Vec<(u64, String)> {
    let registry = APP_REGISTRY.lock();
    registry.web_apps()
        .iter()
        .map(|desc| (desc.id.0, desc.name.clone()))
        .collect()
}
