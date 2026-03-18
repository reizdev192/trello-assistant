pub mod gemini;
pub mod ollama;
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

pub async fn chat_with_fallback(
    providers: &[Arc<dyn AiProvider>],
    system_prompt: &str,
    user_message: &str,
) -> Result<(String, String)> {
    for provider in providers {
        if !provider.is_available().await {
            tracing::warn!("AI provider '{}' is not available, skipping", provider.name());
            continue;
        }

        match provider.chat(system_prompt, user_message).await {
            Ok(response) => {
                tracing::info!("AI response from provider '{}'", provider.name());
                return Ok((response, provider.name().to_string()));
            }
            Err(e) => {
                tracing::warn!(
                    "AI provider '{}' failed: {}, trying next...",
                    provider.name(),
                    e
                );
            }
        }
    }

    anyhow::bail!("Tất cả AI providers đều không khả dụng. Vui lòng kiểm tra cấu hình Gemini API key hoặc Ollama service.")
}

pub fn create_providers(config: &Config) -> Vec<Arc<dyn AiProvider>> {
    let mut providers: Vec<Arc<dyn AiProvider>> = Vec::new();

    match config.ai_provider.as_str() {
        "gemini" => {
            if let Some(ref api_key) = config.gemini_api_key {
                providers.push(Arc::new(gemini::GeminiProvider::new(
                    api_key.clone(),
                    config.gemini_model.clone(),
                )));
            }
        }
        "ollama" => {
            providers.push(Arc::new(ollama::OllamaProvider::new(
                config.ollama_url.clone(),
                config.ollama_model.clone(),
            )));
        }
        _ => {
            // "auto" mode: Gemini first, Ollama fallback
            if let Some(ref api_key) = config.gemini_api_key {
                providers.push(Arc::new(gemini::GeminiProvider::new(
                    api_key.clone(),
                    config.gemini_model.clone(),
                )));
            }
            providers.push(Arc::new(ollama::OllamaProvider::new(
                config.ollama_url.clone(),
                config.ollama_model.clone(),
            )));
        }
    }

    providers
}
