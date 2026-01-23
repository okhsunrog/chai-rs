//! SQLite database connection pool management
//!
//! This module provides a shared SQLite connection pool used by:
//! - Authentication (users table)
//! - HTML cache (cache table)

use anyhow::{Context, Result};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::sync::OnceLock;

/// Global database pool
static DB_POOL: OnceLock<SqlitePool> = OnceLock::new();

/// Database configuration
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Path to SQLite database file
    pub path: String,
}

impl DbConfig {
    /// Load config from environment variables
    pub fn from_env() -> Self {
        let path =
            std::env::var("SQLITE_DATABASE_PATH").unwrap_or_else(|_| "data/chai.db".to_string());

        Self { path }
    }
}

/// Initialize the database pool and create all tables
pub async fn init_database(config: &DbConfig) -> Result<()> {
    // Ensure directory exists
    if let Some(parent) = std::path::Path::new(&config.path).parent() {
        std::fs::create_dir_all(parent).context("Failed to create database directory")?;
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&format!("sqlite:{}?mode=rwc", config.path))
        .await
        .context("Failed to connect to SQLite database")?;

    // Create users table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .context("Failed to create users table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)")
        .execute(&pool)
        .await
        .context("Failed to create email index")?;

    // Create HTML cache table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS html_cache (
            url TEXT PRIMARY KEY,
            html TEXT NOT NULL,
            fetched_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .context("Failed to create html_cache table")?;

    DB_POOL
        .set(pool)
        .map_err(|_| anyhow::anyhow!("Database pool already initialized"))?;

    tracing::info!("SQLite database initialized at {}", config.path);
    Ok(())
}

/// Get the database pool (must call init_database first)
pub fn get_pool() -> Result<&'static SqlitePool> {
    DB_POOL
        .get()
        .ok_or_else(|| anyhow::anyhow!("Database not initialized. Call init_database first."))
}

/// Check if database is initialized
pub fn is_initialized() -> bool {
    DB_POOL.get().is_some()
}
