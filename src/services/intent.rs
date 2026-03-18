use anyhow::Result;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::card::Card;
use crate::services::ai::{AiProvider, prompts};
use crate::services::cache::CacheService;

/// AI-parsed intent from JSON extraction
#[derive(Debug, Deserialize, Default)]
pub struct AiParsedIntent {
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub member: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub list: Option<String>,
    #[serde(default)]
    pub has_due: Option<bool>,
    #[serde(default)]
    pub overdue_only: Option<bool>,
    /// Specific date filter in YYYY-MM-DD format (e.g. "2025-06-25")
    #[serde(default)]
    pub due_date: Option<String>,
}

/// Legacy intent enum (used as fallback)
#[derive(Debug)]
pub enum UserIntent {
    ListAll,
    SearchByKeyword(String),
    FilterByLabel(String),
    FilterByList(String),
    FilterByMember(String),
    DueCards,
    OverdueCards,
    BoardSummary,
}

// ============================================================
// AI-POWERED INTENT PARSING (Pass 1)
// ============================================================

/// Parse user intent using AI — returns structured query JSON.
/// Falls back to keyword matching if AI fails.
pub async fn parse_intent_ai(
    message: &str,
    ai_providers: &[Arc<dyn AiProvider>],
) -> AiParsedIntent {
    // Try AI extraction first
    let mut parsed = match try_ai_extraction(message, ai_providers).await {
        Some(parsed) => {
            tracing::info!("🤖 AI parsed intent: {:?}", parsed);
            parsed
        }
        None => {
            // Fallback to keyword-based parsing
            tracing::warn!("⚠️ AI intent extraction failed, using keyword fallback");
            let legacy = parse_intent_keyword(message);
            convert_legacy_to_ai_intent(legacy, message)
        }
    };

    // Post-processing: extract specific date from message if AI didn't
    if parsed.due_date.is_none() {
        if let Some(date) = extract_date_from_message(message) {
            tracing::info!("📅 Extracted date from message: {}", date);
            parsed.due_date = Some(date);
            parsed.has_due = Some(true);
            if parsed.intent.is_empty() || parsed.intent == "search" {
                parsed.intent = "due".to_string();
            }
        }
    }

    parsed
}

/// Try AI extraction, return None if it fails
async fn try_ai_extraction(
    message: &str,
    ai_providers: &[Arc<dyn AiProvider>],
) -> Option<AiParsedIntent> {
    for provider in ai_providers {
        if !provider.is_available().await {
            continue;
        }

        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            provider.chat(prompts::INTENT_EXTRACTION_PROMPT, message),
        )
        .await
        {
            Ok(Ok(response)) => {
                // Try to parse JSON from response
                if let Some(parsed) = extract_json_from_response(&response) {
                    return Some(parsed);
                }
                tracing::warn!("🤖 AI returned non-JSON: {}", &response[..response.len().min(200)]);
            }
            Ok(Err(e)) => {
                tracing::warn!("🤖 AI error: {}", e);
            }
            Err(_) => {
                tracing::warn!("🤖 AI timeout (>10s)");
            }
        }
    }
    None
}

/// Extract JSON object from AI response (handles markdown code blocks, extra text)
fn extract_json_from_response(response: &str) -> Option<AiParsedIntent> {
    let trimmed = response.trim();

    // Try direct JSON parse
    if let Ok(parsed) = serde_json::from_str::<AiParsedIntent>(trimmed) {
        return Some(parsed);
    }

    // Try extracting JSON from markdown code block
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            let json_str = &trimmed[start..=end];
            if let Ok(parsed) = serde_json::from_str::<AiParsedIntent>(json_str) {
                return Some(parsed);
            }
        }
    }

    None
}

// ============================================================
// EXECUTE AI-PARSED INTENT against Redis
// ============================================================

/// Execute the AI-parsed intent: apply all filters to get cards from cache
pub async fn execute_ai_intent(
    parsed: &AiParsedIntent,
    cache: &CacheService,
) -> Result<(Vec<Card>, String)> {

    // Smart intent normalization: if AI set overdue_only but intent is "search",
    // prioritize the overdue filter instead of searching text "overdue"
    let effective_intent = if parsed.overdue_only == Some(true) && parsed.intent != "overdue" {
        "overdue"
    } else if parsed.has_due == Some(true) && parsed.intent == "search" && parsed.keyword.as_ref().map_or(false, |k| {
        let k_lower = k.to_lowercase();
        ["deadline", "due", "hạn", "ngày"].iter().any(|w| k_lower.contains(w))
    }) {
        "due"
    } else {
        parsed.intent.as_str()
    };

    // Start with the right base set based on primary intent
    let mut cards = match effective_intent {
        "list_all" => cache.get_all_cards().await?,
        "filter_member" => {
            if let Some(ref m) = parsed.member {
                cache.get_cards_by_member(m).await?
            } else {
                cache.get_all_cards().await?
            }
        }
        "filter_label" => {
            if let Some(ref l) = parsed.label {
                cache.get_cards_by_label(l).await?
            } else {
                cache.get_all_cards().await?
            }
        }
        "filter_list" => {
            if let Some(ref ln) = parsed.list {
                cache.get_cards_by_list(ln).await?
            } else {
                cache.get_all_cards().await?
            }
        }
        "due" => cache.get_cards_with_due().await?,
        "overdue" => cache.get_overdue_cards().await?,
        "search" => {
            if let Some(ref kw) = parsed.keyword {
                cache.search_cards(kw).await?
            } else {
                cache.get_all_cards().await?
            }
        }
        _ => {
            // Unknown intent — try keyword search with original message
            if let Some(ref kw) = parsed.keyword {
                cache.search_cards(kw).await?
            } else {
                cache.get_all_cards().await?
            }
        }
    };

    // Apply additional cross-filters (combine multiple conditions)
    if effective_intent != "filter_member" {
        if let Some(ref m) = parsed.member {
            let m_lower = m.to_lowercase();
            cards.retain(|c| {
                c.members.iter().any(|mem| {
                    mem.username.to_lowercase().contains(&m_lower)
                        || mem.full_name.to_lowercase().contains(&m_lower)
                })
            });
        }
    }

    if effective_intent != "filter_label" {
        if let Some(ref l) = parsed.label {
            let l_lower = l.to_lowercase();
            cards.retain(|c| {
                c.labels.iter().any(|lbl| {
                    lbl.name.to_lowercase().contains(&l_lower)
                        || lbl.color.as_ref().map_or(false, |col| col.to_lowercase().contains(&l_lower))
                })
            });
        }
    }

    if effective_intent != "filter_list" {
        if let Some(ref ln) = parsed.list {
            let ln_lower = ln.to_lowercase();
            cards.retain(|c| {
                c.list_name.as_ref().map_or(false, |n| n.to_lowercase().contains(&ln_lower))
            });
        }
    }

    if parsed.overdue_only == Some(true) && effective_intent != "overdue" {
        let now = chrono::Utc::now().to_rfc3339();
        cards.retain(|c| {
            c.due.as_ref().map_or(false, |d| d < &now)
                && !c.due_complete.unwrap_or(false)
        });
    }

    if parsed.has_due == Some(true) && effective_intent != "due" {
        cards.retain(|c| c.due.is_some());
    }

    // Filter by specific due_date (YYYY-MM-DD)
    if let Some(ref target_date) = parsed.due_date {
        cards.retain(|c| {
            c.due.as_ref().map_or(false, |d| d.starts_with(target_date))
        });
    }

    // Build the response header
    let header = prompts::format_ai_result_header(
        effective_intent,
        cards.len(),
        parsed.keyword.as_deref(),
        parsed.member.as_deref(),
        parsed.label.as_deref(),
        parsed.list.as_deref(),
        parsed.due_date.as_deref(),
    );

    Ok((cards, header))
}

// ============================================================
// KEYWORD FALLBACK (legacy)
// ============================================================

/// Parse user intent using fast keyword detection (fallback)
pub fn parse_intent_keyword(message: &str) -> UserIntent {
    let msg = message.to_lowercase();

    // Board summary
    if msg.contains("tóm tắt") || msg.contains("summary") || msg.contains("trạng thái board")
        || msg.contains("thống kê") || msg.contains("tổng quan")
    {
        return UserIntent::BoardSummary;
    }

    // Member filter
    if let Some(member) = extract_member_query(&msg) {
        return UserIntent::FilterByMember(member);
    }

    // List all
    if msg.contains("tất cả") || msg.contains("all") || msg.contains("liệt kê")
        || msg.contains("danh sách") || msg.contains("toàn bộ") || msg.contains("xem hết")
    {
        return UserIntent::ListAll;
    }

    // Overdue
    if msg.contains("quá hạn") || msg.contains("overdue") || msg.contains("trễ hạn")
        || msg.contains("hết hạn")
    {
        return UserIntent::OverdueCards;
    }

    // Due / deadline
    if msg.contains("deadline") || msg.contains("hạn") || msg.contains("due")
        || msg.contains("sắp tới") || msg.contains("ngày") || msg.contains("thời hạn")
    {
        return UserIntent::DueCards;
    }

    // Priority / label
    if msg.contains("ưu tiên") || msg.contains("priority") || msg.contains("urgent")
        || msg.contains("quan trọng") || msg.contains("critical")
    {
        return UserIntent::FilterByLabel("priority".to_string());
    }

    // Bug label
    if msg.contains("bug") || msg.contains("lỗi") || msg.contains("error") {
        return UserIntent::FilterByLabel("bug".to_string());
    }

    // Filter by list name
    let list_keywords = ["done", "doing", "todo", "to do", "in progress", "review",
        "backlog", "release", "task", "complete", "pending", "testing"];
    for kw in &list_keywords {
        if msg.contains(kw) {
            return UserIntent::FilterByList(kw.to_string());
        }
    }

    // Search keyword extraction
    if let Some(keyword) = extract_search_keyword(&msg) {
        return UserIntent::SearchByKeyword(keyword);
    }

    // Default: search by the whole message
    UserIntent::SearchByKeyword(message.to_string())
}

/// Convert legacy UserIntent to AiParsedIntent for unified execution
fn convert_legacy_to_ai_intent(intent: UserIntent, _original: &str) -> AiParsedIntent {
    match intent {
        UserIntent::ListAll => AiParsedIntent { intent: "list_all".into(), ..Default::default() },
        UserIntent::SearchByKeyword(kw) => AiParsedIntent { intent: "search".into(), keyword: Some(kw), ..Default::default() },
        UserIntent::FilterByLabel(l) => AiParsedIntent { intent: "filter_label".into(), label: Some(l), ..Default::default() },
        UserIntent::FilterByList(l) => AiParsedIntent { intent: "filter_list".into(), list: Some(l), ..Default::default() },
        UserIntent::FilterByMember(m) => AiParsedIntent { intent: "filter_member".into(), member: Some(m), ..Default::default() },
        UserIntent::DueCards => AiParsedIntent { intent: "due".into(), has_due: Some(true), ..Default::default() },
        UserIntent::OverdueCards => AiParsedIntent { intent: "overdue".into(), overdue_only: Some(true), ..Default::default() },
        UserIntent::BoardSummary => AiParsedIntent { intent: "summary".into(), ..Default::default() },
    }
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn extract_search_keyword(msg: &str) -> Option<String> {
    let patterns = [
        "có chữ ", "chứa ", "liên quan ", "về ", "tên ", "tìm ",
        "search ", "find ", "keyword ", "card ",
    ];

    for pattern in &patterns {
        if let Some(pos) = msg.find(pattern) {
            let keyword = msg[pos + pattern.len()..].trim();
            let keyword = keyword
                .trim_end_matches('?')
                .trim_end_matches(" không")
                .trim_end_matches(" nào")
                .trim_end_matches(" gì")
                .trim_end_matches(" vậy")
                .trim();
            if !keyword.is_empty() {
                return Some(keyword.to_string());
            }
        }
    }
    None
}

fn extract_member_query(msg: &str) -> Option<String> {
    // @username pattern
    if let Some(pos) = msg.find('@') {
        let after = &msg[pos + 1..];
        let member = after
            .split(|c: char| c.is_whitespace() || c == '?' || c == '!' || c == ',')
            .next()
            .unwrap_or("")
            .trim();
        if !member.is_empty() {
            return Some(member.to_string());
        }
    }
    // "của <name>" pattern
    let of_patterns = ["của ", "assign ", "assigned to ", "belong to "];
    for pat in &of_patterns {
        if let Some(pos) = msg.find(pat) {
            let name = msg[pos + pat.len()..]
                .trim_end_matches('?')
                .trim_end_matches(" không")
                .trim_end_matches(" nào")
                .trim_end_matches(" vậy")
                .trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Extract a specific date from user message (Vietnamese + English formats)
/// Returns date in YYYY-MM-DD format
fn extract_date_from_message(message: &str) -> Option<String> {
    let msg = message.to_lowercase();

    // Pattern: dd/mm/yyyy or dd-mm-yyyy
    let re_slash = regex::Regex::new(r"(\d{1,2})[/\-](\d{1,2})[/\-](\d{4})").ok()?;
    if let Some(caps) = re_slash.captures(&msg) {
        let day: u32 = caps[1].parse().ok()?;
        let month: u32 = caps[2].parse().ok()?;
        let year: u32 = caps[3].parse().ok()?;
        if month >= 1 && month <= 12 && day >= 1 && day <= 31 {
            return Some(format!("{:04}-{:02}-{:02}", year, month, day));
        }
    }

    // Pattern: "ngày X tháng Y năm Z" (Vietnamese)
    let re_vn_full = regex::Regex::new(
        r"ng[àa]y\s+(\d{1,2})\s+th[áa]ng\s+(\d{1,2})\s+n[aă]m\s+(\d{4})"
    ).ok()?;
    if let Some(caps) = re_vn_full.captures(&msg) {
        let day: u32 = caps[1].parse().ok()?;
        let month: u32 = caps[2].parse().ok()?;
        let year: u32 = caps[3].parse().ok()?;
        if month >= 1 && month <= 12 && day >= 1 && day <= 31 {
            return Some(format!("{:04}-{:02}-{:02}", year, month, day));
        }
    }

    // Pattern: "ngày X tháng Y" (no year → current year)
    let re_vn_short = regex::Regex::new(
        r"ng[àa]y\s+(\d{1,2})\s+th[áa]ng\s+(\d{1,2})"
    ).ok()?;
    if let Some(caps) = re_vn_short.captures(&msg) {
        let day: u32 = caps[1].parse().ok()?;
        let month: u32 = caps[2].parse().ok()?;
        let year = chrono::Utc::now().format("%Y").to_string().parse::<u32>().unwrap_or(2026);
        if month >= 1 && month <= 12 && day >= 1 && day <= 31 {
            return Some(format!("{:04}-{:02}-{:02}", year, month, day));
        }
    }

    // Pattern: YYYY-MM-DD (ISO format already)
    let re_iso = regex::Regex::new(r"(\d{4})-(\d{2})-(\d{2})").ok()?;
    if let Some(caps) = re_iso.captures(&msg) {
        return Some(format!("{}-{}-{}", &caps[1], &caps[2], &caps[3]));
    }

    None
}
