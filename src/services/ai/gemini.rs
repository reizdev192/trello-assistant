use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::AiProvider;

pub struct GeminiProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }
}

#[derive(Serialize)]
struct GeminiRequest {
    system_instruction: GeminiContent,
    contents: Vec<GeminiContent>,
}

#[derive(Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let request = GeminiRequest {
            system_instruction: GeminiContent {
                parts: vec![GeminiPart {
                    text: system_prompt.to_string(),
                }],
            },
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: user_message.to_string(),
                }],
            }],
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to call Gemini API")?
            .error_for_status()
            .context("Gemini API returned error")?
            .json::<GeminiResponse>()
            .await
            .context("Failed to parse Gemini response")?;

        let text = response
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content.parts.into_iter().next())
            .map(|p| p.text)
            .unwrap_or_else(|| "Không nhận được phản hồi từ Gemini.".to_string());

        Ok(text)
    }

    fn name(&self) -> &str {
        "gemini"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}
