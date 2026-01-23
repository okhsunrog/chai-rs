use anyhow::{Context, Result};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CountPointsBuilder, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter,
    PointStruct, ScrollPointsBuilder, SearchPointsBuilder, UpsertPointsBuilder, Value,
    VectorParamsBuilder,
};
use std::collections::HashMap;
use tracing::info;

use crate::models::{SearchResult, Tea, generate_point_id};

/// Конфигурация для подключения к Qdrant
#[derive(Debug, Clone)]
pub struct QdrantConfig {
    pub url: String,
    pub collection_name: String,
    pub vector_size: u64,
}

impl QdrantConfig {
    /// Создать конфигурацию из переменных окружения
    pub fn from_env() -> Result<Self> {
        let url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());

        let collection_name =
            std::env::var("QDRANT_COLLECTION").unwrap_or_else(|_| "teas".to_string());

        let vector_size = std::env::var("VECTOR_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(crate::config::DEFAULT_VECTOR_SIZE as u64);

        Ok(Self {
            url,
            collection_name,
            vector_size,
        })
    }

    /// Создать конфигурацию с пользовательскими параметрами
    pub fn new(url: String, collection_name: String, vector_size: u64) -> Self {
        Self {
            url,
            collection_name,
            vector_size,
        }
    }
}

/// Клиент для работы с Qdrant
pub struct QdrantClient {
    client: Qdrant,
    config: QdrantConfig,
}

impl QdrantClient {
    /// Создать новый клиент
    pub async fn new(config: QdrantConfig) -> Result<Self> {
        info!("🔌 Подключение к Qdrant: {}", config.url);

        let client = Qdrant::from_url(&config.url)
            .build()
            .context("Failed to create Qdrant client")?;

        Ok(Self { client, config })
    }

    /// Создать коллекцию если её нет
    pub async fn ensure_collection(&self) -> Result<()> {
        let collections = self.client.list_collections().await?;

        let collection_exists = collections
            .collections
            .iter()
            .any(|c| c.name == self.config.collection_name);

        if collection_exists {
            info!("✅ Коллекция '{}' существует", self.config.collection_name);
            return Ok(());
        }

        info!(
            "📦 Создание коллекции '{}' с размером вектора {}",
            self.config.collection_name, self.config.vector_size
        );

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.config.collection_name).vectors_config(
                    VectorParamsBuilder::new(self.config.vector_size, Distance::Cosine),
                ),
            )
            .await
            .context("Failed to create collection")?;

        info!("✅ Коллекция создана");

        Ok(())
    }

    /// Получить точку по URL (для проверки существования и хеша)
    pub async fn get_by_url(&self, url: &str) -> Result<Option<TeaPoint>> {
        // Поиск по фильтру URL
        let search_result = self
            .client
            .scroll(
                ScrollPointsBuilder::new(&self.config.collection_name)
                    .filter(qdrant_client::qdrant::Filter::must([
                        qdrant_client::qdrant::Condition::matches("url", url.to_string()),
                    ]))
                    .limit(1),
            )
            .await?;

        if let Some(point) = search_result.result.first() {
            let payload = point.payload.clone();

            // Десериализуем tea_data из JSON строки
            let tea_json_str = payload
                .get("tea_data")
                .and_then(|v| v.as_str())
                .with_context(|| format!("tea_data not found or not a string for URL: {}", url))?;

            let tea: Tea = serde_json::from_str(tea_json_str)
                .with_context(|| format!("Failed to parse tea_data JSON for URL: {}", url))?;

            let content_hash = payload
                .get("content_hash")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();

            Ok(Some(TeaPoint {
                id: point.id.clone().context("Point ID not found")?,
                tea,
                content_hash,
            }))
        } else {
            Ok(None)
        }
    }

    /// Сохранить или обновить чай
    pub async fn upsert_tea(
        &self,
        tea: &Tea,
        vector: Vec<f32>,
        content_hash: String,
    ) -> Result<()> {
        // Use full UUID for Qdrant point ID (see generate_point_id docs)
        let point_id = generate_point_id(&tea.url);

        // Создаем payload в формате HashMap<String, Value>
        let mut payload: HashMap<String, Value> = HashMap::new();
        payload.insert("id".to_string(), tea.id.clone().into()); // Короткий ID для API
        payload.insert("url".to_string(), tea.url.clone().into());
        payload.insert(
            "name".to_string(),
            tea.name.clone().unwrap_or_default().into(),
        );
        payload.insert(
            "price".to_string(),
            tea.price.clone().unwrap_or_default().into(),
        );
        payload.insert(
            "series".to_string(),
            tea.series.clone().unwrap_or_default().into(),
        );
        payload.insert("in_stock".to_string(), tea.in_stock.into());
        payload.insert("content_hash".to_string(), content_hash.into());

        // Сериализуем чай в JSON строку для хранения
        let tea_json_str = serde_json::to_string(tea)?;
        payload.insert("tea_data".to_string(), tea_json_str.into());

        let point = PointStruct::new(point_id, vector, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(
                &self.config.collection_name,
                vec![point],
            ))
            .await
            .context("Failed to upsert point")?;

        Ok(())
    }

    /// Удалить чай по URL
    pub async fn delete_by_url(&self, url: &str) -> Result<()> {
        // Сначала находим точку
        if let Some(point) = self.get_by_url(url).await? {
            self.client
                .delete_points(
                    DeletePointsBuilder::new(&self.config.collection_name).points(vec![point.id]),
                )
                .await
                .context("Failed to delete point")?;

            info!("🗑️  Удален чай: {}", url);
        }

        Ok(())
    }

    /// Поиск чаёв по вектору
    pub async fn search(
        &self,
        vector: Vec<f32>,
        limit: u64,
        filter: Option<qdrant_client::qdrant::Filter>,
    ) -> Result<Vec<SearchResult>> {
        let mut search_builder =
            SearchPointsBuilder::new(&self.config.collection_name, vector, limit)
                .with_payload(true);

        if let Some(f) = filter {
            search_builder = search_builder.filter(f);
        }

        let results = self.client.search_points(search_builder).await?;

        let mut search_results = Vec::new();

        for scored_point in results.result {
            let payload = scored_point.payload;

            // Десериализуем tea_data из JSON строки
            let tea_json_str = payload
                .get("tea_data")
                .and_then(|v| v.as_str())
                .with_context(|| {
                    format!(
                        "tea_data not found or not a string in search result (score: {})",
                        scored_point.score
                    )
                })?;

            let tea: Tea = serde_json::from_str(tea_json_str).with_context(|| {
                format!(
                    "Failed to parse tea_data JSON (score: {})",
                    scored_point.score
                )
            })?;

            search_results.push(SearchResult {
                tea,
                score: scored_point.score,
            });
        }

        Ok(search_results)
    }

    /// Получить все URL чаёв из базы
    pub async fn get_all_urls(&self) -> Result<Vec<String>> {
        let mut urls = Vec::new();
        let mut offset = None;

        loop {
            let mut scroll_builder = ScrollPointsBuilder::new(&self.config.collection_name)
                .limit(100)
                .with_payload(true);

            if let Some(offset_id) = offset {
                scroll_builder = scroll_builder.offset(offset_id);
            }

            let result = self.client.scroll(scroll_builder).await?;

            for point in &result.result {
                if let Some(url) = point.payload.get("url").and_then(|v| v.as_str()) {
                    urls.push(url.to_string());
                }
            }

            if result.next_page_offset.is_none() {
                break;
            }

            offset = result.next_page_offset;
        }

        Ok(urls)
    }

    /// Получить статистику по базе данных
    ///
    /// Uses Qdrant's count API for efficient counting instead of loading all documents.
    pub async fn get_stats(&self) -> Result<DatabaseStats> {
        // Count total teas using Qdrant count API
        let total_result = self
            .client
            .count(CountPointsBuilder::new(&self.config.collection_name))
            .await?;
        let total_teas = total_result.result.map(|r| r.count).unwrap_or(0) as usize;

        // Count in_stock teas using filter
        let in_stock_result = self
            .client
            .count(
                CountPointsBuilder::new(&self.config.collection_name).filter(Filter::must([
                    qdrant_client::qdrant::Condition::matches("in_stock", true),
                ])),
            )
            .await?;
        let in_stock = in_stock_result.result.map(|r| r.count).unwrap_or(0) as usize;
        let out_of_stock = total_teas.saturating_sub(in_stock);

        // For series list, we still need to scroll through all teas
        // but only fetch the series field from payload
        let mut series_set = std::collections::HashSet::new();
        let mut offset = None;

        loop {
            let mut scroll_builder = ScrollPointsBuilder::new(&self.config.collection_name)
                .limit(100)
                .with_payload(true);

            if let Some(offset_id) = offset {
                scroll_builder = scroll_builder.offset(offset_id);
            }

            let result = self.client.scroll(scroll_builder).await?;

            for point in &result.result {
                if let Some(series) = point.payload.get("series").and_then(|v| v.as_str())
                    && !series.is_empty()
                {
                    series_set.insert(series.to_string());
                }
            }

            if result.next_page_offset.is_none() {
                break;
            }

            offset = result.next_page_offset;
        }

        let mut series_list: Vec<String> = series_set.into_iter().collect();
        series_list.sort();

        Ok(DatabaseStats {
            total_teas,
            in_stock,
            out_of_stock,
            series_count: series_list.len(),
            series_list,
        })
    }
}

/// Точка с данными чая
pub struct TeaPoint {
    pub id: qdrant_client::qdrant::PointId,
    pub tea: Tea,
    pub content_hash: String,
}

// Note: SearchResult is defined in models.rs and re-exported from lib.rs

/// Статистика по базе данных
#[derive(Debug)]
pub struct DatabaseStats {
    pub total_teas: usize,
    pub in_stock: usize,
    pub out_of_stock: usize,
    pub series_count: usize,
    pub series_list: Vec<String>,
}
