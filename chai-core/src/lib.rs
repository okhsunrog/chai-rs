// Models are always available
pub mod models;

// Server-only modules
#[cfg(feature = "server")]
pub mod ai;
#[cfg(feature = "server")]
pub mod auth;
#[cfg(feature = "server")]
pub mod cache;
#[cfg(feature = "server")]
pub mod config;
#[cfg(feature = "server")]
pub mod embeddings;
#[cfg(feature = "server")]
pub mod http;
#[cfg(feature = "server")]
pub mod openrouter;
#[cfg(feature = "server")]
pub mod scraper;
#[cfg(feature = "server")]
pub mod tea_utils;
#[cfg(feature = "server")]
pub mod turso;

// Re-export commonly used types
pub use models::{
    AIResponse, LLMResponse, PriceVariant, SearchResult, Tea, TeaCard, generate_point_id,
    generate_tea_id,
};

#[cfg(feature = "server")]
pub use auth::{AuthConfig, Claims, UserInfo};
#[cfg(feature = "server")]
pub use cache::CacheStats;
#[cfg(feature = "server")]
pub use config::Config;
#[cfg(feature = "server")]
pub use turso::{CacheStats as TursoCacheStats, DatabaseStats, DbConfig, SearchFilters};
