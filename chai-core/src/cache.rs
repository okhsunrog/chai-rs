//! HTML cache module for storing scraped pages
//!
//! This module provides:
//! - Store HTML content for URLs
//! - Retrieve cached HTML
//! - Check cache freshness
//! - Migrate from JSON cache file
//!
//! This is a thin wrapper around turso database functions.

use crate::turso;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;

/// Cached HTML entry (re-export from turso)
pub use turso::CacheEntry;

/// Cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_size_bytes: usize,
    pub oldest_entry: Option<i64>,
    pub newest_entry: Option<i64>,
}

impl From<turso::CacheStats> for CacheStats {
    fn from(stats: turso::CacheStats) -> Self {
        Self {
            entry_count: stats.entry_count,
            total_size_bytes: stats.total_size_bytes,
            oldest_entry: stats.oldest_entry,
            newest_entry: stats.newest_entry,
        }
    }
}

/// Get cached HTML for a URL
pub async fn get(url: &str) -> Result<Option<CacheEntry>> {
    turso::cache_get(url).await
}

/// Store HTML in cache
pub async fn set(url: &str, html: &str) -> Result<()> {
    turso::cache_set(url, html).await
}

/// Store multiple entries in cache (batch operation)
pub async fn set_many(entries: &[(String, String)]) -> Result<usize> {
    let mut count = 0;
    for (url, html) in entries {
        turso::cache_set(url, html).await?;
        count += 1;
    }
    Ok(count)
}

/// Get all cached URLs
pub async fn list_urls() -> Result<Vec<String>> {
    turso::cache_list_urls().await
}

/// Get cache statistics
pub async fn stats() -> Result<CacheStats> {
    turso::cache_stats().await.map(CacheStats::from)
}

/// Migrate from JSON cache file
pub async fn migrate_from_json(json_path: &str) -> Result<usize> {
    turso::cache_migrate_from_json(json_path).await
}

/// Get all cached entries as a HashMap (for compatibility with existing code)
pub async fn get_all() -> Result<HashMap<String, String>> {
    let urls = turso::cache_list_urls().await?;
    let mut map = HashMap::new();

    for url in urls {
        if let Some(entry) = turso::cache_get(&url).await? {
            map.insert(entry.url, entry.html);
        }
    }

    Ok(map)
}

/// Check if URL is cached
pub async fn contains(url: &str) -> Result<bool> {
    turso::cache_contains(url).await
}

/// Clear all cache entries
pub async fn clear() -> Result<usize> {
    turso::cache_clear().await
}
