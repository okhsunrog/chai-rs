use anyhow::Result;
use chai_core::turso;

/// Get count of teas in database
pub async fn count_teas() -> Result<usize> {
    turso::count_teas().await
}
