use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    // Trello
    pub trello_api_key: String,
    pub trello_token: String,
    pub trello_board_id: String,

    // AI Provider
    pub ai_provider: String,

    // Gemini
    pub gemini_api_key: Option<String>,
    pub gemini_model: String,

    // Ollama
    pub ollama_url: String,
    pub ollama_model: String,

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

            ai_provider: env::var("AI_PROVIDER").unwrap_or_else(|_| "auto".to_string()),

            gemini_api_key: env::var("GEMINI_API_KEY").ok().filter(|s| !s.is_empty()),
            gemini_model: env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-2.0-flash".to_string()),

            ollama_url: env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            ollama_model: env::var("OLLAMA_MODEL")
                .unwrap_or_else(|_| "qwen3.5:0.8b".to_string()),

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
