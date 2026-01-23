//! Web scraping utilities for tea data from beliyles.com

use anyhow::{Context, Result};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use std::sync::LazyLock;
use tracing::info;

use crate::models::{PriceVariant, Tea};

// Pre-compiled regexes for better performance
static PRODUCT_JSON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"var product = (\{.+?\});").expect("Invalid PRODUCT_JSON_RE"));
static VOLUME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+[-~≈]?\d*)").expect("Invalid VOLUME_RE"));
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").expect("Invalid TAG_RE"));
static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("Invalid WHITESPACE_RE"));

// Pre-compiled CSS selectors for better performance
static LOC_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("loc").expect("Invalid loc selector"));
static SCRIPT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("script").expect("Invalid script selector"));

const SITEMAP_URL: &str = "https://beliyles.com/sitemap-store.xml";

/// Strip HTML tags and normalize whitespace
fn strip_html(text: &str) -> String {
    let no_tags = TAG_RE.replace_all(text, " ");
    let normalized = WHITESPACE_RE.replace_all(&no_tags, " ");
    normalized.trim().to_string()
}

/// Get list of all tea URLs from sitemap
pub async fn get_tea_urls(client: &Client) -> Result<Vec<String>> {
    info!("Fetching tea URLs from sitemap");

    let response = client
        .get(SITEMAP_URL)
        .send()
        .await
        .context("Failed to fetch sitemap")?;

    let xml = response.text().await?;
    let document = Html::parse_document(&xml);

    let urls: Vec<String> = document
        .select(&LOC_SELECTOR)
        .filter_map(|element| {
            let url = element.text().collect::<String>();
            // Filter only products (exclude constructors and certificates)
            // Include samples for later processing
            if url.contains("/tproduct/")
                && !url.contains("/constructor/")
                && !url.contains("/card/")
            {
                Some(url)
            } else {
                None
            }
        })
        .collect();

    info!("Found {} teas", urls.len());
    Ok(urls)
}

/// Parse tea from HTML string (for cache)
pub fn parse_tea_from_html(url: &str, html: &str) -> Result<Tea> {
    let document = Html::parse_document(html);
    parse_tea_document(url, &document)
}

/// Scrape a single tea page (fetch from website)
pub async fn scrape_tea(client: &Client, url: &str) -> Result<Tea> {
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch tea page")?;

    if response.status().is_client_error() {
        anyhow::bail!("Page not found ({})", response.status());
    }

    let html = response.text().await?;
    let document = Html::parse_document(&html);
    parse_tea_document(url, &document)
}

/// Common logic for parsing Tea from Html document
fn parse_tea_document(url: &str, document: &Html) -> Result<Tea> {
    let mut tea = Tea::new(url);

    for script in document.select(&SCRIPT_SELECTOR) {
        let text: String = script.text().collect();

        if text.contains("var product = ")
            && let Some(json_str) = extract_product_json(&text)
            && let Ok(product_data) = serde_json::from_str::<serde_json::Value>(&json_str)
        {
            parse_product_json(&mut tea, &product_data);
            break;
        }
    }

    // Determine product type
    let is_sample = url.contains("probnik") || url.contains("/probe/");
    tea.is_sample = is_sample;
    tea.is_set = is_sample_set(url, &tea.name);

    // Check: if no product data at all - skip
    if tea.name.is_none() && tea.images.is_empty() {
        anyhow::bail!("Skipping product with no data: {}", url);
    }

    // Check: if this is a discontinued sample (suffix "r" + no price)
    if is_sample && let Some(name) = &tea.name {
        let name_lower = name.to_lowercase();
        let has_r_suffix = name_lower.ends_with(" r") || name_lower.contains(" r\"");
        let no_price = tea.price.is_none() || tea.price.as_ref().is_none_or(|p| p.is_empty());

        if has_r_suffix && no_price && !tea.in_stock {
            anyhow::bail!("Skipping removed sample (discontinued): {}", name);
        }
    }

    Ok(tea)
}

/// Extract JSON from "var product = {...};" string
fn extract_product_json(text: &str) -> Option<String> {
    let caps = PRODUCT_JSON_RE.captures(text)?;
    caps.get(1).map(|m| m.as_str().to_string())
}

/// Parse data from JSON product object
fn parse_product_json(tea: &mut Tea, data: &serde_json::Value) {
    // Title
    if let Some(title) = data["title"].as_str() {
        tea.name = Some(title.to_string());
    }

    // Price
    if let Some(price) = data["price"].as_str() {
        tea.price = Some(price.to_string());
    }

    // Images
    if let Some(gallery) = data["gallery"].as_array() {
        tea.images = gallery
            .iter()
            .filter_map(|img| img["img"].as_str().map(|s| s.to_string()))
            .collect();
    }

    // Price variants/editions
    if let Some(editions) = data["editions"].as_array() {
        tea.price_variants = editions
            .iter()
            .filter_map(|edition| {
                Some(PriceVariant {
                    packaging: edition["Упаковка"].as_str()?.to_string(),
                    price: edition["price"].as_str()?.to_string(),
                    quantity: edition["quantity"].as_str()?.to_string(),
                })
            })
            .collect();

        // Check stock (if at least one variant has quantity > 0)
        tea.in_stock = tea
            .price_variants
            .iter()
            .any(|v| v.quantity.parse::<i32>().unwrap_or(0) > 0);

        // Extract volume options
        tea.volume_options = tea
            .price_variants
            .iter()
            .filter_map(|v| {
                VOLUME_RE
                    .captures(&v.packaging)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string())
            })
            .collect();
    }

    // If price_variants is empty, check main quantity field
    if !tea.in_stock
        && let Some(quantity_str) = data["quantity"].as_str()
    {
        tea.in_stock = quantity_str.parse::<i32>().unwrap_or(0) > 0;
    }

    // Full description text
    if let Some(text) = data["text"].as_str() {
        // Description (before "Состав:")
        if let Some(desc_end) = text.find("Состав:") {
            let desc = &text[..desc_end];
            tea.description = Some(strip_html(desc));
        }

        // Composition
        if let Some(comp_match) = extract_between(text, "Состав:", "<br") {
            tea.composition = comp_match
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // Detailed composition
        if let Some(detailed_comp) = extract_between(text, "Подробный состав:", "<br")
        {
            tea.full_composition = detailed_comp
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // Search tags
        if let Some(tags) = extract_between(text, "Также для поиска:", "<br") {
            tea.search_tags = tags
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // Storage info
        if let Some(storage) = extract_between(text, "Хранить", "Дата изготовления")
        {
            tea.storage_info = Some(
                storage
                    .replace("<br />", " ")
                    .replace("<br/>", " ")
                    .trim()
                    .to_string(),
            );
        }
    }

    // Package dimensions (from first variant if available)
    if let Some(first_edition) = data["editions"].as_array().and_then(|arr| arr.first()) {
        if let (Some(x), Some(y), Some(z)) = (
            first_edition["pack_x"].as_i64(),
            first_edition["pack_y"].as_i64(),
            first_edition["pack_z"].as_i64(),
        ) {
            tea.dimensions = Some(format!("{}x{}x{} mm", x, y, z));
        }

        if let Some(weight) = first_edition["pack_m"].as_i64() {
            tea.weight = Some(format!("{} g", weight));
        }
    }

    // Series
    if let Some(chars) = data["characteristics"].as_array() {
        for char in chars {
            if char["title"].as_str() == Some("Серия") {
                tea.series = char["value"].as_str().map(|s| s.to_string());
            }
        }
    }
}

/// Extract text between two markers
fn extract_between(text: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start = text.find(start_marker)? + start_marker.len();
    let remaining = &text[start..];
    let end = remaining.find(end_marker)?;
    let raw = remaining[..end].trim();
    Some(clean_html(raw))
}

/// Clean text from HTML tags and entities
fn clean_html(text: &str) -> String {
    let result = TAG_RE.replace_all(text, "");

    let result = result
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");

    WHITESPACE_RE.replace_all(&result, " ").trim().to_string()
}

/// Try to find main product URL from sample URL
#[must_use]
pub fn find_main_product_url(sample_url: &str) -> String {
    sample_url
        .replace("probnik-", "")
        .replace("/probe/", "/")
        .replace("/rasprodazha/", "/")
}

/// Check if product is a sample set
#[must_use]
pub fn is_sample_set(url: &str, name: &Option<String>) -> bool {
    if url.contains("nabor") || url.contains("набор") {
        return true;
    }

    if let Some(n) = name {
        let n_lower = n.to_lowercase();
        if n_lower.contains("набор") || n_lower.contains("nabor") {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_product_json() {
        let text = r#"var product = {"title":"Test","price":"100"}; var other = 1;"#;
        let result = extract_product_json(text);
        assert_eq!(
            result,
            Some(r#"{"title":"Test","price":"100"}"#.to_string())
        );
    }
}
