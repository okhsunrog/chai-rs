use anyhow::{Context, Result};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, Distance, FieldType, PointStruct,
    ScrollPointsBuilder, SearchPointsBuilder, VectorParamsBuilder,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::embeddings::generate_embedding;
use crate::models::{SearchResult, Tea};
use crate::tea_utils::tea_to_text;

/// Helper function to extract and parse Tea from Qdrant payload
fn extract_tea_from_payload(
    payload: &HashMap<String, qdrant_client::qdrant::Value>,
) -> Result<Tea> {
    let tea_json_str = payload
        .get("tea_data")
        .and_then(|v| v.as_str())
        .context("tea_data field not found or not a string in payload")?;

    serde_json::from_str(tea_json_str).context("Failed to parse tea_data JSON")
}

/// Global cached Qdrant client (initialized on first use)
static QDRANT_CLIENT: OnceCell<Arc<Qdrant>> = OnceCell::const_new();

/// Get or create a cached Qdrant client
async fn get_cached_client(config: &Config) -> Result<Arc<Qdrant>> {
    QDRANT_CLIENT
        .get_or_try_init(|| async {
            let client = Qdrant::from_url(&config.qdrant_url).build()?;
            Ok::<_, anyhow::Error>(Arc::new(client))
        })
        .await
        .cloned()
}

/// Создать клиент Qdrant (deprecated: use get_cached_client instead for better performance)
pub async fn create_client(config: &Config) -> Result<Qdrant> {
    Ok(Qdrant::from_url(&config.qdrant_url).build()?)
}

/// Создать или пересоздать коллекцию
pub async fn create_collection(config: &Config) -> Result<()> {
    let client = create_client(config).await?;

    // Удаляем существующую коллекцию если есть
    let _ = client.delete_collection(&config.qdrant_collection).await;

    info!("📦 Создание коллекции {}...", config.qdrant_collection);

    // Создаём новую коллекцию
    client
        .create_collection(
            CreateCollectionBuilder::new(&config.qdrant_collection).vectors_config(
                VectorParamsBuilder::new(config.vector_size as u64, Distance::Cosine),
            ),
        )
        .await?;

    // Создаём индексы для быстрого поиска
    client
        .create_field_index(CreateFieldIndexCollectionBuilder::new(
            &config.qdrant_collection,
            "url",
            FieldType::Keyword,
        ))
        .await?;

    client
        .create_field_index(CreateFieldIndexCollectionBuilder::new(
            &config.qdrant_collection,
            "id",
            FieldType::Keyword,
        ))
        .await?;

    info!(
        "Collection {} created with url and id indexes",
        config.qdrant_collection
    );
    Ok(())
}

/// Добавить чай в Qdrant
pub async fn index_tea(client: &Qdrant, config: &Config, tea: &Tea) -> Result<()> {
    use qdrant_client::qdrant::{UpsertPointsBuilder, Value};
    use std::collections::HashMap;

    // Создаём текст для эмбеддинга (используем tea_to_text для консистентности)
    let text_for_embedding = tea_to_text(tea);

    // Генерируем эмбеддинг
    let embedding = generate_embedding(
        &text_for_embedding,
        &config.openrouter_api_key,
        &config.embedding_model,
    )
    .await?;

    // Создаём payload с данными чая
    let tea_json = serde_json::to_string(tea)?;
    let mut payload = HashMap::new();
    payload.insert("tea_data".to_string(), Value::from(tea_json));
    payload.insert("id".to_string(), Value::from(tea.id.clone()));
    payload.insert("url".to_string(), Value::from(tea.url.clone()));
    if let Some(ref name) = tea.name {
        payload.insert("name".to_string(), Value::from(name.clone()));
    }
    payload.insert("in_stock".to_string(), Value::from(tea.in_stock));
    payload.insert("is_sample".to_string(), Value::from(tea.is_sample));
    payload.insert("is_set".to_string(), Value::from(tea.is_set));

    // Создаём ID для точки
    let point_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, tea.url.as_bytes());

    // Добавляем точку
    let point = PointStruct::new(point_id.to_string(), embedding, payload);

    client
        .upsert_points(UpsertPointsBuilder::new(
            &config.qdrant_collection,
            vec![point],
        ))
        .await?;

    Ok(())
}

/// Search filters for Qdrant queries
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub exclude_samples: bool,
    pub exclude_sets: bool,
    pub only_in_stock: bool,
}

/// Поиск чаёв по векторному запросу
///
/// Uses a cached Qdrant client for better performance.
pub async fn search_teas(query: &str, limit: usize, config: &Config) -> Result<Vec<SearchResult>> {
    search_teas_filtered(query, limit, &SearchFilters::default(), config).await
}

/// Поиск чаёв с фильтрами
pub async fn search_teas_filtered(
    query: &str,
    limit: usize,
    filters: &SearchFilters,
    config: &Config,
) -> Result<Vec<SearchResult>> {
    use qdrant_client::qdrant::{Condition, Filter};

    let client = get_cached_client(config).await?;

    // Создаём эмбеддинг для запроса
    let embedding =
        generate_embedding(query, &config.openrouter_api_key, &config.embedding_model).await?;

    // Build filter conditions
    let mut must_not = Vec::new();
    let mut must = Vec::new();

    if filters.exclude_samples {
        must_not.push(Condition::matches("is_sample", true));
    }
    if filters.exclude_sets {
        must_not.push(Condition::matches("is_set", true));
    }
    if filters.only_in_stock {
        must.push(Condition::matches("in_stock", true));
    }

    // Build search request
    let mut search_builder =
        SearchPointsBuilder::new(&config.qdrant_collection, embedding, limit as u64)
            .with_payload(true);

    // Apply filters if any
    if !must.is_empty() || !must_not.is_empty() {
        let filter = Filter {
            must: must.into_iter().collect(),
            must_not: must_not.into_iter().collect(),
            ..Default::default()
        };
        search_builder = search_builder.filter(filter);
    }

    // Поиск в Qdrant
    let search_result = client.search_points(search_builder).await?;

    // Парсим результаты
    let mut results = Vec::new();

    for scored_point in search_result.result {
        let payload = scored_point.payload;

        // Десериализуем tea_data из JSON строки
        match extract_tea_from_payload(&payload) {
            Ok(tea) => {
                results.push(SearchResult {
                    tea,
                    score: scored_point.score,
                });
            }
            Err(e) => {
                warn!(
                    "Failed to parse tea from search result (score: {}): {}",
                    scored_point.score, e
                );
            }
        }
    }

    Ok(results)
}

/// Получить чай по ID
///
/// Uses a cached Qdrant client for better performance.
pub async fn get_tea_by_id(id: &str, config: &Config) -> Result<Option<Tea>> {
    let client = get_cached_client(config).await?;

    // Ищем по полю id
    let scroll_result = client
        .scroll(
            ScrollPointsBuilder::new(&config.qdrant_collection)
                .filter(qdrant_client::qdrant::Filter::must([
                    qdrant_client::qdrant::Condition::matches("id", id.to_string()),
                ]))
                .limit(1)
                .with_payload(true),
        )
        .await?;

    // Парсим первый результат
    if let Some(point) = scroll_result.result.first() {
        match extract_tea_from_payload(&point.payload) {
            Ok(tea) => return Ok(Some(tea)),
            Err(e) => {
                warn!("Failed to parse tea by id '{}': {}", id, e);
            }
        }
    }

    Ok(None)
}

/// Получить чай по URL
///
/// Uses a cached Qdrant client for better performance.
pub async fn get_tea_by_url(url: &str, config: &Config) -> Result<Option<Tea>> {
    let client = get_cached_client(config).await?;

    // Ищем по полю url
    let scroll_result = client
        .scroll(
            ScrollPointsBuilder::new(&config.qdrant_collection)
                .filter(qdrant_client::qdrant::Filter::must([
                    qdrant_client::qdrant::Condition::matches("url", url.to_string()),
                ]))
                .limit(1)
                .with_payload(true),
        )
        .await?;

    // Парсим первый результат
    if let Some(point) = scroll_result.result.first() {
        match extract_tea_from_payload(&point.payload) {
            Ok(tea) => return Ok(Some(tea)),
            Err(e) => {
                warn!("Failed to parse tea by url '{}': {}", url, e);
            }
        }
    }

    Ok(None)
}

/// Получить количество чаёв в коллекции
///
/// Uses a cached Qdrant client for better performance.
pub async fn count_teas(config: &Config) -> Result<usize> {
    let client = get_cached_client(config).await?;

    let info = client.collection_info(&config.qdrant_collection).await?;

    let count = info.result.and_then(|r| r.points_count).unwrap_or(0) as usize;

    Ok(count)
}
