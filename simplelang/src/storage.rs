//! Everything that touches the plugin's private data folder.
//! Needs the `fs.read.data` and `fs.write.data` permissions (see `metadata()`).

use crate::lang::{Table, normalize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Languages shipped inside the plugin. They are (a) registered in memory as
/// the lowest-priority source, so new keys in a plugin update always work, and
/// (b) copied to `lang/` once, as an editable starting point for admins.
pub const BUNDLED: &[(&str, &str)] = &[
    ("en_us", include_str!("../defaults/en_us.json")),
    ("de_de", include_str!("../defaults/de_de.json")),
    ("pt_br", include_str!("../defaults/pt_br.json")),
];

fn lang_dir(base: &Path) -> PathBuf {
    base.join("lang")
}

fn prefs_path(base: &Path) -> PathBuf {
    base.join("players.json")
}

/// Parse a flat `{ "key": "text" }` JSON object.
pub fn parse_entries(json: &str) -> Result<HashMap<String, String>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Write bundled files to `lang/` if they don't exist yet. Never overwrites.
pub fn seed_defaults(base: &Path) {
    let dir = lang_dir(base);
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("could not create {}: {e}", dir.display());
        return;
    }
    for (code, contents) in BUNDLED {
        let path = dir.join(format!("{code}.json"));
        if !path.exists() {
            if let Err(e) = fs::write(&path, contents) {
                warn!("could not write {}: {e}", path.display());
            }
        }
    }
}

/// Read every `lang/*.json`. The file name (without `.json`) is the language code.
/// A broken file is skipped with a warning instead of failing the whole load.
pub fn load_files(base: &Path) -> Table {
    let mut table = Table::new();
    let entries = match fs::read_dir(lang_dir(base)) {
        Ok(e) => e,
        Err(e) => {
            warn!("could not read language folder: {e}");
            return table;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        match fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| parse_entries(&s).map_err(|e| e.to_string()))
        {
            Ok(map) => {
                table.insert(normalize(stem), map);
            }
            Err(e) => warn!("skipping {}: {e}", path.display()),
        }
    }
    table
}

/// Optional `aliases.json`: `{ "english": "en_us", "tupi": "pt_br" }`.
pub fn load_aliases(base: &Path) -> HashMap<String, String> {
    match fs::read_to_string(base.join("aliases.json")) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            warn!("aliases.json is invalid, ignoring it: {e}");
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

pub fn load_prefs(base: &Path) -> HashMap<String, String> {
    match fs::read_to_string(prefs_path(base)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            warn!("players.json is invalid, ignoring it: {e}");
            HashMap::new()
        }),
        Err(_) => HashMap::new(), // first run
    }
}

pub fn save_prefs(base: &Path, prefs: &HashMap<String, String>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    fs::write(prefs_path(base), json).map_err(|e| e.to_string())
}
