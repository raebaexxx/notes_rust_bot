use chrono::NaiveDateTime;

use crate::models::Note;

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn escape_markdown(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '#' | '*' | '_' | '[' | ']' | '(' | ')' | '`' | '>' | '-' | '+' | '=' | '|' | '{' | '}' | '.' | '!' => {
                result.push('\\');
                result.push(ch);
            }
            _ => result.push(ch),
        }
    }
    result
}

pub fn parse_datetime(input: &str) -> Result<NaiveDateTime, String> {
    let dt = if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        Some(dt)
    } else if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%d.%m.%Y %H:%M:%S") {
        Some(dt)
    } else if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%d.%m.%Y %H:%M") {
        Some(dt)
    } else if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M") {
        Some(dt)
    } else if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        date.and_hms_opt(0, 0, 0)
    } else if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%d.%m.%Y") {
        date.and_hms_opt(0, 0, 0)
    } else {
        None
    };

    match dt {
        Some(dt) => {
            let now = chrono::Utc::now().naive_utc();
            if dt <= now {
                Err("Дата должна быть в будущем".to_string())
            } else {
                Ok(dt)
            }
        }
        None => Err(format!(
            "Не удалось распознать дату: {}. Форматы: YYYY-MM-DD HH:MM:SS, DD.MM.YYYY HH:MM",
            input
        )),
    }
}

pub fn format_note(note: &Note) -> String {
    format!(
        "📝 <b>{}</b>\n\n{}\n\n_ID: {}_\n_Создано: {}_\n_Обновлено: {}_",
        escape_html(&note.title),
        escape_html(&note.content),
        note.id,
        note.created_at.format("%d.%m.%Y %H:%M"),
        note.updated_at.format("%d.%m.%Y %H:%M")
    )
}

pub fn export_notes_to_markdown(notes: &[Note]) -> Vec<u8> {
    let mut result = String::new();
    result.push_str("# Экспорт заметок\n\n");
    result.push_str(&format!(
        "Дата экспорта: {}\n\n",
        chrono::Utc::now().format("%d.%m.%Y %H:%M UTC")
    ));

    for note in notes {
        result.push_str(&format!("## {}\n\n", escape_markdown(&note.title)));
        result.push_str(&format!(
            "**ID:** {} | **Создано:** {} | **Обновлено:** {}\n\n",
            note.id,
            note.created_at.format("%d.%m.%Y %H:%M"),
            note.updated_at.format("%d.%m.%Y %H:%M")
        ));
        result.push_str(&format!("{}\n\n", escape_markdown(&note.content)));
        result.push_str("---\n\n");
    }

    result.into_bytes()
}