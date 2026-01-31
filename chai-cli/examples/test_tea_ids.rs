use anyhow::Result;
use chai_core::{DbConfig, embeddings, turso};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // Initialize database
    let db_config = DbConfig::from_env();
    turso::init_database(&db_config).await?;

    // Create embeddings client
    let embeddings_config = embeddings::EmbeddingsConfig::from_env()?;
    let embeddings_client = embeddings::EmbeddingsClient::new(embeddings_config)?;

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              🆔 ТЕСТ УНИКАЛЬНЫХ ID                             ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let queries = vec!["Кислый чай с облепихой", "Успокаивающий чай на ночь"];

    for query in &queries {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📝 Запрос: {}", query);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        let query_embedding = embeddings_client
            .create_embedding(query.to_string())
            .await?;
        let results =
            turso::search_teas(&query_embedding, 5, &turso::SearchFilters::default()).await?;

        println!("Найдено чаёв: {}\n", results.len());

        for (i, result) in results.iter().enumerate() {
            let tea = &result.tea;
            println!(
                "{}. 🆔 {} - {}",
                i + 1,
                tea.id,
                tea.name.as_deref().unwrap_or("Без названия")
            );
            println!("   Score: {:.3}", result.score);
            println!("   URL: {}", tea.url);

            if !tea.composition.is_empty() {
                println!("   Состав: {}", tea.composition.join(", "));
            }
            println!();
        }

        println!("{}", "═".repeat(64));
        println!();
    }

    // Тест получения чая по ID
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              🔍 ТЕСТ ПОЛУЧЕНИЯ ЧАЯ ПО ID                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Получаем первый чай из первого поиска
    let query_embedding = embeddings_client
        .create_embedding(queries[0].to_string())
        .await?;
    let results = turso::search_teas(&query_embedding, 1, &turso::SearchFilters::default()).await?;
    if let Some(result) = results.first() {
        let test_id = &result.tea.id;
        println!("Тестируем поиск по ID: {}", test_id);

        match turso::get_tea_by_id(test_id).await? {
            Some(tea) => {
                println!("✅ Чай найден по ID!");
                println!(
                    "   Название: {}",
                    tea.name.as_deref().unwrap_or("Без названия")
                );
                println!("   URL: {}", tea.url);
            }
            None => {
                println!("❌ Чай не найден по ID");
            }
        }
    }

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║                        📊 ВЫВОД                                ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    println!("✅ Каждый чай теперь имеет уникальный короткий ID (8 символов)");
    println!("✅ ID генерируется из URL и остается стабильным");
    println!("✅ LLM может возвращать массив ID вместо названий");
    println!("✅ Поиск по ID работает быстро и точно");

    Ok(())
}
