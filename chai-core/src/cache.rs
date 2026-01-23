//! HTML cache module for storing scraped pages in SQLite
//!
//! This module provides:
//! - Store HTML content for URLs
//! - Retrieve cached HTML
//! - Check cache freshness
//! - Migrate from JSON cache file

use crate::db;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cached HTML entry
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CacheEntry {
    pub url: String,
    pub html: String,
    pub fetched_at: i64,
}

/// Get cached HTML for a URL
pub async fn get(url: &str) -> Result<Option<CacheEntry>> {
    let pool = db::get_pool()?;

    sqlx::query_as("SELECT url, html, fetched_at FROM html_cache WHERE url = ?")
        .bind(url)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch from cache")
}

/// Store HTML in cache
pub async fn set(url: &str, html: &str) -> Result<()> {
    let pool = db::get_pool()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_secs() as i64;

    sqlx::query("INSERT OR REPLACE INTO html_cache (url, html, fetched_at) VALUES (?, ?, ?)")
        .bind(url)
        .bind(html)
        .bind(now)
        .execute(pool)
        .await
        .context("Failed to store in cache")?;

    Ok(())
}

/// Store multiple entries in cache (batch operation)
pub async fn set_many(entries: &[(String, String)]) -> Result<usize> {
    let pool = db::get_pool()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_secs() as i64;

    let mut count = 0;
    for (url, html) in entries {
        sqlx::query("INSERT OR REPLACE INTO html_cache (url, html, fetched_at) VALUES (?, ?, ?)")
            .bind(url)
            .bind(html)
            .bind(now)
            .execute(pool)
            .await
            .context("Failed to store in cache")?;
        count += 1;
    }

    Ok(count)
}

/// Delete cached entry
pub async fn delete(url: &str) -> Result<bool> {
    let pool = db::get_pool()?;

    let result = sqlx::query("DELETE FROM html_cache WHERE url = ?")
        .bind(url)
        .execute(pool)
        .await
        .context("Failed to delete from cache")?;

    Ok(result.rows_affected() > 0)
}

/// Get all cached URLs
pub async fn list_urls() -> Result<Vec<String>> {
    let pool = db::get_pool()?;

    let rows: Vec<(String,)> = sqlx::query_as("SELECT url FROM html_cache")
        .fetch_all(pool)
        .await
        .context("Failed to list cache URLs")?;

    Ok(rows.into_iter().map(|(url,)| url).collect())
}

/// Get cache statistics
pub async fn stats() -> Result<CacheStats> {
    let pool = db::get_pool()?;

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM html_cache")
        .fetch_one(pool)
        .await
        .context("Failed to get cache count")?;

    let (total_size,): (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(LENGTH(html)), 0) FROM html_cache")
            .fetch_one(pool)
            .await
            .context("Failed to get cache size")?;

    let oldest: Option<(i64,)> = sqlx::query_as("SELECT MIN(fetched_at) FROM html_cache")
        .fetch_optional(pool)
        .await
        .context("Failed to get oldest entry")?;

    let newest: Option<(i64,)> = sqlx::query_as("SELECT MAX(fetched_at) FROM html_cache")
        .fetch_optional(pool)
        .await
        .context("Failed to get newest entry")?;

    Ok(CacheStats {
        entry_count: count as usize,
        total_size_bytes: total_size as usize,
        oldest_entry: oldest.and_then(|(t,)| if t > 0 { Some(t) } else { None }),
        newest_entry: newest.and_then(|(t,)| if t > 0 { Some(t) } else { None }),
    })
}

/// Cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_size_bytes: usize,
    pub oldest_entry: Option<i64>,
    pub newest_entry: Option<i64>,
}

/// Migrate from JSON cache file to SQLite
///
/// Reads a JSON file in the format `{ "url": "html", ... }` and imports it into SQLite.
pub async fn migrate_from_json(json_path: &str) -> Result<usize> {
    let content = std::fs::read_to_string(json_path).context("Failed to read JSON cache file")?;

    let cache_map: HashMap<String, String> =
        serde_json::from_str(&content).context("Failed to parse JSON cache file")?;

    let entries: Vec<(String, String)> = cache_map.into_iter().collect();
    let count = entries.len();

    set_many(&entries).await?;

    tracing::info!("Migrated {} entries from JSON cache to SQLite", count);
    Ok(count)
}

/// Get all cached entries as a HashMap (for compatibility with existing code)
pub async fn get_all() -> Result<HashMap<String, String>> {
    let pool = db::get_pool()?;

    let rows: Vec<(String, String)> = sqlx::query_as("SELECT url, html FROM html_cache")
        .fetch_all(pool)
        .await
        .context("Failed to fetch all cache entries")?;

    Ok(rows.into_iter().collect())
}

/// Check if URL is cached
pub async fn contains(url: &str) -> Result<bool> {
    let pool = db::get_pool()?;

    let result: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM html_cache WHERE url = ? LIMIT 1")
        .bind(url)
        .fetch_optional(pool)
        .await
        .context("Failed to check cache")?;

    Ok(result.is_some())
}

/// Clear all cache entries
pub async fn clear() -> Result<usize> {
    let pool = db::get_pool()?;

    let result = sqlx::query("DELETE FROM html_cache")
        .execute(pool)
        .await
        .context("Failed to clear cache")?;

    Ok(result.rows_affected() as usize)
}
