//! KPIO Developer Tools
//!
//! This crate provides browser developer tools similar to Chrome DevTools.
//! It implements the Chrome DevTools Protocol (CDP) for tool integration.
//!
//! # Modules
//!
//! - `inspector`: DOM tree viewer and CSS inspector
//! - `console`: JavaScript console with REPL
//! - `network`: Network request monitoring
//! - `profiler`: Performance profiling
//! - `debugger`: JavaScript debugger
//! - `protocol`: CDP protocol implementation

#![no_std]

extern crate alloc;

pub mod console;
pub mod debugger;
pub mod inspector;
pub mod network;
pub mod profiler;
pub mod protocol;

use alloc::string::String;
use alloc::vec::Vec;

/// DevTools session ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// DevTools target ID (usually a tab or frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetId(pub u64);

/// DevTools agent that connects tools to a browser tab.
pub struct DevToolsAgent {
    /// Session ID.
    session_id: SessionId,
    /// Target ID.
    target_id: TargetId,
    /// Enabled domains.
    enabled_domains: Vec<String>,
}

impl DevToolsAgent {
    /// Create a new DevTools agent.
    pub fn new(session_id: SessionId, target_id: TargetId) -> Self {
        Self {
            session_id,
            target_id,
            enabled_domains: Vec::new(),
        }
    }

    /// Get session ID.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Get target ID.
    pub fn target_id(&self) -> TargetId {
        self.target_id
    }

    /// Enable a domain.
    pub fn enable_domain(&mut self, domain: &str) {
        if !self.enabled_domains.iter().any(|d| d == domain) {
            self.enabled_domains.push(domain.into());
        }
    }

    /// Disable a domain.
    pub fn disable_domain(&mut self, domain: &str) {
        self.enabled_domains.retain(|d| d != domain);
    }

    /// Check if a domain is enabled.
    pub fn is_domain_enabled(&self, domain: &str) -> bool {
        self.enabled_domains.iter().any(|d| d == domain)
    }
}
