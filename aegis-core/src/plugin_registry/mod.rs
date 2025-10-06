//! Plugin registry implementation with database backend
//!
//! This module provides a comprehensive plugin registry system with:
//! - Plugin metadata management
//! - Version control and dependencies
//! - Database storage
//! - Security scanning and compliance
//! - Distribution and packaging

pub mod models;
pub mod manifest;
pub mod operations;
pub mod dependency;

#[cfg(feature = "api")]
pub mod api;

#[cfg(feature = "registry-api")]
pub mod database;

// Re-export commonly used types
pub use models::*;
pub use manifest::*;
pub use operations::*;