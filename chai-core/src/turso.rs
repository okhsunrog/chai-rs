//! Unified database module using Turso (embedded SQLite with vector support)
//!
//! This module provides:
//! - Database connection management
//! - User authentication storage
//! - HTML cache storage
//! - Tea storage with vector embeddings for semantic search

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::info;
use turso::{Builder, Connection, Database};

use crate::models::{SearchResult, Tea, generate_point_id};

/// Global database instance
static DATABASE: OnceCell<Arc<Database>> = OnceCell::const_new();

/// Default vector size for embeddings
pub const DEFAULT_VECTOR_SIZE: usize = 4096;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Path to database file
    pub path: String,
    /// Vector size for embeddings
    pub vector_size: usize,
}

impl DbConfig {
    /// Load config from environment variables
    ///
    /// Environment variables:
    /// - `DATABASE_PATH`: Path to the database file (default: "data/chai.db")
    /// - `SQLITE_DATABASE_PATH`: Legacy alias for DATABASE_PATH (for backward compatibility)
    /// - `VECTOR_SIZE`: Embedding vector dimension (default: 4096)
    pub fn from_env() -> Self {
        // Support both DATABASE_PATH and legacy SQLITE_DATABASE_PATH for backward compatibility
        let path = std::env::var("DATABASE_PATH")
            .or_else(|_| std::env::var("SQLITE_DATABASE_PATH"))
            .unwrap_or_else(|_| "data/chai.db".to_string());
        let vector_size = std::env::var("VECTOR_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_VECTOR_SIZE);

        Self { path, vector_size }
    }
}

/// Initialize the database and create all tables
pub async fn init_database(config: &DbConfig) -> Result<()> {
    // Ensure directory exists
    if let Some(parent) = std::path::Path::new(&config.path).parent() {
        std::fs::create_dir_all(parent).context("Failed to create database directory")?;
    }

    let db = Builder::new_local(&config.path)
        .build()
        .await
        .context("Failed to open database")?;

    let conn = db.connect().context("Failed to connect to database")?;

    // Create users table
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )
        "#,
        (),
    )
    .await
    .context("Failed to create users table")?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)",
        (),
    )
    .await
    .context("Failed to create email index")?;

    // Create HTML cache table
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS html_cache (
            url TEXT PRIMARY KEY,
            html TEXT NOT NULL,
            fetched_at INTEGER NOT NULL
        )
        "#,
        (),
    )
    .await
    .context("Failed to create html_cache table")?;

    // Create teas table with vector column
    // Note: We store tea data as JSON and embedding as F32_BLOB
    conn.execute(
        &format!(
            r#"
            CREATE TABLE IF NOT EXISTS teas (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL UNIQUE,
                tea_data TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                embedding F32_BLOB({}),
                in_stock INTEGER NOT NULL DEFAULT 0,
                is_sample INTEGER NOT NULL DEFAULT 0,
                is_set INTEGER NOT NULL DEFAULT 0,
                series TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
            config.vector_size
        ),
        (),
    )
    .await
    .context("Failed to create teas table")?;

    // Create indexes for common queries
    conn.execute("CREATE INDEX IF NOT EXISTS idx_teas_url ON teas(url)", ())
        .await
        .context("Failed to create teas url index")?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_teas_in_stock ON teas(in_stock)",
        (),
    )
    .await
    .context("Failed to create teas in_stock index")?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_teas_series ON teas(series)",
        (),
    )
    .await
    .context("Failed to create teas series index")?;

    // Store database in global
    DATABASE
        .set(Arc::new(db))
        .map_err(|_| anyhow::anyhow!("Database already initialized"))?;

    info!("Database initialized at {}", config.path);
    Ok(())
}

/// Get a database connection
pub fn get_connection() -> Result<Connection> {
    let db = DATABASE
        .get()
        .ok_or_else(|| anyhow::anyhow!("Database not initialized. Call init_database first."))?;

    db.connect().context("Failed to get database connection")
}

/// Check if database is initialized
pub fn is_initialized() -> bool {
    DATABASE.get().is_some()
}

// ============================================================================
// User Operations
// ============================================================================

/// User model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    /// Password hash - excluded from serialization for security
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: i64,
}

/// Get user by email
pub async fn get_user_by_email(email: &str) -> Result<Option<User>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query(
            "SELECT id, email, password_hash, created_at FROM users WHERE email = ?",
            [email],
        )
        .await
        .context("Failed to query user")?;

    if let Some(row) = rows.next().await? {
        Ok(Some(User {
            id: row.get::<i64>(0)?,
            email: row.get::<String>(1)?,
            password_hash: row.get::<String>(2)?,
            created_at: row.get::<i64>(3)?,
        }))
    } else {
        Ok(None)
    }
}

/// Get user by ID
pub async fn get_user_by_id(user_id: i64) -> Result<Option<User>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query(
            "SELECT id, email, password_hash, created_at FROM users WHERE id = ?",
            [user_id],
        )
        .await
        .context("Failed to query user")?;

    if let Some(row) = rows.next().await? {
        Ok(Some(User {
            id: row.get::<i64>(0)?,
            email: row.get::<String>(1)?,
            password_hash: row.get::<String>(2)?,
            created_at: row.get::<i64>(3)?,
        }))
    } else {
        Ok(None)
    }
}

/// Create a new user
pub async fn create_user(email: &str, password_hash: &str) -> Result<User> {
    let conn = get_connection()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("System time error")?
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO users (email, password_hash, created_at) VALUES (?, ?, ?)",
        (email, password_hash, now),
    )
    .await
    .context("Failed to create user")?;

    // Get the created user
    get_user_by_email(email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to retrieve created user"))
}

// ============================================================================
// Cache Operations
// ============================================================================

/// Cache entry
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub url: String,
    pub html: String,
    pub fetched_at: i64,
}

/// Get cached HTML for a URL
pub async fn cache_get(url: &str) -> Result<Option<CacheEntry>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query(
            "SELECT url, html, fetched_at FROM html_cache WHERE url = ?",
            [url],
        )
        .await
        .context("Failed to query cache")?;

    if let Some(row) = rows.next().await? {
        Ok(Some(CacheEntry {
            url: row.get::<String>(0)?,
            html: row.get::<String>(1)?,
            fetched_at: row.get::<i64>(2)?,
        }))
    } else {
        Ok(None)
    }
}

/// Store HTML in cache
pub async fn cache_set(url: &str, html: &str) -> Result<()> {
    let conn = get_connection()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("System time error")?
        .as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO html_cache (url, html, fetched_at) VALUES (?, ?, ?)",
        (url, html, now),
    )
    .await
    .context("Failed to store in cache")?;

    Ok(())
}

/// Check if URL is cached
pub async fn cache_contains(url: &str) -> Result<bool> {
    let conn = get_connection()?;

    let mut rows = conn
        .query("SELECT 1 FROM html_cache WHERE url = ? LIMIT 1", [url])
        .await
        .context("Failed to check cache")?;

    Ok(rows.next().await?.is_some())
}

/// Get all cached URLs
pub async fn cache_list_urls() -> Result<Vec<String>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query("SELECT url FROM html_cache", ())
        .await
        .context("Failed to list cache URLs")?;

    let mut urls = Vec::new();
    while let Some(row) = rows.next().await? {
        urls.push(row.get::<String>(0)?);
    }

    Ok(urls)
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_size_bytes: usize,
    pub oldest_entry: Option<i64>,
    pub newest_entry: Option<i64>,
}

/// Get cache statistics
pub async fn cache_stats() -> Result<CacheStats> {
    let conn = get_connection()?;

    // Single query for all stats - more efficient than 4 separate queries
    let mut rows = conn
        .query(
            r#"
            SELECT
                COUNT(*) as count,
                COALESCE(SUM(LENGTH(html)), 0) as total_size,
                MIN(fetched_at) as oldest,
                MAX(fetched_at) as newest
            FROM html_cache
            "#,
            (),
        )
        .await
        .context("Failed to query cache stats")?;

    if let Some(row) = rows.next().await? {
        let count: i64 = row.get(0)?;
        let total_size: i64 = row.get(1)?;
        let oldest: Option<i64> = row.get::<Option<i64>>(2).ok().flatten();
        let newest: Option<i64> = row.get::<Option<i64>>(3).ok().flatten();

        Ok(CacheStats {
            entry_count: count as usize,
            total_size_bytes: total_size as usize,
            oldest_entry: oldest.filter(|&t| t > 0),
            newest_entry: newest.filter(|&t| t > 0),
        })
    } else {
        Ok(CacheStats {
            entry_count: 0,
            total_size_bytes: 0,
            oldest_entry: None,
            newest_entry: None,
        })
    }
}

/// Clear all cache entries
pub async fn cache_clear() -> Result<usize> {
    let conn = get_connection()?;

    let result = conn
        .execute("DELETE FROM html_cache", ())
        .await
        .context("Failed to clear cache")?;

    Ok(result as usize)
}

/// Migrate from JSON cache file
pub async fn cache_migrate_from_json(json_path: &str) -> Result<usize> {
    let content = std::fs::read_to_string(json_path).context("Failed to read JSON cache file")?;
    let cache_map: std::collections::HashMap<String, String> =
        serde_json::from_str(&content).context("Failed to parse JSON cache file")?;

    let count = cache_map.len();
    for (url, html) in cache_map {
        cache_set(&url, &html).await?;
    }

    info!("Migrated {} entries from JSON cache", count);
    Ok(count)
}

// ============================================================================
// Tea Operations (with Vector Search)
// ============================================================================

/// Search filters
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub exclude_samples: bool,
    pub exclude_sets: bool,
    pub only_in_stock: bool,
    /// Filter by tea series (exact match)
    pub series: Option<String>,
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_teas: usize,
    pub in_stock: usize,
    pub out_of_stock: usize,
    pub series_count: usize,
    pub series_list: Vec<String>,
}

/// Upsert a tea (insert or update)
///
/// If embedding is None, the tea is stored without an embedding (can be added later)
pub async fn upsert_tea(tea: &Tea, embedding: Option<Vec<f32>>, content_hash: &str) -> Result<()> {
    let conn = get_connection()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("System time error")?
        .as_secs() as i64;

    let tea_json = serde_json::to_string(tea).context("Failed to serialize tea")?;
    let id = generate_point_id(&tea.url);

    // Format embedding as vector string for turso
    let embedding_str = embedding.map(|emb| {
        let values: Vec<String> = emb.iter().map(|v| v.to_string()).collect();
        format!("[{}]", values.join(","))
    });

    // Handle series - use empty string if None
    let series_str = tea.series.as_deref().unwrap_or("");

    if let Some(ref emb_str) = embedding_str {
        conn.execute(
            r#"
            INSERT INTO teas (id, url, tea_data, content_hash, embedding, in_stock, is_sample, is_set, series, created_at, updated_at)
            VALUES (?, ?, ?, ?, vector32(?), ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                tea_data = excluded.tea_data,
                content_hash = excluded.content_hash,
                embedding = excluded.embedding,
                in_stock = excluded.in_stock,
                is_sample = excluded.is_sample,
                is_set = excluded.is_set,
                series = excluded.series,
                updated_at = excluded.updated_at
            "#,
            (
                id.as_str(),
                tea.url.as_str(),
                tea_json.as_str(),
                content_hash,
                emb_str.as_str(),
                tea.in_stock as i64,
                tea.is_sample as i64,
                tea.is_set as i64,
                series_str,
                now,
                now,
            ),
        )
        .await
        .context("Failed to upsert tea")?;
    } else {
        // Insert without embedding
        conn.execute(
            r#"
            INSERT INTO teas (id, url, tea_data, content_hash, embedding, in_stock, is_sample, is_set, series, created_at, updated_at)
            VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                tea_data = excluded.tea_data,
                content_hash = excluded.content_hash,
                in_stock = excluded.in_stock,
                is_sample = excluded.is_sample,
                is_set = excluded.is_set,
                series = excluded.series,
                updated_at = excluded.updated_at
            "#,
            (
                id.as_str(),
                tea.url.as_str(),
                tea_json.as_str(),
                content_hash,
                tea.in_stock as i64,
                tea.is_sample as i64,
                tea.is_set as i64,
                series_str,
                now,
                now,
            ),
        )
        .await
        .context("Failed to upsert tea without embedding")?;
    }

    Ok(())
}

/// Update embedding for a tea
pub async fn update_tea_embedding(url: &str, embedding: Vec<f32>) -> Result<()> {
    let conn = get_connection()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("System time error")?
        .as_secs() as i64;

    let values: Vec<String> = embedding.iter().map(|v| v.to_string()).collect();
    let embedding_str = format!("[{}]", values.join(","));

    conn.execute(
        "UPDATE teas SET embedding = vector32(?), updated_at = ? WHERE url = ?",
        (embedding_str.as_str(), now, url),
    )
    .await
    .context("Failed to update tea embedding")?;

    Ok(())
}

/// Get tea by URL
pub async fn get_tea_by_url(url: &str) -> Result<Option<Tea>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query("SELECT tea_data FROM teas WHERE url = ?", [url])
        .await
        .context("Failed to query tea by URL")?;

    if let Some(row) = rows.next().await? {
        let tea_json: String = row.get(0)?;
        let tea: Tea = serde_json::from_str(&tea_json).context("Failed to parse tea JSON")?;
        Ok(Some(tea))
    } else {
        Ok(None)
    }
}

/// Get tea by ID
pub async fn get_tea_by_id(id: &str) -> Result<Option<Tea>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query("SELECT tea_data FROM teas WHERE id = ?", [id])
        .await
        .context("Failed to query tea by ID")?;

    if let Some(row) = rows.next().await? {
        let tea_json: String = row.get(0)?;
        let tea: Tea = serde_json::from_str(&tea_json).context("Failed to parse tea JSON")?;
        Ok(Some(tea))
    } else {
        Ok(None)
    }
}

/// Get tea with content hash by URL
pub async fn get_tea_with_hash(url: &str) -> Result<Option<(Tea, String)>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query(
            "SELECT tea_data, content_hash FROM teas WHERE url = ?",
            [url],
        )
        .await
        .context("Failed to query tea")?;

    if let Some(row) = rows.next().await? {
        let tea_json: String = row.get(0)?;
        let content_hash: String = row.get(1)?;
        let tea: Tea = serde_json::from_str(&tea_json).context("Failed to parse tea JSON")?;
        Ok(Some((tea, content_hash)))
    } else {
        Ok(None)
    }
}

/// Delete tea by URL
pub async fn delete_tea_by_url(url: &str) -> Result<bool> {
    let conn = get_connection()?;

    let result = conn
        .execute("DELETE FROM teas WHERE url = ?", [url])
        .await
        .context("Failed to delete tea")?;

    Ok(result > 0)
}

/// Get all tea URLs
pub async fn get_all_tea_urls() -> Result<Vec<String>> {
    let conn = get_connection()?;

    let mut rows = conn
        .query("SELECT url FROM teas", ())
        .await
        .context("Failed to query tea URLs")?;

    let mut urls = Vec::new();
    while let Some(row) = rows.next().await? {
        urls.push(row.get::<String>(0)?);
    }

    Ok(urls)
}

/// Search teas by vector similarity (cosine distance)
pub async fn search_teas(
    query_embedding: &[f32],
    limit: usize,
    filters: &SearchFilters,
) -> Result<Vec<SearchResult>> {
    let conn = get_connection()?;

    // Build filter conditions
    let mut conditions = Vec::new();
    conditions.push("embedding IS NOT NULL".to_string());

    if filters.exclude_samples {
        conditions.push("is_sample = 0".to_string());
    }
    if filters.exclude_sets {
        conditions.push("is_set = 0".to_string());
    }
    if filters.only_in_stock {
        conditions.push("in_stock = 1".to_string());
    }
    if let Some(ref series) = filters.series {
        // Escape single quotes in series name to prevent SQL injection
        let escaped = series.replace('\'', "''");
        conditions.push(format!("series = '{}'", escaped));
    }

    let where_clause = conditions.join(" AND ");

    // Format query embedding
    let values: Vec<String> = query_embedding.iter().map(|v| v.to_string()).collect();
    let query_vec_str = format!("[{}]", values.join(","));

    // Use cosine distance for similarity search
    // Lower distance = more similar, so we order ASC
    // Score = 1 - distance to get similarity score (higher = better)
    let sql = format!(
        r#"
        SELECT
            tea_data,
            1.0 - vector_distance_cos(embedding, vector32(?)) as score
        FROM teas
        WHERE {}
        ORDER BY vector_distance_cos(embedding, vector32(?)) ASC
        LIMIT ?
        "#,
        where_clause
    );

    let mut rows = conn
        .query(
            &sql,
            (query_vec_str.as_str(), query_vec_str.as_str(), limit as i64),
        )
        .await
        .context("Failed to search teas")?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await? {
        let tea_json: String = row.get(0)?;
        let score: f64 = row.get(1)?;

        match serde_json::from_str::<Tea>(&tea_json) {
            Ok(tea) => {
                results.push(SearchResult {
                    tea,
                    score: score as f32,
                });
            }
            Err(e) => {
                tracing::warn!("Failed to parse tea from search result: {}", e);
            }
        }
    }

    Ok(results)
}

/// Get database statistics
pub async fn get_stats() -> Result<DatabaseStats> {
    let conn = get_connection()?;

    // Total count
    let mut rows = conn.query("SELECT COUNT(*) FROM teas", ()).await?;
    let total: i64 = rows
        .next()
        .await?
        .map(|r| r.get(0))
        .transpose()?
        .unwrap_or(0);

    // In stock count
    let mut rows = conn
        .query("SELECT COUNT(*) FROM teas WHERE in_stock = 1", ())
        .await?;
    let in_stock: i64 = rows
        .next()
        .await?
        .map(|r| r.get(0))
        .transpose()?
        .unwrap_or(0);

    // Get unique series
    let mut rows = conn
        .query(
            "SELECT DISTINCT series FROM teas WHERE series IS NOT NULL AND series != ''",
            (),
        )
        .await?;

    let mut series_list = Vec::new();
    while let Some(row) = rows.next().await? {
        if let Ok(series) = row.get::<String>(0) {
            series_list.push(series);
        }
    }
    series_list.sort();

    Ok(DatabaseStats {
        total_teas: total as usize,
        in_stock: in_stock as usize,
        out_of_stock: (total - in_stock) as usize,
        series_count: series_list.len(),
        series_list,
    })
}

/// Count total teas
pub async fn count_teas() -> Result<usize> {
    let conn = get_connection()?;

    let mut rows = conn.query("SELECT COUNT(*) FROM teas", ()).await?;
    let count: i64 = rows
        .next()
        .await?
        .map(|r| r.get(0))
        .transpose()?
        .unwrap_or(0);

    Ok(count as usize)
}
