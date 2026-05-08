use std::path::PathBuf;

use notify::{Event, EventKind};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EventType {
    Open,
    Save,
    Delete,
}

impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Save => "save",
            Self::Delete => "delete",
        }
    }
}

pub fn event_paths(event: Event) -> Vec<(PathBuf, EventType)> {
    let Some(event_type) = event_type(&event.kind) else {
        return Vec::new();
    };
    event
        .paths
        .into_iter()
        .map(|path| (path, event_type))
        .collect()
}

fn event_type(kind: &EventKind) -> Option<EventType> {
    match kind {
        EventKind::Access(_) => Some(EventType::Open),
        EventKind::Create(_) | EventKind::Modify(_) => Some(EventType::Save),
        EventKind::Remove(_) => Some(EventType::Delete),
        _ => None,
    }
}
