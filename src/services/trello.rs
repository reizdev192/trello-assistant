use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::Config;
use crate::models::card::{BoardData, TrelloBoard, TrelloCard, TrelloList};

const TRELLO_API_BASE: &str = "https://api.trello.com/1";

#[derive(Clone)]
pub struct TrelloService {
    client: Client,
    api_key: String,
    token: String,
    board_id: String,
}

impl TrelloService {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            api_key: config.trello_api_key.clone(),
            token: config.trello_token.clone(),
            board_id: config.trello_board_id.clone(),
        }
    }

    fn auth_params(&self) -> Vec<(&str, &str)> {
        vec![("key", &self.api_key), ("token", &self.token)]
    }

    pub async fn fetch_board(&self) -> Result<TrelloBoard> {
        let url = format!("{}/boards/{}", TRELLO_API_BASE, self.board_id);
        let board = self
            .client
            .get(&url)
            .query(&self.auth_params())
            .query(&[("fields", "id,name,desc")])
            .send()
            .await
            .context("Failed to connect to Trello API")?
            .error_for_status()
            .context("Trello API returned error for board")?
            .json::<TrelloBoard>()
            .await
            .context("Failed to parse board response")?;

        Ok(board)
    }

    pub async fn fetch_lists(&self) -> Result<Vec<TrelloList>> {
        let url = format!("{}/boards/{}/lists", TRELLO_API_BASE, self.board_id);
        let lists = self
            .client
            .get(&url)
            .query(&self.auth_params())
            .query(&[("fields", "id,name")])
            .send()
            .await
            .context("Failed to fetch lists")?
            .error_for_status()
            .context("Trello API returned error for lists")?
            .json::<Vec<TrelloList>>()
            .await
            .context("Failed to parse lists response")?;

        Ok(lists)
    }

    pub async fn fetch_cards(&self) -> Result<Vec<TrelloCard>> {
        let url = format!("{}/boards/{}/cards", TRELLO_API_BASE, self.board_id);
        let cards = self
            .client
            .get(&url)
            .query(&self.auth_params())
            .query(&[(
                "fields",
                "id,name,desc,idList,due,dueComplete,labels,shortUrl",
            ), (
                "members", "true",
            ), (
                "member_fields", "id,fullName,username",
            )])
            .send()
            .await
            .context("Failed to fetch cards")?
            .error_for_status()
            .context("Trello API returned error for cards")?
            .json::<Vec<TrelloCard>>()
            .await
            .context("Failed to parse cards response")?;

        Ok(cards)
    }

    pub async fn fetch_board_data(&self) -> Result<BoardData> {
        let (board, lists, mut cards) =
            tokio::try_join!(self.fetch_board(), self.fetch_lists(), self.fetch_cards())?;

        let list_map: HashMap<String, String> =
            lists.iter().map(|l| (l.id.clone(), l.name.clone())).collect();

        for card in &mut cards {
            card.list_name = list_map.get(&card.id_list).cloned();
        }

        Ok(BoardData {
            board,
            lists,
            cards,
        })
    }

    /// Fetch a single card by ID (used by webhook handler)
    pub async fn fetch_card(&self, card_id: &str) -> Result<TrelloCard> {
        let url = format!("{}/cards/{}", TRELLO_API_BASE, card_id);
        let mut card = self
            .client
            .get(&url)
            .query(&self.auth_params())
            .query(&[("fields", "id,name,desc,idList,due,dueComplete,labels,shortUrl")])
            .query(&[("members", "true"), ("member_fields", "id,fullName,username")])
            .send()
            .await
            .context("Failed to fetch card")?
            .error_for_status()
            .context("Trello API returned error for card")?
            .json::<TrelloCard>()
            .await
            .context("Failed to parse card response")?;

        // Resolve list name
        if let Ok(lists) = self.fetch_lists().await {
            let list_map: HashMap<String, String> =
                lists.iter().map(|l| (l.id.clone(), l.name.clone())).collect();
            card.list_name = list_map.get(&card.id_list).cloned();
        }

        Ok(card)
    }

    pub async fn health_check(&self) -> bool {
        self.fetch_board().await.is_ok()
    }

    /// Fetch all webhooks registered for this token
    pub async fn fetch_webhooks(&self) -> Result<Vec<TrelloWebhook>> {
        let url = format!("{}/tokens/{}/webhooks", TRELLO_API_BASE, self.token);
        let webhooks: Vec<TrelloWebhook> = self
            .client
            .get(&url)
            .query(&self.auth_params())
            .send()
            .await
            .context("Failed to fetch webhooks")?
            .json()
            .await
            .context("Failed to parse webhooks response")?;
        Ok(webhooks)
    }

    /// Update a webhook by ID (PUT /1/webhooks/{id})
    pub async fn update_webhook(&self, webhook_id: &str, update: &TrelloWebhookUpdate) -> Result<TrelloWebhook> {
        let url = format!("{}/webhooks/{}", TRELLO_API_BASE, webhook_id);
        let mut params: Vec<(&str, String)> = vec![
            ("key", self.api_key.clone()),
            ("token", self.token.clone()),
        ];
        if let Some(ref desc) = update.description {
            params.push(("description", desc.clone()));
        }
        if let Some(ref cb) = update.callback_url {
            params.push(("callbackURL", cb.clone()));
        }
        if let Some(active) = update.active {
            params.push(("active", active.to_string()));
        }

        let resp = self.client.put(&url).query(&params).send().await
            .context("Failed to update webhook")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Trello API error {}: {}", status, body);
        }

        let webhook: TrelloWebhook = resp.json().await.context("Failed to parse webhook response")?;
        Ok(webhook)
    }

    /// Delete a webhook by ID (DELETE /1/webhooks/{id})
    pub async fn delete_webhook_by_id(&self, webhook_id: &str) -> Result<()> {
        let url = format!("{}/webhooks/{}", TRELLO_API_BASE, webhook_id);
        let resp = self.client.delete(&url).query(&self.auth_params()).send().await
            .context("Failed to delete webhook")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Trello API error {}: {}", status, body);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrelloWebhook {
    pub id: String,
    pub description: Option<String>,
    #[serde(rename = "callbackURL")]
    pub callback_url: String,
    #[serde(rename = "idModel")]
    pub id_model: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrelloWebhookUpdate {
    pub description: Option<String>,
    pub callback_url: Option<String>,
    pub active: Option<bool>,
}
