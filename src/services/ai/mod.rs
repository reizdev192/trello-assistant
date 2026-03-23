pub mod openai;
pub mod prompts;

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::config::Config;

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String>;
    fn name(&self) -> &str;
    async fn is_available(&self) -> bool;
}

pub fn create_providers(config: &Config) -> Vec<Arc<dyn AiProvider>> {
    let provider = Arc::new(openai::OpenAiProvider::new(
        config.ai_base_url.clone(),
        config.ai_api_key.clone(),
        config.ai_model.clone(),
    ));
    vec![provider]
}
