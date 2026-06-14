use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}


#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Reminder {
    pub id: i64,
    pub note_id: i64,
    pub remind_at: NaiveDateTime,
    pub is_sent: bool,
}