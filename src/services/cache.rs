use anyhow::{Context, Result};
use redis::AsyncCommands;

use crate::config::Config;
use crate::models::card::{BoardData, Card};

#[derive(Clone)]
pub struct CacheService {
    client: redis::Client,
    board_id: String,
}

impl CacheService {
    pub fn new(config: &Config) -> Result<Self> {
        let client =
            redis::Client::open(config.redis_url.as_str()).context("Failed to connect to Redis")?;

        Ok(Self {
            client,
            board_id: config.trello_board_id.clone(),
        })
    }

    // === Key patterns ===
    fn card_key(&self, card_id: &str) -> String {
        format!("trello:{}:card:{}", self.board_id, card_id)
    }

    fn cards_index_key(&self) -> String {
        format!("trello:{}:cards_index", self.board_id)
    }

    fn lists_key(&self) -> String {
        format!("trello:{}:lists", self.board_id)
    }

    fn board_key(&self) -> String {
        format!("trello:{}:board_info", self.board_id)
    }

    // === Bulk sync on startup ===
    pub async fn bulk_sync(&self, board_data: &BoardData) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .context("Failed to get Redis connection")?;

        // Store board info
        let board_json = serde_json::to_string(&board_data.board)?;
        conn.set::<_, _, ()>(self.board_key(), board_json).await?;

        // Store lists
        let lists_json = serde_json::to_string(&board_data.lists)?;
        conn.set::<_, _, ()>(self.lists_key(), lists_json).await?;

        // Store each card individually
        let mut card_ids: Vec<String> = Vec::new();
        for card in &board_data.cards {
            let card_json = serde_json::to_string(card)?;
            conn.set::<_, _, ()>(self.card_key(&card.id), card_json).await?;
            card_ids.push(card.id.clone());
        }

        // Store card ID index
        let index_json = serde_json::to_string(&card_ids)?;
        conn.set::<_, _, ()>(self.cards_index_key(), index_json).await?;

        tracing::info!("📦 Bulk synced {} cards to Redis", card_ids.len());
        Ok(())
    }

    // === Single card operations (for webhooks) ===
    pub async fn upsert_card(&self, card: &Card) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // Update card data
        let card_json = serde_json::to_string(card)?;
        conn.set::<_, _, ()>(self.card_key(&card.id), card_json).await?;

        // Add to index if not present
        let mut card_ids = self.get_card_ids().await?;
        if !card_ids.contains(&card.id) {
            card_ids.push(card.id.clone());
            let index_json = serde_json::to_string(&card_ids)?;
            conn.set::<_, _, ()>(self.cards_index_key(), index_json).await?;
        }

        tracing::debug!("📝 Upserted card: {}", card.name);
        Ok(())
    }

    pub async fn delete_card(&self, card_id: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // Remove card data
        conn.del::<_, ()>(self.card_key(card_id)).await?;

        // Remove from index
        let mut card_ids = self.get_card_ids().await?;
        card_ids.retain(|id| id != card_id);
        let index_json = serde_json::to_string(&card_ids)?;
        conn.set::<_, _, ()>(self.cards_index_key(), index_json).await?;

        tracing::debug!("🗑️  Deleted card: {}", card_id);
        Ok(())
    }

    // === Query operations ===
    pub async fn get_all_cards(&self) -> Result<Vec<Card>> {
        let card_ids = self.get_card_ids().await?;
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        let mut cards = Vec::new();
        for id in &card_ids {
            let data: Option<String> = conn.get(self.card_key(id)).await?;
            if let Some(json) = data {
                if let Ok(card) = serde_json::from_str::<Card>(&json) {
                    cards.push(card);
                }
            }
        }
        Ok(cards)
    }

    pub async fn search_cards(&self, query: &str) -> Result<Vec<Card>> {
        let cards = self.get_all_cards().await?;
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query_lower.split_whitespace().collect();
        let word_count = words.len();

        // Build searchable text for each card and score it
        let build_searchable = |card: &Card| -> String {
            format!(
                "{} {} {} {} {}",
                card.name.to_lowercase(),
                card.desc.to_lowercase(),
                card.labels.iter().map(|l| l.name.to_lowercase()).collect::<Vec<_>>().join(" "),
                card.list_name.as_deref().unwrap_or("").to_lowercase(),
                card.members.iter().map(|m| format!("{} {}", m.username, m.full_name).to_lowercase()).collect::<Vec<_>>().join(" "),
            )
        };

        // For single-word queries: simple contains
        if word_count <= 1 {
            let mut results: Vec<Card> = cards
                .into_iter()
                .filter(|card| build_searchable(card).contains(&query_lower))
                .collect();
            // Prioritize name matches over desc matches
            results.sort_by(|a, b| {
                let a_name = a.name.to_lowercase().contains(&query_lower);
                let b_name = b.name.to_lowercase().contains(&query_lower);
                b_name.cmp(&a_name)
            });
            return Ok(results);
        }

        // For multi-word: try strict (all words must match), prioritize exact phrase
        let mut strict: Vec<(Card, u8)> = cards
            .iter()
            .filter_map(|card| {
                let s = build_searchable(card);
                let match_count = words.iter().filter(|w| s.contains(**w)).count();
                if match_count == word_count {
                    let score = if s.contains(&query_lower) { 2 } else { 1 };
                    Some((card.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        if !strict.is_empty() {
            strict.sort_by(|a, b| b.1.cmp(&a.1));
            return Ok(strict.into_iter().map(|(c, _)| c).collect());
        }

        // Fallback: find cards matching any "specific" word (numbers or long unique words)
        let specific_words: Vec<&&str> = words.iter()
            .filter(|w| w.chars().any(|c| c.is_numeric()))
            .collect();

        if !specific_words.is_empty() {
            let mut partial: Vec<Card> = cards
                .into_iter()
                .filter(|card| {
                    let s = build_searchable(card);
                    specific_words.iter().any(|w| s.contains(**w))
                })
                .collect();
            partial.sort_by(|a, b| {
                let a_score: usize = specific_words.iter().filter(|w| build_searchable(a).contains(**w)).count();
                let b_score: usize = specific_words.iter().filter(|w| build_searchable(b).contains(**w)).count();
                b_score.cmp(&a_score)
            });
            return Ok(partial);
        }

        Ok(Vec::new())
    }

    pub async fn get_cards_by_label(&self, label: &str) -> Result<Vec<Card>> {
        let cards = self.get_all_cards().await?;
        let label_lower = label.to_lowercase();

        Ok(cards
            .into_iter()
            .filter(|card| {
                card.labels.iter().any(|l| {
                    l.name.to_lowercase().contains(&label_lower)
                        || l.color.as_ref().map_or(false, |c| c.to_lowercase().contains(&label_lower))
                })
            })
            .collect())
    }

    pub async fn get_cards_by_list(&self, list_name: &str) -> Result<Vec<Card>> {
        let cards = self.get_all_cards().await?;
        let list_lower = list_name.to_lowercase();

        Ok(cards
            .into_iter()
            .filter(|card| {
                card.list_name.as_ref().map_or(false, |ln| ln.to_lowercase().contains(&list_lower))
            })
            .collect())
    }

    pub async fn get_cards_by_member(&self, member: &str) -> Result<Vec<Card>> {
        let cards = self.get_all_cards().await?;
        let member_lower = member.to_lowercase();

        Ok(cards
            .into_iter()
            .filter(|card| {
                card.members.iter().any(|m| {
                    m.username.to_lowercase().contains(&member_lower)
                        || m.full_name.to_lowercase().contains(&member_lower)
                })
            })
            .collect())
    }

    pub async fn get_cards_with_due(&self) -> Result<Vec<Card>> {
        let cards = self.get_all_cards().await?;
        let mut due_cards: Vec<Card> = cards.into_iter().filter(|c| c.due.is_some()).collect();
        due_cards.sort_by(|a, b| a.due.cmp(&b.due));
        Ok(due_cards)
    }

    pub async fn get_overdue_cards(&self) -> Result<Vec<Card>> {
        let cards = self.get_all_cards().await?;
        let now = chrono::Utc::now().to_rfc3339();

        Ok(cards
            .into_iter()
            .filter(|c| {
                c.due.as_ref().map_or(false, |d| d < &now)
                    && !c.due_complete.unwrap_or(false)
            })
            .collect())
    }

    // === Get board lists ===
    pub async fn get_lists(&self) -> Result<Vec<crate::models::card::TrelloList>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let data: Option<String> = conn.get(self.lists_key()).await?;
        match data {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Ok(Vec::new()),
        }
    }

    // === Helpers ===
    async fn get_card_ids(&self) -> Result<Vec<String>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let data: Option<String> = conn.get(self.cards_index_key()).await?;
        match data {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Ok(Vec::new()),
        }
    }

    pub async fn get_board_summary(&self) -> Result<String> {
        let cards = self.get_all_cards().await?;
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        let board_name: Option<String> = conn.get(self.board_key()).await?;
        let board_name = board_name
            .and_then(|j| serde_json::from_str::<crate::models::card::Board>(&j).ok())
            .map(|b| b.name)
            .unwrap_or_else(|| "Unknown".to_string());

        let total = cards.len();
        let with_due = cards.iter().filter(|c| c.due.is_some()).count();

        // Group by list
        let mut list_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for card in &cards {
            let list = card.list_name.clone().unwrap_or_else(|| "Unknown".to_string());
            *list_counts.entry(list).or_insert(0) += 1;
        }

        let mut summary = format!("Board '{}': {} cards total", board_name, total);
        for (list, count) in &list_counts {
            summary.push_str(&format!(", {} in '{}'", count, list));
        }
        if with_due > 0 {
            summary.push_str(&format!(". {} cards have deadlines", with_due));
        }

        Ok(summary)
    }

    pub async fn health_check(&self) -> bool {
        self.client
            .get_multiplexed_async_connection()
            .await
            .is_ok()
    }
}
