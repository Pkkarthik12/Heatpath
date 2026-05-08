PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
  id         INTEGER PRIMARY KEY,
  path       TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS events (
  id          INTEGER PRIMARY KEY,
  project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  filepath    TEXT NOT NULL,
  event_type  TEXT NOT NULL,
  occurred_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS scores (
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  filepath   TEXT NOT NULL,
  score      REAL NOT NULL,
  touches    INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (project_id, filepath)
);

CREATE INDEX IF NOT EXISTS idx_events_project_time
  ON events(project_id, occurred_at);

CREATE INDEX IF NOT EXISTS idx_events_project_file
  ON events(project_id, filepath);
