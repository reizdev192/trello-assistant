use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::models::card::Card;
use crate::AppState;

pub async fn list_cards_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Card>>, (StatusCode, String)> {
    let cards = state
        .cache
        .get_all_cards()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Cache error: {}", e)))?;

    Ok(Json(cards))
}

pub async fn refresh_cards_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Card>>, (StatusCode, String)> {
    // Fetch fresh from Trello and bulk sync
    let data = state
        .trello
        .fetch_board_data()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Trello API error: {}", e)))?;

    state
        .cache
        .bulk_sync(&data)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Sync error: {}", e)))?;

    tracing::info!("🔄 Force refreshed {} cards", data.cards.len());

    Ok(Json(data.cards))
}
