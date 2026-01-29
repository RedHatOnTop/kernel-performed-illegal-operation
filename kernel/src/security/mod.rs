//! Security Module
//!
//! This module provides security policies, sandboxing, and resource
//! management for browser processes.

pub mod policy;
pub mod sandbox;
pub mod resource;
pub mod audit;
pub mod hardening;

pub use policy::{SecurityPolicy, PolicyError};
pub use sandbox::{Sandbox, SandboxConfig, SandboxError};
pub use resource::{ResourceLimits, ResourceUsage, ResourceManager};
pub use audit::{AuditEvent, AuditLog};
pub use hardening::{HardeningConfig, HardeningStatus};
