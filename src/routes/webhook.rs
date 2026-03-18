use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::models::card::Card;
use crate::AppState;

/// Trello webhook callback: HEAD for validation, POST for events
pub async fn webhook_head_handler() -> StatusCode {
    // Trello sends HEAD to validate callback URL
    StatusCode::OK
}

#[derive(Deserialize, Debug)]
pub struct TrelloWebhookPayload {
    pub action: TrelloAction,
}

#[derive(Deserialize, Debug)]
pub struct TrelloAction {
    #[serde(rename = "type")]
    pub action_type: String,
    pub data: TrelloActionData,
}

#[derive(Deserialize, Debug)]
pub struct TrelloActionData {
    pub card: Option<TrelloWebhookCard>,
    pub list: Option<TrelloWebhookList>,
    #[serde(rename = "listAfter")]
    pub list_after: Option<TrelloWebhookList>,
    #[serde(rename = "listBefore")]
    pub list_before: Option<TrelloWebhookList>,
}

#[derive(Deserialize, Debug)]
pub struct TrelloWebhookCard {
    pub id: String,
    pub name: Option<String>,
    pub desc: Option<String>,
    #[serde(rename = "shortUrl")]
    pub short_url: Option<String>,
    pub due: Option<String>,
    #[serde(rename = "dueComplete")]
    pub due_complete: Option<bool>,
    #[serde(rename = "idList")]
    pub id_list: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TrelloWebhookList {
    pub id: String,
    pub name: Option<String>,
}

pub async fn webhook_post_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TrelloWebhookPayload>,
) -> StatusCode {
    let action_type = &payload.action.action_type;
    tracing::info!("🔔 Webhook event: {}", action_type);

    match action_type.as_str() {
        "createCard" | "updateCard" | "copyCard" => {
            if let Some(wh_card) = &payload.action.data.card {
                // For updates, we need to fetch the full card from Trello to get all fields
                let card_id = &wh_card.id;
                match state.trello.fetch_card(card_id).await {
                    Ok(mut card) => {
                        // Set list name from webhook data if available
                        if card.list_name.is_none() {
                            if let Some(list) = &payload.action.data.list_after {
                                card.list_name = list.name.clone();
                            } else if let Some(list) = &payload.action.data.list {
                                card.list_name = list.name.clone();
                            }
                        }
                        if let Err(e) = state.cache.upsert_card(&card).await {
                            tracing::error!("Failed to upsert card: {}", e);
                        } else {
                            tracing::info!("📝 Card synced: {}", card.name);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch card {}: {}", card_id, e);
                        // Fallback: use webhook data directly
                        let card = Card {
                            id: wh_card.id.clone(),
                            name: wh_card.name.clone().unwrap_or_default(),
                            desc: wh_card.desc.clone().unwrap_or_default(),
                            id_list: wh_card.id_list.clone().unwrap_or_default(),
                            due: wh_card.due.clone(),
                            due_complete: wh_card.due_complete,
                            labels: Vec::new(),
                            short_url: wh_card.short_url.clone().unwrap_or_default(),
                            list_name: payload.action.data.list.as_ref().and_then(|l| l.name.clone()),
                            members: Vec::new(),
                        };
                        if let Err(e) = state.cache.upsert_card(&card).await {
                            tracing::error!("Failed to upsert card from webhook: {}", e);
                        }
                    }
                }
            }
        }
        "deleteCard" => {
            if let Some(wh_card) = &payload.action.data.card {
                if let Err(e) = state.cache.delete_card(&wh_card.id).await {
                    tracing::error!("Failed to delete card: {}", e);
                } else {
                    tracing::info!("🗑️  Card deleted: {}", wh_card.name.as_deref().unwrap_or("unknown"));
                }
            }
        }
        "moveCardToBoard" => {
            if let Some(wh_card) = &payload.action.data.card {
                match state.trello.fetch_card(&wh_card.id).await {
                    Ok(card) => { let _ = state.cache.upsert_card(&card).await; }
                    Err(e) => tracing::error!("Failed to sync moved card: {}", e),
                }
            }
        }
        "moveCardFromBoard" => {
            if let Some(wh_card) = &payload.action.data.card {
                let _ = state.cache.delete_card(&wh_card.id).await;
            }
        }
        _ => {
            tracing::debug!("Unhandled webhook action: {}", action_type);
        }
    }

    StatusCode::OK
}
