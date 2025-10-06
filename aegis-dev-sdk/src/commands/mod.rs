//! Command handlers for the Aegis Development SDK

use anyhow::Result;
use std::path::PathBuf;

pub mod new_project;
pub mod dev_server;
pub mod testing;
pub mod docs;
pub mod signing;
pub mod marketplace;
pub mod config;
pub mod profile;
pub mod validate;

pub use new_project::*;
pub use dev_server::*;
pub use testing::*;
pub use docs::*;
pub use signing::*;
pub use marketplace::*;
pub use config::*;
pub use profile::*;
pub use validate::*;

use crate::{TestCommands, DocsCommands, SignCommands, MarketCommands, ConfigCommands};

/// Handle new project creation
pub async fn handle_new_project(
    name: String,
    template: String,
    engine: Option<String>,
    no_interactive: bool,
    output: Option<PathBuf>,
) -> Result<()> {
    new_project::create_new_project(name, template, engine, no_interactive, output).await
}

/// Handle development server
pub async fn handle_dev_server(
    project: Option<PathBuf>,
    port: u16,
    watch: Vec<PathBuf>,
    test_pattern: Option<String>,
) -> Result<()> {
    dev_server::start_dev_server(project, port, watch, test_pattern).await
}

/// Handle test commands
pub async fn handle_test_commands(command: TestCommands) -> Result<()> {
    testing::handle_test_command(command).await
}

/// Handle documentation commands
pub async fn handle_docs_commands(command: DocsCommands) -> Result<()> {
    docs::handle_docs_command(command).await
}

/// Handle signing commands
pub async fn handle_sign_commands(command: SignCommands) -> Result<()> {
    signing::handle_sign_command(command).await
}

/// Handle marketplace commands
pub async fn handle_market_commands(command: MarketCommands) -> Result<()> {
    marketplace::handle_market_command(command).await
}

/// Handle configuration commands
pub async fn handle_config_commands(command: ConfigCommands) -> Result<()> {
    config::handle_config_command(command).await
}

/// Handle performance profiling
pub async fn handle_profile(
    test_files: Vec<PathBuf>,
    format: String,
    iterations: usize,
) -> Result<()> {
    profile::run_performance_profile(test_files, format, iterations).await
}

/// Handle plugin validation
pub async fn handle_validate(
    plugin: PathBuf,
    profile: String,
    report: bool,
    fix: bool,
) -> Result<()> {
    validate::validate_plugin(plugin, profile, report, fix).await
}