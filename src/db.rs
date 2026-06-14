use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use chrono::NaiveDateTime;

use crate::models::{Note, Reminder};

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.create_tables().await?;
        Ok(db)
    }

    async fn create_tables(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS note_tags (
                note_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (note_id, tag_id),
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note_id INTEGER NOT NULL,
                remind_at DATETIME NOT NULL,
                is_sent BOOLEAN DEFAULT FALSE,
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_note(&self, user_id: i64, title: &str, content: &str) -> Result<Note, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();
        let result = sqlx::query(
            "INSERT INTO notes (user_id, title, content, created_at, updated_at) VALUES (?, ?, ?, ?, ?) RETURNING *"
        )
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(Note {
            id: result.get("id"),
            user_id: result.get("user_id"),
            title: result.get("title"),
            content: result.get("content"),
            created_at: result.get("created_at"),
            updated_at: result.get("updated_at"),
        })
    }

    pub async fn get_notes(&self, user_id: i64, page: u32, per_page: u32) -> Result<Vec<Note>, sqlx::Error> {
        let offset = (page - 1) * per_page;
        let rows = sqlx::query(
            "SELECT * FROM notes WHERE user_id = ? ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(user_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| Note {
            id: row.get("id"),
            user_id: row.get("user_id"),
            title: row.get("title"),
            content: row.get("content"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }).collect())
    }

    pub async fn get_note(&self, note_id: i64) -> Result<Option<Note>, sqlx::Error> {
        let row = sqlx::query("SELECT * FROM notes WHERE id = ?")
            .bind(note_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|row| Note {
            id: row.get("id"),
            user_id: row.get("user_id"),
            title: row.get("title"),
            content: row.get("content"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn delete_note(&self, note_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(note_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn search_notes(&self, user_id: i64, query: &str) -> Result<Vec<Note>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT * FROM notes WHERE user_id = ? AND (title LIKE ? OR content LIKE ?) ORDER BY updated_at DESC"
        )
        .bind(user_id)
        .bind(format!("%{}%", query))
        .bind(format!("%{}%", query))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| Note {
            id: row.get("id"),
            user_id: row.get("user_id"),
            title: row.get("title"),
            content: row.get("content"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }).collect())
    }

    pub async fn add_tag(&self, note_id: i64, tag_name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR IGNORE INTO tags (name) VALUES (?)")
            .bind(tag_name)
            .execute(&self.pool)
            .await?;

        let row = sqlx::query("SELECT id FROM tags WHERE name = ?")
            .bind(tag_name)
            .fetch_one(&self.pool)
            .await?;

        let tag_id: i64 = row.get("id");

        sqlx::query("INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?, ?)")
            .bind(note_id)
            .bind(tag_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_notes_by_tag(&self, user_id: i64, tag_name: &str) -> Result<Vec<Note>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT n.id, n.user_id, n.title, n.content, n.created_at, n.updated_at FROM notes n
            JOIN note_tags nt ON n.id = nt.note_id
            JOIN tags t ON nt.tag_id = t.id
            WHERE n.user_id = ? AND t.name = ?
            ORDER BY n.updated_at DESC
            "#,
        )
        .bind(user_id)
        .bind(tag_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| Note {
            id: row.get("id"),
            user_id: row.get("user_id"),
            title: row.get("title"),
            content: row.get("content"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }).collect())
    }

    pub async fn set_reminder(&self, note_id: i64, remind_at: NaiveDateTime) -> Result<Reminder, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO reminders (note_id, remind_at) VALUES (?, ?) RETURNING *"
        )
        .bind(note_id)
        .bind(remind_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(Reminder {
            id: result.get("id"),
            note_id: result.get("note_id"),
            remind_at: result.get("remind_at"),
            is_sent: result.get("is_sent"),
        })
    }

    pub async fn get_pending_reminders(&self) -> Result<Vec<(Reminder, Note)>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();
        let rows = sqlx::query(
            r#"
            SELECT r.id as r_id, r.note_id as r_note_id, r.remind_at as r_remind_at, r.is_sent as r_is_sent,
                   n.id as n_id, n.user_id as n_user_id, n.title as n_title, n.content as n_content,
                   n.created_at as n_created_at, n.updated_at as n_updated_at
            FROM reminders r
            JOIN notes n ON r.note_id = n.id
            WHERE r.is_sent = FALSE AND r.remind_at <= ?
            "#,
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            let reminder = Reminder {
                id: row.get("r_id"),
                note_id: row.get("r_note_id"),
                remind_at: row.get("r_remind_at"),
                is_sent: row.get("r_is_sent"),
            };
            let note = Note {
                id: row.get("n_id"),
                user_id: row.get("n_user_id"),
                title: row.get("n_title"),
                content: row.get("n_content"),
                created_at: row.get("n_created_at"),
                updated_at: row.get("n_updated_at"),
            };
            result.push((reminder, note));
        }

        Ok(result)
    }

    pub async fn mark_reminder_sent(&self, reminder_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE reminders SET is_sent = TRUE WHERE id = ?")
            .bind(reminder_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}