use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::models::chat::{ChatRequest, ChatResponse};
use crate::services::{analysis, intent};
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

    // 3. AI analysis (Pass 2) — only for analysis-type intents
    let analysis_data = if intent::needs_analysis_pass(&parsed_intent.intent, &payload.message) && !matched_cards.is_empty() {
        tracing::info!("📊 Running AI analysis on {} cards...", matched_cards.len());
        match analysis::run_analysis(&matched_cards, &payload.message, &state.ai_providers).await {
            Ok(data) => {
                tracing::info!("📊 Analysis complete");
                Some(data)
            }
            Err(e) => {
                tracing::warn!("📊 Analysis failed, continuing without: {}", e);
                None
            }
        }
    } else {
        None
    };

    let elapsed = start.elapsed().as_millis();

    let provider_info = if analysis_data.is_some() {
        format!("ai+analysis ({}ms)", elapsed)
    } else if parsed_intent.intent.is_empty() {
        format!("keyword ({}ms)", elapsed)
    } else {
        format!("ai+local ({}ms)", elapsed)
    };

    tracing::info!("⚡ Response in {}ms ({} cards found, analysis: {})",
        elapsed, matched_cards.len(), analysis_data.is_some());

    Ok(Json(ChatResponse {
        response,
        matched_cards,
        provider: provider_info,
        analysis: analysis_data,
    }))
}

