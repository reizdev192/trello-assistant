use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Serialize)]
pub struct SettingsResponse {
    pub card_count: usize,
    pub last_sync: Option<String>,
    pub webhooks: Vec<WebhookStatus>,
    pub services: ServicesStatus,
}

#[derive(Serialize)]
pub struct WebhookStatus {
    pub id: String,
    pub description: Option<String>,
    pub callback_url: String,
    pub active: bool,
}

#[derive(Serialize)]
pub struct ServicesStatus {
    pub trello: bool,
    pub redis: bool,
    pub ai_providers: Vec<AiProviderStatus>,
}

#[derive(Serialize)]
pub struct AiProviderStatus {
    pub name: String,
    pub available: bool,
}

#[derive(Deserialize)]
pub struct WebhookRequest {
    pub url: String,
}

/// GET /api/settings — return current system info
pub async fn get_settings(State(state): State<Arc<AppState>>) -> Json<SettingsResponse> {
    let card_count = state.cache.get_all_cards().await.map(|c| c.len()).unwrap_or(0);

    let last_sync = state.last_sync.lock().await.map(|t| t.to_rfc3339());

    let trello_ok = state.trello.health_check().await;
    let redis_ok = state.cache.health_check().await;

    let mut ai_statuses = Vec::new();
    for provider in &state.ai_providers {
        ai_statuses.push(AiProviderStatus {
            name: provider.name().to_string(),
            available: provider.is_available().await,
        });
    }

    // Fetch webhooks from Trello API
    let webhooks = match state.trello.fetch_webhooks().await {
        Ok(wh_list) => wh_list
            .into_iter()
            .map(|w| WebhookStatus {
                id: w.id,
                description: w.description,
                callback_url: w.callback_url,
                active: w.active,
            })
            .collect(),
        Err(_) => vec![],
    };

    Json(SettingsResponse {
        card_count,
        last_sync,
        webhooks,
        services: ServicesStatus {
            trello: trello_ok,
            redis: redis_ok,
            ai_providers: ai_statuses,
        },
    })
}

/// POST /api/settings/webhook — register webhook with custom domain
pub async fn register_webhook(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WebhookRequest>,
) -> Result<Json<WebhookStatus>, (axum::http::StatusCode, String)> {
    let callback = format!("{}/api/webhook", body.url.trim_end_matches('/'));

    match state.webhook_service.register(&callback).await {
        Ok(info) => {
            let status = WebhookStatus {
                id: info.id.clone(),
                description: None,
                callback_url: info.callback_url.clone(),
                active: info.active,
            };
            // Store webhook info in state
            let mut lock = state.webhook_info.lock().await;
            *lock = Some(StoredWebhookInfo {
                id: info.id,
                url: info.callback_url,
                active: info.active,
            });
            Ok(Json(status))
        }
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to register webhook: {}", e),
        )),
    }
}

/// PUT /api/settings/webhook/:id — update webhook via Trello API
pub async fn update_webhook_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(webhook_id): axum::extract::Path<String>,
    Json(body): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookStatus>, (axum::http::StatusCode, String)> {
    let update = crate::services::trello::TrelloWebhookUpdate {
        description: body.description,
        callback_url: body.callback_url,
        active: body.active,
    };

    match state.trello.update_webhook(&webhook_id, &update).await {
        Ok(wh) => Ok(Json(WebhookStatus {
            id: wh.id,
            description: wh.description,
            callback_url: wh.callback_url,
            active: wh.active,
        })),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update webhook: {}", e),
        )),
    }
}

/// DELETE /api/settings/webhook/:id — delete webhook via Trello API
pub async fn delete_webhook_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(webhook_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    match state.trello.delete_webhook_by_id(&webhook_id).await {
        Ok(()) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete webhook: {}", e),
        )),
    }
}

#[derive(Deserialize)]
pub struct UpdateWebhookRequest {
    pub description: Option<String>,
    pub callback_url: Option<String>,
    pub active: Option<bool>,
}

/// GET /api/members — return unique board members from cached cards
pub async fn get_members(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<MemberResponse>> {
    let cards = state.cache.get_all_cards().await.unwrap_or_default();

    let mut seen = std::collections::HashSet::new();
    let mut members = Vec::new();

    for card in &cards {
        for m in &card.members {
            if seen.insert(m.id.clone()) {
                members.push(MemberResponse {
                    id: m.id.clone(),
                    full_name: m.full_name.clone(),
                    username: m.username.clone(),
                });
            }
        }
    }

    members.sort_by(|a, b| a.full_name.to_lowercase().cmp(&b.full_name.to_lowercase()));
    Json(members)
}

#[derive(Serialize)]
pub struct MemberResponse {
    pub id: String,
    pub full_name: String,
    pub username: String,
}

/// Stored webhook info in AppState
#[derive(Clone)]
pub struct StoredWebhookInfo {
    pub id: String,
    pub url: String,
    pub active: bool,
}

// ════════════════════════════════════════════════════════════
// BOARD DATA ENDPOINTS (zero-LLM)
// ════════════════════════════════════════════════════════════

/// GET /api/lists — board lists with card count
pub async fn get_lists(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ListResponse>> {
    let lists = state.cache.get_lists().await.unwrap_or_default();
    let cards = state.cache.get_all_cards().await.unwrap_or_default();

    let result: Vec<ListResponse> = lists.iter().map(|l| {
        let card_count = cards.iter()
            .filter(|c| c.id_list == l.id)
            .count();
        ListResponse {
            id: l.id.clone(),
            name: l.name.clone(),
            card_count,
        }
    }).collect();

    Json(result)
}

#[derive(Serialize)]
pub struct ListResponse {
    pub id: String,
    pub name: String,
    pub card_count: usize,
}

/// GET /api/labels — unique labels from cached cards
pub async fn get_labels(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<LabelResponse>> {
    let cards = state.cache.get_all_cards().await.unwrap_or_default();

    let mut seen = std::collections::HashSet::new();
    let mut labels = Vec::new();

    for card in &cards {
        for l in &card.labels {
            if !l.name.is_empty() && seen.insert(l.id.clone()) {
                labels.push(LabelResponse {
                    id: l.id.clone(),
                    name: l.name.clone(),
                    color: l.color.clone(),
                });
            }
        }
    }

    labels.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Json(labels)
}

#[derive(Serialize)]
pub struct LabelResponse {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

/// GET /api/stats — board analytics
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Json<StatsResponse> {
    let cards = state.cache.get_all_cards().await.unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();
    let soon = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    // Cards by list
    let mut by_list: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for card in &cards {
        let list = card.list_name.clone().unwrap_or_else(|| "Unknown".to_string());
        *by_list.entry(list).or_insert(0) += 1;
    }

    // Cards by label
    let mut by_label: std::collections::HashMap<String, (usize, Option<String>)> = std::collections::HashMap::new();
    for card in &cards {
        for l in &card.labels {
            if !l.name.is_empty() {
                let entry = by_label.entry(l.name.clone()).or_insert((0, l.color.clone()));
                entry.0 += 1;
            }
        }
    }

    // Cards by member
    let mut by_member: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for card in &cards {
        for m in &card.members {
            *by_member.entry(m.full_name.clone()).or_insert(0) += 1;
        }
    }

    // Overdue & due soon
    let overdue_count = cards.iter().filter(|c| {
        c.due.as_ref().map_or(false, |d| d < &now) && !c.due_complete.unwrap_or(false)
    }).count();

    let due_soon_count = cards.iter().filter(|c| {
        c.due.as_ref().map_or(false, |d| d >= &now && d <= &soon) && !c.due_complete.unwrap_or(false)
    }).count();

    let no_due = cards.iter().filter(|c| c.due.is_none()).count();

    Json(StatsResponse {
        total_cards: cards.len(),
        overdue_count,
        due_soon_count,
        no_due_count: no_due,
        by_list: by_list.into_iter().map(|(name, count)| StatItem { name, count, color: None }).collect(),
        by_label: by_label.into_iter().map(|(name, (count, color))| StatItem { name, count, color }).collect(),
        by_member: by_member.into_iter().map(|(name, count)| StatItem { name, count, color: None }).collect(),
    })
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_cards: usize,
    pub overdue_count: usize,
    pub due_soon_count: usize,
    pub no_due_count: usize,
    pub by_list: Vec<StatItem>,
    pub by_label: Vec<StatItem>,
    pub by_member: Vec<StatItem>,
}

#[derive(Serialize)]
pub struct StatItem {
    pub name: String,
    pub count: usize,
    pub color: Option<String>,
}
