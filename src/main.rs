mod config;
mod models;
mod routes;
mod services;

use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;

use axum::{
    routing::{delete, get, head, post, put},
    Router,
};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use routes::settings::StoredWebhookInfo;
use services::ai::AiProvider;
use services::cache::CacheService;
use services::trello::TrelloService;
use services::webhook::WebhookService;

pub struct AppState {
    pub trello: TrelloService,
    pub cache: CacheService,
    pub ai_providers: Vec<Arc<dyn AiProvider>>,
    pub webhook_service: WebhookService,
    pub last_sync: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    pub webhook_info: Mutex<Option<StoredWebhookInfo>>,
}

/// Check if Ollama process is reachable
async fn is_ollama_running(url: &str) -> bool {
    let client = reqwest::Client::new();
    client
        .get(format!("{}/api/tags", url))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

/// Check if `ollama` binary exists on the system
fn is_ollama_installed() -> bool {
    std::process::Command::new("which")
        .arg("ollama")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Auto-start Ollama server if it's installed but not running
async fn ensure_ollama_running(url: &str) -> bool {
    if is_ollama_running(url).await {
        tracing::info!("✅ Ollama is already running");
        return true;
    }

    if !is_ollama_installed() {
        tracing::warn!("⚠️  Ollama is not installed. Install it from https://ollama.com");
        tracing::warn!("   macOS: brew install ollama  |  Linux: curl -fsSL https://ollama.com/install.sh | sh");
        return false;
    }

    tracing::info!("🔄 Ollama is installed but not running. Starting 'ollama serve'...");

    let child = std::process::Command::new("ollama")
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match child {
        Ok(_) => {
            tracing::info!("🔄 Spawned 'ollama serve' process. Waiting for it to be ready...");
            for i in 1..=15 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if is_ollama_running(url).await {
                    tracing::info!("✅ Ollama started successfully (took {}s)", i);
                    return true;
                }
            }
            tracing::warn!("⚠️  Ollama process spawned but still not responding after 15s");
            false
        }
        Err(e) => {
            tracing::error!("❌ Failed to start Ollama: {}", e);
            false
        }
    }
}

/// Detect ngrok tunnel URL for webhook callback
async fn detect_ngrok_url(server_port: u16) -> Option<String> {
    // Try ngrok API (default at localhost:4040)
    let client = reqwest::Client::new();
    if let Ok(resp) = client
        .get("http://localhost:4040/api/tunnels")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(tunnels) = json["tunnels"].as_array() {
                for tunnel in tunnels {
                    if let Some(public_url) = tunnel["public_url"].as_str() {
                        if public_url.starts_with("https://") {
                            tracing::info!("🌐 Detected ngrok tunnel: {}", public_url);
                            return Some(public_url.to_string());
                        }
                    }
                }
            }
        }
    }

    tracing::warn!(
        "⚠️  ngrok not detected on port {}. Start it with: ngrok http {}",
        server_port,
        server_port
    );
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("🚀 Starting Trello Assistant...");

    // Load configuration
    let config = config::Config::from_env()?;
    tracing::info!("✅ Configuration loaded");

    // Initialize services
    let trello = TrelloService::new(&config);
    tracing::info!("✅ Trello service initialized (board: {})", config.trello_board_id);

    let cache = CacheService::new(&config)?;
    tracing::info!("✅ Redis cache initialized");

    // --- Auto-start Ollama if needed ---
    let needs_ollama = config.ai_provider == "ollama" || config.ai_provider == "auto";
    if needs_ollama {
        let ollama_ready = ensure_ollama_running(&config.ollama_url).await;
        if ollama_ready {
            let ollama = services::ai::ollama::OllamaProvider::new(
                config.ollama_url.clone(),
                config.ollama_model.clone(),
            );
            match ollama.ensure_model().await {
                Ok(()) => tracing::info!("✅ Ollama model '{}' is ready", config.ollama_model),
                Err(e) => tracing::warn!("⚠️  Ollama model check failed: {}", e),
            }
        }
    }

    // Initialize AI providers
    let ai_providers = services::ai::create_providers(&config);
    tracing::info!("✅ AI providers initialized ({} providers)", ai_providers.len());

    // --- Initial Trello data sync to Redis ---
    tracing::info!("📦 Syncing Trello board data to Redis...");
    match trello.fetch_board_data().await {
        Ok(board_data) => {
            let card_count = board_data.cards.len();
            if let Err(e) = cache.bulk_sync(&board_data).await {
                tracing::error!("❌ Failed to sync board data: {}", e);
            } else {
                tracing::info!("✅ Synced {} cards to Redis", card_count);
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to fetch board data: {}", e);
        }
    }

    // Build webhook service
    let webhook_service = WebhookService::new(&config);

    // Build application state
    let state = Arc::new(AppState {
        trello,
        cache,
        ai_providers,
        webhook_service,
        last_sync: Mutex::new(Some(chrono::Utc::now())),
        webhook_info: Mutex::new(None),
    });

    // Build router
    let api_routes = Router::new()
        .route("/chat", post(routes::chat::chat_handler))
        .route("/cards", get(routes::cards::list_cards_handler))
        .route("/cards/refresh", post(routes::cards::refresh_cards_handler))
        .route("/health", get(routes::health::health_handler))
        .route("/settings", get(routes::settings::get_settings))
        .route("/settings/webhook", post(routes::settings::register_webhook))
        .route("/settings/webhook/{id}", put(routes::settings::update_webhook_handler))
        .route("/settings/webhook/{id}", delete(routes::settings::delete_webhook_handler))
        .route("/members", get(routes::settings::get_members))
        .route("/lists", get(routes::settings::get_lists))
        .route("/labels", get(routes::settings::get_labels))
        .route("/stats", get(routes::settings::get_stats))
        .route("/webhook", head(routes::webhook::webhook_head_handler))
        .route("/webhook", post(routes::webhook::webhook_post_handler));

    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new("dist"))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.server_host, config.server_port)
        .parse()
        .expect("Invalid server address");

    tracing::info!("🌐 Server listening on http://{}", addr);

    // --- Register Trello webhook (after server is ready) ---
    let webhook_config = config.clone();
    tokio::spawn(async move {
        // Give the server a moment to start
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let callback_url = if let Some(url) = &webhook_config.webhook_url {
            Some(url.clone())
        } else {
            detect_ngrok_url(webhook_config.server_port).await
        };

        if let Some(base_url) = callback_url {
            let callback = format!("{}/api/webhook", base_url);
            match state.webhook_service.register(&callback).await {
                Ok(info) => {
                    tracing::info!("✅ Webhook registered: {} (ID: {})", callback, info.id);
                    let mut lock = state.webhook_info.lock().await;
                    *lock = Some(StoredWebhookInfo {
                        id: info.id,
                        url: info.callback_url,
                        active: info.active,
                    });
                }
                Err(e) => tracing::error!("❌ Failed to register webhook: {}", e),
            }
        } else {
            tracing::warn!("⚠️  No WEBHOOK_URL set and ngrok not detected. Webhook not registered.");
            tracing::warn!("   Set WEBHOOK_URL in .env or start ngrok: ngrok http {}", webhook_config.server_port);
        }
    });

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
