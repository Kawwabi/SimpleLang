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
//!   [H] TextComponent::click_event_run_command(&str) (the click-event method name)
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

/// [H] Make a message clickable: clicking it runs `command` (e.g. "/lang list").
/// UNVERIFIED API CALL: the method name is a guess. If the compiler says there's no
/// such method, look at the `click*` methods on `TextComponent` in the API docs (or
/// the compiler's "similar name" hint) and fix just this line. Every message that
/// uses this also tells players the plain command, so it still works if clicking doesn't.
fn clickable(tc: TextComponent, command: &str) -> TextComponent {
    tc.click_run_command(command)
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

fn say_click(sender: &CommandSender, key: &str, args: &[String], command: &str) {
    let msg = tr(sender, key, args);
    sender.send_message(clickable(info(&msg), command));
}

fn fail(sender: &CommandSender, key: &str, args: &[String]) -> CommandError {
    CommandError::CommandFailed(error(&tr(sender, key, args)))
}

fn fail_click(sender: &CommandSender, key: &str, args: &[String], command: &str) -> CommandError {
    let msg = tr(sender, key, args);
    CommandError::CommandFailed(clickable(error(&msg), command))
}

// ---- /lang ---------------------------------------------------------------

/// `/lang` - show the current language and how to change it.
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
        let (lang, is_auto) = {
            let st = state::read();
            let lang = st.store.resolve_lang(id.as_deref(), Some(&locale));
            let is_auto = id.as_deref().map_or(true, |i| st.store.pref(i).is_none());
            (lang, is_auto)
        };
        let key = if is_auto {
            "simplelang.current.auto"
        } else {
            "simplelang.current.set"
        };
        say(&sender, key, &[lang]);
        // Don't dump 100+ languages into chat: point at `/lang list` instead.
        say_click(&sender, "simplelang.available.hint", &[], "/lang list");
        say(&sender, "simplelang.usage", &[]);
        Ok(1)
    }
}

/// `/lang list` - every language SimpleLang has translations for.
struct List;
impl CommandHandler for List {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let available = state::read().store.describe().join(", ");
        say(&sender, "simplelang.available", &[available]);
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
                    // Selectable (Minecraft lists it) but no strings yet?
                    let untranslated = !st.store.is_translated(&code);
                    Ok((st.dir.clone(), st.store.prefs().clone(), label, untranslated))
                }
                None => Err(()),
            }
        };

        match outcome {
            Ok((dir, prefs, label, untranslated)) => {
                if let Err(e) = storage::save_prefs(&dir, &prefs) {
                    warn!("could not save players.json: {e}");
                }
                say(&sender, "simplelang.set.ok", &[label.clone()]);
                if untranslated {
                    say(&sender, "simplelang.set.untranslated", &[label]);
                }
                Ok(1)
            }
            // {1} is only used by language files from v0.1.0 ("Available: {1}"); new ones ignore it.
            Err(()) => Err(fail_click(
                &sender,
                "simplelang.set.unknown",
                &[raw, "/lang list".to_string()],
                "/lang list",
            )),
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
        .then(CommandNode::literal("list").execute(List))
        .then(CommandNode::literal("reload").execute(Reload))
        .then(
            CommandNode::argument("language", &ArgumentType::String(StringType::SingleWord)) // [A]
                .execute(SetLang),
        );

    context.register_command(command, PERM_USE);
    Ok(())
}
