#[cfg(feature = "ssr")]
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(feature = "ssr")]
pub const GIT_HASH: &str = env!("GIT_HASH");
#[cfg(feature = "ssr")]
pub const BUILD_TIME: &str = env!("BUILD_TIME");

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::response::Json;
    use axum::{Router, routing::get};
    use axum_governor::GovernorLayer;
    use chai_core::{DbConfig, turso};
    use chai_web::app::App;
    use lazy_limit::{Duration, RuleConfig, init_rate_limiter};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use real::RealIpLayer;
    use serde_json::json;
    use std::net::SocketAddr;
    use tower_http::cors::{AllowOrigin, CorsLayer};
    use tower_http::services::ServeDir;

    // Load .env
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    tracing::info!(
        "Starting Tea Advisor v{}-{} (built {})",
        VERSION,
        GIT_HASH,
        BUILD_TIME
    );

    // Validate required environment variables
    if std::env::var("JWT_SECRET").is_err() {
        return Err("JWT_SECRET environment variable is not set. Add it to .env file.".into());
    }
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        tracing::warn!("OPENROUTER_API_KEY not set - AI features will not work");
    }

    // Initialize Turso database
    let db_config = DbConfig::from_env();
    turso::init_database(&db_config).await?;
    tracing::info!("Database initialized at {}", db_config.path);

    // Initialize rate limiter: 10 requests per second globally, 2 req/sec for AI endpoint
    init_rate_limiter!(
        default: RuleConfig::new(Duration::seconds(1), 10),
        routes: [
            ("/api/*", RuleConfig::new(Duration::seconds(1), 2)),
        ]
    )
    .await;
    tracing::info!("Rate limiting enabled: 10 req/s global, 2 req/s for /api/*");

    // Leptos configuration
    let conf = get_configuration(None).expect("Failed to load Leptos configuration");
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    // Version endpoint handler
    async fn version_handler() -> Json<serde_json::Value> {
        Json(json!({
            "version": VERSION,
            "git_hash": GIT_HASH,
            "build_time": BUILD_TIME
        }))
    }

    // Build Axum router with rate limiting
    let app = Router::new()
        .route("/api/version", get(version_handler))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || {
                use leptos::prelude::*;
                use leptos_meta::MetaTags;

                view! {
                    <!DOCTYPE html>
                    <html lang="ru">
                        <head>
                            <meta charset="utf-8" />
                            <meta name="viewport" content="width=device-width, initial-scale=1" />
                            <AutoReload options=leptos_options.clone() />
                            <HydrationScripts options=leptos_options.clone() />
                            <MetaTags />
                            <link rel="stylesheet" href="/pkg/chai-web.css" />
                        </head>
                        <body>
                            <App />
                        </body>
                    </html>
                }
            }
        })
        .fallback_service(ServeDir::new(leptos_options.site_root.as_ref()))
        .layer(
            tower::ServiceBuilder::new()
                .layer(RealIpLayer::default())
                .layer(GovernorLayer::default())
                .layer(
                    CorsLayer::new()
                        .allow_origin(AllowOrigin::list([
                            "https://chai.okhsunrog.ru".parse().unwrap(),
                            "http://localhost:3000".parse().unwrap(), // For local development
                            "http://127.0.0.1:3000".parse().unwrap(),
                        ]))
                        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                        .allow_headers([axum::http::header::CONTENT_TYPE]),
                ),
        )
        .with_state(leptos_options);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    tracing::info!("Server running at http://{}", addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // Client-side main is empty - everything is managed via wasm
}
