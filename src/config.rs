use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    // Trello
    pub trello_api_key: String,
    pub trello_token: String,
    pub trello_board_id: String,

    // AI (OpenAI-compatible API)
    pub ai_base_url: String,
    pub ai_api_key: String,
    pub ai_model: String,

    // Redis
    pub redis_url: String,
    pub cache_ttl_seconds: u64,

    // Webhook
    pub webhook_url: Option<String>,

    // Server
    pub server_host: String,
    pub server_port: u16,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            trello_api_key: env::var("API_TRELLO_KEY")
                .map_err(|_| anyhow::anyhow!("API_TRELLO_KEY is required"))?,
            trello_token: env::var("TRELLO_TOKEN")
                .map_err(|_| anyhow::anyhow!("TRELLO_TOKEN is required"))?,
            trello_board_id: env::var("TRELLO_BOARD_ID")
                .map_err(|_| anyhow::anyhow!("TRELLO_BOARD_ID is required"))?,

            ai_base_url: env::var("AI_BASE_URL")
                .map_err(|_| anyhow::anyhow!("AI_BASE_URL is required"))?,
            ai_api_key: env::var("AI_API_KEY")
                .map_err(|_| anyhow::anyhow!("AI_API_KEY is required"))?,
            ai_model: env::var("AI_MODEL")
                .unwrap_or_else(|_| "gemini-2.5-flash-lite".to_string()),

            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            cache_ttl_seconds: env::var("CACHE_TTL_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300),

            webhook_url: env::var("WEBHOOK_URL").ok().filter(|s| !s.is_empty()),

            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap_or(3000),
        })
    }
}
