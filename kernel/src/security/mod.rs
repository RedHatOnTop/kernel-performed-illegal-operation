//! Security Module
//!
//! This module provides security policies, sandboxing, and resource
//! management for browser processes.

pub mod audit;
pub mod hardening;
pub mod policy;
pub mod resource;
pub mod sandbox;

pub use audit::{AuditEvent, AuditLog};
pub use hardening::{HardeningConfig, HardeningStatus};
pub use policy::{PolicyError, SecurityPolicy};
pub use resource::{ResourceLimits, ResourceManager, ResourceUsage};
pub use sandbox::{Sandbox, SandboxConfig, SandboxError};
