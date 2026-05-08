use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

pub mod decay;
pub mod git;

#[derive(Debug, Clone)]
pub struct ScoringEvent {
    pub filepath: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileHeat {
    pub filepath: String,
    pub touches: u64,
    pub last_touched: DateTime<Utc>,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct ScoreOptions {
    pub now: DateTime<Utc>,
    pub decay_days: i64,
    pub decay_rate: f64,
    pub git_enabled: bool,
    pub git_commit_boost: f64,
}

#[derive(Debug, Clone)]
struct FileAccumulator {
    touches: u64,
    weighted_touches: f64,
    last_touched: DateTime<Utc>,
}

pub fn score_events(
    events: &[ScoringEvent],
    git_recent: &HashSet<String>,
    options: ScoreOptions,
) -> Vec<FileHeat> {
    let mut files: HashMap<String, FileAccumulator> = HashMap::new();

    for event in events {
        let filepath = normalize_path(&event.filepath);
        let weight = recency_weight(event.occurred_at, options.now);
        files
            .entry(filepath)
            .and_modify(|file| {
                file.touches += 1;
                file.weighted_touches += weight;
                if event.occurred_at > file.last_touched {
                    file.last_touched = event.occurred_at;
                }
            })
            .or_insert(FileAccumulator {
                touches: 1,
                weighted_touches: weight,
                last_touched: event.occurred_at,
            });
    }

    let mut scored: Vec<FileHeat> = files
        .into_iter()
        .map(|(filepath, file)| {
            let mut score = file.weighted_touches;
            if options.git_enabled && git_recent.contains(&filepath) {
                score *= 1.0 + options.git_commit_boost;
            }
            score *= decay::factor(
                file.last_touched,
                options.now,
                options.decay_days,
                options.decay_rate,
            );
            FileHeat {
                filepath,
                touches: file.touches,
                last_touched: file.last_touched,
                score,
            }
        })
        .collect();

    scored.sort_by(compare_heat);
    scored
}

pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn recency_weight(occurred_at: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let age = now.signed_duration_since(occurred_at);
    if age <= Duration::hours(24) {
        2.0
    } else if age <= Duration::days(7) {
        1.5
    } else {
        1.0
    }
}

fn compare_heat(left: &FileHeat, right: &FileHeat) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.touches.cmp(&left.touches))
        .then_with(|| right.last_touched.cmp(&left.last_touched))
        .then_with(|| left.filepath.cmp(&right.filepath))
}
