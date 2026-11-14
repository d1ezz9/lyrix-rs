use anyhow::{Context, Result};
use std::process::Command;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Track {
    pub artist: String,
    pub title: String,
}

impl Track {
    pub fn cache_key(&self) -> String {
        format!("{}\u{1f}{}", normalize(&self.artist), normalize(&self.title))
    }
}

fn normalize(text: &str) -> String {
    text.trim().to_lowercase()
}

fn playerctl(player: Option<&str>) -> Command {
    let mut command = Command::new("playerctl");
    if let Some(name) = player {
        command.args(["-p", name]);
    }
    command
}

pub fn current_track(player: Option<&str>) -> Result<Option<Track>> {
    let output = playerctl(player)
        .args(["metadata", "--format", "{{artist}}\u{1f}{{title}}"])
        .output()
        .context("failed to run playerctl")?;
    if !output.status.success() {
        return Ok(None);
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let Some((artist, title)) = raw.trim().split_once('\u{1f}') else {
        return Ok(None);
    };
    let artist = artist.trim();
    let title = title.trim();
    if artist.is_empty() || title.is_empty() {
        return Ok(None);
    }
    Ok(Some(Track { artist: artist.to_owned(), title: title.to_owned() }))
}

pub fn position(player: Option<&str>) -> f64 {
    playerctl(player)
        .args(["position"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0)
}
