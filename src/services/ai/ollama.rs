use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::AiProvider;

pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            model,
        }
    }

    /// Check if the model exists locally in Ollama, if not pull it automatically
    pub async fn ensure_model(&self) -> Result<()> {
        if self.model_exists().await? {
            tracing::info!("Ollama model '{}' is ready", self.model);
            return Ok(());
        }

        tracing::info!(
            "Ollama model '{}' not found locally. Pulling... (this may take a few minutes)",
            self.model
        );

        let url = format!("{}/api/pull", self.base_url);
        let request = OllamaPullRequest {
            name: self.model.clone(),
            stream: false,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Ollama for model pull")?;

        if response.status().is_success() {
            tracing::info!("Successfully pulled Ollama model '{}'", self.model);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Failed to pull Ollama model '{}': {} - {}",
                self.model,
                status,
                body
            )
        }
    }

    async fn model_exists(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to Ollama")?
            .json::<OllamaTagsResponse>()
            .await
            .context("Failed to parse Ollama tags")?;

        Ok(response
            .models
            .iter()
            .any(|m| m.name == self.model || m.name == format!("{}:latest", self.model)))
    }
}

#[derive(Serialize)]
struct OllamaPullRequest {
    name: String,
    stream: bool,
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaMessage>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelInfo>,
}

#[derive(Deserialize)]
struct OllamaModelInfo {
    name: String,
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            stream: false,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Ollama")?
            .error_for_status()
            .context("Ollama returned error")?
            .json::<OllamaChatResponse>()
            .await
            .context("Failed to parse Ollama response")?;

        let text = response
            .message
            .map(|m| m.content)
            .unwrap_or_else(|| "Không nhận được phản hồi từ Ollama.".to_string());

        Ok(text)
    }

    fn name(&self) -> &str {
        "ollama"
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.client.get(&url).send().await.is_ok()
    }
}
