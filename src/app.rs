use crate::lyrics::{self, LyricsRequest, LyricsResult, LyricSet};
use crate::player::{self, Track};
use crate::ui::{self, ScreenState};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use std::io;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, crossterm::cursor::Hide) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, crossterm::cursor::Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

pub fn run(player_name: Option<String>, offset_ms: i64, poll_interval: Duration) -> Result<()> {
    let _terminal = TerminalGuard::enter()?;
    let mut stdout = io::stdout();
    let (request_tx, result_rx) = lyrics::start_worker();
    let mut state = ScreenState::default();
    let mut current_track: Option<Track> = None;
    let mut last_track_check = Instant::now() - Duration::from_secs(1);
    let mut last_position_check = Instant::now() - poll_interval;
    let mut position = 0.0;
    let mut last_drawn_index: Option<usize> = None;
    let mut dirty = true;

    loop {
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if should_quit(key) => break,
                Event::Key(key) => {
                    if handle_key(key, &mut state, current_track.as_ref(), &request_tx) {
                        dirty = true;
                    }
                }
                Event::Resize(_, _) => dirty = true,
                _ => {}
            }
        }

        while let Ok(result) = result_rx.try_recv() {
            if current_track.as_ref().is_some_and(|track| track.cache_key() == result.key) {
                apply_result(result, &mut state);
                last_drawn_index = None;
                dirty = true;
            }
        }

        if last_track_check.elapsed() >= Duration::from_millis(600) {
            last_track_check = Instant::now();
            let found = player::current_track(player_name.as_deref())?;
            if found != current_track {
                current_track = found;
                state.lyrics = None;
                state.fetching = false;
                state.error = None;
                state.scroll = 0;
                position = 0.0;
                last_position_check = Instant::now() - poll_interval;
                last_drawn_index = None;
                if let Some(track) = current_track.as_ref() {
                    state.fetching = request_tx.send(LyricsRequest { track: track.clone(), force: false }).is_ok();
                }
                dirty = true;
            }
        }

        if last_position_check.elapsed() >= poll_interval {
            last_position_check = Instant::now();
            position = player::position(player_name.as_deref());
            if let Some(lyrics) = state.lyrics.as_ref().filter(|lyrics| lyrics.synced) {
                let index = current_lyric_index(lyrics, position + offset_ms as f64 / 1000.0);
                if index != last_drawn_index {
                    last_drawn_index = index;
                    dirty = true;
                }
            }
        }

        if dirty {
            ui::draw(&mut stdout, &mut state, current_track.as_ref(), position, offset_ms)?;
            dirty = false;
        }
    }
    Ok(())
}

fn current_lyric_index(lyrics: &LyricSet, position: f64) -> Option<usize> {
    let count = lyrics.lines.partition_point(|line| line.time.unwrap_or(f64::INFINITY) <= position);
    count.checked_sub(1)
}

fn should_quit(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn handle_key(key: KeyEvent, state: &mut ScreenState, track: Option<&Track>, request_tx: &Sender<LyricsRequest>) -> bool {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            if state.lyrics.as_ref().is_some_and(|lyrics| !lyrics.synced) {
                state.scroll = state.scroll.saturating_sub(1);
                true
            } else {
                false
            }
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            if state.lyrics.as_ref().is_some_and(|lyrics| !lyrics.synced) {
                state.scroll = state.scroll.saturating_add(1);
                true
            } else {
                false
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if let Some(track) = track {
                state.lyrics = None;
                state.error = None;
                state.fetching = request_tx.send(LyricsRequest { track: track.clone(), force: true }).is_ok();
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

fn apply_result(result: LyricsResult, state: &mut ScreenState) {
    state.fetching = false;
    match result.result {
        Ok(Some(lyrics)) => {
            state.lyrics = Some(lyrics);
            state.error = None;
            state.scroll = 0;
        }
        Ok(None) => {
            state.lyrics = None;
            state.error = None;
        }
        Err(error) => {
            state.lyrics = None;
            state.error = Some(error);
        }
    }
}
