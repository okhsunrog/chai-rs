use anyhow::{Context, Result};

/// Default embedding model used when EMBEDDING_MODEL env var is not set
pub const DEFAULT_EMBEDDING_MODEL: &str = "qwen/qwen3-embedding-8b";

/// Default vector size for the embedding model
pub const DEFAULT_VECTOR_SIZE: usize = 4096;

/// Application configuration from environment
#[derive(Debug, Clone)]
pub struct Config {
    pub openrouter_api_key: String,
    pub embedding_model: String,
    pub vector_size: usize,
}

impl Config {
    /// Load configuration from .env file and environment
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok(); // Not an error if .env is missing

        let openrouter_api_key =
            std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY not set")?;

        let embedding_model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| DEFAULT_EMBEDDING_MODEL.to_string());

        let vector_size = std::env::var("VECTOR_SIZE")
            .unwrap_or_else(|_| DEFAULT_VECTOR_SIZE.to_string())
            .parse()
            .context("Invalid VECTOR_SIZE")?;

        Ok(Self {
            openrouter_api_key,
            embedding_model,
            vector_size,
        })
    }
}
