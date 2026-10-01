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
- Falls back **per message** to English, so partial translations still work.
- Language files are plain JSON you can edit and hot-reload with `/lang reload`.
  English, German and Brazilian Portuguese are included.
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

Put `simplelang.wasm` in your server's `plugins/` folder and restart. On first start it
creates `plugins/simplelang/` with editable language files.

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

Copy `simplelang/defaults/en_us.json` to a new file named after the language code
(for example `fr_fr.json`), translate the values, and set `simplelang.lang.name` to the
language's own name so `/lang francais` works automatically. Pull requests welcome.

CI checks every language file against `en_us.json`: same keys, and the same `{0}`, `{1}`
placeholders in each message. You can run the same check locally:

```bash
python3 .github/scripts/check_lang_files.py
```

## License

[MIT](LICENSE)
