use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::gitignore::{Gitignore, GitignoreBuilder};

pub struct IgnoreMatcher {
    root: PathBuf,
    gitignore: Gitignore,
    globset: GlobSet,
}

impl IgnoreMatcher {
    pub fn new(root: &Path, use_gitignore: bool, extra_patterns: &[String]) -> Result<Self> {
        let mut gitignore_builder = GitignoreBuilder::new(root);
        if use_gitignore {
            let gitignore = root.join(".gitignore");
            if gitignore.exists() {
                if let Some(err) = gitignore_builder.add(gitignore) {
                    return Err(err.into());
                }
            }
        }
        let gitignore = gitignore_builder.build()?;

        let mut glob_builder = GlobSetBuilder::new();
        for pattern in default_patterns()
            .into_iter()
            .chain(extra_patterns.iter().cloned())
        {
            for expanded in expand_pattern(&pattern) {
                glob_builder.add(Glob::new(&expanded)?);
            }
        }

        Ok(Self {
            root: root.to_path_buf(),
            gitignore,
            globset: glob_builder.build()?,
        })
    }

    pub fn is_ignored(&self, path: &Path) -> bool {
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        if relative.as_os_str().is_empty() {
            return false;
        }

        if has_ignored_component(relative) {
            return true;
        }

        if self.globset.is_match(relative) {
            return true;
        }

        self.gitignore
            .matched_path_or_any_parents(relative, path.is_dir())
            .is_ignore()
    }
}

fn default_patterns() -> Vec<String> {
    vec![
        ".git/".to_string(),
        "node_modules/".to_string(),
        "target/".to_string(),
        "dist/".to_string(),
        "build/".to_string(),
    ]
}

fn expand_pattern(pattern: &str) -> Vec<String> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Vec::new();
    }
    if let Some(directory) = pattern.strip_suffix('/') {
        return vec![format!("**/{directory}/**"), format!("{directory}/**")];
    }
    vec![pattern.to_string(), format!("**/{pattern}")]
}

fn has_ignored_component(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(value) => matches!(
            value.to_string_lossy().as_ref(),
            ".git" | "node_modules" | "target" | "dist" | "build"
        ),
        _ => false,
    })
}
