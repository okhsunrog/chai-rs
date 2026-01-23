use anyhow::{Context, Result};

/// Default embedding model used when EMBEDDING_MODEL env var is not set
pub const DEFAULT_EMBEDDING_MODEL: &str = "qwen/qwen3-embedding-8b";

/// Default vector size for the embedding model
pub const DEFAULT_VECTOR_SIZE: usize = 4096;

/// Конфигурация приложения из environment
#[derive(Debug, Clone)]
pub struct Config {
    pub openrouter_api_key: String,
    pub embedding_model: String,
    pub qdrant_url: String,
    pub qdrant_collection: String,
    pub vector_size: usize,
}

impl Config {
    /// Загрузить конфигурацию из .env файла и environment
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok(); // Не ошибка если .env отсутствует

        let openrouter_api_key =
            std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY not set")?;

        let embedding_model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| DEFAULT_EMBEDDING_MODEL.to_string());

        let qdrant_url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());

        let qdrant_collection =
            std::env::var("QDRANT_COLLECTION").unwrap_or_else(|_| "teas".to_string());

        let vector_size = std::env::var("VECTOR_SIZE")
            .unwrap_or_else(|_| DEFAULT_VECTOR_SIZE.to_string())
            .parse()
            .context("Invalid VECTOR_SIZE")?;

        Ok(Self {
            openrouter_api_key,
            embedding_model,
            qdrant_url,
            qdrant_collection,
            vector_size,
        })
    }
}
