use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generates a short unique ID from URL (first 8 characters of UUID v5)
///
/// This ID is used for API communication and LLM prompts where a shorter
/// identifier is more practical. The ID is deterministic based on the URL.
///
/// For database primary keys, use [`generate_point_id`] which returns the full UUID.
#[must_use]
pub fn generate_tea_id(url: &str) -> String {
    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_URL, url.as_bytes());
    uuid.to_string()[..8].to_string()
}

/// Generates a full UUID v5 from URL for use as database primary key
///
/// This ensures uniqueness for database storage. The short [`generate_tea_id`]
/// is used for API/LLM communication, while this full UUID is for internal storage.
#[must_use]
pub fn generate_point_id(url: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, url.as_bytes()).to_string()
}

/// Информация о чае (полная версия для всех крейтов)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tea {
    /// Уникальный ID (первые 8 символов UUID, генерируется из URL)
    pub id: String,
    pub url: String,
    pub name: Option<String>,
    pub price: Option<String>,

    // Ценовые варианты
    #[serde(default)]
    pub price_variants: Vec<PriceVariant>,

    // Состав
    #[serde(default)]
    pub composition: Vec<String>,
    #[serde(default)]
    pub full_composition: Vec<String>,

    // Описание и категоризация
    pub description: Option<String>,
    pub series: Option<String>,

    // Варианты и хранение
    #[serde(default)]
    pub volume_options: Vec<String>,
    pub storage_info: Option<String>,

    // Медиа
    #[serde(default)]
    pub images: Vec<String>,

    // Поиск и метаданные
    #[serde(default)]
    pub search_tags: Vec<String>,
    pub dimensions: Option<String>,
    pub weight: Option<String>,

    // Наличие и тип
    #[serde(default)]
    pub in_stock: bool,
    #[serde(default)]
    pub is_sample: bool,
    #[serde(default)]
    pub is_set: bool,
    #[serde(default)]
    pub sample_url: Option<String>,
}

impl Tea {
    /// Create a new Tea with the given URL, auto-generating the ID
    #[must_use]
    pub fn new(url: &str) -> Self {
        Self {
            id: generate_tea_id(url),
            url: url.to_string(),
            ..Default::default()
        }
    }
}

/// Вариант цены/упаковки
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PriceVariant {
    pub packaging: String,
    pub price: String,
    pub quantity: String,
}

/// Результат векторного поиска
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub tea: Tea,
    pub score: f32,
}

/// Карточка чая для UI (упрощённая версия для фронтенда)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeaCard {
    pub url: String,
    pub title: String,
    pub tags: Vec<String>,
    pub match_score: f32,
    /// Short LLM-generated description (1-2 sentences)
    pub short_description: String,

    // Обогащённые данные из базы данных
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub in_stock: bool,
    #[serde(default)]
    pub composition: Vec<String>,
    #[serde(default)]
    pub sample_url: Option<String>,
    #[serde(default)]
    pub sample_in_stock: bool,

    // Дополнительные поля для детальной карточки
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub full_composition: Vec<String>,
    #[serde(default)]
    pub price_variants: Vec<PriceVariant>,
}

/// Ответ от LLM (сырой, без обогащения)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub answer: String,
    pub tea_ids: Vec<String>,
    pub tags: std::collections::HashMap<String, Vec<String>>,
    /// Short descriptions for each tea (1-2 sentences)
    #[serde(default)]
    pub descriptions: std::collections::HashMap<String, String>,
    /// Prompt injection detected in Stage 3 (backup detection)
    #[serde(default)]
    pub is_prompt_injection: bool,
}

/// Ответ от AI с рекомендациями (после обогащения данными)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub answer: String,
    pub tea_cards: Vec<TeaCard>,
}
