use teloxide::prelude::*;
use teloxide::types::Message;
use teloxide::utils::command::BotCommands;

use crate::db::Database;
use crate::utils;

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "Бот для личных заметок. Используйте команды:"
)]
pub enum Command {
    #[command(description = "Начать работу с ботом")]
    Start,
    #[command(description = "Добавить заметку: /add <название> | <текст>")]
    Add(String),
    #[command(description = "Посмотреть все заметки")]
    List,
    #[command(description = "Посмотреть заметку: /view <id>")]
    View(String),
    #[command(description = "Удалить заметку: /delete <id>")]
    Delete(String),
    #[command(description = "Поиск заметок: /search <запрос>")]
    Search(String),
    #[command(description = "Добавить тег: /tag <id> <тег>")]
    Tag(String),
    #[command(description = "Заметки по тегу: /bytag <тег>")]
    ByTag(String),
    #[command(description = "Напоминание: /remind <id> <дата>")]
    Remind(String),
    #[command(description = "Экспорт всех заметок")]
    Export,
    #[command(description = "Помощь")]
    Help,
}

#[derive(Clone)]
pub struct Handlers {
    db: Database,
}

impl Handlers {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn handle_message(
        &self,
        bot: Bot,
        msg: Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_id = msg.from.as_ref().unwrap().id.0 as i64;

        if let Some(text) = msg.text() {
            if text.starts_with('/') {
                return Ok(());
            }

            let title = text.lines().next().unwrap_or("Без названия").to_string();
            let content = text.to_string();

            self.db.create_note(user_id, &title, &content).await?;
            bot.send_message(msg.chat.id, "✅ Заметка сохранена!")
                .await?;
        }
        Ok(())
    }

    pub async fn handle_command(
        &self,
        bot: Bot,
        msg: Message,
        cmd: Command,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_id = msg.from.as_ref().unwrap().id.0 as i64;

        match cmd {
            Command::Start => {
                bot.send_message(
                    msg.chat.id,
                    "👋 Привет! Я бот для личных заметок.\n\n\
                     Просто отправьте текст — и он сохранится как заметка.\n\
                     Или используйте команды из /help.",
                )
                .await?;
            }
            Command::Help => {
                bot.send_message(msg.chat.id, Command::descriptions().to_string())
                    .await?;
            }
            Command::Add(args) => {
                let parts: Vec<&str> = args.splitn(2, '|').collect();
                let (title, content) = if parts.len() == 2 {
                    (parts[0].trim().to_string(), parts[1].trim().to_string())
                } else {
                    (args.trim().lines().next().unwrap_or("Без названия").to_string(), args.trim().to_string())
                };

                self.db.create_note(user_id, &title, &content).await?;
                bot.send_message(msg.chat.id, "✅ Заметка сохранена!")
                    .await?;
            }
            Command::List => {
                let notes = self.db.get_notes(user_id, 1, 10).await?;
                let text = utils::format_note_list(&notes, 1);
                bot.send_message(msg.chat.id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Command::View(id_str) => {
                let id: i64 = match id_str.trim().parse() {
                    Ok(id) => id,
                    Err(_) => {
                        bot.send_message(msg.chat.id, "❌ Неверный ID. Используйте /view <число>")
                            .await?;
                        return Ok(());
                    }
                };
                match self.db.get_note(id).await? {
                    Some(note) => {
                        bot.send_message(msg.chat.id, utils::format_note(&note))
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .await?;
                    }
                    None => {
                        bot.send_message(msg.chat.id, "❌ Заметка не найдена.")
                            .await?;
                    }
                }
            }
            Command::Delete(id_str) => {
                let id: i64 = match id_str.trim().parse() {
                    Ok(id) => id,
                    Err(_) => {
                        bot.send_message(msg.chat.id, "❌ Неверный ID.")
                            .await?;
                        return Ok(());
                    }
                };
                match self.db.get_note(id).await? {
                    Some(_) => {
                        self.db.delete_note(id).await?;
                        bot.send_message(msg.chat.id, "✅ Заметка удалена.")
                            .await?;
                    }
                    None => {
                        bot.send_message(msg.chat.id, "❌ Заметка не найдена.")
                            .await?;
                    }
                }
            }
            Command::Search(query) => {
                let notes = self.db.search_notes(user_id, query.trim()).await?;
                let text = utils::format_note_list(&notes, 1);
                bot.send_message(msg.chat.id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Command::Tag(args) => {
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                if parts.len() < 2 {
                    bot.send_message(msg.chat.id, "❌ Формат: /tag <id> <тег>")
                        .await?;
                    return Ok(());
                }
                let note_id: i64 = match parts[0].trim().parse() {
                    Ok(id) => id,
                    Err(_) => {
                        bot.send_message(msg.chat.id, "❌ Неверный ID.")
                            .await?;
                        return Ok(());
                    }
                };
                let tag_name = parts[1].trim();
                match self.db.get_note(note_id).await? {
                    Some(_) => {
                        self.db.add_tag(note_id, tag_name).await?;
                        bot.send_message(
                            msg.chat.id,
                            format!("✅ Тег '{}' добавлен к заметке #{}", tag_name, note_id),
                        )
                        .await?;
                    }
                    None => {
                        bot.send_message(msg.chat.id, "❌ Заметка не найдена.")
                            .await?;
                    }
                }
            }
            Command::ByTag(tag) => {
                let notes = self.db.get_notes_by_tag(user_id, tag.trim()).await?;
                let text = utils::format_note_list(&notes, 1);
                bot.send_message(msg.chat.id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Command::Remind(args) => {
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                if parts.len() < 2 {
                    bot.send_message(
                        msg.chat.id,
                        "❌ Формат: /remind <id> <дата>\nПример: /remind 1 2025-12-31 18:00",
                    )
                    .await?;
                    return Ok(());
                }
                let note_id: i64 = match parts[0].trim().parse() {
                    Ok(id) => id,
                    Err(_) => {
                        bot.send_message(msg.chat.id, "❌ Неверный ID.")
                            .await?;
                        return Ok(());
                    }
                };
                let date_str = parts[1].trim();
                match utils::parse_datetime(date_str) {
                    Ok(remind_at) => {
                        match self.db.get_note(note_id).await? {
                            Some(_) => {
                                self.db.set_reminder(note_id, remind_at).await?;
                                bot.send_message(
                                    msg.chat.id,
                                    format!(
                                        "⏰ Напоминание на {}",
                                        remind_at.format("%d.%m.%Y %H:%M")
                                    ),
                                )
                                .await?;
                            }
                            None => {
                                bot.send_message(msg.chat.id, "❌ Заметка не найдена.")
                                    .await?;
                            }
                        }
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("❌ {}", e))
                            .await?;
                    }
                }
            }
            Command::Export => {
                let notes = self.db.get_notes(user_id, 1, 1000).await?;
                let text = utils::export_notes_to_text(&notes);
                bot.send_message(msg.chat.id, text).await?;
            }
        }

        Ok(())
    }
}