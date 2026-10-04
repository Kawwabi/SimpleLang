<p align="center">
  <img src="SimpleLang.png" width="256" height="256" alt="SimpleLang icon">
</p>

<h1 align="center">SimpleLang</h1>

Per-player language support for [Pumpkin](https://pumpkinmc.org) Minecraft servers.

Players pick a language with `/lang`; plugins that use SimpleLang show their messages
in each player's language, falling back to English when a translation is missing.
It's also a small API: other plugins can register their own translations and have
SimpleLang deliver messages per player, over Pumpkin's plugin IPC.

## Features

- `/lang`: choose a language by code (`pt_br`) or by name (`english`, `portugues`, `deutsch`);
  `/lang auto` follows the player's game language setting.
- **Server default language**: set `default_language` in `config.json` (for example `"pt_br"`) and
  everyone who hasn't chosen sees that, instead of following their game's language.
- Falls back **per message** to English, so partial translations still work.
- **Every Minecraft language is selectable**, by code or name (`/lang tlh_aa`, `/lang francais`),
  even if nobody has translated SimpleLang into it yet. Those players see English wherever
  a translation is missing. (129 languages are built in.)
- Language files are plain JSON you can edit and hot-reload with `/lang reload`.
  Many languages are already translated (see [TRANSLATIONS.md](TRANSLATIONS.md)); more are welcome.
- A send bridge: other plugins ask SimpleLang to deliver a message to one player or to
  everyone, each in their own language (chat, action bar or title).
- **Optional for plugin authors**: plugins can use it when present and keep working
  when it isn't. See the example plugin.

## Repository layout

| Folder | What it is |
|---|---|
| [`simplelang/`](simplelang) | The plugin. Start with its README for commands, language files and the IPC API. |
| [`simplelang-example/`](simplelang-example) | A small, heavily commented plugin showing how to use SimpleLang as an *optional* dependency. |

## Installing (server owners)

Download `simplelang.wasm` from the [latest release](https://github.com/Kawwabi/SimpleLang/releases/latest),
put it in your server's `plugins/` folder and restart. On first start it creates
`plugins/simplelang/` with the language files in a `translations/` folder.

Each release also includes `simplelang_example.wasm` (only needed if you want to try the
example plugin) and `SHA256SUMS.txt` to verify your downloads.

## For plugin developers

> [!TIP]
> **Start by reading the example plugin:
> [`simplelang-example/src/lib.rs`](simplelang-example/src/lib.rs).**
> It's a complete, heavily commented plugin that shows every step below working together,
> including the fallback when SimpleLang isn't installed. Copy it as a starting point for your own.

Any plugin can use SimpleLang. There's no crate to depend on: you send it small JSON
messages over Pumpkin's plugin IPC, and it translates and delivers your messages in each
player's language.

**Make it optional.** The recommended pattern keeps your plugin working whether or not the
server owner installed SimpleLang, so they can decide whether they want extra languages:

1. **Don't** list `simplelang` in `PluginMetadata::dependencies` (that would make it mandatory).
2. Keep a built-in English text for every message in your code.
3. Register your strings with SimpleLang, under a namespace of your own.
4. Ask SimpleLang to send the message (or to translate it for you).
5. If the call fails because SimpleLang isn't installed, send your English text yourself.

Add `serde_json = "1"` to your `Cargo.toml`, then:

```rust
use pumpkin_plugin_api::ipc;
use serde_json::{Value, json};

/// One helper for every call. Returns None if SimpleLang isn't installed
/// or rejected the request.
fn simplelang(request: Value) -> Option<Value> {
    let bytes = serde_json::to_vec(&request).ok()?;
    let reply = ipc::send_ipc_message("simplelang", &bytes)
        .ok()?  // outer Result: did the call reach a plugin at all?
        .ok()?; // inner Result: did SimpleLang accept it?
    let reply: Value = serde_json::from_slice(&reply).ok()?;
    (reply["ok"] == true).then_some(reply)
}

// 1) Register your strings: in on_load, and again before you first need them
//    (see "Load order" below). Keys become "<namespace>.<key>".
simplelang(json!({
    "op": "register", "namespace": "myplugin", "lang": "en_us",
    "entries": { "welcome": "Welcome, {0}!" }
}));
simplelang(json!({
    "op": "register", "namespace": "myplugin", "lang": "pt_br",
    "entries": { "welcome": "Bem-vindo, {0}!" }
}));

// 2) Send it to a player, in their language. Fall back to English if SimpleLang is absent.
let name = player.get_name();
let delivered = simplelang(json!({
    "op": "send", "to": name, "key": "myplugin.welcome", "args": [name], "channel": "chat"
}))
.is_some();
if !delivered {
    player.send_system_message(TextComponent::text(&format!("Welcome, {name}!")), false);
}
```

The snippet above is the short version. For the full, working version with error handling and
load-order retries, read [`simplelang-example/src/lib.rs`](simplelang-example/src/lib.rs); its
header comment explains the pattern step by step.

### Operations

Every request is a JSON object with an `"op"`; every reply has `"ok": true|false`.

| `op` | Fields (`?` = optional) | Reply |
|---|---|---|
| `register` | `namespace`, `lang`, `entries` (`{ key: text }`) | `{ok, registered}` |
| `send` | `to` (exact player name), `key`, `args`?, `channel`?, `subtitle_key`? | `{ok: true}`, or `{ok: false, error: "player not online"}` |
| `broadcast` | `key`, `args`?, `channel`?, `subtitle_key`? | `{ok, delivered}`: everyone online, each in their own language |
| `translate` | `key`, `args`?, and `player`? / `client_locale`? / `lang`? | `{ok, text, lang}`: the translated string, to deliver yourself |
| `get_lang` | `player`?, `client_locale`? | `{ok, lang}` |
| `languages` | none | `{ok, languages}` |

`channel` is `"chat"` (default), `"actionbar"` or `"title"`. For a title, `subtitle_key` adds
the second line.

For `translate` and `get_lang`, identify the player with `format!("{:?}", player.get_id())`
(as `player`) and `format!("{:?}", player.get_locale())` (as `client_locale`). Or skip the
player and pass `"lang": "de_de"` to force a language.

### Things to know

- **Args must be strings.** `"args": ["5"]` works; `"args": [5]` is rejected. Use `{0}`, `{1}`, ...
  in your texts.
- **Namespaces** are lowercase letters, digits, `_` and `-`. Use something unique to your
  plugin; `simplelang` is reserved. Registered keys are always `<namespace>.<key>`.
- **Fallback is per key:** a player's language, then the same language family
  (`de_at` → `de_de`), then English, then the key itself. A missing translation shows English,
  not a blank.
- **Load order:** without a declared dependency, SimpleLang may load after your plugin, so
  registering in `on_load` can fail even though it's installed. Retry before each use until it
  succeeds once. The example plugin does this.
- **Admins can override you:** a `translations/*.json` file on disk beats registered strings, so a
  server owner can reword your messages by adding `"myplugin.welcome": "..."` to their file.
- **Every Minecraft language is selectable** with `/lang`, whether or not it has any strings,
  so `get_lang` can return a language you've never heard of (and one with no translations in
  SimpleLang). Treat the result as the player's *preferred* language and run your own
  fallback: exact code → same family → English. You don't need to register empty languages
  to make yours selectable.
- **Plain text only** for now: no JSON/tellraw components.
- **Nested `Result`:** `ipc::send_ipc_message` returns `Result<Result<Vec<u8>, String>, ()>`.
  The outer error means the call never reached the plugin (not installed); the inner one is
  the plugin's own reply.

More detail, including the IPC protocol reference, is in [`simplelang/README.md`](simplelang/README.md).

## Building

Requires Rust 1.85+ and the WebAssembly target:

```bash
rustup target add wasm32-wasip2

cd simplelang
cargo build --release
# -> target/wasm32-wasip2/release/simplelang.wasm

cd ../simplelang-example
cargo build --release
# -> target/wasm32-wasip2/release/simplelang_example.wasm
```

Built against `pumpkin-plugin-api` 0.2.x. Pumpkin's plugin API is still changing between
releases, so a newer Pumpkin may need small updates; all calls into the Pumpkin API are
isolated in `simplelang/src/host.rs` and `simplelang/src/bridge.rs`.

## Contributing translations

Translating SimpleLang's 13 short messages into your language is a great way to help, and
no Rust is needed. **See [TRANSLATING.md](TRANSLATING.md)** for the steps, including a tool
that checks your file and a workflow for translating with an LLM. [TRANSLATIONS.md](TRANSLATIONS.md)
shows which languages are done and which are still needed. Pull requests welcome.

Missing a Minecraft language, or spot a wrong name? The list lives in
[`simplelang/defaults/minecraft_languages.json`](simplelang/defaults/minecraft_languages.json).
Put the main variant of a language family first, since the first entry wins for short names
like `spanish` or `chinese`.

CI checks every language file against `en_us.json`: same keys, and the same `{0}`, `{1}`
placeholders in each message. You can run the same check locally:

```bash
python3 .github/scripts/check_lang_files.py
```

## License

[MIT](LICENSE)
