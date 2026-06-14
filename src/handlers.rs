use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, CallbackQuery, Message, InputFile,
};
use serde::{Serialize, Deserialize};

use crate::db::Database;
use crate::utils;

const MAX_NOTE_TITLE: usize = 200;
const MAX_NOTE_CONTENT: usize = 4000;
const MAX_MESSAGE_LEN: usize = 4000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallbackAction {
    MainMenu,
    AddNote,
    ListNotes(u32),
    ViewNote(i64),
    DeleteNote(i64),
    ConfirmDelete(i64),
    Search,
    TagNote(i64),
    RemindNote(i64),
    Export,
    Settings,
}

#[derive(Debug, Clone)]
pub enum DialogueState {
    Idle,
    WaitingTitle,
    WaitingContent,
    WaitingSearch,
    WaitingTag(i64),
    WaitingRemindDate(i64),
}

#[derive(Clone)]
pub struct Handlers {
    db: Database,
    states: Arc<Mutex<HashMap<i64, DialogueState>>>,
    creating_notes: Arc<Mutex<HashMap<i64, (String, String)>>>,
}

impl Handlers {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            states: Arc::new(Mutex::new(HashMap::new())),
            creating_notes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn cb(action: &CallbackAction) -> String {
        serde_json::to_string(action).expect("serialization should not fail")
    }

    pub fn main_menu() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("📝 Добавить", Self::cb(&CallbackAction::AddNote)),
                InlineKeyboardButton::callback("📋 Мои заметки", Self::cb(&CallbackAction::ListNotes(1))),
            ],
            vec![
                InlineKeyboardButton::callback("🔍 Поиск", Self::cb(&CallbackAction::Search)),
                InlineKeyboardButton::callback("⚙️ Настройки", Self::cb(&CallbackAction::Settings)),
            ],
        ])
    }

    pub fn back_menu() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback("◀ Меню", Self::cb(&CallbackAction::MainMenu))],
        ])
    }

    pub fn note_actions(note_id: i64) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("✅ Удалить", Self::cb(&CallbackAction::DeleteNote(note_id))),
                InlineKeyboardButton::callback("🏷 Тег", Self::cb(&CallbackAction::TagNote(note_id))),
            ],
            vec![
                InlineKeyboardButton::callback("⏰ Напоминание", Self::cb(&CallbackAction::RemindNote(note_id))),
                InlineKeyboardButton::callback("◀ Назад", Self::cb(&CallbackAction::ListNotes(1))),
            ],
        ])
    }

    pub fn settings_menu() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback("📥 Экспорт .md", Self::cb(&CallbackAction::Export))],
            vec![InlineKeyboardButton::callback("◀ Меню", Self::cb(&CallbackAction::MainMenu))],
        ])
    }

    pub async fn handle_message(
        &self,
        bot: Bot,
        msg: Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_id = match msg.from.as_ref() {
            Some(u) => u.id.0 as i64,
            None => return Ok(()),
        };
        let chat_id = msg.chat.id;
        let text = msg.text().unwrap_or("");

        let (state, creating_title) = {
            let states = self.states.lock().await;
            let creating = self.creating_notes.lock().await;
            (
                states.get(&user_id).cloned().unwrap_or(DialogueState::Idle),
                creating.get(&user_id).map(|(t, _)| t.clone()),
            )
        };

        match state {
            DialogueState::WaitingTitle => {
                let title = text.trim().to_string();
                if title.is_empty() {
                    bot.send_message(chat_id, "❌ Название не может быть пустым. Попробуйте ещё раз:")
                        .await?;
                    return Ok(());
                }
                if title.len() > MAX_NOTE_TITLE {
                    bot.send_message(chat_id, format!("❌ Слишком длинное название (макс. {} символов)", MAX_NOTE_TITLE))
                        .await?;
                    return Ok(());
                }
                {
                    let mut creating = self.creating_notes.lock().await;
                    creating.insert(user_id, (title, String::new()));
                }
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::WaitingContent);
                }
                bot.send_message(chat_id, "✏️ Теперь введите текст заметки:").await?;
            }
            DialogueState::WaitingContent => {
                let content = text.trim().to_string();
                if content.is_empty() {
                    bot.send_message(chat_id, "❌ Текст не может быть пустым. Попробуйте ещё раз:")
                        .await?;
                    return Ok(());
                }
                if content.len() > MAX_NOTE_CONTENT {
                    bot.send_message(chat_id, format!("❌ Слишком длинный текст (макс. {} символов)", MAX_NOTE_CONTENT))
                        .await?;
                    return Ok(());
                }
                let title = match creating_title {
                    Some(t) => t,
                    None => {
                        bot.send_message(chat_id, "❌ Ошибка: название утеряно. Начните заново.")
                            .reply_markup(Self::main_menu())
                            .await?;
                        {
                            let mut states = self.states.lock().await;
                            states.insert(user_id, DialogueState::Idle);
                        }
                        return Ok(());
                    }
                };
                self.db.create_note(user_id, &title, &content).await?;
                {
                    let mut creating = self.creating_notes.lock().await;
                    creating.remove(&user_id);
                }
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::Idle);
                }
                bot.send_message(chat_id, "✅ Заметка сохранена!")
                    .reply_markup(Self::back_menu())
                    .await?;
            }
            DialogueState::WaitingSearch => {
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::Idle);
                }
                let notes = self.db.search_notes(user_id, text).await?;
                if notes.is_empty() {
                    bot.send_message(chat_id, "🔍 Ничего не найдено.")
                        .reply_markup(Self::back_menu())
                        .await?;
                } else {
                    let mut response = "🔍 Результаты поиска:\n\n".to_string();
                    let mut buttons = Vec::new();
                    for note in &notes {
                        let title = utils::escape_html(&note.title);
                        response.push_str(&format!("• {} (ID: {})\n", title, note.id));
                        buttons.push(vec![
                            InlineKeyboardButton::callback(
                                format!("👁 #{}", note.id),
                                Self::cb(&CallbackAction::ViewNote(note.id)),
                            ),
                        ]);
                    }
                    if response.len() > MAX_MESSAGE_LEN {
                        response.truncate(MAX_MESSAGE_LEN);
                        response.push_str("\n\n...(обрезано)");
                    }
                    buttons.push(vec![
                        InlineKeyboardButton::callback("◀ Меню", Self::cb(&CallbackAction::MainMenu)),
                    ]);
                    bot.send_message(chat_id, response)
                        .reply_markup(InlineKeyboardMarkup::new(buttons))
                        .await?;
                }
            }
            DialogueState::WaitingTag(note_id) => {
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::Idle);
                }
                let tag = text.trim().to_string();
                if tag.is_empty() {
                    bot.send_message(chat_id, "❌ Тег не может быть пустым.")
                        .reply_markup(Self::note_actions(note_id))
                        .await?;
                    return Ok(());
                }
                self.db.add_tag(note_id, user_id, &tag).await?;
                bot.send_message(chat_id, format!("✅ Тег '{}' добавлен к заметке #{}", utils::escape_html(&tag), note_id))
                    .reply_markup(Self::note_actions(note_id))
                    .await?;
            }
            DialogueState::WaitingRemindDate(note_id) => {
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::Idle);
                }
                match utils::parse_datetime(text) {
                    Ok(remind_at) => {
                        self.db.set_reminder(note_id, user_id, remind_at).await?;
                        bot.send_message(
                            chat_id,
                            format!("⏰ Напоминание на {}", remind_at.format("%d.%m.%Y %H:%M")),
                        )
                        .reply_markup(Self::note_actions(note_id))
                        .await?;
                    }
                    Err(e) => {
                        bot.send_message(chat_id, format!("❌ {}", e))
                            .reply_markup(Self::note_actions(note_id))
                            .await?;
                    }
                }
            }
            DialogueState::Idle => {
                bot.send_message(chat_id, "👋 Привет! Я бот для заметок.")
                    .reply_markup(Self::main_menu())
                    .await?;
            }
        }

        Ok(())
    }

    async fn edit_or_send(
        bot: &Bot,
        msg: &Option<teloxide::types::MaybeInaccessibleMessage>,
        text: String,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = msg {
            let _ = bot.edit_message_text(msg.chat.id, msg.id, text)
                .reply_markup(markup)
                .await;
        } else {
            bot.send_message(ChatId(0), text)
                .reply_markup(markup)
                .await?;
        }
        Ok(())
    }

    pub async fn handle_callback_query(
        &self,
        bot: Bot,
        q: CallbackQuery,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_id = q.from.id.0 as i64;
        let chat_id = q.message.as_ref().map(|m| m.chat().id).unwrap_or(ChatId(user_id));
        let data = q.data.clone();
        let data = data.as_deref().unwrap_or("");
        let msg = q.message.clone();

        bot.answer_callback_query(q.id).await?;

        let action: CallbackAction = match serde_json::from_str(data) {
            Ok(a) => a,
            Err(_) => return Ok(()),
        };

        match action {
            CallbackAction::MainMenu => {
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::Idle);
                }
                Self::edit_or_send(&bot, &msg, "👋 Главное меню:".into(), Self::main_menu()).await?;
            }
            CallbackAction::AddNote => {
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::WaitingTitle);
                }
                bot.send_message(chat_id, "✏️ Введите название заметки:").await?;
            }
            CallbackAction::ListNotes(page) => {
                let notes = self.db.get_notes(user_id, page, 5).await?;
                if notes.is_empty() {
                    Self::edit_or_send(&bot, &msg, "📭 У вас пока нет заметок.".into(), Self::main_menu()).await?;
                } else {
                    let mut response = format!("📋 Заметки (стр. {}):\n\n", page);
                    let mut buttons = Vec::new();
                    for note in &notes {
                        let title = utils::escape_html(&note.title);
                        response.push_str(&format!("• {} (ID: {})\n", title, note.id));
                        buttons.push(vec![
                            InlineKeyboardButton::callback(
                                format!("👁 #{}", note.id),
                                Self::cb(&CallbackAction::ViewNote(note.id)),
                            ),
                        ]);
                    }
                    let mut nav = Vec::new();
                    if page > 1 {
                        nav.push(InlineKeyboardButton::callback(
                            "◀",
                            Self::cb(&CallbackAction::ListNotes(page - 1)),
                        ));
                    }
                    nav.push(InlineKeyboardButton::callback(
                        "◀ Меню",
                        Self::cb(&CallbackAction::MainMenu),
                    ));
                    if notes.len() == 5 {
                        nav.push(InlineKeyboardButton::callback(
                            "▶",
                            Self::cb(&CallbackAction::ListNotes(page + 1)),
                        ));
                    }
                    buttons.push(nav);
                    Self::edit_or_send(&bot, &msg, response, InlineKeyboardMarkup::new(buttons)).await?;
                }
            }
            CallbackAction::ViewNote(id) => {
                match self.db.get_note(id, user_id).await? {
                    Some(note) => {
                        let text = utils::format_note(&note);
                        bot.send_message(chat_id, text)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .reply_markup(Self::note_actions(id))
                            .await?;
                    }
                    None => {
                        bot.send_message(chat_id, "❌ Заметка не найдена.")
                            .reply_markup(Self::main_menu())
                            .await?;
                    }
                }
            }
            CallbackAction::DeleteNote(id) => {
                if self.db.get_note(id, user_id).await?.is_none() {
                    bot.send_message(chat_id, "❌ Заметка не найдена.")
                        .reply_markup(Self::main_menu())
                        .await?;
                    return Ok(());
                }
                let buttons = InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback("✅ Да, удалить", Self::cb(&CallbackAction::ConfirmDelete(id))),
                        InlineKeyboardButton::callback("❌ Нет", Self::cb(&CallbackAction::ViewNote(id))),
                    ],
                ]);
                bot.send_message(chat_id, format!("⚠️ Удалить заметку #{}?", id))
                    .reply_markup(buttons)
                    .await?;
            }
            CallbackAction::ConfirmDelete(id) => {
                self.db.delete_note(id, user_id).await?;
                bot.send_message(chat_id, "✅ Заметка удалена.")
                    .reply_markup(Self::main_menu())
                    .await?;
            }
            CallbackAction::Search => {
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::WaitingSearch);
                }
                bot.send_message(chat_id, "🔍 Введите поисковый запрос:").await?;
            }
            CallbackAction::TagNote(id) => {
                if self.db.get_note(id, user_id).await?.is_none() {
                    bot.send_message(chat_id, "❌ Заметка не найдена.")
                        .reply_markup(Self::main_menu())
                        .await?;
                    return Ok(());
                }
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::WaitingTag(id));
                }
                bot.send_message(chat_id, "🏷 Введите название тега:").await?;
            }
            CallbackAction::RemindNote(id) => {
                if self.db.get_note(id, user_id).await?.is_none() {
                    bot.send_message(chat_id, "❌ Заметка не найдена.")
                        .reply_markup(Self::main_menu())
                        .await?;
                    return Ok(());
                }
                {
                    let mut states = self.states.lock().await;
                    states.insert(user_id, DialogueState::WaitingRemindDate(id));
                }
                bot.send_message(chat_id, "⏰ Введите дату напоминания\n(формат: ГГГГ-ММ-ДД ЧЧ:ММ:СС или ДД.ММ.ГГГГ ЧЧ:ММ):").await?;
            }
            CallbackAction::Settings => {
                Self::edit_or_send(&bot, &msg, "⚙️ Настройки:".into(), Self::settings_menu()).await?;
            }
            CallbackAction::Export => {
                let notes = self.db.get_notes(user_id, 1, 1000).await?;
                if notes.is_empty() {
                    bot.send_message(chat_id, "📭 Нечего экспортировать.")
                        .reply_markup(Self::main_menu())
                        .await?;
                } else {
                    let md_content = utils::export_notes_to_markdown(&notes);
                    let file_name = format!("notes_export_{}.md", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
                    bot.send_document(chat_id, InputFile::memory(md_content).file_name(file_name))
                        .reply_markup(Self::main_menu())
                        .await?;
                }
            }
        }

        Ok(())
    }
}