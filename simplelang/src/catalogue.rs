//! The built-in list of Minecraft languages (`defaults/minecraft_languages.json`).
//!
//! It makes every Minecraft language selectable with `/lang`, by code or name,
//! even when nobody has translated SimpleLang into it. Players who pick such a
//! language see English wherever a translation is missing.
//!
//! To fix or extend the list, edit the JSON file. Order matters: for a short
//! name shared by several languages ("spanish", "chinese"), the first entry wins,
//! so list the primary variant of each family first.

use crate::lang::CatalogueEntry;
use serde::Deserialize;

const BUNDLED: &str = include_str!("../defaults/minecraft_languages.json");

#[derive(Deserialize)]
struct Raw {
    code: String,
    name: String,
    #[serde(default)]
    native: String,
}

pub fn load() -> Result<Vec<CatalogueEntry>, serde_json::Error> {
    let raw: Vec<Raw> = serde_json::from_str(BUNDLED)?;
    Ok(raw
        .into_iter()
        .map(|r| CatalogueEntry {
            code: r.code,
            name: r.name,
            native: r.native,
        })
        .collect())
}
