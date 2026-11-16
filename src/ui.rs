use crate::lyrics::{active_line_index, LyricSet};
use crate::player::Track;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Stdout, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Default)]
pub struct ScreenState {
    pub lyrics: Option<LyricSet>,
    pub fetching: bool,
    pub error: Option<String>,
    pub scroll: usize,
}

pub fn draw(
    stdout: &mut Stdout,
    state: &mut ScreenState,
    track: Option<&Track>,
    position: f64,
    offset_ms: i64,
    minimal_ui: bool,
) -> io::Result<()> {
    let (columns, rows) = terminal::size()?;
    let width = columns as usize;
    let height = rows as usize;
    queue!(stdout, Clear(ClearType::All))?;
    if width == 0 || height == 0 {
        return stdout.flush();
    }

    let chrome_rows = if minimal_ui { 0 } else { 4 };
    if let Some(lyrics) = state.lyrics.as_ref().filter(|lyrics| !lyrics.synced) {
        let wrapped_count: usize = lyrics.lines.iter().map(|line| wrap(&line.text, width).len()).sum();
        let visible = height.saturating_sub(chrome_rows).max(1);
        state.scroll = state.scroll.min(wrapped_count.saturating_sub(visible));
    }

    if !minimal_ui {
        put_at(stdout, 0, 0, "lyrix-rs by d1ezz9", Color::Cyan)?;
        let controls = "q quit · r retry";
        put_at(stdout, width.saturating_sub(UnicodeWidthStr::width(controls)), 0, controls, Color::DarkGrey)?;
        if let Some(track) = track {
            let heading = format!("{} — {}", track.artist, track.title);
            let heading = wrap(&heading, width).into_iter().next().unwrap_or_default();
            put_centered(stdout, 1.min(height - 1), &heading, width, Color::DarkGrey)?;
        }
    }

    match (&state.lyrics, track) {
        (_, None) => put_centered(stdout, height / 2, "Waiting for a playing track…", width, Color::DarkGrey)?,
        (None, Some(_)) if state.fetching => put_centered(stdout, height / 2, "Looking up lyrics…", width, Color::DarkGrey)?,
        (None, Some(_)) => {
            let message = state.error.as_deref().unwrap_or("No lyrics found");
            let message = wrap(message, width).into_iter().next().unwrap_or_default();
            put_centered(stdout, height / 2, &message, width, Color::DarkGrey)?;
        }
        (Some(lyrics), Some(_)) if lyrics.synced => {
            draw_synced(stdout, lyrics, position, offset_ms, width, height, minimal_ui)?;
        }
        (Some(lyrics), Some(_)) => {
            draw_plain(stdout, lyrics, state.scroll, width, height, minimal_ui)?;
        }
    }

    let is_plain_lyrics = state.lyrics.as_ref().is_some_and(|lyrics| !lyrics.synced);
    if !minimal_ui && is_plain_lyrics && height > 2 {
        put_centered(stdout, height - 1, "↑/↓ or j/k scroll", width, Color::DarkGrey)?;
    }
    stdout.flush()
}

fn draw_synced(
    stdout: &mut Stdout,
    lyrics: &LyricSet,
    position: f64,
    offset_ms: i64,
    width: usize,
    height: usize,
    minimal_ui: bool,
) -> io::Result<()> {
    let current = active_line_index(lyrics, position, offset_ms);
    let mut display: Vec<(String, Color)> = Vec::new();
    if let Some(index) = current {
        if index > 0 {
            let previous = wrap(&lyrics.lines[index - 1].text, width);
            if let Some(text) = previous.last() {
                display.push((text.clone(), Color::DarkGrey));
            }
        }
        display.extend(wrap(&lyrics.lines[index].text, width).into_iter().map(|text| (text, Color::White)));
        if let Some(next) = lyrics.lines.get(index + 1) {
            if let Some(text) = wrap(&next.text, width).first() {
                display.push((text.clone(), Color::DarkGrey));
            }
        }
    } else {
        display.push(("…".to_owned(), Color::DarkGrey));
    }

    let chrome_rows = if minimal_ui { 0 } else { 4 };
    let available = height.saturating_sub(chrome_rows).max(1);
    if display.len() > available {
        // Preserve the current lyric while clipping unusually long wrapped lines.
        let current_start = display.iter().position(|(_, color)| *color == Color::White).unwrap_or(0);
        let current_end = display.iter().rposition(|(_, color)| *color == Color::White).unwrap_or(current_start);
        let allowed_current = available.saturating_sub(2).max(1);
        let current_slice: Vec<_> = display[current_start..=current_end].iter().take(allowed_current).cloned().collect();
        display = current_slice;
    }
    let start_y = height.saturating_sub(display.len()) / 2;
    for (offset, (text, color)) in display.iter().enumerate() {
        put_centered(stdout, start_y + offset, text, width, *color)?;
    }
    Ok(())
}

fn draw_plain(
    stdout: &mut Stdout,
    lyrics: &LyricSet,
    scroll: usize,
    width: usize,
    height: usize,
    minimal_ui: bool,
) -> io::Result<()> {
    let wrapped: Vec<String> = lyrics.lines.iter().flat_map(|line| wrap(&line.text, width)).collect();
    let chrome_rows = if minimal_ui { 0 } else { 4 };
    let visible = height.saturating_sub(chrome_rows).max(1);
    let max_scroll = wrapped.len().saturating_sub(visible);
    let start = scroll.min(max_scroll);
    let end = (start + visible).min(wrapped.len());
    let displayed = &wrapped[start..end];
    let first_y = if minimal_ui {
        height.saturating_sub(displayed.len()) / 2
    } else {
        2 + visible.saturating_sub(displayed.len()) / 2
    };
    for (offset, text) in displayed.iter().enumerate() {
        put_centered(stdout, first_y + offset, text, width, Color::White)?;
    }
    Ok(())
}

fn put_centered(stdout: &mut Stdout, row: usize, text: &str, width: usize, color: Color) -> io::Result<()> {
    let text_width = UnicodeWidthStr::width(text);
    let x = width.saturating_sub(text_width) / 2;
    put_at(stdout, x, row, text, color)
}

fn put_at(stdout: &mut Stdout, column: usize, row: usize, text: &str, color: Color) -> io::Result<()> {
    queue!(stdout, MoveTo(column.min(u16::MAX as usize) as u16, row.min(u16::MAX as usize) as u16), SetForegroundColor(color), Print(text), ResetColor)
}

fn wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    for word in text.split_whitespace() {
        let word_width = UnicodeWidthStr::width(word);
        if !current.is_empty() && current_width + 1 + word_width <= max_width {
            current.push(' ');
            current_width += 1;
        } else if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        for ch in word.chars() {
            let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width + char_width > max_width && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            current.push(ch);
            current_width += char_width;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
