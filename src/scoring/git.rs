use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use anyhow::Result;

use super::normalize_path;

pub fn recently_committed_files(root: &Path, lookback_days: i64) -> Result<HashSet<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg(format!("--since={lookback_days} days ago"))
        .arg("--name-only")
        .arg("--format=")
        .output();

    let Ok(output) = output else {
        return Ok(HashSet::new());
    };
    if !output.status.success() {
        return Ok(HashSet::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(normalize_path)
        .collect())
}
