//! Shared configuration for server modules

use anyhow::Result;
use chai_core::Config;
use std::sync::OnceLock;

/// Cached config to avoid re-parsing environment on every request
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Get or initialize cached config
pub fn get() -> Result<&'static Config> {
    if let Some(config) = CONFIG.get() {
        return Ok(config);
    }

    let config = Config::from_env()?;
    // Ignore error if another thread initialized it first
    let _ = CONFIG.set(config);
    CONFIG
        .get()
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize config"))
}
