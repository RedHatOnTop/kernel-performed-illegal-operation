//! Network interface management.

use alloc::string::String;
use alloc::vec::Vec;
use spin::RwLock;

use crate::{InterfaceConfig, Ipv4Addr, MacAddr, NetworkError};

/// Global interface list.
static INTERFACES: RwLock<Vec<InterfaceConfig>> = RwLock::new(Vec::new());

/// Initialize interfaces.
pub fn init() -> Result<(), NetworkError> {
    Ok(())
}

/// List all interfaces.
pub fn list_interfaces() -> Vec<InterfaceConfig> {
    INTERFACES.read().clone()
}

/// Add an interface.
pub fn add_interface(config: InterfaceConfig) {
    INTERFACES.write().push(config);
}

/// Get interface by name.
pub fn get_interface(name: &str) -> Option<InterfaceConfig> {
    INTERFACES.read().iter().find(|i| i.name == name).cloned()
}

/// Configure an interface.
pub fn configure_interface(
    name: &str,
    ip: Ipv4Addr,
    netmask: Ipv4Addr,
    gateway: Option<Ipv4Addr>,
) -> Result<(), NetworkError> {
    let mut interfaces = INTERFACES.write();
    if let Some(iface) = interfaces.iter_mut().find(|i| i.name == name) {
        iface.ipv4 = Some(ip);
        iface.netmask = Some(netmask);
        iface.gateway = gateway;
        Ok(())
    } else {
        Err(NetworkError::InterfaceNotFound(name.into()))
    }
}
