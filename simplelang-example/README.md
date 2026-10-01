# simplelang-example

A small plugin showing how to use [SimpleLang](../simplelang) as an **optional**
dependency. Read the comments at the top of `src/lib.rs`; they walk through the
pattern step by step.

## Build and install
    cargo build --release
    cp target/wasm32-wasip2/release/simplelang_example.wasm /path/to/server/plugins/

SimpleLang itself is optional. Try the plugin both with and without it.

## In game
1. `/langtest`           -> sentence in your current language
2. `/lang portugues`, then `/langtest` -> Portuguese
3. `/lang german`, then `/langtest`    -> German
4. `/lang english`, then `/langtest`   -> English
5. Remove SimpleLang from `plugins/`, restart: `/langtest` still works, in English.

## What the server log tells you
| Log line | Meaning |
|---|---|
| `SimpleLang found, strings registered` | everything is wired up |
| `SimpleLang not found ... retrying on first use` | not installed, or loaded after this plugin; both are fine |
| `registration failed` / `SimpleLang call failed` | SimpleLang is present but rejected something; the message says what |
