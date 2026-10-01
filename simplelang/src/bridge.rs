//! The send bridge: another plugin says "show this key to that player" and
//! SimpleLang translates it into that player's language and delivers it.
//!
//! Host calls used (all read from docs.rs, pumpkin-plugin-api 0.2.0):
//!   Server::{get_player_by_name(&str) -> Option<Player>, get_all_players() -> Vec<Player>}
//!   Player::{send_system_message(TextComponent, bool), show_actionbar, show_title, show_subtitle}
//!   Player::{get_id, get_locale}

use crate::host::player_key;
use crate::lang::normalize_debug_locale;
use crate::state;
use pumpkin_plugin_api::{Player, text::TextComponent};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Chat,
    ActionBar,
    Title,
}

impl Channel {
    /// Unknown or missing values mean plain chat.
    pub fn parse(s: Option<&str>) -> Self {
        match s.map(|s| s.trim().to_lowercase()).as_deref() {
            Some("actionbar") | Some("action_bar") => Channel::ActionBar,
            Some("title") => Channel::Title,
            _ => Channel::Chat,
        }
    }
}

fn locale_of(player: &Player) -> String {
    normalize_debug_locale(&format!("{:?}", player.get_locale()))
}

/// Translate `key` for this player. The state guard is dropped before returning.
pub fn translate_for(player: &Player, key: &str, args: &[String]) -> String {
    let id = player_key(player);
    let locale = locale_of(player);
    let st = state::read();
    let lang = st.store.resolve_lang(Some(&id), Some(&locale));
    st.store.translate(&lang, key, args)
}

/// Translate and deliver to one player. Returns nothing: delivery is best-effort.
pub fn send(
    player: &Player,
    channel: Channel,
    key: &str,
    args: &[String],
    subtitle_key: Option<&str>,
) {
    // Everything that needs the lock happens here, before any host call.
    let text = translate_for(player, key, args);
    let subtitle = subtitle_key.map(|k| translate_for(player, k, args));

    match channel {
        Channel::Chat => player.send_system_message(TextComponent::text(&text), false),
        Channel::ActionBar => player.show_actionbar(TextComponent::text(&text)),
        Channel::Title => {
            // Subtitle first so both appear together.
            if let Some(sub) = subtitle {
                player.show_subtitle(TextComponent::text(&sub));
            }
            player.show_title(TextComponent::text(&text));
        }
    }
}

/// Deliver to everyone online, each in their own language. Returns how many got it.
pub fn broadcast(
    channel: Channel,
    key: &str,
    args: &[String],
    subtitle_key: Option<&str>,
) -> Result<usize, String> {
    let server = state::server().ok_or("SimpleLang is still starting up")?;
    let players = server.get_all_players();
    for p in &players {
        send(p, channel, key, args, subtitle_key);
    }
    Ok(players.len())
}

/// Deliver to one player by exact name. `Ok(false)` means they're not online.
pub fn send_to_name(
    name: &str,
    channel: Channel,
    key: &str,
    args: &[String],
    subtitle_key: Option<&str>,
) -> Result<bool, String> {
    let server = state::server().ok_or("SimpleLang is still starting up")?;
    match server.get_player_by_name(name) {
        Some(p) => {
            send(&p, channel, key, args, subtitle_key);
            Ok(true)
        }
        None => Ok(false),
    }
}
