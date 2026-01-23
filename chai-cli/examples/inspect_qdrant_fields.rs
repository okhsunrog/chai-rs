use anyhow::Result;
use chai_core::{Config, qdrant};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;

    let results = qdrant::search_teas("облепиха", 3, &config).await?;

    for (i, result) in results.iter().enumerate() {
        let tea = &result.tea;

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Чай #{}", i + 1);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        println!("id: {}", tea.id);
        println!("url: {}", tea.url);
        println!("name: {:?}", tea.name);
        println!("price: {:?}", tea.price);
        println!("in_stock: {}", tea.in_stock);
        println!("is_sample: {}", tea.is_sample);
        println!();

        println!("composition ({}):", tea.composition.len());
        for item in &tea.composition {
            println!("  - {}", item);
        }
        println!();

        println!("full_composition ({}):", tea.full_composition.len());
        for item in &tea.full_composition {
            println!("  - {}", item);
        }
        println!();

        if let Some(desc) = &tea.description {
            if desc.chars().count() > 300 {
                let short: String = desc.chars().take(300).collect();
                println!("description: \"{}...\" ({} символов)", short, desc.len());
            } else {
                println!("description: {:?}", desc);
            }
        } else {
            println!("description: None");
        }
        println!();

        println!("series: {:?}", tea.series);
        println!("images ({}): {:?}", tea.images.len(), tea.images.first());
        println!("search_tags: {:?}", tea.search_tags);
        println!("sample_url: {:?}", tea.sample_url);
        println!();

        println!("price_variants ({}):", tea.price_variants.len());
        for pv in &tea.price_variants {
            println!("  - {}: {} ({})", pv.packaging, pv.price, pv.quantity);
        }
        println!();

        println!("volume_options: {:?}", tea.volume_options);
        println!("storage_info: {:?}", tea.storage_info);
        println!("dimensions: {:?}", tea.dimensions);
        println!("weight: {:?}", tea.weight);
    }

    Ok(())
}
