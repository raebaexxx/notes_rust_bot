use chrono::{NaiveDateTime, NaiveDate};

use crate::models::Note;

pub fn parse_datetime(input: &str) -> Result<NaiveDateTime, String> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt);
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%d.%m.%Y %H:%M:%S") {
        return Ok(dt);
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%d.%m.%Y %H:%M") {
        return Ok(dt);
    }

    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date.and_hms_opt(0, 0, 0).unwrap());
    }

    if let Ok(date) = NaiveDate::parse_from_str(input, "%d.%m.%Y") {
        return Ok(date.and_hms_opt(0, 0, 0).unwrap());
    }

    Err(format!("Не удалось распознать дату: {}. Форматы: YYYY-MM-DD HH:MM:SS, DD.MM.YYYY HH:MM", input))
}

pub fn format_note(note: &Note) -> String {
    format!(
        "📝 <b>{}</b>\n\n{}\n\n_ID: {}_\n_Создано: {}_\n_Обновлено: {}_",
        note.title,
        note.content,
        note.id,
        note.created_at.format("%d.%m.%Y %H:%M"),
        note.updated_at.format("%d.%m.%Y %H:%M")
    )
}

pub fn format_note_list(notes: &[Note], page: u32) -> String {
    if notes.is_empty() {
        return "📭 У вас пока нет заметок.".to_string();
    }

    let total_notes = notes.len();
    let mut result = format!("📋 <b>Заметки</b> (стр. {}) — всего {}\n\n", page, total_notes);

    for note in notes {
        let preview: String = note.content.chars().take(50).collect();
        result.push_str(&format!(
            "• <b>{}</b> (ID: {})\n  <i>{}</i>\n\n",
            note.title,
            note.id,
            preview
        ));
    }

    result
}

pub fn export_notes_to_text(notes: &[Note]) -> String {
    let mut result = String::from("📝 ЭКСПОРТ ЗАМЕТОК\n");
    result.push_str(&format!("Дата экспорта: {}\n", chrono::Utc::now().format("%d.%m.%Y %H:%M UTC")));
    result.push_str(&"=".repeat(50));
    result.push_str("\n\n");

    for note in notes {
        result.push_str(&format!(
            "Заметка #{}\nНазвание: {}\nСоздано: {}\nОбновлено: {}\n{}\n",
            note.id,
            note.title,
            note.created_at.format("%d.%m.%Y %H:%M"),
            note.updated_at.format("%d.%m.%Y %H:%M"),
            note.content
        ));
        result.push_str(&"-".repeat(50));
        result.push_str("\n\n");
    }

    result
}