//! JSON-over-IPC protocol. This is how other plugins use SimpleLang.
//!
//! Every request is a JSON object with an `"op"` field; every reply is a JSON
//! object with `"ok": true|false`. See the README for a copy-paste client.

use crate::bridge::{self, Channel};
use crate::lang::{normalize, normalize_debug_locale};
use crate::state;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    /// Add strings under `<namespace>.<key>` for one language.
    Register {
        namespace: String,
        lang: String,
        entries: HashMap<String, String>,
    },
    /// Translate a key for a player (or an explicit language).
    Translate {
        key: String,
        #[serde(default)]
        args: Vec<String>,
        /// Player id, exactly as `format!("{:?}", player.get_id())`.
        player: Option<String>,
        /// The player's client locale, e.g. `format!("{:?}", player.get_locale())`.
        client_locale: Option<String>,
        /// Force a specific language instead of resolving one for the player.
        lang: Option<String>,
    },
    /// Which language would this player see?
    GetLang {
        player: Option<String>,
        client_locale: Option<String>,
    },
    /// List available language codes.
    Languages,
    /// Translate `key` into one online player's language and deliver it.
    /// `channel`: "chat" (default), "actionbar" or "title".
    Send {
        /// Exact player name.
        to: String,
        key: String,
        #[serde(default)]
        args: Vec<String>,
        channel: Option<String>,
        /// For "title": key of the subtitle line.
        subtitle_key: Option<String>,
    },
    /// Same as `send`, but to everyone online - each in their own language.
    Broadcast {
        key: String,
        #[serde(default)]
        args: Vec<String>,
        channel: Option<String>,
        subtitle_key: Option<String>,
    },
}

fn valid_namespace(ns: &str) -> bool {
    !ns.is_empty()
        && ns
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

pub fn handle(sender: &str, message: &[u8]) -> Result<Vec<u8>, String> {
    let request: Request =
        serde_json::from_slice(message).map_err(|e| format!("bad SimpleLang request: {e}"))?;

    let reply = match request {
        Request::Register {
            namespace,
            lang,
            entries,
        } => {
            if !valid_namespace(&namespace) || namespace == "simplelang" {
                return Err(format!("invalid or reserved namespace '{namespace}'"));
            }
            let count = entries.len();
            state::write().store.register(&namespace, &lang, entries);
            tracing::info!("{sender} registered {count} string(s) for {namespace}/{}", normalize(&lang));
            json!({ "ok": true, "registered": count })
        }

        Request::Translate {
            key,
            args,
            player,
            client_locale,
            lang,
        } => {
            let st = state::read();
            let lang = match lang {
                Some(l) => normalize(&l),
                None => st.store.resolve_lang(
                    player.as_deref(),
                    client_locale.as_deref().map(normalize_debug_locale).as_deref(),
                ),
            };
            let text = st.store.translate(&lang, &key, &args);
            json!({ "ok": true, "text": text, "lang": lang })
        }

        Request::GetLang {
            player,
            client_locale,
        } => {
            let st = state::read();
            let lang = st.store.resolve_lang(
                player.as_deref(),
                client_locale.as_deref().map(normalize_debug_locale).as_deref(),
            );
            json!({ "ok": true, "lang": lang })
        }

        Request::Languages => {
            json!({ "ok": true, "languages": state::read().store.listed() })
        }

        Request::Send {
            to,
            key,
            args,
            channel,
            subtitle_key,
        } => {
            let ch = Channel::parse(channel.as_deref());
            match bridge::send_to_name(&to, ch, &key, &args, subtitle_key.as_deref())? {
                true => json!({ "ok": true }),
                false => json!({ "ok": false, "error": "player not online" }),
            }
        }

        Request::Broadcast {
            key,
            args,
            channel,
            subtitle_key,
        } => {
            let ch = Channel::parse(channel.as_deref());
            let n = bridge::broadcast(ch, &key, &args, subtitle_key.as_deref())?;
            json!({ "ok": true, "delivered": n })
        }
    };

    serde_json::to_vec(&reply).map_err(|e| e.to_string())
}
