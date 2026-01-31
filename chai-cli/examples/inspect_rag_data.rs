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

    let queries = vec!["Кислый чай с облепихой", "Успокаивающий чай на ночь"];

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              🔍 ИНСПЕКЦИЯ ДАННЫХ ИЗ БАЗЫ ДАННЫХ               ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    for query in queries {
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
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
                "{}. {} (score: {:.3})",
                i + 1,
                tea.name.as_deref().unwrap_or("Без названия"),
                result.score
            );
            println!("   🔗 URL: {}", tea.url);

            if let Some(desc) = &tea.description {
                println!("   📄 Описание ({} символов):", desc.len());
                println!("   {}", desc);
            } else {
                println!("   📄 Описание: ❌ НЕТ");
            }

            if !tea.composition.is_empty() {
                println!("   🧪 Состав: {}", tea.composition.join(", "));
            } else {
                println!("   🧪 Состав: ❌ НЕТ");
            }

            if !tea.full_composition.is_empty() {
                println!("   🧪 Полный состав: {}", tea.full_composition.join(", "));
            }

            if let Some(series) = &tea.series {
                println!("   📚 Серия: {}", series);
            }

            if !tea.search_tags.is_empty() {
                println!("   🏷️  Search tags: {}", tea.search_tags.join(", "));
            }

            if let Some(price) = &tea.price {
                println!("   💰 Цена: {}", price);
            }

            println!(
                "   📦 В наличии: {}",
                if tea.in_stock { "✅" } else { "❌" }
            );

            println!();
        }

        println!("{}", "═".repeat(64));
    }

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║                        📊 ВЫВОДЫ                               ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    println!("Анализ данных из базы данных показывает:");
    println!("1. Какие поля заполнены у всех чаёв");
    println!("2. Качество описаний (если есть)");
    println!("3. Полнота данных о составе");
    println!("4. Нужно ли генерировать описания через LLM или достаточно данных из БД");

    Ok(())
}
