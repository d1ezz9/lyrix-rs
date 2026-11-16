use crate::player::Track;
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time: Option<f64>,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct LyricSet {
    pub lines: Vec<LyricLine>,
    pub synced: bool,
}

/// Return the active synchronized line. Negative offsets make lines appear earlier.
pub fn active_line_index(lyrics: &LyricSet, position: f64, offset_ms: i64) -> Option<usize> {
    let effective_position = position - offset_ms as f64 / 1000.0;
    let count = lyrics.lines.partition_point(|line| {
        line.time.unwrap_or(f64::INFINITY) <= effective_position
    });
    count.checked_sub(1)
}

#[derive(Debug)]
pub struct LyricsRequest {
    pub track: Track,
    pub force: bool,
}

#[derive(Debug)]
pub struct LyricsResult {
    pub key: String,
    pub result: std::result::Result<Option<LyricSet>, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibRecord {
    artist_name: Option<String>,
    track_name: Option<String>,
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
}

pub fn start_worker() -> (Sender<LyricsRequest>, Receiver<LyricsResult>) {
    let (request_tx, request_rx) = mpsc::channel::<LyricsRequest>();
    let (result_tx, result_rx) = mpsc::channel::<LyricsResult>();
    thread::Builder::new()
        .name("lyrix-lyrics-fetcher".to_owned())
        .spawn(move || worker(request_rx, result_tx))
        .expect("could not start lyrics worker");
    (request_tx, result_rx)
}

fn worker(requests: Receiver<LyricsRequest>, results: Sender<LyricsResult>) {
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!("lyrix/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| error.to_string());
    // Cache successful lookups. Misses and network errors stay retryable with the r key.
    let mut cache: HashMap<String, LyricSet> = HashMap::new();

    while let Ok(request) = requests.recv() {
        let key = request.track.cache_key();
        let result = match &client {
            Err(error) => Err(error.clone()),
            Ok(client) => {
                if !request.force {
                    if let Some(cached) = cache.get(&key) {
                        Ok(Some(cached.clone()))
                    } else {
                        fetch_and_cache(client, &request.track, &key, &mut cache)
                    }
                } else {
                    fetch_and_cache(client, &request.track, &key, &mut cache)
                }
            }
        };
        if results.send(LyricsResult { key, result }).is_err() {
            break;
        }
    }
}

fn fetch_and_cache(client: &Client, track: &Track, key: &str, cache: &mut HashMap<String, LyricSet>) -> std::result::Result<Option<LyricSet>, String> {
    match fetch(client, track) {
        Ok(value) => {
            if let Some(lyrics) = value.as_ref() {
                cache.insert(key.to_owned(), lyrics.clone());
            }
            Ok(value)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn fetch(client: &Client, track: &Track) -> Result<Option<LyricSet>> {
    let mut url = reqwest::Url::parse("https://lrclib.net/api/search")?;
    url.query_pairs_mut()
        .append_pair("track_name", &track.title)
        .append_pair("artist_name", &track.artist);
    let records: Vec<LrclibRecord> = client
        .get(url)
        .send()
        .context("could not connect to LRCLIB")?
        .error_for_status()
        .context("LRCLIB returned an error")?
        .json()
        .context("LRCLIB returned invalid data")?;

    let mut candidates: Vec<(i32, String)> = records
        .into_iter()
        .filter_map(|record| {
            let is_synced = record.synced_lyrics.as_ref().is_some_and(|text| !text.trim().is_empty());
            let title_score = record.track_name.as_deref()
                .map(|value| similarity(&normalize(value), &normalize(&track.title)))
                .unwrap_or(0);
            let artist_score = record.artist_name.as_deref()
                .map(|value| similarity(&normalize(value), &normalize(&track.artist)))
                .unwrap_or(0);
            let lyric_text = record.synced_lyrics.filter(|text| !text.trim().is_empty())
                .or_else(|| record.plain_lyrics.filter(|text| !text.trim().is_empty()))?;
            // Prefer exact metadata matches, then synced text when candidates are otherwise equal.
            let score = title_score * 4 + artist_score * 3 + (if is_synced { 1 } else { 0 });
            Some((score, lyric_text))
        })
        .collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(candidates.into_iter().next().and_then(|(_, text)| parse_lrc(&text)))
}

fn similarity(left: &str, right: &str) -> i32 {
    if left.is_empty() || right.is_empty() {
        0
    } else if left == right {
        2
    } else if left.contains(right) || right.contains(left) {
        1
    } else {
        0
    }
}

fn normalize(value: &str) -> String {
    value.chars().filter(|ch| ch.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

pub fn parse_lrc(input: &str) -> Option<LyricSet> {
    let mut lines = Vec::new();
    let mut has_timestamp = false;

    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let (timestamps, text) = extract_timestamps(line);
        if !timestamps.is_empty() {
            has_timestamp = true;
            for time in timestamps {
                lines.push(LyricLine { time: Some(time), text: text.trim().to_owned() });
            }
        } else {
            // Ignore standard LRC metadata (artist, album, offset, length, etc.).
            let is_metadata = line.strip_prefix('[')
                .and_then(|value| value.split_once(']'))
                .and_then(|(tag, _)| tag.split_once(':'))
                .is_some_and(|(name, _)| !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphabetic()));
            if !is_metadata {
                lines.push(LyricLine { time: None, text: line.to_owned() });
            }
        }
    }

    if has_timestamp {
        lines.retain(|line| line.time.is_some());
        lines.sort_by(|a, b| a.time.unwrap_or_default().total_cmp(&b.time.unwrap_or_default()));
    } else if lines.is_empty() {
        return None;
    }
    Some(LyricSet { lines, synced: has_timestamp })
}

fn extract_timestamps(line: &str) -> (Vec<f64>, &str) {
    let mut timestamps = Vec::new();
    let mut rest = line;
    loop {
        let Some(after_open) = rest.strip_prefix('[') else { break };
        let Some((tag, after_tag)) = after_open.split_once(']') else { break };
        let Some(time) = parse_timestamp(tag) else { break };
        timestamps.push(time);
        rest = after_tag;
    }
    (timestamps, rest)
}

fn parse_timestamp(tag: &str) -> Option<f64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes = minutes.parse::<u64>().ok()?;
    let seconds = seconds.parse::<f64>().ok()?;
    if !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some(minutes as f64 * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_timestamps_and_sorts_lines() {
        let lyrics = parse_lrc("[00:02.50]second\n[00:01.00][00:03.00]first").unwrap();
        assert!(lyrics.synced);
        assert_eq!(lyrics.lines.len(), 3);
        assert_eq!(lyrics.lines[0].text, "first");
        assert_eq!(lyrics.lines[1].time, Some(2.5));
        assert_eq!(lyrics.lines[2].time, Some(3.0));
    }

    #[test]
    fn ignores_metadata_and_keeps_plain_lyrics() {
        let lyrics = parse_lrc("[ar:An Artist]\n[ti:A Song]\nFirst line\nSecond line").unwrap();
        assert!(!lyrics.synced);
        assert_eq!(lyrics.lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>(), vec!["First line", "Second line"]);
    }

    #[test]
    fn negative_offset_activates_a_line_earlier() {
        let lyrics = parse_lrc("[00:10.00]line").unwrap();
        assert_eq!(active_line_index(&lyrics, 9.7, 0), None);
        assert_eq!(active_line_index(&lyrics, 9.7, -300), Some(0));
        assert_eq!(active_line_index(&lyrics, 10.3, 300), Some(0));
    }
}
