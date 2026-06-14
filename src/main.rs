mod db;
mod handlers;
mod models;
mod utils;

use std::env;
use teloxide::prelude::*;
use tokio::time::{interval, Duration};

use crate::db::Database;
use crate::handlers::Handlers;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv::dotenv().ok();

    let token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN not set");
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:notes.db".to_string());

    let db = Database::new(&database_url)
        .await
        .expect("Failed to initialize database");

    let bot = Bot::new(token);
    let handlers = Handlers::new(db.clone());

    let db_clone = db.clone();
    let bot_clone = bot.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            check_reminders(&db_clone, &bot_clone).await;
        }
    });

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint({
            let handlers = handlers.clone();
            move |bot: Bot, msg: Message| {
                let handlers = handlers.clone();
                async move { handlers.handle_message(bot, msg).await }
            }
        }))
        .branch(Update::filter_callback_query().endpoint({
            let handlers = handlers.clone();
            move |bot: Bot, q: CallbackQuery| {
                let handlers = handlers.clone();
                async move { handlers.handle_callback_query(bot, q).await }
            }
        }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn check_reminders(db: &Database, bot: &Bot) {
    match db.get_pending_reminders().await {
        Ok(reminders) => {
            for (reminder, note) in reminders {
                let text = format!(
                    "⏰ <b>Напоминание</b>\n\n<b>{}</b>\n\n{}",
                    utils::escape_html(&note.title),
                    utils::escape_html(&note.content)
                );

                match bot
                    .send_message(ChatId(note.user_id), text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await
                {
                    Ok(_) => {
                        if let Err(e) = db.mark_reminder_sent(reminder.id).await {
                            log::error!("Failed to mark reminder as sent: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to send reminder: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to check reminders: {}", e);
        }
    }
}