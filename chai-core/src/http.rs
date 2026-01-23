//! Shared HTTP client utilities
//!
//! This module provides a shared, lazily-initialized HTTP client for all API calls.
//! Using a single client allows connection pooling and avoids resource duplication.

use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

/// Default HTTP timeout for API requests in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Embeddings requests need longer timeout due to larger payloads
const EMBEDDINGS_TIMEOUT_SECS: u64 = 120;

/// Global HTTP client for general API calls (60s timeout)
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Global HTTP client for embeddings API calls (120s timeout)
static EMBEDDINGS_CLIENT: OnceLock<Client> = OnceLock::new();

/// Get or create the shared HTTP client for general API calls
///
/// This client has a 60-second timeout, suitable for chat completions
/// and other standard API requests.
pub fn get_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("chai-rs/1.0")
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client - this should never fail")
    })
}

/// Get or create the shared HTTP client for embeddings API calls
///
/// This client has a 120-second timeout, as embedding requests
/// can take longer due to larger payloads.
pub fn get_embeddings_client() -> &'static Client {
    EMBEDDINGS_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("chai-rs/1.0")
            .timeout(Duration::from_secs(EMBEDDINGS_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client - this should never fail")
    })
}

/// Strip markdown code blocks from JSON response
///
/// Some models wrap their JSON responses in markdown code blocks like:
/// ```json
/// {"key": "value"}
/// ```
///
/// This function removes such wrappers and returns the clean JSON content.
pub fn strip_markdown_json(content: &str) -> &str {
    let trimmed = content.trim();

    // Handle ```json ... ```
    if let Some(stripped) = trimmed
        .strip_prefix("```json")
        .and_then(|s| s.strip_suffix("```"))
    {
        return stripped.trim();
    }

    // Handle ``` ... ```
    if let Some(stripped) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        return stripped.trim();
    }

    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_markdown_json_with_json_block() {
        let input = r#"```json
{"answer": "test"}
```"#;
        assert_eq!(strip_markdown_json(input), r#"{"answer": "test"}"#);
    }

    #[test]
    fn test_strip_markdown_json_with_plain_block() {
        let input = r#"```
{"answer": "test"}
```"#;
        assert_eq!(strip_markdown_json(input), r#"{"answer": "test"}"#);
    }

    #[test]
    fn test_strip_markdown_json_no_block() {
        let input = r#"{"answer": "test"}"#;
        assert_eq!(strip_markdown_json(input), input);
    }

    #[test]
    fn test_strip_markdown_json_with_whitespace() {
        let input = r#"  ```json
{"answer": "test"}
```  "#;
        assert_eq!(strip_markdown_json(input), r#"{"answer": "test"}"#);
    }

    #[test]
    fn test_get_client_returns_same_instance() {
        let client1 = get_client();
        let client2 = get_client();
        assert!(std::ptr::eq(client1, client2));
    }

    #[test]
    fn test_get_embeddings_client_returns_same_instance() {
        let client1 = get_embeddings_client();
        let client2 = get_embeddings_client();
        assert!(std::ptr::eq(client1, client2));
    }
}
