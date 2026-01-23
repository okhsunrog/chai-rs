//! Utility functions for Tea data processing

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::models::Tea;

/// Create text representation of tea for embedding
#[must_use]
pub fn tea_to_text(tea: &Tea) -> String {
    let mut parts = Vec::new();

    if let Some(name) = &tea.name {
        parts.push(format!("Название: {}", name));
    }

    if let Some(description) = &tea.description {
        parts.push(format!("Описание: {}", description));
    }

    if !tea.composition.is_empty() {
        parts.push(format!("Состав: {}", tea.composition.join(", ")));
    }

    if !tea.full_composition.is_empty() {
        parts.push(format!(
            "Подробный состав: {}",
            tea.full_composition.join(", ")
        ));
    }

    if let Some(series) = &tea.series {
        parts.push(format!("Серия: {}", series));
    }

    if !tea.search_tags.is_empty() {
        parts.push(format!("Теги: {}", tea.search_tags.join(", ")));
    }

    parts.join("\n")
}

/// Compute SHA256 hash for tea
///
/// Returns a hex-encoded SHA256 hash of the Tea's JSON representation.
/// This is used for incremental sync to detect changes.
pub fn compute_tea_hash(tea: &Tea) -> Result<String> {
    let json = serde_json::to_string(tea)?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tea() -> Tea {
        Tea {
            id: "test1234".to_string(),
            url: "https://example.com".to_string(),
            name: Some("Test Tea".to_string()),
            price: Some("100".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_tea_to_text() {
        let mut tea = test_tea();
        tea.composition = vec!["black tea".to_string(), "bergamot".to_string()];
        tea.description = Some("Tea description".to_string());
        tea.series = Some("Test series".to_string());
        tea.search_tags = vec!["tag1".to_string(), "tag2".to_string()];

        let text = tea_to_text(&tea);
        assert!(text.contains("Название: Test Tea"));
        assert!(text.contains("Описание: Tea description"));
        assert!(text.contains("Состав: black tea, bergamot"));
        assert!(text.contains("Серия: Test series"));
        assert!(text.contains("Теги: tag1, tag2"));
    }

    #[test]
    fn test_compute_tea_hash() {
        let tea = test_tea();

        let hash1 = compute_tea_hash(&tea).unwrap();
        let hash2 = compute_tea_hash(&tea).unwrap();

        // Hash should be stable
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 = 64 hex chars
    }

    #[test]
    fn test_hash_changes_on_modification() {
        let mut tea = test_tea();

        let hash1 = compute_tea_hash(&tea).unwrap();

        tea.price = Some("200".to_string());
        let hash2 = compute_tea_hash(&tea).unwrap();

        // Hash should change
        assert_ne!(hash1, hash2);
    }
}
