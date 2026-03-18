use axum::{extract::State, Json};
use std::sync::Arc;

use crate::models::chat::{HealthResponse, ProviderStatus};
use crate::AppState;

pub async fn health_handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let mut provider_statuses = Vec::new();

    for provider in &state.ai_providers {
        provider_statuses.push(ProviderStatus {
            name: provider.name().to_string(),
            available: provider.is_available().await,
        });
    }

    let redis_ok = state.cache.health_check().await;
    let trello_ok = state.trello.health_check().await;

    Json(HealthResponse {
        status: if redis_ok && trello_ok {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        ai_providers: provider_statuses,
        redis: redis_ok,
        trello: trello_ok,
    })
}
