use crate::http::get_embeddings_client;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

/// HTTP request timeout in seconds (used by EmbeddingsClient)
const HTTP_TIMEOUT_SECS: u64 = 120;

/// Конфигурация для API эмбеддингов
#[derive(Debug, Clone)]
pub struct EmbeddingsConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl EmbeddingsConfig {
    /// Создать конфигурацию из переменных окружения
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY not set")?;

        let model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| crate::config::DEFAULT_EMBEDDING_MODEL.to_string());

        let base_url = std::env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

        Ok(Self {
            api_key,
            model,
            base_url,
        })
    }

    /// Создать конфигурацию с пользовательскими параметрами
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://openrouter.ai/api/v1".to_string(),
        }
    }
}

/// Запрос для создания эмбеддингов
#[derive(Debug, Serialize)]
struct EmbeddingsRequest {
    model: String,
    input: Vec<String>,
}

/// Ответ от API эмбеддингов
#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingObject>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingObject {
    embedding: Vec<f32>,
    index: usize,
}

/// Клиент для работы с API эмбеддингов
pub struct EmbeddingsClient {
    client: Client,
    config: EmbeddingsConfig,
}

impl EmbeddingsClient {
    /// Создать новый клиент
    pub fn new(config: EmbeddingsConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent("chai-rs/1.0")
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()?;

        Ok(Self { client, config })
    }

    /// Создать эмбеддинги для текстов (батч обработка)
    pub async fn create_embeddings(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        info!("📊 Создание эмбеддингов для {} текстов", texts.len());

        let request = EmbeddingsRequest {
            model: self.config.model.clone(),
            input: texts,
        };

        let url = format!("{}/embeddings", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send embeddings request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("API error ({}): {}", status, error_text);
        }

        let embeddings_response: EmbeddingsResponse = response
            .json()
            .await
            .context("Failed to parse embeddings response")?;

        // Сортируем по индексу (на случай если порядок не совпадает)
        let mut embeddings: Vec<(usize, Vec<f32>)> = embeddings_response
            .data
            .into_iter()
            .map(|obj| (obj.index, obj.embedding))
            .collect();

        embeddings.sort_by_key(|(index, _)| *index);

        let result: Vec<Vec<f32>> = embeddings.into_iter().map(|(_, emb)| emb).collect();

        info!("✅ Создано {} эмбеддингов", result.len());

        Ok(result)
    }

    /// Создать эмбеддинг для одного текста
    pub async fn create_embedding(&self, text: String) -> Result<Vec<f32>> {
        let embeddings = self.create_embeddings(vec![text]).await?;
        embeddings
            .into_iter()
            .next()
            .context("No embedding returned")
    }
}

/// Удобная функция для создания эмбеддинга (использует кэшированный HTTP клиент)
pub async fn generate_embedding(text: &str, api_key: &str, model: &str) -> Result<Vec<f32>> {
    let client = get_embeddings_client();

    let request = EmbeddingsRequest {
        model: model.to_string(),
        input: vec![text.to_string()],
    };

    let url = "https://openrouter.ai/api/v1/embeddings";

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request)
        .send()
        .await
        .context("Failed to send embeddings request")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        anyhow::bail!("API error ({}): {}", status, error_text);
    }

    let embeddings_response: EmbeddingsResponse = response
        .json()
        .await
        .context("Failed to parse embeddings response")?;

    embeddings_response
        .data
        .into_iter()
        .next()
        .map(|obj| obj.embedding)
        .context("No embedding returned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = EmbeddingsConfig::new("test-key".to_string(), "test-model".to_string());
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.model, "test-model");
        assert_eq!(config.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn test_default_model_constant() {
        // Verify the default model matches the config constant
        assert_eq!(
            crate::config::DEFAULT_EMBEDDING_MODEL,
            "qwen/qwen3-embedding-8b"
        );
    }
}
