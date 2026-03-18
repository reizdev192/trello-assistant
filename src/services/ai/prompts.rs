/// System prompts for AI intent extraction

/// System prompt for extracting structured query from natural language.
/// Focused on Trello card search only.
pub const INTENT_EXTRACTION_PROMPT: &str = r#"You are a Trello card search parser. Extract search filters as JSON.

RESPOND WITH ONLY VALID JSON. No explanation, no markdown.

JSON format:
{"intent":"<type>","keyword":null,"member":null,"label":null,"list":null,"has_due":null,"overdue_only":null,"due_date":null}

Intent types:
- "list_all": show all cards
- "search": search by keyword
- "filter_member": cards assigned to a person
- "filter_label": filter by label/tag
- "filter_list": filter by list/column
- "due": cards with deadlines
- "overdue": overdue cards

Rules:
- Set only relevant fields, leave others null
- For @username, set member without @
- For "của <name>", set member to that name
- When user mentions a SPECIFIC DATE (e.g. "ngày 25/6/2025", "June 25 2025", "25 tháng 6"), set due_date to "YYYY-MM-DD" format
- due_date must always be in YYYY-MM-DD format (e.g. "2025-06-25")

Examples:
User: "card của @khanhttq"
{"intent":"filter_member","keyword":null,"member":"khanhttq","label":null,"list":null,"has_due":null,"overdue_only":null,"due_date":null}

User: "card bug đang overdue"
{"intent":"search","keyword":null,"member":null,"label":"bug","list":null,"has_due":null,"overdue_only":true,"due_date":null}

User: "tất cả card"
{"intent":"list_all","keyword":null,"member":null,"label":null,"list":null,"has_due":null,"overdue_only":null,"due_date":null}

User: "card sắp deadline"
{"intent":"due","keyword":null,"member":null,"label":null,"list":null,"has_due":true,"overdue_only":null,"due_date":null}

User: "card có deadline ngày 25 tháng 6 năm 2025"
{"intent":"due","keyword":null,"member":null,"label":null,"list":null,"has_due":true,"overdue_only":null,"due_date":"2025-06-25"}

User: "deadline 15/3/2026"
{"intent":"due","keyword":null,"member":null,"label":null,"list":null,"has_due":true,"overdue_only":null,"due_date":"2026-03-15"}

User: "card trong list Done"
{"intent":"filter_list","keyword":null,"member":null,"label":null,"list":"Done","has_due":null,"overdue_only":null,"due_date":null}

User: "tìm thanh toán"
{"intent":"search","keyword":"thanh toán","member":null,"label":null,"list":null,"has_due":null,"overdue_only":null,"due_date":null}

User: "card urgent của @tuyen"
{"intent":"filter_member","keyword":null,"member":"tuyen","label":"urgent","list":null,"has_due":null,"overdue_only":null,"due_date":null}
"#;

/// Format a minimal result header
pub fn format_ai_result_header(
    intent: &str,
    count: usize,
    keyword: Option<&str>,
    member: Option<&str>,
    label: Option<&str>,
    list: Option<&str>,
    due_date: Option<&str>,
) -> String {
    if count == 0 {
        let mut filters = Vec::new();
        if let Some(k) = keyword { filters.push(format!("\"{}\"", k)); }
        if let Some(m) = member { filters.push(format!("@{}", m)); }
        if let Some(l) = label { filters.push(format!("label:{}", l)); }
        if let Some(li) = list { filters.push(format!("list:{}", li)); }
        if let Some(d) = due_date { filters.push(format!("deadline:{}", d)); }

        if filters.is_empty() {
            return "Không tìm thấy card nào.".to_string();
        }
        return format!("Không tìm thấy card nào cho {}", filters.join(", "));
    }

    let mut parts = Vec::new();
    match intent {
        "list_all" => parts.push(format!("{} card", count)),
        "filter_member" => {
            let who = member.unwrap_or("?");
            parts.push(format!("{} card của @{}", count, who));
        }
        "filter_label" => {
            let lbl = label.unwrap_or("?");
            parts.push(format!("{} card [{}]", count, lbl));
        }
        "filter_list" => {
            let ln = list.unwrap_or("?");
            parts.push(format!("{} card trong {}", count, ln));
        }
        "due" => {
            if let Some(d) = due_date {
                parts.push(format!("{} card deadline ngày {}", count, d));
            } else {
                parts.push(format!("{} card có deadline", count));
            }
        }
        "overdue" => parts.push(format!("{} card quá hạn", count)),
        _ => {
            if let Some(k) = keyword {
                parts.push(format!("{} card cho \"{}\"", count, k));
            } else {
                parts.push(format!("{} card", count));
            }
        }
    }

    // Add secondary filters
    if intent != "filter_member" { if let Some(m) = member { parts.push(format!("@{}", m)); } }
    if intent != "filter_label" { if let Some(l) = label { parts.push(format!("[{}]", l)); } }
    if intent != "filter_list" { if let Some(li) = list { parts.push(format!("trong {}", li)); } }

    parts.join(" · ")
}
