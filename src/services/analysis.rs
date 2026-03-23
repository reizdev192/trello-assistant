use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::card::TrelloCard;
use crate::models::chat::{
    AnalysisData, ChartData, ChartDataset, ListTimeStat, MemberTimeStat, TimeStats,
};
use crate::services::ai::{prompts, AiProvider};

/// Extract estimated hours from card title.
/// Supports patterns: "Est: 4h", "Est: 2.5h", "est:0.5h" (case-insensitive)
pub fn extract_est_hours(card_name: &str) -> Option<f64> {
    let re = Regex::new(r"(?i)est(?:[a-zA-Z\s_-]*?)[:\s]+(\d+\.?\d*)\s*h").ok()?;
    re.captures(card_name)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
}

/// Pre-compute time statistics from cards (no AI needed)
pub fn compute_time_stats(cards: &[TrelloCard]) -> TimeStats {
    let mut by_member: HashMap<String, (usize, f64)> = HashMap::new();
    let mut by_list: HashMap<String, (usize, f64)> = HashMap::new();
    let mut total_hours = 0.0;
    let mut cards_with_est = 0usize;

    for card in cards {
        let est = extract_est_hours(&card.name);
        let hours = est.unwrap_or(0.0);

        if est.is_some() {
            cards_with_est += 1;
            total_hours += hours;
        }

        // Aggregate by list
        let list_name = card
            .list_name
            .as_deref()
            .unwrap_or("Unknown")
            .to_string();
        let entry = by_list.entry(list_name).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += hours;

        // Aggregate by member
        if card.members.is_empty() {
            let entry = by_member.entry("Unassigned".to_string()).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += hours;
        } else {
            for member in &card.members {
                let entry = by_member
                    .entry(member.full_name.clone())
                    .or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 += hours;
            }
        }
    }

    let cards_without_est = cards.len() - cards_with_est;
    let avg = if cards_with_est > 0 {
        total_hours / cards_with_est as f64
    } else {
        0.0
    };

    // Sort by hours descending
    let mut member_stats: Vec<MemberTimeStat> = by_member
        .into_iter()
        .map(|(name, (cards, hours))| MemberTimeStat {
            name,
            cards,
            hours,
        })
        .collect();
    member_stats.sort_by(|a, b| b.hours.partial_cmp(&a.hours).unwrap_or(std::cmp::Ordering::Equal));

    let mut list_stats: Vec<ListTimeStat> = by_list
        .into_iter()
        .map(|(name, (cards, hours))| ListTimeStat {
            name,
            cards,
            hours,
        })
        .collect();
    list_stats.sort_by(|a, b| b.cards.partial_cmp(&a.cards).unwrap_or(std::cmp::Ordering::Equal));

    TimeStats {
        total_est_hours: (total_hours * 10.0).round() / 10.0,
        avg_hours_per_card: (avg * 10.0).round() / 10.0,
        cards_with_est,
        cards_without_est,
        by_member: member_stats,
        by_list: list_stats,
    }
}

/// Build compact context string for AI analysis from cards data
fn build_analysis_context(cards: &[TrelloCard], question: &str, time_stats: &TimeStats) -> String {
    let mut ctx = String::new();

    ctx.push_str(&format!("USER QUESTION: {}\n\n", question));
    ctx.push_str(&format!(
        "TIME STATS: total_est={}h, avg={}h/card, with_est={}, without_est={}\n",
        time_stats.total_est_hours,
        time_stats.avg_hours_per_card,
        time_stats.cards_with_est,
        time_stats.cards_without_est
    ));

    // Member hours summary
    if !time_stats.by_member.is_empty() {
        ctx.push_str("MEMBER HOURS: ");
        let member_parts: Vec<String> = time_stats
            .by_member
            .iter()
            .map(|m| format!("{}({}cards,{}h)", m.name, m.cards, m.hours))
            .collect();
        ctx.push_str(&member_parts.join(", "));
        ctx.push('\n');
    }

    // List summary
    if !time_stats.by_list.is_empty() {
        ctx.push_str("LIST STATS: ");
        let list_parts: Vec<String> = time_stats
            .by_list
            .iter()
            .map(|l| format!("{}({}cards,{}h)", l.name, l.cards, l.hours))
            .collect();
        ctx.push_str(&list_parts.join(", "));
        ctx.push('\n');
    }

    ctx.push_str(&format!("\nTOTAL CARDS: {}\n", cards.len()));
    ctx.push_str("CARDS DATA:\n");

    // Compact card listing (limit to prevent token overflow)
    let max_cards = 100.min(cards.len());
    for (i, card) in cards.iter().take(max_cards).enumerate() {
        let list = card.list_name.as_deref().unwrap_or("?");
        let members: Vec<&str> = card.members.iter().map(|m| m.full_name.as_str()).collect();
        let members_str = if members.is_empty() {
            "-".to_string()
        } else {
            members.join(",")
        };
        let labels: Vec<&str> = card.labels.iter().map(|l| l.name.as_str()).collect();
        let labels_str = if labels.is_empty() {
            "-".to_string()
        } else {
            labels.join(",")
        };
        let due = card.due.as_deref().unwrap_or("-");
        let est = extract_est_hours(&card.name)
            .map(|h| format!("{}h", h))
            .unwrap_or_else(|| "-".to_string());
        let overdue_marker = if card.due.is_some() && !card.due_complete.unwrap_or(false) {
            if let Some(ref d) = card.due {
                if d < &chrono::Utc::now().to_rfc3339() {
                    "⚠OVERDUE"
                } else {
                    ""
                }
            } else {
                ""
            }
        } else if card.due_complete == Some(true) {
            "✓DONE"
        } else {
            ""
        };

        ctx.push_str(&format!(
            "{}. [{}] {} | members:{} | labels:{} | due:{} {} | est:{}\n",
            i + 1,
            list,
            card.name,
            members_str,
            labels_str,
            due,
            overdue_marker,
            est
        ));
    }

    if cards.len() > max_cards {
        ctx.push_str(&format!(
            "... and {} more cards\n",
            cards.len() - max_cards
        ));
    }

    ctx
}

/// Run AI analysis (Pass 2): send cards data to AI for contextual analysis
pub async fn run_analysis(
    cards: &[TrelloCard],
    question: &str,
    ai_providers: &[Arc<dyn AiProvider>],
) -> Result<AnalysisData> {
    let time_stats = compute_time_stats(cards);
    let context = build_analysis_context(cards, question, &time_stats);

    // Try AI analysis with longer timeout (analysis takes more time)
    for provider in ai_providers {
        if !provider.is_available().await {
            continue;
        }

        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            provider.chat(prompts::ANALYSIS_SYSTEM_PROMPT, &context),
        )
        .await
        {
            Ok(Ok(response)) => {
                tracing::info!("📊 AI analysis response received ({} chars)", response.len());
                if let Some(mut analysis) = parse_analysis_response(&response) {
                    // Always attach pre-computed time_stats
                    analysis.time_stats = Some(time_stats);
                    return Ok(analysis);
                }
                tracing::warn!(
                    "📊 AI analysis returned non-JSON: {}",
                    &response[..response.len().min(300)]
                );
            }
            Ok(Err(e)) => {
                tracing::warn!("📊 AI analysis error: {}", e);
            }
            Err(_) => {
                tracing::warn!("📊 AI analysis timeout (>30s)");
            }
        }
    }

    // Fallback: generate basic analysis without AI
    Ok(generate_fallback_analysis(cards, &time_stats))
}

/// Parse structured JSON from AI analysis response
fn parse_analysis_response(response: &str) -> Option<AnalysisData> {
    let trimmed = response.trim();

    // Try direct parse
    if let Ok(parsed) = serde_json::from_str::<AnalysisData>(trimmed) {
        return Some(parsed);
    }

    // Try extracting JSON from markdown code block
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            let json_str = &trimmed[start..=end];
            if let Ok(parsed) = serde_json::from_str::<AnalysisData>(json_str) {
                return Some(parsed);
            }
        }
    }

    None
}

/// Generate fallback analysis when AI is unavailable
fn generate_fallback_analysis(cards: &[TrelloCard], time_stats: &TimeStats) -> AnalysisData {
    // Build a basic summary
    let mut summary = format!("## 📊 Tổng quan Board\n\n");
    summary.push_str(&format!("- **Tổng cards:** {}\n", cards.len()));
    if time_stats.cards_with_est > 0 {
        summary.push_str(&format!(
            "- **Tổng Est:** {}h ({} cards có estimate)\n",
            time_stats.total_est_hours, time_stats.cards_with_est
        ));
        summary.push_str(&format!(
            "- **Trung bình:** {}h/card\n",
            time_stats.avg_hours_per_card
        ));
    }

    // Count overdue
    let now = chrono::Utc::now().to_rfc3339();
    let overdue_count = cards
        .iter()
        .filter(|c| {
            c.due
                .as_ref()
                .map_or(false, |d| d < &now && !c.due_complete.unwrap_or(false))
        })
        .count();
    if overdue_count > 0 {
        summary.push_str(&format!("- **Quá hạn:** {} cards ⚠️\n", overdue_count));
    }

    // Build chart data from list stats
    let chart_data = if !time_stats.by_list.is_empty() {
        Some(ChartData {
            chart_type: "bar".to_string(),
            labels: time_stats.by_list.iter().map(|l| l.name.clone()).collect(),
            datasets: vec![
                ChartDataset {
                    label: "Cards".to_string(),
                    data: time_stats.by_list.iter().map(|l| l.cards as f64).collect(),
                },
                ChartDataset {
                    label: "Est Hours".to_string(),
                    data: time_stats.by_list.iter().map(|l| l.hours).collect(),
                },
            ],
        })
    } else {
        None
    };

    let mut insights = vec![format!("Tổng cộng {} cards trên board", cards.len())];
    if overdue_count > 0 {
        insights.push(format!("{} cards đang quá hạn cần xử lý", overdue_count));
    }
    if time_stats.cards_without_est > 0 {
        insights.push(format!(
            "{} cards chưa có estimate — cần PM bổ sung",
            time_stats.cards_without_est
        ));
    }

    AnalysisData {
        analysis_type: "summary".to_string(),
        summary: Some(summary),
        chart_data,
        insights,
        time_stats: Some(time_stats.clone()),
    }
}
