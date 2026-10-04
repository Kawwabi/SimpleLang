# SimpleLang

Per-player language selection for a [Pumpkin](https://pumpkinmc.org) server, with English fallback. Other plugins can use it too.

Built against `pumpkin-plugin-api` 0.2.x (0.2.0+26.3-26.51). Needs Rust 1.85+ and the `wasm32-wasip2` target (`rustup target add wasm32-wasip2`).

## Build and install

```bash
cargo build --release
cp target/wasm32-wasip2/release/simplelang.wasm /path/to/server/plugins/
```

On first start it creates `plugins/simplelang/` (the plugin's data folder) containing:

| File | Purpose |
|---|---|
| `translations/*.json` | The language files, one per language, copied here on first start so you can read and edit them. Edit one, or add `fr_fr.json`, then `/lang reload`. |
| `seeded.json` | Bookkeeping: lets a plugin update refresh translation files you never touched, without ever overwriting ones you edited. |
| `aliases.json` | Optional. You create it to add custom `/lang` names (see below). |
| `config.json` | Server settings: `default_language` and `supported_languages` (see [Server configuration](#server-configuration)). Created on first start. |
| `players.json` | Each player's explicit choice, including `"auto"` for "follow my game's language". Written automatically. |

Language files are flat `{ "key": "text" }` objects. Placeholders are `{0}`, `{1}`, ...

**Upgrading from 0.1?** The `lang/` folder is renamed to `translations/` automatically on first start, keeping your edits.

## `/lang`

| Command | Effect |
|---|---|
| `/lang` | Show your language, with a clickable hint that lists every language |
| `/lang list` | Every language that has translations (the hint runs this when clicked) |
| `/lang <language>` | Choose a language by code (`pt_br`, `pt-BR`) **or by name** (`english`, `portugues`, `deutsch`, `brasil`, `pt`) |
| `/lang auto` | Follow the game's language setting, even if the server has a different default |
| `/lang reload` | Re-read `translations/*.json` (permission `simplelang:reload`, console always allowed) |

Which language a player sees: **their `/lang` choice → the server's default language (if you set one) → their game's language setting → English.**

## Server configuration

`plugins/simplelang/config.json` is created on first start:

```json
{
  "default_language": "auto",
  "supported_languages": "all"
}
```

(The real file also has a `_help` entry that explains the options; it's ignored.)

### `default_language`

| `default_language` | Effect |
|---|---|
| `"auto"` (default) | Players who haven't chosen see their **game's language**, or English if it isn't translated |
| a language, such as `"pt_br"` or `"portuguese"` | Players who haven't chosen see **that language**, whatever their game is set to |

A fixed default is a starting point, not a lock. Players can still pick another language with `/lang <language>`,
or `/lang auto` to follow their own game's language. Edit the file, then run `/lang reload` (no restart).
The value is a code or any name `/lang` accepts, and an unknown one is logged and ignored.
It also applies to plugins that ask SimpleLang for a player's language (`get_lang`, `translate`, `send`, `broadcast`).

### `supported_languages`

By default players can choose from every language. Not every server maintains translations for all
of them, so you can limit the choice to the languages you actually support:

```json
"supported_languages": ["en_us", "pt_br", "es_es"]
```

Entries are codes or any name `/lang` accepts (`"portuguese"`). With a list set:

- `/lang` shows your list right away (instead of the "every Minecraft language" hint), and `/lang list` lists exactly these.
- `/lang <language>` only accepts them. A regional variant is mapped to the closest supported language
  (`es_mx` becomes `es_es`, `pt_pt` becomes `pt_br`); anything else gets the "unknown language" message.
- A player's game language that isn't supported is mapped the same way. If nothing fits, they get the server's
  `default_language`, else English if it's supported, else the first language in your list.
- So `get_lang` (and `translate`, `send`, `broadcast`) always gives plugins **one of your supported languages**.
- `default_language` always counts as supported, even if it isn't in the list.
- Listed languages don't need a SimpleLang translation: any Minecraft language works, with the usual English
  fallback for missing messages.
- A choice a player made earlier for a language you've since removed is ignored, not deleted, so it works
  again if you add the language back.
- An entry that isn't a language is logged and skipped. A list with no valid entry is treated as `"all"`, so a
  typo can never lock everyone out.

Edit the file, then `/lang reload`.

## Every Minecraft language

Unless the server owner limits them with [`supported_languages`](#supported_languages), players can select
**any Minecraft language** with `/lang`, by code or by name, even when nobody has translated SimpleLang into it: `/lang tlh_aa`, `/lang francais`, `/lang spanish-mexico`.
129 languages are built in (see `defaults/minecraft_languages.json`).

There are two different things here:

| | What it means | Where it shows |
|---|---|---|
| **Translated** | At least one string exists (a `translations/*.json` file, or a plugin registered strings) | Listed by `/lang list` and by the `languages` IPC op |
| **Selectable** | It's in Minecraft's language list | Accepted by `/lang <name>`, returned by `get_lang` |

A player who picks a language that's selectable but not translated gets a short notice, and
sees English wherever a message is missing. That's the normal per-message fallback, so nothing
breaks. As soon as someone adds `translations/fr_fr.json` (or a plugin registers French strings), those
players start seeing it, with no change on their side.

Installed languages always win over the built-in list, so it never overrides a real translation.

## Language aliases

`/lang english` sets `en_us`, `/lang portugues` sets `pt_br`. Matching ignores case and accents, and tries, in order:

1. the exact code (`pt_br`)
2. an alias: a small built-in list plus your own `aliases.json` in the data folder
3. the language's own display name, taken from its `simplelang.lang.name` string (so a new `fr_fr.json` with `"simplelang.lang.name": "Français"` makes `/lang francais` work automatically)
4. a bare prefix (`pt`, `de`)
5. Minecraft's language list: the code, the English name (`french`, `spanish-mexico`) or the
   native name (`français`, `čeština`), and bare prefixes (`fr`)

Custom aliases, `plugins/simplelang/aliases.json`, then `/lang reload`:

```json
{ "tupi": "pt_br", "deu": "de_de" }
```

An alias pointing at a language that isn't installed is ignored. If two languages share a first word (say "Português (Brasil)" and "Português (Portugal)"), the alphabetically first wins for the short form; the full code always works. In the built-in list,
the entry listed first wins instead, and the primary variant of each family is listed first
(`spanish` → `es_es`, `chinese` → `zh_cn`, `portuguese` → `pt_br`). A few short names are inherently
ambiguous, such as `bahasa` (Indonesian or Malay); the full name or code always works.

## Fallback rules

For a player using `de_at`, each key is looked up in this order, first hit wins:

1. `de_at` → 2. `de_de` → 3. any other `de_*` → 4. `en_us` → 5. the key itself (so gaps are visible, not blank)

This is per **key**, so a half-finished translation still works: missing lines show in English.

Priority between sources inside one language: `translations/*.json` on disk beats strings registered by plugins, which beat the bundled defaults. That lets an admin override any plugin's wording.

## For other plugin authors

Talk to it over IPC; the plugin id is `"simplelang"`.

**Prefer making it optional.** Don't list `simplelang` in your `PluginMetadata::dependencies`
(that makes it mandatory). Instead, keep a built-in English text for each message and fall back
to it when the IPC call fails because SimpleLang isn't installed. Because there's then no load-order
guarantee, register your strings lazily (retry until it succeeds). **Read the example plugin first:**
[`simplelang-example/src/lib.rs`](../simplelang-example/src/lib.rs) is a complete, heavily
commented plugin doing exactly this.

Note: `ipc::send_ipc_message` returns a nested `Result<Result<Vec<u8>, String>, ()>`: the outer error means the call never reached the plugin, the inner one is the plugin's own reply.

```rust
use pumpkin_plugin_api::{Player, ipc};
use serde_json::{Value, json};

/// Register your strings once, in on_load. Keys become "<namespace>.<key>".
pub fn register_strings() {
    let req = json!({
        "op": "register", "namespace": "myplugin", "lang": "en_us",
        "entries": { "welcome": "Welcome, {0}!" }
    });
    let _ = ipc::send_ipc_message("simplelang", &serde_json::to_vec(&req).unwrap());
    // repeat with "lang": "de_de", ... or just ship translations/*.json entries on disk
}

/// Translate for a specific player.
pub fn tr(player: &Player, key: &str, args: &[&str]) -> String {
    let req = json!({
        "op": "translate", "key": key, "args": args,
        "player": format!("{:?}", player.get_id()),
        "client_locale": format!("{:?}", player.get_locale()),
    });
    ipc::send_ipc_message("simplelang", &serde_json::to_vec(&req).unwrap())
        .ok()                      // outer Result: did the call reach SimpleLang?
        .and_then(|reply| reply.ok()) // inner Result: SimpleLang's own reply
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|v| v["text"].as_str().map(String::from))
        .unwrap_or_else(|| key.to_string())
}

// player.send_system_message(TextComponent::text(&tr(&player, "myplugin.welcome", &["Steve"])), false);
```

### The send bridge

Instead of translating and delivering yourself, ask SimpleLang to do both:

```rust
fn send(to: &str, key: &str, args: &[&str], channel: &str) {
    let req = json!({ "op": "send", "to": to, "key": key, "args": args, "channel": channel });
    let _ = ipc::send_ipc_message("simplelang", &serde_json::to_vec(&req).unwrap());
}

send("Steve", "myplugin.welcome", &["Steve"], "chat");        // chat (default)
send("Steve", "myplugin.low_hp", &[], "actionbar");           // above the hotbar

// everyone online, each in their own language:
let req = json!({ "op": "broadcast", "key": "myplugin.restart", "args": ["5"], "channel": "title",
                  "subtitle_key": "myplugin.restart_sub" });
```

`send` replies `{"ok":false,"error":"player not online"}` if the name isn't online; `broadcast` replies `{"ok":true,"delivered":N}`.
Messages are plain text for now (no JSON/tellraw components).
`args` must be JSON strings (`["5"]`, not `[5]`), or the request is rejected.

Other ops: `get_lang` (which language would this player see), `languages` (list codes). You can also pass `"lang": "de_de"` to `translate` to force a language.
`register` for the reserved `simplelang` namespace is rejected.

## Limits

- **Vanilla and third-party messages are not intercepted.** SimpleLang translates strings that go through it: its own `/lang` output and any plugin that calls it (including via the send bridge). Pumpkin's plugin API has no hook that lets a plugin rewrite arbitrary outgoing chat per viewer.
- Broadcast events (e.g. the join message) carry a single text for everyone, so they can't be per-viewer either.
- Tab-completion for `/lang <code>` isn't wired up.

## Tests

The pure logic (fallback chain, placeholders, precedence) has unit tests and needs no wasm toolchain:

```bash
rustc --edition 2024 --test src/lang.rs -o /tmp/lang_tests && /tmp/lang_tests
```
