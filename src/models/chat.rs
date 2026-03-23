use serde::{Deserialize, Serialize};

use super::card::TrelloCard;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub matched_cards: Vec<TrelloCard>,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisData {
    pub analysis_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_data: Option<ChartData>,
    #[serde(default)]
    pub insights: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_stats: Option<TimeStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub chart_type: String,
    pub labels: Vec<String>,
    pub datasets: Vec<ChartDataset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataset {
    pub label: String,
    pub data: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeStats {
    pub total_est_hours: f64,
    pub avg_hours_per_card: f64,
    pub cards_with_est: usize,
    pub cards_without_est: usize,
    pub by_member: Vec<MemberTimeStat>,
    pub by_list: Vec<ListTimeStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberTimeStat {
    pub name: String,
    pub cards: usize,
    pub hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTimeStat {
    pub name: String,
    pub cards: usize,
    pub hours: f64,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub ai_providers: Vec<ProviderStatus>,
    pub redis: bool,
    pub trello: bool,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub available: bool,
}
