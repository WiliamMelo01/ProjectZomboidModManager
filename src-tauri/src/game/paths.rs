use std::{collections::HashSet, path::PathBuf};

pub(super) fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();

    paths
        .into_iter()
        .filter(|path| seen.insert(path.display().to_string().to_lowercase()))
        .collect()
}
