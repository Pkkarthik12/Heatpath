use std::fs;

use heatpath::watcher::events::{event_paths, EventType};
use heatpath::watcher::filter::IgnoreMatcher;
use notify::{Event, EventKind};
use tempfile::tempdir;

#[test]
fn ignore_matcher_respects_defaults_gitignore_and_extra_patterns() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();

    let matcher = IgnoreMatcher::new(root, true, &["*.test.ts".to_string()]).unwrap();

    assert!(matcher.is_ignored(&root.join("node_modules").join("pkg").join("index.js")));
    assert!(matcher.is_ignored(&root.join("ignored.txt")));
    assert!(matcher.is_ignored(&root.join("src").join("thing.test.ts")));
    assert!(!matcher.is_ignored(&root.join("src").join("thing.rs")));
}

#[test]
fn notify_events_map_to_heatpath_event_types() {
    let save =
        Event::new(EventKind::Modify(notify::event::ModifyKind::Any)).add_path("src/lib.rs".into());
    let delete = Event::new(EventKind::Remove(notify::event::RemoveKind::File))
        .add_path("src/lib.rs".into());

    assert_eq!(event_paths(save)[0].1, EventType::Save);
    assert_eq!(event_paths(delete)[0].1, EventType::Delete);
}
