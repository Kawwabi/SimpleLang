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
| `lang/en_us.json`, `lang/de_de.json`, `lang/pt_br.json` | Editable language files, copied here on first start (never overwritten). Add `fr_fr.json`, ... and `/lang reload`. |
| `aliases.json` | Optional. You create it to add custom `/lang` names (see below). |
| `players.json` | Each player's explicit choice. Written automatically. |

Language files are flat `{ "key": "text" }` objects. Placeholders are `{0}`, `{1}`, ...

## `/lang`

| Command | Effect |
|---|---|
| `/lang` | Show your language and the available ones |
| `/lang <language>` | Choose a language by code (`pt_br`, `pt-BR`) **or by name** (`english`, `portugues`, `deutsch`, `brasil`, `pt`) |
| `/lang auto` | Follow the game's language setting again |
| `/lang reload` | Re-read `lang/*.json` (permission `simplelang:reload`, console always allowed) |

Which language a player sees: **their `/lang` choice → their game's language setting → English.**

## Language aliases

`/lang english` sets `en_us`, `/lang portugues` sets `pt_br`. Matching ignores case and accents, and tries, in order:

1. the exact code (`pt_br`)
2. an alias: a small built-in list plus your own `aliases.json` in the data folder
3. the language's own display name, taken from its `simplelang.lang.name` string (so a new `fr_fr.json` with `"simplelang.lang.name": "Français"` makes `/lang francais` work automatically)
4. a bare prefix (`pt`, `de`)

Custom aliases, `plugins/simplelang/aliases.json`, then `/lang reload`:

```json
{ "tupi": "pt_br", "deu": "de_de" }
```

An alias pointing at a language that isn't installed is ignored. If two languages share a first word (say "Português (Brasil)" and "Português (Portugal)"), the alphabetically first wins for the short form; the full code always works.

## Fallback rules

For a player using `de_at`, each key is looked up in this order, first hit wins:

1. `de_at` → 2. `de_de` → 3. any other `de_*` → 4. `en_us` → 5. the key itself (so gaps are visible, not blank)

This is per **key**, so a half-finished translation still works: missing lines show in English.

Priority between sources inside one language: `lang/*.json` on disk beats strings registered by plugins, which beat the bundled defaults. That lets an admin override any plugin's wording.

## For other plugin authors

Talk to it over IPC; the plugin id is `"simplelang"`.

**Prefer making it optional.** Don't list `simplelang` in your `PluginMetadata::dependencies`
(that makes it mandatory). Instead, keep a built-in English text for each message and fall back
to it when the IPC call fails because SimpleLang isn't installed. Because there's then no load-order
guarantee, register your strings lazily (retry until it succeeds). `simplelang-example` is a complete,
heavily commented plugin doing exactly this.

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
    // repeat with "lang": "de_de", ... or just ship lang/*.json entries on disk
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
