use anyhow::Result;
use chai_core::{SearchResult, Tea};

/// Поиск чаёв по векторному запросу
#[allow(dead_code)]
pub async fn search_teas(query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let config = super::config::get()?;
    chai_core::qdrant::search_teas(query, limit, config).await
}

/// Получить чай по URL
#[allow(dead_code)]
pub async fn get_tea_by_url(url: &str) -> Result<Option<Tea>> {
    let config = super::config::get()?;
    chai_core::qdrant::get_tea_by_url(url, config).await
}

/// Получить количество чаёв в базе
pub async fn count_teas() -> Result<usize> {
    let config = super::config::get()?;
    chai_core::qdrant::count_teas(config).await
}
