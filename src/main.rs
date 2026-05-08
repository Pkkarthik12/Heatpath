use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration as StdDuration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use heatpath::config::HeatpathConfig;
use heatpath::db::Database;
use heatpath::export;
use heatpath::scoring::git::recently_committed_files;
use heatpath::scoring::{score_events, FileHeat, ScoreOptions, ScoringEvent};
use heatpath::ui::{SortMode, UiOptions, ViewMode};
use heatpath::watcher::{self, WatchOptions};

#[derive(Debug, Parser)]
#[command(name = "heatpath")]
#[command(about = "Watch your filesystem and see where you actually work.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start watching a directory.
    Watch {
        /// Directory to watch.
        path: PathBuf,
        /// Extra glob pattern to exclude. Repeatable.
        #[arg(long = "ignore")]
        ignore: Vec<String>,
        /// Do not read .gitignore files.
        #[arg(long)]
        no_gitignore: bool,
        /// Custom SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Open the terminal UI immediately.
        #[arg(long)]
        ui: bool,
        /// Disable git-aware scoring.
        #[arg(long)]
        no_git: bool,
        /// Start decaying files after this many untouched days.
        #[arg(long)]
        decay_days: Option<i64>,
    },
    /// Open the live terminal dashboard.
    Ui {
        /// Initial view mode.
        #[arg(long, value_enum)]
        mode: Option<ModeArg>,
        /// Initial sort mode.
        #[arg(long, value_enum)]
        sort: Option<SortArg>,
        /// Maximum displayed path depth.
        #[arg(long)]
        depth: Option<usize>,
        /// Custom SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Print a summary to stdout.
    Stats {
        /// Number of hottest files to show.
        #[arg(long, default_value_t = 10)]
        top: usize,
        /// Restrict to a relative window, such as "7 days" or "24 hours".
        #[arg(long)]
        since: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
        /// Custom SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Disable git-aware scoring.
        #[arg(long)]
        no_git: bool,
        /// Start decaying files after this many untouched days.
        #[arg(long)]
        decay_days: Option<i64>,
    },
    /// Stop a running watcher.
    Stop {
        /// Stop and delete all recorded data.
        #[arg(long)]
        purge: bool,
        /// Custom SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ModeArg {
    Session,
    Lifetime,
    #[value(name = "git-weighted", alias = "git")]
    GitWeighted,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum SortArg {
    Touches,
    Recency,
    Git,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Csv,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = HeatpathConfig::load().unwrap_or_default();

    match cli.command {
        Commands::Watch {
            path,
            ignore,
            no_gitignore,
            db,
            ui,
            no_git,
            decay_days,
        } => {
            let db_path = db.unwrap_or_else(Database::default_path);
            let mut ignore_patterns = config.ignore.patterns.clone();
            ignore_patterns.extend(ignore);

            let decay_rate = if config.decay.enabled {
                config.decay.rate
            } else {
                0.0
            };
            let options = WatchOptions {
                root: path,
                db_path: db_path.clone(),
                ignore_patterns,
                use_gitignore: !no_gitignore,
                git_enabled: config.git.enabled && !no_git,
                git_lookback_days: config.git.lookback_days,
                git_commit_boost: config.git.commit_boost,
                decay_days: decay_days.unwrap_or(config.decay.window_days),
                decay_rate,
            };

            if ui {
                let ui_options = UiOptions {
                    db_path: db_path.clone(),
                    mode: ViewMode::Session,
                    sort: SortMode::Touches,
                    depth: config.defaults.depth,
                };
                let stop_db_path = db_path.clone();
                let handle = thread::spawn(move || watcher::run(options));
                thread::sleep(StdDuration::from_millis(500));
                let ui_result = heatpath::ui::run(ui_options);
                watcher::request_stop(&stop_db_path)?;
                match handle.join() {
                    Ok(Err(err)) => return Err(err),
                    Err(_) => bail!("watcher thread panicked"),
                    Ok(Ok(())) => {}
                }
                ui_result?;
            } else {
                watcher::run(options)?;
            }
        }
        Commands::Ui {
            mode,
            sort,
            depth,
            db,
        } => {
            let db_path = db.unwrap_or_else(Database::default_path);
            heatpath::ui::run(UiOptions {
                db_path,
                mode: mode.map(Into::into).unwrap_or(config.defaults.mode),
                sort: sort.map(Into::into).unwrap_or(config.defaults.sort),
                depth: depth.or(config.defaults.depth),
            })?;
        }
        Commands::Stats {
            top,
            since,
            format,
            db,
            no_git,
            decay_days,
        } => {
            let db_path = db.unwrap_or_else(Database::default_path);
            let decay_rate = if config.decay.enabled {
                config.decay.rate
            } else {
                0.0
            };
            let files = load_stats(
                &db_path,
                since.as_deref(),
                !no_git && config.git.enabled,
                config.git.lookback_days,
                config.git.commit_boost,
                decay_days.unwrap_or(config.decay.window_days),
                decay_rate,
            )?;
            let files: Vec<FileHeat> = files.into_iter().take(top).collect();
            match format {
                OutputFormat::Table => print_table(&files),
                OutputFormat::Json => export::print_json(&files)?,
                OutputFormat::Csv => export::print_csv(&files)?,
            }
        }
        Commands::Stop { purge, db } => {
            let db_path = db.unwrap_or_else(Database::default_path);
            watcher::request_stop(&db_path)?;
            if purge && db_path.exists() {
                std::fs::remove_file(&db_path)
                    .with_context(|| format!("failed to remove {}", db_path.display()))?;
            }
            println!("heatpath watcher stop requested");
        }
    }

    Ok(())
}

impl From<ModeArg> for ViewMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Session => Self::Session,
            ModeArg::Lifetime => Self::Lifetime,
            ModeArg::GitWeighted => Self::GitWeighted,
        }
    }
}

impl From<SortArg> for SortMode {
    fn from(value: SortArg) -> Self {
        match value {
            SortArg::Touches => Self::Touches,
            SortArg::Recency => Self::Recency,
            SortArg::Git => Self::Git,
        }
    }
}

fn load_stats(
    db_path: &Path,
    since: Option<&str>,
    git_enabled: bool,
    git_lookback_days: i64,
    git_commit_boost: f64,
    decay_days: i64,
    decay_rate: f64,
) -> Result<Vec<FileHeat>> {
    let db = Database::open(db_path)?;
    let Some(project) = db.most_recent_project()? else {
        return Ok(Vec::new());
    };

    let since = since.map(parse_since).transpose()?;
    let events = db.events_for_project(project.id, since)?;
    let scoring_events: Vec<ScoringEvent> = events
        .into_iter()
        .map(|event| ScoringEvent {
            filepath: event.filepath,
            occurred_at: event.occurred_at,
        })
        .collect();

    let git_recent = if git_enabled {
        recently_committed_files(&project.path, git_lookback_days)?
    } else {
        HashSet::new()
    };

    Ok(score_events(
        &scoring_events,
        &git_recent,
        ScoreOptions {
            now: Utc::now(),
            decay_days,
            decay_rate,
            git_enabled,
            git_commit_boost,
        },
    ))
}

fn parse_since(value: &str) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }

    let mut parts = value.split_whitespace();
    let amount: i64 = parts
        .next()
        .context("missing since amount")?
        .parse()
        .context("since amount must be a number")?;
    let unit = parts.next().unwrap_or("days").to_ascii_lowercase();
    let duration = match unit.as_str() {
        "minute" | "minutes" | "min" | "mins" => Duration::minutes(amount),
        "hour" | "hours" | "hr" | "hrs" => Duration::hours(amount),
        "day" | "days" => Duration::days(amount),
        "week" | "weeks" => Duration::weeks(amount),
        other => bail!("unsupported since unit: {other}"),
    };
    Ok(Utc::now() - duration)
}

fn print_table(files: &[FileHeat]) {
    println!("Top files");
    println!("------------------------------------------");
    for (index, file) in files.iter().enumerate() {
        println!(
            "{:>2}.  {:<32} {:>5} touches",
            index + 1,
            file.filepath,
            file.touches
        );
    }
    println!("------------------------------------------");
    println!("Total files shown: {}", files.len());
}
