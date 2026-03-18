use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::models::chat::{ChatRequest, ChatResponse};
use crate::services::intent;
use crate::AppState;

pub async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let start = std::time::Instant::now();

    // 1. AI-powered intent extraction (with keyword fallback)
    let parsed_intent = intent::parse_intent_ai(&payload.message, &state.ai_providers).await;
    tracing::info!("🎯 Parsed intent: {:?}", parsed_intent);

    // 2. Execute intent against local Redis data — instant
    let (matched_cards, response) = intent::execute_ai_intent(&parsed_intent, &state.cache)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Query error: {}", e)))?;

    let elapsed = start.elapsed().as_millis();

    let provider_info = if parsed_intent.intent.is_empty() {
        format!("keyword ({}ms)", elapsed)
    } else {
        format!("ai+local ({}ms)", elapsed)
    };

    tracing::info!("⚡ Response in {}ms ({} cards found)", elapsed, matched_cards.len());

    Ok(Json(ChatResponse {
        response,
        matched_cards,
        provider: provider_info,
    }))
}
