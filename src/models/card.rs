use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrelloCard {
    pub id: String,
    pub name: String,
    pub desc: String,
    #[serde(rename = "idList")]
    pub id_list: String,
    pub due: Option<String>,
    #[serde(rename = "dueComplete")]
    pub due_complete: Option<bool>,
    pub labels: Vec<TrelloLabel>,
    #[serde(rename = "shortUrl")]
    pub short_url: String,
    #[serde(default)]
    pub list_name: Option<String>,
    #[serde(default)]
    pub members: Vec<TrelloMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrelloLabel {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrelloMember {
    pub id: String,
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrelloList {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrelloBoard {
    pub id: String,
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardData {
    pub board: TrelloBoard,
    pub lists: Vec<TrelloList>,
    pub cards: Vec<TrelloCard>,
}

// Type aliases for cleaner code
pub type Card = TrelloCard;
pub type Board = TrelloBoard;

