use crate::models::card::BoardData;

pub fn build_system_prompt(board_data: &BoardData) -> String {
    let mut prompt = String::new();

    prompt.push_str("Bạn là trợ lý thông minh chuyên về quản lý dự án Trello. ");
    prompt.push_str("Hãy trả lời câu hỏi của người dùng bằng tiếng Việt, dựa trên dữ liệu board Trello dưới đây.\n");
    prompt.push_str("Khi trả lời, hãy đưa ra thông tin chính xác, ngắn gọn và hữu ích.\n");
    prompt.push_str("Nếu câu hỏi liên quan đến deadline, hãy so sánh với ngày hiện tại.\n");
    prompt.push_str("Nếu không tìm thấy thông tin phù hợp, hãy nói rõ.\n\n");

    // Board info
    prompt.push_str(&format!("## Board: {}\n", board_data.board.name));
    if !board_data.board.desc.is_empty() {
        prompt.push_str(&format!("Mô tả: {}\n", board_data.board.desc));
    }
    prompt.push('\n');

    // Lists summary
    prompt.push_str("## Danh sách (Lists):\n");
    for list in &board_data.lists {
        let card_count = board_data
            .cards
            .iter()
            .filter(|c| c.id_list == list.id)
            .count();
        prompt.push_str(&format!("- {} ({} cards)\n", list.name, card_count));
    }
    prompt.push('\n');

    // Cards table
    prompt.push_str("## Cards:\n");
    prompt.push_str("| # | Tên | List | Deadline | Labels | Link |\n");
    prompt.push_str("|---|-----|------|----------|--------|------|\n");

    for (i, card) in board_data.cards.iter().enumerate() {
        let list_name = card.list_name.as_deref().unwrap_or("Unknown");
        let due = card.due.as_deref().unwrap_or("Không có");
        let due_status = if card.due_complete.unwrap_or(false) {
            " ✅"
        } else if card.due.is_some() {
            " ⏰"
        } else {
            ""
        };
        let labels: Vec<String> = card
            .labels
            .iter()
            .map(|l| {
                if l.name.is_empty() {
                    l.color.clone().unwrap_or_default()
                } else {
                    l.name.clone()
                }
            })
            .collect();
        let labels_str = if labels.is_empty() {
            "—".to_string()
        } else {
            labels.join(", ")
        };

        prompt.push_str(&format!(
            "| {} | {} | {} | {}{} | {} | {} |\n",
            i + 1,
            card.name,
            list_name,
            due,
            due_status,
            labels_str,
            card.short_url
        ));
    }

    prompt
}

pub fn extract_matched_cards(board_data: &BoardData, query: &str) -> Vec<usize> {
    let query_lower = query.to_lowercase();
    board_data
        .cards
        .iter()
        .enumerate()
        .filter(|(_, card)| {
            card.name.to_lowercase().contains(&query_lower)
                || card.desc.to_lowercase().contains(&query_lower)
                || card
                    .labels
                    .iter()
                    .any(|l| l.name.to_lowercase().contains(&query_lower))
        })
        .map(|(i, _)| i)
        .collect()
}
