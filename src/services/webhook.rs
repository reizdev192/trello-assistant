use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::config::Config;

pub struct WebhookService {
    client: Client,
    api_key: String,
    token: String,
    board_id: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhookInfo {
    pub id: String,
    #[serde(rename = "callbackURL")]
    pub callback_url: String,
    pub active: bool,
}

impl WebhookService {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            api_key: config.trello_api_key.clone(),
            token: config.trello_token.clone(),
            board_id: config.trello_board_id.clone(),
        }
    }

    /// Register a webhook for the board with the given callback URL
    pub async fn register(&self, callback_url: &str) -> Result<WebhookInfo> {
        // Resolve full board ID (shortLink won't work for webhook idModel)
        let board_id = self.resolve_board_id().await?;

        // First, check if a webhook already exists for this board
        if let Ok(existing) = self.list_webhooks().await {
            for wh in &existing {
                if wh.callback_url == callback_url && wh.active {
                    tracing::info!("♻️  Webhook already registered: {}", wh.id);
                    return Ok(WebhookInfo {
                        id: wh.id.clone(),
                        callback_url: wh.callback_url.clone(),
                        active: wh.active,
                    });
                }
            }
            // Clean up old webhooks for same callback
            for wh in &existing {
                if wh.callback_url == callback_url && !wh.active {
                    let _ = self.delete(&wh.id).await;
                }
            }
        }

        let url = format!(
            "https://api.trello.com/1/webhooks/?key={}&token={}",
            self.api_key, self.token
        );

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "callbackURL": callback_url,
                "idModel": board_id,
                "description": "Trello Assistant Real-time Sync",
                "active": true
            }))
            .send()
            .await
            .context("Failed to register webhook with Trello")?;

        if response.status().is_success() {
            let info: WebhookInfo = response.json().await
                .context("Failed to parse webhook response")?;
            tracing::info!("✅ Webhook registered: {} → {}", info.id, callback_url);
            Ok(info)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to register webhook: {} - {}", status, body)
        }
    }

    /// Resolve shortLink to full board ID
    async fn resolve_board_id(&self) -> Result<String> {
        let url = format!(
            "https://api.trello.com/1/boards/{}?key={}&token={}&fields=id",
            self.board_id, self.api_key, self.token
        );
        let resp = self.client.get(&url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        json["id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Could not resolve board ID"))
    }

    /// List all webhooks for this token
    pub async fn list_webhooks(&self) -> Result<Vec<WebhookInfo>> {
        let url = format!(
            "https://api.trello.com/1/tokens/{}/webhooks?key={}",
            self.token, self.api_key
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to list webhooks")?;

        if response.status().is_success() {
            let webhooks: Vec<WebhookInfo> = response.json().await?;
            Ok(webhooks)
        } else {
            Ok(Vec::new())
        }
    }

    /// Delete a webhook by ID
    pub async fn delete(&self, webhook_id: &str) -> Result<()> {
        let url = format!(
            "https://api.trello.com/1/webhooks/{}?key={}&token={}",
            webhook_id, self.api_key, self.token
        );

        self.client.delete(&url).send().await?;
        tracing::info!("🗑️  Deleted webhook: {}", webhook_id);
        Ok(())
    }
}
