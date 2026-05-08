use chrono::{DateTime, Duration, Utc};

pub fn factor(last_touched: DateTime<Utc>, now: DateTime<Utc>, window_days: i64, rate: f64) -> f64 {
    if window_days <= 0 || rate <= 0.0 {
        return 1.0;
    }

    let age = now.signed_duration_since(last_touched);
    let decay_window = Duration::days(window_days);
    if age <= decay_window {
        return 1.0;
    }

    let extra_days = age.num_days() - window_days;
    let weeks = ((extra_days + 6) / 7).max(1) as i32;
    (1.0 - rate).powi(weeks).max(0.05)
}
