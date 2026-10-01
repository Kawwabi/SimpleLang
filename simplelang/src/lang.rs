//! Pure translation logic. Nothing in here touches the Pumpkin host API,
//! so it can be unit-tested natively, without the wasm toolchain:
//!     rustc --edition 2024 --test src/lang.rs -o /tmp/lang_tests && /tmp/lang_tests

use std::collections::{BTreeSet, HashMap};

pub const DEFAULT_LANG: &str = "en_us";

/// language code -> (key -> text)
pub type Table = HashMap<String, HashMap<String, String>>;

/// `pt-BR`, `PT_br`, ` pt_br ` -> `pt_br`
pub fn normalize(code: &str) -> String {
    code.trim().to_lowercase().replace('-', "_")
}

/// Built-in nicknames. Only used if the target language actually exists.
/// Admins can add more in `aliases.json`. A language's own display name
/// (`simplelang.lang.name`, e.g. "Português (Brasil)" -> "portugues") and bare
/// prefixes ("pt", "de") are matched automatically as well.
pub const BUILTIN_ALIASES: &[(&str, &str)] = &[
    ("english", "en_us"),
    ("ingles", "en_us"),
    ("german", "de_de"),
    ("alemao", "de_de"),
    ("aleman", "de_de"),
    ("portuguese", "pt_br"),
    ("brasil", "pt_br"),
    ("brazil", "pt_br"),
    ("br", "pt_br"),
    ("ptbr", "pt_br"),
];

/// Lowercase-insensitive accent stripping for the letters our languages use,
/// so "português", "Portugues" and "PORTUGUÊS" all match.
pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Turns whatever `format!("{:?}", locale)` produced into a language code.
/// Handles `"en_us"` (a string), `en-US`, and enum-style names like `EnUs`.
pub fn normalize_debug_locale(raw: &str) -> String {
    let s = raw.trim().trim_matches('"');
    // Debug output of an enum includes its type: `Locale::PtBr`. Keep the last part.
    let s = s.rsplit("::").next().unwrap_or(s).trim_start_matches('_');
    if s.contains('_') || s.contains('-') {
        return normalize(s);
    }
    let mut out = String::with_capacity(s.len() + 2);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Replaces `{0}`, `{1}`, ... in a single pass (so an argument that itself
/// contains `{1}` is never re-expanded). Unknown indices are left untouched.
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('}') {
            if let Ok(i) = after[..end].parse::<usize>() {
                if let Some(arg) = args.get(i) {
                    out.push_str(arg);
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        out.push('{');
        rest = after;
    }
    out.push_str(rest);
    out
}

pub struct Store {
    /// Loaded from `<data>/lang/*.json`. Admins edit these; they win.
    files: Table,
    /// Bundled defaults and strings registered by other plugins over IPC.
    registered: Table,
    /// player id -> chosen language
    prefs: HashMap<String, String>,
    /// Admin-defined nicknames from `aliases.json` (keys are folded).
    aliases: HashMap<String, String>,
    default_lang: String,
}

impl Store {
    pub fn new() -> Self {
        Self {
            files: Table::new(),
            registered: Table::new(),
            prefs: HashMap::new(),
            aliases: HashMap::new(),
            default_lang: DEFAULT_LANG.to_string(),
        }
    }

    // ---- loading -------------------------------------------------------

    pub fn set_files(&mut self, files: Table) {
        self.files = files;
    }

    /// Insert entries exactly as given (used for the bundled defaults).
    pub fn register_raw(&mut self, lang: &str, entries: HashMap<String, String>) {
        self.registered
            .entry(normalize(lang))
            .or_default()
            .extend(entries);
    }

    /// Insert entries under `namespace.` (used for other plugins).
    pub fn register(&mut self, namespace: &str, lang: &str, entries: HashMap<String, String>) {
        let prefixed = entries
            .into_iter()
            .map(|(k, v)| (format!("{namespace}.{k}"), v))
            .collect();
        self.register_raw(lang, prefixed);
    }

    pub fn set_aliases(&mut self, aliases: HashMap<String, String>) {
        self.aliases = aliases
            .into_iter()
            .map(|(k, v)| (fold(&normalize(&k)), normalize(&v)))
            .collect();
    }

    pub fn set_prefs(&mut self, prefs: HashMap<String, String>) {
        self.prefs = prefs;
    }

    // ---- preferences ---------------------------------------------------

    pub fn prefs(&self) -> &HashMap<String, String> {
        &self.prefs
    }

    pub fn pref(&self, player_id: &str) -> Option<&String> {
        self.prefs.get(player_id)
    }

    pub fn set_pref(&mut self, player_id: &str, lang: &str) {
        self.prefs.insert(player_id.to_string(), normalize(lang));
    }

    pub fn clear_pref(&mut self, player_id: &str) {
        self.prefs.remove(player_id);
    }

    // ---- languages -----------------------------------------------------

    /// Sorted list of every language that has at least one string.
    pub fn available(&self) -> Vec<String> {
        let set: BTreeSet<&String> = self.files.keys().chain(self.registered.keys()).collect();
        set.into_iter().cloned().collect()
    }

    pub fn has_lang(&self, lang: &str) -> bool {
        let l = normalize(lang);
        self.files.contains_key(&l) || self.registered.contains_key(&l)
    }

    /// `["de_de (Deutsch)", "en_us (English)"]`
    pub fn describe(&self) -> Vec<String> {
        self.available()
            .into_iter()
            .map(|code| self.label(&code))
            .collect()
    }

    /// Turn what a player typed into a language code, or `None`.
    /// Order: exact code -> alias -> language's own name -> bare prefix.
    /// So `english`, `English`, `en` and `en-US` all give `en_us`.
    pub fn resolve_code(&self, input: &str) -> Option<String> {
        let n = fold(&normalize(input));
        if n.is_empty() {
            return None;
        }
        if self.has_lang(&n) {
            return Some(n);
        }
        let target = self.aliases.get(&n).cloned().or_else(|| {
            BUILTIN_ALIASES
                .iter()
                .find(|(a, _)| *a == n)
                .map(|(_, c)| c.to_string())
        });
        if let Some(t) = target {
            if self.has_lang(&t) {
                return Some(normalize(&t));
            }
        }
        for code in self.available() {
            if let Some(name) = self.lookup(&code, "simplelang.lang.name") {
                let f = fold(&normalize(name));
                let first = f
                    .split(|c: char| !c.is_alphanumeric())
                    .next()
                    .unwrap_or("");
                if f == n || first == n {
                    return Some(code);
                }
            }
        }
        if !n.contains('_') {
            let same = format!("{n}_{n}");
            if self.has_lang(&same) {
                return Some(same);
            }
            let prefix = format!("{n}_");
            if let Some(c) = self.available().into_iter().find(|a| a.starts_with(&prefix)) {
                return Some(c);
            }
        }
        None
    }

    /// `pt_br (Português (Brasil))`
    pub fn label(&self, code: &str) -> String {
        match self.lookup(code, "simplelang.lang.name") {
            Some(name) => format!("{code} ({name})"),
            None => code.to_string(),
        }
    }

    /// Which language should this player see?
    /// explicit choice > client setting > server default.
    pub fn resolve_lang(&self, player_id: Option<&str>, client_locale: Option<&str>) -> String {
        if let Some(id) = player_id {
            if let Some(chosen) = self.prefs.get(id) {
                return chosen.clone();
            }
        }
        if let Some(loc) = client_locale {
            let n = normalize(loc);
            if !n.is_empty() {
                return n;
            }
        }
        self.default_lang.clone()
    }

    /// The order in which languages are tried for `lang`:
    /// exact -> `xx_xx` -> any other `xx_*` -> default (English).
    pub fn chain(&self, lang: &str) -> Vec<String> {
        let l = normalize(lang);
        let mut out: Vec<String> = Vec::new();
        let push = |s: String, out: &mut Vec<String>| {
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        };

        push(l.clone(), &mut out);
        let base = l.split('_').next().unwrap_or("").to_string();
        if !base.is_empty() {
            push(format!("{base}_{base}"), &mut out);
            let prefix = format!("{base}_");
            for a in self.available() {
                if a.starts_with(&prefix) {
                    push(a, &mut out);
                }
            }
        }
        push(self.default_lang.clone(), &mut out);
        out
    }

    // ---- lookup --------------------------------------------------------

    /// Exact lookup in one language. Files override registered strings.
    pub fn lookup(&self, lang: &str, key: &str) -> Option<&str> {
        let l = normalize(lang);
        self.files
            .get(&l)
            .and_then(|m| m.get(key))
            .or_else(|| self.registered.get(&l).and_then(|m| m.get(key)))
            .map(String::as_str)
    }

    /// Translate with fallback. If nothing has the key, the key itself is
    /// returned so missing strings are visible instead of blank.
    pub fn translate(&self, lang: &str, key: &str, args: &[String]) -> String {
        for l in self.chain(lang) {
            if let Some(template) = self.lookup(&l, key) {
                return fill(template, args);
            }
        }
        key.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn store() -> Store {
        let mut s = Store::new();
        s.register_raw("en_us", map(&[("hi", "Hello {0}"), ("only_en", "English only")]));
        s.register_raw("de_de", map(&[("hi", "Hallo {0}")]));
        s
    }

    #[test]
    fn normalizes_codes() {
        assert_eq!(normalize(" pt-BR "), "pt_br");
        assert_eq!(normalize_debug_locale("EnUs"), "en_us");
        assert_eq!(normalize_debug_locale("\"en_us\""), "en_us");
        assert_eq!(normalize_debug_locale("de-DE"), "de_de");
        assert_eq!(normalize_debug_locale("Locale::PtBr"), "pt_br");
    }

    #[test]
    fn fills_placeholders_once() {
        let args = vec!["{1}".to_string(), "B".to_string()];
        assert_eq!(fill("{0} and {1}", &args), "{1} and B");
        assert_eq!(fill("{5} stays", &args), "{5} stays");
        assert_eq!(fill("no braces", &args), "no braces");
        assert_eq!(fill("dangling {", &args), "dangling {");
    }

    #[test]
    fn exact_language_wins() {
        assert_eq!(store().translate("de_de", "hi", &["Ana".into()]), "Hallo Ana");
    }

    #[test]
    fn regional_variant_uses_base_language() {
        assert_eq!(store().translate("de_at", "hi", &["Ana".into()]), "Hallo Ana");
    }

    #[test]
    fn missing_language_falls_back_to_english() {
        assert_eq!(store().translate("fr_fr", "hi", &["Ana".into()]), "Hello Ana");
    }

    #[test]
    fn missing_key_falls_back_per_key() {
        assert_eq!(store().translate("de_de", "only_en", &[]), "English only");
    }

    #[test]
    fn unknown_key_returns_key() {
        assert_eq!(store().translate("de_de", "nope", &[]), "nope");
    }

    #[test]
    fn files_override_registered() {
        let mut s = store();
        let mut files = Table::new();
        files.insert("de_de".into(), map(&[("hi", "Moin {0}")]));
        s.set_files(files);
        assert_eq!(s.translate("de_de", "hi", &["Ana".into()]), "Moin Ana");
    }

    #[test]
    fn preference_beats_client_locale() {
        let mut s = store();
        s.set_pref("p1", "de-DE");
        assert_eq!(s.resolve_lang(Some("p1"), Some("en_us")), "de_de");
        s.clear_pref("p1");
        assert_eq!(s.resolve_lang(Some("p1"), Some("fr_fr")), "fr_fr");
        assert_eq!(s.resolve_lang(None, None), "en_us");
    }

    fn store_with_names() -> Store {
        let mut s = Store::new();
        s.register_raw("en_us", map(&[("simplelang.lang.name", "English")]));
        s.register_raw("de_de", map(&[("simplelang.lang.name", "Deutsch")]));
        s.register_raw("pt_br", map(&[("simplelang.lang.name", "Português (Brasil)")]));
        s
    }

    #[test]
    fn aliases_resolve_to_codes() {
        let s = store_with_names();
        assert_eq!(s.resolve_code("english").as_deref(), Some("en_us"));
        assert_eq!(s.resolve_code("English").as_deref(), Some("en_us"));
        assert_eq!(s.resolve_code("en").as_deref(), Some("en_us"));
        assert_eq!(s.resolve_code("en-US").as_deref(), Some("en_us"));
        assert_eq!(s.resolve_code("portugues").as_deref(), Some("pt_br"));
        assert_eq!(s.resolve_code("Português").as_deref(), Some("pt_br"));
        assert_eq!(s.resolve_code("brasil").as_deref(), Some("pt_br"));
        assert_eq!(s.resolve_code("pt").as_deref(), Some("pt_br"));
        assert_eq!(s.resolve_code("deutsch").as_deref(), Some("de_de"));
        assert_eq!(s.resolve_code("german").as_deref(), Some("de_de"));
        assert_eq!(s.resolve_code("klingon"), None);
    }

    #[test]
    fn alias_to_missing_language_is_ignored() {
        let mut s = Store::new();
        s.register_raw("en_us", map(&[("x", "y")]));
        assert_eq!(s.resolve_code("portuguese"), None);
    }

    #[test]
    fn admin_aliases_work() {
        let mut s = store_with_names();
        s.set_aliases(map(&[("Ami", "DE-de")]));
        assert_eq!(s.resolve_code("ami").as_deref(), Some("de_de"));
    }

    #[test]
    fn namespaced_registration() {
        let mut s = store();
        s.register("shop", "de_de", map(&[("buy", "Kaufen")]));
        assert_eq!(s.translate("de_de", "shop.buy", &[]), "Kaufen");
        assert_eq!(s.translate("en_us", "shop.buy", &[]), "shop.buy");
    }
}
