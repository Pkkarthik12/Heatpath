use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use crossterm::event::{self, Event as TerminalEvent, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};
use serde::Deserialize;

use crate::db::{Database, Project};
use crate::export;
use crate::scoring::git::recently_committed_files;
use crate::scoring::{score_events, FileHeat, ScoreOptions, ScoringEvent};

pub mod colours;
pub mod treemap;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewMode {
    Session,
    Lifetime,
    GitWeighted,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortMode {
    Touches,
    Recency,
    Git,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::Session
    }
}

impl Default for SortMode {
    fn default() -> Self {
        Self::Touches
    }
}

#[derive(Debug, Clone)]
pub struct UiOptions {
    pub db_path: PathBuf,
    pub mode: ViewMode,
    pub sort: SortMode,
    pub depth: Option<usize>,
}

#[derive(Debug)]
struct UiState {
    options: UiOptions,
    project: Option<Project>,
    files: Vec<FileHeat>,
    selected: usize,
    message: String,
}

pub fn run(options: UiOptions) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_terminal(&mut terminal, options);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    options: UiOptions,
) -> Result<()> {
    let mut state = UiState {
        options,
        project: None,
        files: Vec::new(),
        selected: 0,
        message: String::new(),
    };

    loop {
        refresh_state(&mut state)?;
        terminal.draw(|frame| draw(frame, &mut state))?;

        if event::poll(Duration::from_millis(250))? {
            if let TerminalEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('s') => state.options.mode = ViewMode::Session,
                    KeyCode::Char('l') => state.options.mode = ViewMode::Lifetime,
                    KeyCode::Char('g') => state.options.mode = ViewMode::GitWeighted,
                    KeyCode::Char('r') => reset_session(&mut state)?,
                    KeyCode::Char('e') => export_current_view(&mut state)?,
                    KeyCode::Down => {
                        if state.selected + 1 < state.files.len() {
                            state.selected += 1;
                        }
                    }
                    KeyCode::Up => {
                        state.selected = state.selected.saturating_sub(1);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn refresh_state(state: &mut UiState) -> Result<()> {
    let db = Database::open(&state.options.db_path)?;
    state.project = db.most_recent_project()?;
    let Some(project) = &state.project else {
        state.files.clear();
        state.message = "No project is being tracked yet.".to_string();
        return Ok(());
    };

    let since = match state.options.mode {
        ViewMode::Session => Some(Utc::now() - ChronoDuration::hours(24)),
        ViewMode::Lifetime | ViewMode::GitWeighted => None,
    };
    let events = db.events_for_project(project.id, since)?;
    let scoring_events: Vec<ScoringEvent> = events
        .into_iter()
        .map(|event| ScoringEvent {
            filepath: event.filepath,
            occurred_at: event.occurred_at,
        })
        .collect();
    let git_recent = if state.options.mode == ViewMode::GitWeighted {
        recently_committed_files(&project.path, 14)?
    } else {
        HashSet::new()
    };
    state.files = score_events(
        &scoring_events,
        &git_recent,
        ScoreOptions {
            now: Utc::now(),
            decay_days: 30,
            decay_rate: 0.10,
            git_enabled: state.options.mode == ViewMode::GitWeighted,
            git_commit_boost: 0.20,
        },
    );
    sort_files(&mut state.files, state.options.sort);
    if let Some(depth) = state.options.depth {
        for file in &mut state.files {
            file.filepath = limit_depth(&file.filepath, depth);
        }
    }
    if state.selected >= state.files.len() {
        state.selected = state.files.len().saturating_sub(1);
    }
    state.message.clear();
    Ok(())
}

fn draw(frame: &mut Frame, state: &mut UiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.size());

    let project = state
        .project
        .as_ref()
        .map(|project| project.path.display().to_string())
        .unwrap_or_else(|| "no project".to_string());
    let header = Paragraph::new(format!(
        "heatpath  |  {project}  |  mode: {}",
        mode_label(state.options.mode)
    ))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    let max_score = state
        .files
        .iter()
        .map(|file| file.score)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let items: Vec<ListItem> = state
        .files
        .iter()
        .map(|file| {
            let ratio = (file.score / max_score).clamp(0.0, 1.0);
            let bar = treemap::heat_bar(ratio, 12);
            ListItem::new(Line::from(vec![
                Span::styled(bar, Style::default().fg(colours::heat_color(ratio))),
                Span::raw(format!(
                    "  {:<48} {:>5} touches  score {:>6.2}",
                    file.filepath, file.touches, file.score
                )),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Files"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut list_state = ListState::default();
    if !state.files.is_empty() {
        list_state.select(Some(state.selected));
    }
    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    let footer = if state.message.is_empty() {
        "[s] session  [l] lifetime  [g] git-weighted  [r] reset  [e] export  [q] quit"
            .to_string()
    } else {
        state.message.clone()
    };
    frame.render_widget(Paragraph::new(footer), chunks[2]);
}

fn sort_files(files: &mut [FileHeat], sort: SortMode) {
    match sort {
        SortMode::Touches => files.sort_by(|left, right| {
            right
                .touches
                .cmp(&left.touches)
                .then_with(|| right.score.total_cmp(&left.score))
        }),
        SortMode::Recency => {
            files.sort_by(|left, right| right.last_touched.cmp(&left.last_touched))
        }
        SortMode::Git => files.sort_by(|left, right| right.score.total_cmp(&left.score)),
    }
}

fn reset_session(state: &mut UiState) -> Result<()> {
    let db = Database::open(&state.options.db_path)?;
    let Some(project) = db.most_recent_project()? else {
        return Ok(());
    };
    let deleted = db.delete_events_since(project.id, Some(Utc::now() - ChronoDuration::hours(24)))?;
    state.message = format!("Reset {deleted} session events");
    Ok(())
}

fn export_current_view(state: &mut UiState) -> Result<()> {
    export::write_json_file(&PathBuf::from("heatpath-export.json"), &state.files)?;
    state.message = "Exported heatpath-export.json".to_string();
    Ok(())
}

fn mode_label(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Session => "session",
        ViewMode::Lifetime => "lifetime",
        ViewMode::GitWeighted => "git-weighted",
    }
}

fn limit_depth(filepath: &str, depth: usize) -> String {
    if depth == 0 {
        return filepath.to_string();
    }
    let parts: Vec<&str> = filepath.split('/').collect();
    if parts.len() <= depth {
        filepath.to_string()
    } else {
        format!("{}/...", parts[..depth].join("/"))
    }
}
