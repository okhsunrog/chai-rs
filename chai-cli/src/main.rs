use anyhow::{Context, Result};
use chai_core::{DbConfig, Tea, cache, scraper, tea_utils, turso};
use clap::{Parser, Subcommand};
use reqwest::Client;
use std::path::PathBuf;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "chai-rs")]
#[command(about = "Tea discovery CLI tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scrape tea data from website
    Scrape {
        /// Output JSON file path
        #[arg(short, long, default_value = "teas_data.json")]
        output: PathBuf,

        /// Limit number of teas (for testing)
        #[arg(short, long)]
        limit: Option<usize>,

        /// Only save items in stock
        #[arg(long)]
        only_available: bool,
    },

    /// Sync teas from website to database (incremental update)
    Sync {
        /// Limit number of teas (for testing)
        #[arg(short, long)]
        limit: Option<usize>,

        /// Force recreate all embeddings
        #[arg(long)]
        force: bool,

        /// Use cached HTML instead of fetching from website
        #[arg(long)]
        from_cache: bool,
    },

    /// Cache HTML pages to database
    Cache {
        /// Limit number of pages (for testing)
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Migrate JSON cache to database
    MigrateCache {
        /// Path to JSON cache file
        #[arg(short, long, default_value = "cache.json")]
        input: PathBuf,
    },

    /// Show cache statistics
    CacheStats,

    /// Search teas by description
    Search {
        /// Search query
        query: String,

        /// Number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Only items in stock
        #[arg(long)]
        only_available: bool,

        /// Filter by series
        #[arg(long)]
        series: Option<String>,
    },

    /// Get tea by URL without vector search
    Get {
        /// Tea URL
        url: String,
    },

    /// Show database statistics
    Stats,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    // Load .env
    dotenvy::dotenv().ok();

    // Initialize database
    let db_config = DbConfig::from_env();
    turso::init_database(&db_config).await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Scrape {
            output,
            limit,
            only_available,
        } => {
            scrape_command(output, limit, only_available).await?;
        }
        Commands::Sync {
            limit,
            force,
            from_cache,
        } => {
            sync_command(limit, force, from_cache).await?;
        }
        Commands::Cache { limit } => {
            cache_command(limit).await?;
        }
        Commands::MigrateCache { input } => {
            migrate_cache_command(input).await?;
        }
        Commands::CacheStats => {
            cache_stats_command().await?;
        }
        Commands::Search {
            query,
            limit,
            only_available,
            series,
        } => {
            search_command(query, limit, only_available, series).await?;
        }
        Commands::Get { url } => {
            get_command(url).await?;
        }
        Commands::Stats => {
            stats_command().await?;
        }
    }

    Ok(())
}

async fn scrape_command(output: PathBuf, limit: Option<usize>, only_available: bool) -> Result<()> {
    info!("Starting tea scraping");
    if only_available {
        info!("Filter: only items in stock");
    }

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    // Get URL list
    let urls = scraper::get_tea_urls(&client).await?;

    // Apply limit if specified
    let urls_to_scrape = if let Some(limit) = limit {
        &urls[..limit.min(urls.len())]
    } else {
        &urls
    };

    info!("Will process {} teas", urls_to_scrape.len());

    // Parse each tea
    let mut teas = Vec::new();
    let total = urls_to_scrape.len();

    for (i, url) in urls_to_scrape.iter().enumerate() {
        match scraper::scrape_tea(&client, url).await {
            Ok(tea) => {
                // Filter by availability if flag is set
                if only_available && !tea.in_stock {
                    if let Some(name) = &tea.name {
                        info!("[{}/{}] - {} (out of stock)", i + 1, total, name);
                    }
                    continue;
                }

                if let Some(name) = &tea.name {
                    info!("[{}/{}] + {}", i + 1, total, name);
                } else {
                    warn!("[{}/{}] ! No name: {}", i + 1, total, url);
                }
                teas.push(tea);
            }
            Err(e) => {
                error!("[{}/{}] x {}", i + 1, total, e);
            }
        }

        // Small delay to not overload the server
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Intermediate save every 50 teas
        if (i + 1) % 50 == 0 {
            save_teas(&teas, &output)?;
            info!("Intermediate save: {} teas", teas.len());
        }
    }

    // Final save
    save_teas(&teas, &output)?;

    info!("Done! Saved {} teas to {}", teas.len(), output.display());

    // Statistics
    let with_images = teas.iter().filter(|t| !t.images.is_empty()).count();
    let with_composition = teas.iter().filter(|t| !t.composition.is_empty()).count();

    info!("Statistics:");
    info!("  Total teas: {}", teas.len());
    info!("  With images: {}", with_images);
    info!("  With composition: {}", with_composition);

    Ok(())
}

fn save_teas(teas: &[Tea], path: &PathBuf) -> Result<()> {
    let json = serde_json::to_string_pretty(teas).context("Failed to serialize teas to JSON")?;
    std::fs::write(path, json)
        .with_context(|| format!("Failed to write teas to {}", path.display()))?;
    Ok(())
}

async fn cache_command(limit: Option<usize>) -> Result<()> {
    info!("Caching HTML pages to database");

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    // Get URL list
    let urls = scraper::get_tea_urls(&client).await?;

    // Apply limit if specified
    let urls_to_cache: Vec<String> = if let Some(limit) = limit {
        urls[..limit.min(urls.len())].to_vec()
    } else {
        urls
    };

    let total = urls_to_cache.len();
    info!("Will cache {} pages", total);

    let mut cached_count = 0;
    let mut error_count = 0;

    for (i, url) in urls_to_cache.iter().enumerate() {
        // Check if already cached
        if cache::contains(url).await? {
            info!("[{}/{}] = {} (already cached)", i + 1, total, url);
            cached_count += 1;
            continue;
        }

        match client.get(url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(html) => {
                            cache::set(url, &html).await?;
                            cached_count += 1;
                            info!("[{}/{}] + {}", i + 1, total, url);
                        }
                        Err(e) => {
                            error_count += 1;
                            error!("[{}/{}] x Read error: {}", i + 1, total, e);
                        }
                    }
                } else {
                    error_count += 1;
                    error!("[{}/{}] x HTTP {}", i + 1, total, response.status());
                }
            }
            Err(e) => {
                error_count += 1;
                error!("[{}/{}] x Request error: {}", i + 1, total, e);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Progress every 100 pages
        if (i + 1) % 100 == 0 {
            info!("Progress: {}/{} pages cached", cached_count, i + 1);
        }
    }

    info!(
        "Done! Cached {} pages, {} errors",
        cached_count, error_count
    );

    Ok(())
}

async fn migrate_cache_command(input: PathBuf) -> Result<()> {
    info!("Migrating JSON cache to database from {}", input.display());

    let count = cache::migrate_from_json(input.to_str().unwrap()).await?;

    info!("Done! Migrated {} entries to database", count);

    Ok(())
}

async fn cache_stats_command() -> Result<()> {
    let stats = cache::stats().await?;

    println!("\nCache Statistics:");
    println!("  Entries: {}", stats.entry_count);
    println!("  Total size: {} KB", stats.total_size_bytes / 1024);

    if let Some(oldest) = stats.oldest_entry {
        let dt = chrono_lite(oldest);
        println!("  Oldest entry: {}", dt);
    }

    if let Some(newest) = stats.newest_entry {
        let dt = chrono_lite(newest);
        println!("  Newest entry: {}", dt);
    }

    Ok(())
}

fn chrono_lite(timestamp: i64) -> String {
    // Simple timestamp formatting without chrono dependency
    let secs_per_day = 86400;
    let secs_per_hour = 3600;
    let secs_per_min = 60;

    let days_since_epoch = timestamp / secs_per_day;
    let remaining = timestamp % secs_per_day;
    let hours = remaining / secs_per_hour;
    let remaining = remaining % secs_per_hour;
    let minutes = remaining / secs_per_min;

    // Approximate date calculation (not accounting for leap years properly, but good enough)
    let years = 1970 + days_since_epoch / 365;
    let day_of_year = days_since_epoch % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        years, month, day, hours, minutes
    )
}

async fn sync_command(limit: Option<usize>, force: bool, from_cache: bool) -> Result<()> {
    info!("Syncing teas from website to database");

    // Create clients
    let http_client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let embeddings_config = chai_core::embeddings::EmbeddingsConfig::from_env()?;
    info!("Embeddings model: {}", embeddings_config.model);

    let embeddings_client = chai_core::embeddings::EmbeddingsClient::new(embeddings_config)?;

    // Get URL list from cache or website
    let urls = if from_cache {
        info!("Loading URLs from cache");
        cache::list_urls().await?
    } else {
        info!("Fetching URLs from sitemap");
        scraper::get_tea_urls(&http_client).await?
    };

    let urls_to_process = if let Some(limit) = limit {
        &urls[..limit.min(urls.len())]
    } else {
        &urls
    };

    info!("Will process {} teas", urls_to_process.len());

    // Get list of all URLs from database for checking deleted items
    let existing_urls = if !force {
        turso::get_all_tea_urls().await?
    } else {
        Vec::new()
    };

    // Statistics
    let mut stats = SyncStats::default();
    let total = urls_to_process.len();

    // STEP 1: Parse all products
    info!("Step 1/3: Parsing all products...");
    let mut all_teas: std::collections::HashMap<String, Tea> = std::collections::HashMap::new();
    let mut samples: Vec<String> = Vec::new();
    let mut main_products: Vec<String> = Vec::new();

    for (i, url) in urls_to_process.iter().enumerate() {
        let scrape_result = if from_cache {
            // Use cache
            match cache::get(url).await? {
                Some(entry) => scraper::parse_tea_from_html(url, &entry.html),
                None => {
                    warn!(
                        "[{}/{}] ! URL not in cache, skipping: {}",
                        i + 1,
                        total,
                        url
                    );
                    stats.errors += 1;
                    continue;
                }
            }
        } else {
            // Fetch from website
            scraper::scrape_tea(&http_client, url).await
        };

        match scrape_result {
            Ok(tea) => {
                // Save tea to map for later processing
                all_teas.insert(url.clone(), tea.clone());

                // Classify product
                if tea.is_sample {
                    if scraper::is_sample_set(url, &tea.name) {
                        main_products.push(url.clone()); // Sets are treated as main products
                    } else {
                        samples.push(url.clone());
                    }
                } else {
                    main_products.push(url.clone());
                }

                if let Some(name) = &tea.name {
                    let tea_type = if tea.is_sample {
                        if scraper::is_sample_set(url, &tea.name) {
                            "set"
                        } else {
                            "sample"
                        }
                    } else {
                        "product"
                    };
                    info!("[{}/{}] + {} ({})", i + 1, total, name, tea_type);
                }
            }
            Err(e) => {
                stats.errors += 1;
                error!("[{}/{}] x {}", i + 1, total, e);
            }
        }

        // Delay only when fetching from website (not for cache)
        if !from_cache {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        }
    }

    info!(
        "Parsing done: {} main products, {} samples\n",
        main_products.len(),
        samples.len()
    );

    // STEP 2: Link samples to main products (by name)
    info!("Step 2/3: Linking samples to main products...");
    let mut linked_count = 0;
    let mut not_linked_count = 0;

    for sample_url in &samples {
        let sample_tea = match all_teas.get(sample_url) {
            Some(tea) => tea,
            None => continue,
        };

        // Get sample name and remove prefix
        let sample_name = match &sample_tea.name {
            Some(name) => name,
            None => {
                not_linked_count += 1;
                continue;
            }
        };

        // Normalize name: remove "пробник ", "Copy: пробник ", "Copy: " at start
        let sample_lower = sample_name.trim().to_lowercase();
        let normalized_sample_name = sample_lower
            .trim_start_matches("copy: ")
            .trim_start_matches("пробник ")
            .trim();

        // Find main product with matching name
        let mut found = false;
        let mut best_match: Option<(&str, usize)> = None;

        for main_url in &main_products {
            if let Some(main_tea) = all_teas.get(main_url)
                && let Some(main_name) = &main_tea.name
            {
                let normalized_main_name = main_name.trim().to_lowercase();

                // Exact match is best
                if normalized_main_name == normalized_sample_name {
                    best_match = Some((main_url, usize::MAX));
                    break;
                }

                // For prefix matches, prefer longer matches
                let sample_len = normalized_sample_name.chars().count();
                let main_len = normalized_main_name.chars().count();
                let min_len = sample_len.min(main_len);
                let max_len = sample_len.max(main_len);

                // Require at least 80% overlap
                if min_len * 100 / max_len >= 80
                    && (normalized_main_name.starts_with(normalized_sample_name)
                        || normalized_sample_name.starts_with(&normalized_main_name))
                {
                    let match_len = min_len;
                    if best_match.is_none_or(|(_, len)| match_len > len) {
                        best_match = Some((main_url, match_len));
                    }
                }
            }
        }

        if let Some((main_url, _)) = best_match
            && let Some(main_tea_mut) = all_teas.get_mut(main_url)
        {
            main_tea_mut.sample_url = Some(sample_url.clone());
            linked_count += 1;
            found = true;
        }

        if !found {
            not_linked_count += 1;
        }
    }

    info!(
        "Linking done: {} linked, {} not found\n",
        linked_count, not_linked_count
    );

    // STEP 3: Vectorize and save only main products
    info!("Step 3/3: Vectorizing and saving to database...");

    #[derive(Clone, Copy)]
    enum UpdateType {
        Add,
        Update,
    }

    const BATCH_SIZE: usize = 50;
    let mut batch_items: Vec<(Tea, String, UpdateType)> = Vec::new();
    let mut batch_texts: Vec<String> = Vec::new();

    for (i, url) in main_products.iter().enumerate() {
        if let Some(tea) = all_teas.get(url) {
            let content_hash = tea_utils::compute_tea_hash(tea)?;

            // Check if update is needed
            let update_info = if force {
                Some(UpdateType::Update)
            } else {
                match turso::get_tea_with_hash(url).await? {
                    Some((_, existing_hash)) => {
                        if existing_hash != content_hash {
                            Some(UpdateType::Update)
                        } else {
                            stats.skipped += 1;
                            None
                        }
                    }
                    None => Some(UpdateType::Add),
                }
            };

            if let Some(update_type) = update_info {
                let text = tea_utils::tea_to_text(tea);
                batch_items.push((tea.clone(), content_hash, update_type));
                batch_texts.push(text);
            }

            // Vectorize and save batch
            if (batch_items.len() >= BATCH_SIZE || i == main_products.len() - 1)
                && !batch_items.is_empty()
            {
                info!("Vectorizing batch of {} teas...", batch_items.len());

                let embeddings = embeddings_client
                    .create_embeddings(batch_texts.clone())
                    .await
                    .context("Failed to create embeddings")?;

                // Validate embedding count
                if embeddings.len() != batch_items.len() {
                    warn!(
                        "Embedding count mismatch: expected {}, got {}",
                        batch_items.len(),
                        embeddings.len()
                    );
                }

                for ((tea, hash, update_type), embedding) in
                    batch_items.iter().zip(embeddings.iter())
                {
                    turso::upsert_tea(tea, Some(embedding.clone()), hash).await?;

                    match update_type {
                        UpdateType::Add => stats.added += 1,
                        UpdateType::Update => stats.updated += 1,
                    }
                }

                info!("Batch of {} teas processed", batch_items.len());

                batch_items.clear();
                batch_texts.clear();
            }
        }
    }

    // Delete teas that are no longer on the website
    if !force {
        let current_urls: std::collections::HashSet<_> =
            main_products.iter().map(|s| s.as_str()).collect();
        for existing_url in existing_urls {
            if !current_urls.contains(existing_url.as_str()) {
                turso::delete_tea_by_url(&existing_url).await?;
                stats.deleted += 1;
            }
        }
    }

    // Print statistics
    info!("Sync completed!");
    info!("Statistics:");
    info!("  Main products: {}", main_products.len());
    info!("  Samples linked: {}", linked_count);
    info!("  Added: {}", stats.added);
    info!("  Updated: {}", stats.updated);
    info!("  Skipped: {}", stats.skipped);
    info!("  Deleted: {}", stats.deleted);
    info!("  Errors: {}", stats.errors);

    Ok(())
}

#[derive(Default)]
struct SyncStats {
    added: usize,
    updated: usize,
    skipped: usize,
    deleted: usize,
    errors: usize,
}

async fn search_command(
    query: String,
    limit: usize,
    only_available: bool,
    series: Option<String>,
) -> Result<()> {
    info!("Search: \"{}\"", query);
    if let Some(ref s) = series {
        info!("Filter by series: {}", s);
    }

    // Create embedding for query
    let embeddings_config = chai_core::embeddings::EmbeddingsConfig::from_env()?;
    let embeddings_client = chai_core::embeddings::EmbeddingsClient::new(embeddings_config)?;

    info!("Creating embedding for query...");
    let query_embedding = embeddings_client.create_embedding(query.clone()).await?;

    // Create filters
    let filters = turso::SearchFilters {
        exclude_samples: false,
        exclude_sets: false,
        only_in_stock: only_available,
        series,
    };

    // Execute search
    info!("Searching similar teas...");
    let results = turso::search_teas(&query_embedding, limit, &filters).await?;

    // Print results
    if results.is_empty() {
        warn!("No results found");
        return Ok(());
    }

    info!("Found {} results:\n", results.len());

    for (i, result) in results.iter().enumerate() {
        let tea = &result.tea;
        let score = result.score;

        let relevance = (score * 100.0).round() as u32;

        let emoji = if relevance >= 90 {
            "**"
        } else if relevance >= 75 {
            "*"
        } else {
            ""
        };

        println!(
            "{}. {}{} ({}%)",
            i + 1,
            emoji,
            tea.name.as_deref().unwrap_or("No name"),
            relevance
        );

        if let Some(price) = &tea.price {
            let stock = if tea.in_stock {
                "In stock"
            } else {
                "Out of stock"
            };
            println!("   Price: {} | {}", price, stock);
        }

        if let Some(series) = &tea.series {
            println!("   Series: {}", series);
        }

        if !tea.composition.is_empty() {
            let ingredients: Vec<_> = tea.composition.iter().take(5).collect();
            let more = if tea.composition.len() > 5 {
                format!(" (+{})", tea.composition.len() - 5)
            } else {
                String::new()
            };
            println!(
                "   Composition: {}{}",
                ingredients
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                more
            );
        }

        println!("   URL: {}", tea.url);
        println!();
    }

    Ok(())
}

async fn get_command(url: String) -> Result<()> {
    info!("Getting tea by URL: {}", url);

    let tea_option = turso::get_tea_with_hash(&url).await?;

    match tea_option {
        Some((tea, content_hash)) => {
            info!("Tea found\n");

            println!("Name: {}", tea.name.as_deref().unwrap_or("No name"));
            println!("URL: {}", tea.url);

            if let Some(sample_url) = &tea.sample_url {
                println!("Sample URL: {}", sample_url);
            }

            if let Some(price) = &tea.price {
                let stock = if tea.in_stock {
                    "In stock"
                } else {
                    "Out of stock"
                };
                println!("Price: {} | {}", price, stock);
            }

            if let Some(series) = &tea.series {
                println!("Series: {}", series);
            }

            if !tea.composition.is_empty() {
                println!("Composition: {}", tea.composition.join(", "));
            }

            if !tea.full_composition.is_empty() {
                println!("Full composition: {}", tea.full_composition.join(", "));
            }

            if let Some(description) = &tea.description {
                println!("Description: {}", description);
            }

            if !tea.price_variants.is_empty() {
                println!("Price variants:");
                for variant in &tea.price_variants {
                    println!(
                        "   {} - {} ({})",
                        variant.packaging, variant.price, variant.quantity
                    );
                }
            }

            if !tea.images.is_empty() {
                println!("Images: {} total", tea.images.len());
            }

            if !tea.search_tags.is_empty() {
                println!("Tags: {}", tea.search_tags.join(", "));
            }

            println!("Is sample: {}", tea.is_sample);
            println!("Content hash: {}", content_hash);

            Ok(())
        }
        None => {
            warn!("Tea with URL {} not found", url);
            Ok(())
        }
    }
}

async fn stats_command() -> Result<()> {
    info!("Getting statistics");

    let stats = turso::get_stats().await?;

    println!("\n=== Tea Database Statistics ===\n");

    println!("General:");
    println!("  Total teas: {}", stats.total_teas);
    println!("  In stock: {}", stats.in_stock);
    println!("  Out of stock: {}", stats.out_of_stock);

    if stats.total_teas > 0 {
        let in_stock_percent = (stats.in_stock as f32 / stats.total_teas as f32 * 100.0).round();
        println!("  In stock %: {}%", in_stock_percent);
    }

    println!("\nSeries:");
    println!("  Total series: {}", stats.series_count);

    if !stats.series_list.is_empty() {
        println!("\n  Series list:");
        for (i, series) in stats.series_list.iter().enumerate() {
            println!("    {}. {}", i + 1, series);
        }
    }

    // Cache stats
    if let Ok(cache_stats) = cache::stats().await {
        println!("\nCache:");
        println!("  Entries: {}", cache_stats.entry_count);
        println!("  Size: {} KB", cache_stats.total_size_bytes / 1024);
    }

    println!();

    Ok(())
}
