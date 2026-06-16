use crate::app_config_dir;
use crate::models::ZomboidMod;
use crate::util::read_ini_values;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const CACHE_VERSION: u32 = 1;
const CACHE_FILE_NAME: &str = "mods-library-cache.json";
const LOCAL_WORKSHOP_ID_FILE: &str = ".pzmm-workshop-id";

#[derive(Default)]
pub(super) struct ModsLibraryCache {
    entries: HashMap<String, CachedModEntry>,
    active_keys: HashSet<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedModsFile {
    version: u32,
    entries: HashMap<String, CachedModEntry>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedModEntry {
    signature: PackageSignature,
    mod_item: ZomboidMod,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PackageSignature {
    files: Vec<FileSignature>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileSignature {
    path: String,
    modified_ms: u128,
    len: u64,
}

impl ModsLibraryCache {
    pub(super) fn load() -> Self {
        let Ok(path) = cache_path() else {
            return Self::default();
        };
        let Ok(content) = fs::read_to_string(path) else {
            return Self::default();
        };
        let Some(entries) = parse_cache_entries(&content) else {
            return Self::default();
        };

        Self {
            entries,
            active_keys: HashSet::new(),
        }
    }

    pub(super) fn key(package: &Path, source: &str, workshop_id: Option<&str>) -> String {
        [
            normalize_path(package),
            source.to_string(),
            workshop_id.unwrap_or_default().to_string(),
        ]
        .join("|")
    }

    pub(super) fn get_valid(
        &mut self,
        key: &str,
        signature: &PackageSignature,
    ) -> Option<ZomboidMod> {
        let entry = self.entries.get(key)?;

        if &entry.signature == signature {
            self.active_keys.insert(key.to_string());
            Some(entry.mod_item.clone())
        } else {
            None
        }
    }

    pub(super) fn store(&mut self, key: String, signature: PackageSignature, mod_item: ZomboidMod) {
        self.active_keys.insert(key.clone());
        self.entries.insert(
            key,
            CachedModEntry {
                signature,
                mod_item,
            },
        );
    }

    pub(super) fn retain_active_and_save(mut self) {
        self.entries
            .retain(|key, _entry| self.active_keys.contains(key));

        let Ok(path) = cache_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let cache = CachedModsFile {
            version: CACHE_VERSION,
            entries: self.entries,
        };
        let Ok(content) = serde_json::to_string_pretty(&cache) else {
            return;
        };

        let _ = fs::write(path, content);
    }
}

fn parse_cache_entries(content: &str) -> Option<HashMap<String, CachedModEntry>> {
    let cache = serde_json::from_str::<CachedModsFile>(content).ok()?;

    (cache.version == CACHE_VERSION).then_some(cache.entries)
}

pub(super) fn package_signature(package_dir: &Path) -> PackageSignature {
    let mut files = Vec::new();

    push_file_signature(&mut files, package_dir.join(LOCAL_WORKSHOP_ID_FILE));
    let mut mod_infos = collect_mod_info_paths(package_dir);

    for mod_info in &mod_infos {
        push_file_signature(&mut files, mod_info);
        collect_mod_info_image_paths(mod_info)
            .into_iter()
            .for_each(|path| push_file_signature(&mut files, path));
        if let Some(mod_dir) = mod_info.parent() {
            collect_map_info_paths(mod_dir)
                .into_iter()
                .for_each(|path| push_file_signature(&mut files, path));
        }
    }

    package_image_paths(package_dir)
        .into_iter()
        .for_each(|path| push_file_signature(&mut files, path));

    mod_infos.clear();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.dedup_by(|left, right| left.path == right.path);

    PackageSignature { files }
}

pub(super) fn clear_persisted_cache() -> Result<(), String> {
    let path = cache_path()?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Nao foi possivel remover {}: {error}", path.display()))?;
    }

    Ok(())
}

fn cache_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join(CACHE_FILE_NAME))
}

fn collect_mod_info_paths(package_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let root_info = package_dir.join("mod.info");

    if root_info.is_file() {
        paths.push(root_info);
    }

    let Ok(entries) = fs::read_dir(package_dir) else {
        return paths;
    };
    let mut b42_infos = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && is_b42_dir(path))
        .map(|path| path.join("mod.info"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();

    b42_infos.sort_by_key(|path| normalize_path(path));
    paths.extend(b42_infos);
    paths
}

fn collect_map_info_paths(mod_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(mod_dir.join("media").join("maps")) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| path.join("map.info"))
        .filter(|path| path.is_file())
        .collect()
}

fn collect_mod_info_image_paths(mod_info: &Path) -> Vec<PathBuf> {
    let Ok(content) = fs::read_to_string(mod_info) else {
        return Vec::new();
    };
    let Some(mod_dir) = mod_info.parent() else {
        return Vec::new();
    };

    read_ini_values(&content, "poster")
        .into_iter()
        .chain(read_ini_values(&content, "icon"))
        .filter(|value| !value.trim().is_empty())
        .map(|candidate| mod_dir.join(candidate))
        .filter(|path| path.is_file())
        .collect()
}

fn package_image_paths(package_dir: &Path) -> Vec<PathBuf> {
    ["poster.png", "poster.jpg", "icon.png", "icon.jpg"]
        .into_iter()
        .map(|name| package_dir.join(name))
        .filter(|path| path.is_file())
        .collect()
}

fn push_file_signature(files: &mut Vec<FileSignature>, path: impl AsRef<Path>) {
    let path = path.as_ref();
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let Ok(modified) = metadata.modified() else {
        return;
    };
    let modified_ms = modified
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    files.push(FileSignature {
        path: normalize_path(path),
        modified_ms,
        len: metadata.len(),
    });
}

fn is_b42_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "42" || name.starts_with("42."))
        .unwrap_or(false)
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ZomboidModVariant;
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{fs, thread, time::Duration};

    fn temp_dir(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("pzmm-cache-{label}-{timestamp}"))
    }

    fn sample_mod(package: &Path) -> ZomboidMod {
        ZomboidMod {
            id: "Example".to_string(),
            name: "Example".to_string(),
            author: "Tester".to_string(),
            version: "-".to_string(),
            workshop_id: "123".to_string(),
            description: "Test".to_string(),
            size: "-".to_string(),
            is_installed: false,
            source: "steam".to_string(),
            path: package.display().to_string(),
            image_url: None,
            dependencies: Vec::new(),
            map_names: Vec::new(),
            compatible_builds: vec!["b41".to_string()],
            variants: vec![ZomboidModVariant {
                game_build: "b41".to_string(),
                id: "Example".to_string(),
                path: package.display().to_string(),
                dependencies: Vec::new(),
                map_names: Vec::new(),
            }],
            package_path: package.display().to_string(),
        }
    }

    fn tick() {
        thread::sleep(Duration::from_millis(20));
    }

    #[test]
    fn cache_hit_when_signature_is_unchanged() {
        let package = temp_dir("hit");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("mod.info"), "name=Example\nid=Example").unwrap();
        let signature = package_signature(&package);
        let key = ModsLibraryCache::key(&package, "steam", Some("123"));
        let mut cache = ModsLibraryCache::default();

        cache.store(key.clone(), signature.clone(), sample_mod(&package));

        assert!(cache.get_valid(&key, &signature).is_some());
        let _ = fs::remove_dir_all(package);
    }

    #[test]
    fn cache_misses_when_mod_info_changes() {
        let package = temp_dir("mod-info");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("mod.info"), "name=Example\nid=Example").unwrap();
        let old_signature = package_signature(&package);
        tick();
        fs::write(package.join("mod.info"), "name=Example 2\nid=Example").unwrap();
        let new_signature = package_signature(&package);

        assert_ne!(old_signature, new_signature);
        let _ = fs::remove_dir_all(package);
    }

    #[test]
    fn cache_misses_when_local_workshop_marker_changes() {
        let package = temp_dir("marker");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("mod.info"), "name=Example\nid=Example").unwrap();
        fs::write(package.join(LOCAL_WORKSHOP_ID_FILE), "123\n").unwrap();
        let old_signature = package_signature(&package);
        tick();
        fs::write(package.join(LOCAL_WORKSHOP_ID_FILE), "456\n").unwrap();
        let new_signature = package_signature(&package);

        assert_ne!(old_signature, new_signature);
        let _ = fs::remove_dir_all(package);
    }

    #[test]
    fn cache_misses_when_resolved_image_changes() {
        let package = temp_dir("image");
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("mod.info"),
            "name=Example\nid=Example\nposter=poster.png",
        )
        .unwrap();
        fs::write(package.join("poster.png"), "one").unwrap();
        let old_signature = package_signature(&package);
        tick();
        fs::write(package.join("poster.png"), "two").unwrap();
        let new_signature = package_signature(&package);

        assert_ne!(old_signature, new_signature);
        let _ = fs::remove_dir_all(package);
    }

    #[test]
    fn ignores_entries_from_incompatible_cache_versions() {
        let package = temp_dir("version");
        fs::create_dir_all(&package).unwrap();
        let key = ModsLibraryCache::key(&package, "steam", Some("123"));
        let mut entries = HashMap::new();
        entries.insert(
            key,
            CachedModEntry {
                signature: package_signature(&package),
                mod_item: sample_mod(&package),
            },
        );
        let cache = CachedModsFile {
            version: CACHE_VERSION + 1,
            entries,
        };
        let content = serde_json::to_string(&cache).unwrap();

        assert!(parse_cache_entries(&content).is_none());
        let _ = fs::remove_dir_all(package);
    }

    #[test]
    fn retain_active_removes_orphan_entries() {
        let package = temp_dir("orphan");
        fs::create_dir_all(&package).unwrap();
        let signature = package_signature(&package);
        let active_key = ModsLibraryCache::key(&package, "steam", Some("1"));
        let orphan_key = ModsLibraryCache::key(&package, "steam", Some("2"));
        let mut cache = ModsLibraryCache::default();

        cache.store(active_key.clone(), signature.clone(), sample_mod(&package));
        cache.entries.insert(
            orphan_key.clone(),
            CachedModEntry {
                signature,
                mod_item: sample_mod(&package),
            },
        );
        cache
            .entries
            .retain(|key, _entry| cache.active_keys.contains(key));

        assert!(cache.entries.contains_key(&active_key));
        assert!(!cache.entries.contains_key(&orphan_key));
        let _ = fs::remove_dir_all(package);
    }
}
