use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::types::Type;
use rusqlite::{params, OptionalExtension};

use crate::scoring::FileHeat;

use super::{normalize_project_path, parse_db_datetime, Database, EventRecord, Project};

impl Database {
    pub fn ensure_project(&self, path: &Path) -> Result<i64> {
        let path = normalize_project_path(path)?;
        let path = path.to_string_lossy().to_string();
        self.conn.execute(
            "INSERT OR IGNORE INTO projects(path, created_at) VALUES (?1, ?2)",
            params![path, Utc::now().to_rfc3339()],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM projects WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn most_recent_project(&self) -> Result<Option<Project>> {
        self.conn
            .query_row(
                "SELECT id, path, created_at FROM projects ORDER BY id DESC LIMIT 1",
                [],
                |row| {
                    let created_at: String = row.get(2)?;
                    let created_at = parse_db_datetime(&created_at).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(err))
                    })?;
                    Ok(Project {
                        id: row.get(0)?,
                        path: std::path::PathBuf::from(row.get::<_, String>(1)?),
                        created_at,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn record_event(
        &self,
        project_id: i64,
        filepath: &str,
        event_type: &str,
        occurred_at: DateTime<Utc>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events(project_id, filepath, event_type, occurred_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![project_id, filepath, event_type, occurred_at.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn events_for_project(
        &self,
        project_id: i64,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<EventRecord>> {
        let since = since.map(|value| value.to_rfc3339());
        let mut statement = self.conn.prepare(
            "SELECT project_id, filepath, event_type, occurred_at
             FROM events
             WHERE project_id = ?1
               AND (?2 IS NULL OR occurred_at >= ?2)
             ORDER BY occurred_at ASC",
        )?;
        let rows = statement.query_map(params![project_id, since], |row| {
            let occurred_at: String = row.get(3)?;
            let occurred_at = parse_db_datetime(&occurred_at).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(err))
            })?;
            Ok(EventRecord {
                project_id: row.get(0)?,
                filepath: row.get(1)?,
                event_type: row.get(2)?,
                occurred_at,
            })
        })?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn delete_events_since(
        &self,
        project_id: i64,
        since: Option<DateTime<Utc>>,
    ) -> Result<usize> {
        let changed = if let Some(since) = since {
            self.conn.execute(
                "DELETE FROM events WHERE project_id = ?1 AND occurred_at >= ?2",
                params![project_id, since.to_rfc3339()],
            )?
        } else {
            self.conn
                .execute("DELETE FROM events WHERE project_id = ?1", params![project_id])?
        };
        Ok(changed)
    }

    pub fn replace_scores(&mut self, project_id: i64, files: &[FileHeat]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM scores WHERE project_id = ?1",
            params![project_id],
        )?;
        for file in files {
            tx.execute(
                "INSERT INTO scores(project_id, filepath, score, touches, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project_id,
                    file.filepath,
                    file.score,
                    file.touches as i64,
                    Utc::now().to_rfc3339()
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
