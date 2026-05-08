use std::collections::HashSet;

use chrono::{Duration, TimeZone, Utc};
use heatpath::scoring::decay;
use heatpath::scoring::{recency_weight, score_events, ScoreOptions, ScoringEvent};

#[test]
fn recent_touches_are_weighted_more_heavily() {
    let now = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();

    assert_eq!(recency_weight(now - Duration::hours(12), now), 2.0);
    assert_eq!(recency_weight(now - Duration::days(3), now), 1.5);
    assert_eq!(recency_weight(now - Duration::days(10), now), 1.0);
}

#[test]
fn scoring_combines_touch_count_git_boost_and_decay() {
    let now = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
    let events = vec![
        ScoringEvent {
            filepath: "src/hot.rs".to_string(),
            occurred_at: now - Duration::hours(1),
        },
        ScoringEvent {
            filepath: "src/hot.rs".to_string(),
            occurred_at: now - Duration::days(2),
        },
        ScoringEvent {
            filepath: "src/cold.rs".to_string(),
            occurred_at: now - Duration::days(45),
        },
    ];
    let git_recent = HashSet::from(["src/hot.rs".to_string()]);

    let scored = score_events(
        &events,
        &git_recent,
        ScoreOptions {
            now,
            decay_days: 30,
            decay_rate: 0.10,
            git_enabled: true,
            git_commit_boost: 0.20,
        },
    );

    assert_eq!(scored[0].filepath, "src/hot.rs");
    assert_eq!(scored[0].touches, 2);
    assert!(scored[0].score > 4.0);
    assert!(scored[1].score < 1.0);
}

#[test]
fn decay_starts_after_window_and_has_floor() {
    let now = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();

    assert_eq!(decay::factor(now - Duration::days(10), now, 30, 0.10), 1.0);
    assert!(decay::factor(now - Duration::days(45), now, 30, 0.10) < 1.0);
    assert!(decay::factor(now - Duration::days(500), now, 30, 0.90) >= 0.05);
}
