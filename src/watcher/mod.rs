use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::db::Database;
use crate::scoring::git::recently_committed_files;
use crate::scoring::{score_events, ScoreOptions, ScoringEvent};

pub mod events;
pub mod filter;

use events::event_paths;
use filter::IgnoreMatcher;

#[derive(Debug, Clone)]
pub struct WatchOptions {
    pub root: PathBuf,
    pub db_path: PathBuf,
    pub ignore_patterns: Vec<String>,
    pub use_gitignore: bool,
    pub git_enabled: bool,
    pub git_lookback_days: i64,
    pub git_commit_boost: f64,
    pub decay_days: i64,
    pub decay_rate: f64,
}

pub fn run(options: WatchOptions) -> Result<()> {
    let root = options
        .root
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", options.root.display()))?;
    let stop_file = stop_file_path(&options.db_path);
    if stop_file.exists() {
        fs::remove_file(&stop_file)
            .with_context(|| format!("failed to remove stale {}", stop_file.display()))?;
    }

    let mut db = Database::open(&options.db_path)?;
    let project_id = db.ensure_project(&root)?;
    let matcher = IgnoreMatcher::new(&root, options.use_gitignore, &options.ignore_patterns)?;
    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |result| {
        let _ = tx.send(result);
    })?;
    watcher.watch(&root, RecursiveMode::Recursive)?;

    println!("heatpath watching {}", root.display());
    let mut last_score_update = Instant::now() - Duration::from_secs(60);

    loop {
        if stop_file.exists() {
            break;
        }

        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(event)) => {
                let now = Utc::now();
                for (path, event_type) in event_paths(event) {
                    if matcher.is_ignored(&path) {
                        continue;
                    }
                    let Some(relative) = relative_path(&root, &path) else {
                        continue;
                    };
                    if relative.is_empty() {
                        continue;
                    }
                    db.record_event(project_id, &relative, event_type.as_str(), now)?;
                }
            }
            Ok(Err(err)) => eprintln!("watch error: {err}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if last_score_update.elapsed() >= Duration::from_secs(60) {
            recompute_scores(&mut db, project_id, &root, &options)?;
            last_score_update = Instant::now();
        }
    }

    recompute_scores(&mut db, project_id, &root, &options)?;
    if stop_file.exists() {
        let _ = fs::remove_file(stop_file);
    }
    Ok(())
}

pub fn request_stop(db_path: &Path) -> Result<()> {
    let path = stop_file_path(db_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, b"stop")?;
    Ok(())
}

pub fn stop_file_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("stop")
}

fn recompute_scores(
    db: &mut Database,
    project_id: i64,
    root: &Path,
    options: &WatchOptions,
) -> Result<()> {
    let events = db.events_for_project(project_id, None)?;
    let scoring_events: Vec<ScoringEvent> = events
        .into_iter()
        .map(|event| ScoringEvent {
            filepath: event.filepath,
            occurred_at: event.occurred_at,
        })
        .collect();
    let git_recent = if options.git_enabled {
        recently_committed_files(root, options.git_lookback_days)?
    } else {
        HashSet::new()
    };
    let files = score_events(
        &scoring_events,
        &git_recent,
        ScoreOptions {
            now: Utc::now(),
            decay_days: options.decay_days,
            decay_rate: options.decay_rate,
            git_enabled: options.git_enabled,
            git_commit_boost: options.git_commit_boost,
        },
    );
    db.replace_scores(project_id, &files)?;
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let relative = absolute.strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}
