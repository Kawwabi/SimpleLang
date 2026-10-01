//! Everything that talks to the Pumpkin host lives here, so if the API shifts
//! under a new Pumpkin release this is the only file you should need to touch.
//!
//! Verified against pumpkin-plugin-api 0.2.0+26.3-26.51 (docs.rs):
//!   CommandSender::{as_player, get_locale, send_message, has_permission, is_console}
//!   CommandNode::{literal, argument, then, execute}, ConsumedArgs::get_value -> Arg
//!   Arg::{Simple, Msg}, CommandError::CommandFailed(TextComponent)
//!   Context::{register_command, register_permission, get_data_folder}
//!
//! NOT verified (couldn't read the WIT source) - if the compiler complains,
//! it will be about one of these, and each is a one-line fix:
//!   [A] ArgumentType::String(StringType::SingleWord)   (shape of the enum)
//!   [B] PermissionDefault::Deny                        (variant name)
//!   [C] format!("{:?}", player.get_id()) as a stable id (uuid type is opaque)
//!   [E] TextComponent::text(..).color_named(NamedColor::Green) chaining
//!   [D] format!("{:?}", sender.get_locale()) -> language code

use crate::lang::normalize_debug_locale;
use crate::{state, storage};
use pumpkin_plugin_api::{
    Context, Player, Server,
    command::{
        Arg, ArgumentType, Command, CommandError, CommandNode, CommandSender, ConsumedArgs,
        StringType,
    },
    commands::CommandHandler,
    permission::{Permission, PermissionDefault},
    text::{NamedColor, TextComponent},
};
use tracing::warn;

const PERM_USE: &str = "simplelang:command.lang";
const PERM_RELOAD: &str = "simplelang:reload";

// ---- small helpers -------------------------------------------------------

/// [E] Minecraft's "green" is the light green (§a); "dark_green" is the dark one.
/// If `color_named` doesn't chain by value on your API version, adjust here only.
fn info(s: &str) -> TextComponent {
    TextComponent::text(s).color_named(NamedColor::Green)
}

fn error(s: &str) -> TextComponent {
    TextComponent::text(s).color_named(NamedColor::Red)
}

/// [C] Stable per-player key used in players.json and over IPC.
pub fn player_key(player: &Player) -> String {
    format!("{:?}", player.get_id())
}

/// [D] The sender's client language, normalised to `xx_yy`.
fn client_locale(sender: &CommandSender) -> String {
    normalize_debug_locale(&format!("{:?}", sender.get_locale()))
}

fn sender_id(sender: &CommandSender) -> Option<String> {
    sender.as_player().map(|p| player_key(&p))
}

/// Translate `key` for whoever ran the command. Guard is dropped before return.
fn tr(sender: &CommandSender, key: &str, args: &[String]) -> String {
    let id = sender_id(sender);
    let locale = client_locale(sender);
    let st = state::read();
    let lang = st.store.resolve_lang(id.as_deref(), Some(&locale));
    st.store.translate(&lang, key, args)
}

fn say(sender: &CommandSender, key: &str, args: &[String]) {
    let msg = tr(sender, key, args);
    sender.send_message(info(&msg));
}

fn fail(sender: &CommandSender, key: &str, args: &[String]) -> CommandError {
    CommandError::CommandFailed(error(&tr(sender, key, args)))
}

// ---- /lang ---------------------------------------------------------------

/// `/lang` - show the current language and what's available.
struct Show;
impl CommandHandler for Show {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let id = sender_id(&sender);
        let locale = client_locale(&sender);
        let (lang, is_auto, available) = {
            let st = state::read();
            let lang = st.store.resolve_lang(id.as_deref(), Some(&locale));
            let is_auto = id.as_deref().map_or(true, |i| st.store.pref(i).is_none());
            (lang, is_auto, st.store.describe().join(", "))
        };
        let key = if is_auto {
            "simplelang.current.auto"
        } else {
            "simplelang.current.set"
        };
        say(&sender, key, &[lang]);
        say(&sender, "simplelang.available", &[available]);
        say(&sender, "simplelang.usage", &[]);
        Ok(1)
    }
}

/// `/lang <code>` - choose a language.
struct SetLang;
impl CommandHandler for SetLang {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let Some(player) = sender.as_player() else {
            return Err(fail(&sender, "simplelang.set.players_only", &[]));
        };
        let raw = match args.get_value("language") {
            Arg::Simple(s) | Arg::Msg(s) => s,
            _ => return Err(CommandError::CommandFailed(error("Expected a language code"))),
        };
        let id = player_key(&player);

        let outcome = {
            let mut st = state::write();
            match st.store.resolve_code(&raw) {
                Some(code) => {
                    st.store.set_pref(&id, &code);
                    let label = st.store.label(&code);
                    Ok((st.dir.clone(), st.store.prefs().clone(), label))
                }
                None => Err(st.store.describe().join(", ")),
            }
        };

        match outcome {
            Ok((dir, prefs, label)) => {
                if let Err(e) = storage::save_prefs(&dir, &prefs) {
                    warn!("could not save players.json: {e}");
                }
                say(&sender, "simplelang.set.ok", &[label]);
                Ok(1)
            }
            Err(available) => Err(fail(&sender, "simplelang.set.unknown", &[raw, available])),
        }
    }
}

/// `/lang auto` - go back to following the client's language setting.
struct Auto;
impl CommandHandler for Auto {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let Some(player) = sender.as_player() else {
            return Err(fail(&sender, "simplelang.set.players_only", &[]));
        };
        let (dir, prefs) = {
            let mut st = state::write();
            st.store.clear_pref(&player_key(&player));
            (st.dir.clone(), st.store.prefs().clone())
        };
        if let Err(e) = storage::save_prefs(&dir, &prefs) {
            warn!("could not save players.json: {e}");
        }
        say(&sender, "simplelang.auto.ok", &[client_locale(&sender)]);
        Ok(1)
    }
}

/// `/lang reload` - re-read `lang/*.json` without restarting the server.
struct Reload;
impl CommandHandler for Reload {
    fn handle(
        &self,
        sender: CommandSender,
        server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        if !(sender.is_console() || sender.has_permission(&server, PERM_RELOAD)) {
            return Err(fail(&sender, "simplelang.reload.denied", &[]));
        }
        let dir = state::read().dir.clone();
        let files = storage::load_files(&dir);
        let count = files.len();
        {
            let mut st = state::write();
            st.store.set_files(files);
            st.store.set_aliases(storage::load_aliases(&dir));
        }
        say(&sender, "simplelang.reload.ok", &[count.to_string()]);
        Ok(1)
    }
}

// ---- registration --------------------------------------------------------

pub fn register(context: &Context) -> pumpkin_plugin_api::Result<()> {
    context.register_permission(&Permission {
        node: PERM_USE.to_string(),
        description: "Allows using /lang to choose a language".to_string(),
        default: PermissionDefault::Allow,
        children: Vec::new(),
    })?;
    context.register_permission(&Permission {
        node: PERM_RELOAD.to_string(),
        description: "Allows /lang reload".to_string(),
        default: PermissionDefault::Deny, // [B]
        children: Vec::new(),
    })?;

    let names = ["lang".to_string()];
    let command = Command::new(&names, "Choose the language used for server messages")
        .execute(Show)
        .then(CommandNode::literal("auto").execute(Auto))
        .then(CommandNode::literal("reload").execute(Reload))
        .then(
            CommandNode::argument("language", &ArgumentType::String(StringType::SingleWord)) // [A]
                .execute(SetLang),
        );

    context.register_command(command, PERM_USE);
    Ok(())
}
