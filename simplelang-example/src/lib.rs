//! # SimpleLang example: using SimpleLang as an OPTIONAL dependency
//!
//! This plugin has one command, `/langtest`, which replies with a sentence in
//! the player's own language, as chosen with SimpleLang's `/lang` command.
//!
//! The point of this file is the pattern. If you write a plugin and want it to
//! support extra languages *when the server owner has SimpleLang installed*,
//! but still work perfectly when they don't, do exactly this:
//!
//! 1. **Don't declare SimpleLang in `PluginMetadata::dependencies`.**
//!    Declaring it makes your plugin refuse to load without it. Leaving it out
//!    keeps SimpleLang optional; the server owner chooses.
//! 2. **Talk to SimpleLang over IPC** with JSON messages (`call_simplelang`).
//!    If SimpleLang isn't installed, the call fails cleanly and you get
//!    `SimpleLangError::NotAvailable`.
//! 3. **Keep a built-in English text** (`ENGLISH`) for every message. When
//!    SimpleLang is missing, send that yourself. Your plugin never depends on
//!    SimpleLang to say something.
//! 4. **Register your strings** with SimpleLang (`ensure_registered`), under
//!    your own namespace so they can't collide with other plugins.
//! 5. **Ask SimpleLang to deliver** the message (`send_with_simplelang`). It
//!    picks the player's language, falls back to English per key if a
//!    translation is missing, and sends it.
//!
//! ## Load order caveat
//! Without a declared dependency, Pumpkin doesn't promise SimpleLang loads
//! before your plugin. So registration in `on_load` may fail even though
//! SimpleLang IS installed. That's why `ensure_registered` is retried every
//! time a message is about to be sent, until it succeeds once.
//!
//! ## Try it
//! `/langtest`, then `/lang portugues`, `/langtest`, `/lang german`, `/langtest`.
//! Remove SimpleLang from `plugins/` and `/langtest` still works, in English.

use pumpkin_plugin_api::{
    Context, Plugin, PluginMetadata, Server, ipc,
    command::{Command, CommandError, CommandSender, ConsumedArgs},
    commands::CommandHandler,
    permission::{Permission, PermissionDefault},
    text::TextComponent,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn};

// ---- configuration -------------------------------------------------------

/// SimpleLang's plugin id, which is the target of every IPC call.
const SIMPLELANG: &str = "simplelang";

/// Your plugin's namespace in SimpleLang. Lowercase letters, digits, `_` and
/// `-` only. Use something unique to your plugin. ("simplelang" is reserved.)
const NAMESPACE: &str = "langexample";

/// The key as SimpleLang knows it: `<namespace>.<key you registered>`.
/// Below we register the entry "fox", so the full key is "langexample.fox".
const KEY: &str = "langexample.fox";

/// Permission nodes MUST start with your plugin's own name (see `metadata`).
const PERM: &str = "simplelang-example:command.langtest";

/// Step 3: the built-in English text. This is what players see if SimpleLang
/// isn't installed, and it's also registered as the `en_us` translation.
const ENGLISH: &str = "Testing the lang file! The quick brown fox jumps over the lazy dog.";

/// Step 4: your translations: (language code, text). Codes look like `en_us`,
/// `pt_br`, `de_de`. Players whose language isn't listed here get English.
/// Placeholders like `{0}` and `{1}` are filled from the `args` you send.
const STRINGS: &[(&str, &str)] = &[
    ("en_us", ENGLISH),
    (
        "pt_br",
        "Testando o arquivo de idioma! A rápida raposa marrom pula sobre o cão preguiçoso.",
    ),
    (
        "de_de",
        "Teste der Sprachdatei! Der schnelle braune Fuchs springt über den faulen Hund.",
    ),
];

// ---- talking to SimpleLang (step 2) --------------------------------------

/// Why a SimpleLang call didn't work. The distinction matters: "not installed"
/// is normal and silent; "failed" means something is actually wrong.
enum SimpleLangError {
    /// SimpleLang isn't installed (or isn't loaded yet). Not an error.
    NotAvailable,
    /// SimpleLang is there but rejected or garbled the request.
    Failed(String),
}

/// Remembers that registration already succeeded, so we only do it once.
static REGISTERED: AtomicBool = AtomicBool::new(false);

/// Send one JSON request to SimpleLang and return its JSON reply.
///
/// `ipc::send_ipc_message` returns a NESTED result:
///  - outer `Err(())`  -> the call never reached a plugin (SimpleLang missing)
///  - inner `Err(msg)` -> SimpleLang received it and returned an error
///
/// Every SimpleLang reply has `"ok": true|false`; `false` becomes `Failed`.
fn call_simplelang(request: &Value) -> Result<Value, SimpleLangError> {
    let bytes =
        serde_json::to_vec(request).map_err(|e| SimpleLangError::Failed(e.to_string()))?;

    let reply = ipc::send_ipc_message(SIMPLELANG, &bytes)
        .map_err(|_| SimpleLangError::NotAvailable)?
        .map_err(SimpleLangError::Failed)?;

    let reply: Value = serde_json::from_slice(&reply)
        .map_err(|e| SimpleLangError::Failed(format!("bad reply from SimpleLang: {e}")))?;

    if reply["ok"] == true {
        Ok(reply)
    } else {
        Err(SimpleLangError::Failed(reply.to_string()))
    }
}

/// Step 4: give SimpleLang our strings, once per language. Safe to call
/// repeatedly: after the first success it returns immediately.
///
/// The `register` operation:
///   { "op": "register", "namespace": "...", "lang": "de_de",
///     "entries": { "fox": "..." } }
/// Entry keys become `<namespace>.<key>`.
fn ensure_registered() -> Result<(), SimpleLangError> {
    if REGISTERED.load(Ordering::Relaxed) {
        return Ok(());
    }
    for (lang, text) in STRINGS {
        call_simplelang(&json!({
            "op": "register",
            "namespace": NAMESPACE,
            "lang": lang,
            "entries": { "fox": text },
        }))?;
    }
    REGISTERED.store(true, Ordering::Relaxed);
    Ok(())
}

/// Step 5: ask SimpleLang to deliver `KEY` to one player, in their language.
///
/// The `send` operation:
///   { "op": "send", "to": "<player name>", "key": "...", "args": [...],
///     "channel": "chat" | "actionbar" | "title" }
/// (`"title"` also accepts an optional `"subtitle_key"`.)
///
/// There's also `"op": "broadcast"` (same fields, no `"to"`): everyone online
/// gets the message, each in their own language.
///
/// Need the translated string itself instead (to build your own title, GUI
/// text, etc.)? Use `"op": "translate"` with `"key"`, `"args"` and
/// `"player"` / `"client_locale"`; the reply has `"text"`. See SimpleLang's README.
fn send_with_simplelang(player_name: &str) -> Result<(), SimpleLangError> {
    ensure_registered()?;
    call_simplelang(&json!({
        "op": "send",
        "to": player_name,
        "key": KEY,
        "args": [],
        "channel": "chat",
    }))?;
    Ok(())
}

// ---- the command ---------------------------------------------------------

struct LangTestCommand;

impl CommandHandler for LangTestCommand {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let Some(player) = sender.as_player() else {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Only players can run /langtest.",
            )));
        };

        match send_with_simplelang(&player.get_name()) {
            // SimpleLang delivered the message in the player's language.
            Ok(()) => {}

            // SimpleLang isn't installed: perfectly fine. Say it in English ourselves.
            Err(SimpleLangError::NotAvailable) => {
                sender.send_message(TextComponent::text(ENGLISH));
            }

            // SimpleLang is installed but something went wrong. Log it for the
            // server owner, and still give the player a message.
            Err(SimpleLangError::Failed(why)) => {
                warn!("simplelang-example: SimpleLang call failed: {why}");
                sender.send_message(TextComponent::text(ENGLISH));
            }
        }
        Ok(1)
    }
}

// ---- plugin --------------------------------------------------------------

struct SimpleLangExample;

impl Plugin for SimpleLangExample {
    fn new() -> Self {
        SimpleLangExample
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "simplelang-example".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["SimpleLang".into()],
            description: "Example of using SimpleLang as an optional dependency".into(),
            // Step 1: intentionally EMPTY. Putting "simplelang" here would make
            // it mandatory: the server owner couldn't run this plugin without it.
            dependencies: vec![],
            permissions: vec![],
        }
    }

    fn on_load(&self, context: Context) -> pumpkin_plugin_api::Result<()> {
        // Try to register now. It's fine if this fails: see "Load order caveat"
        // at the top of the file. We retry before every message.
        match ensure_registered() {
            Ok(()) => info!("simplelang-example: SimpleLang found, strings registered"),
            Err(SimpleLangError::NotAvailable) => info!(
                "simplelang-example: SimpleLang not found (not installed, or not loaded yet); \
                 using built-in English and retrying on first use"
            ),
            Err(SimpleLangError::Failed(why)) => {
                warn!("simplelang-example: SimpleLang is there but registration failed: {why}")
            }
        }

        context.register_permission(&Permission {
            node: PERM.to_string(),
            description: "Allows /langtest".to_string(),
            default: PermissionDefault::Allow,
            children: Vec::new(),
        })?;

        let names = ["langtest".to_string()];
        let command = Command::new(&names, "Show a sentence in your SimpleLang language")
            .execute(LangTestCommand);
        context.register_command(command, PERM);

        info!("simplelang-example: registered /langtest");
        Ok(())
    }
}

pumpkin_plugin_api::register_plugin!(SimpleLangExample);
