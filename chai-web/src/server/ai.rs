use anyhow::Result;
use chai_core::AIResponse;

/// Главная функция: получить рекомендации чаёв от AI
///
/// Прослойка для веб-слоя, вызывает функцию из chai_core
pub async fn chat_completion(user_query: String, api_key: String) -> Result<AIResponse> {
    let config = super::config::get()?;
    chai_core::ai::chat_completion(user_query, api_key, config).await
}
