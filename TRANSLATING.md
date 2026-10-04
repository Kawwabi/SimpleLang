# Translating SimpleLang

SimpleLang's own messages are **13 short strings** in
[`simplelang/defaults/en_us.json`](simplelang/defaults/en_us.json). Translating them lets
players who pick your language see `/lang` in it. No Rust needed, only Python 3 (standard
library) for the checking tools. Languages without a translation still work: they fall back to English.

[TRANSLATIONS.md](TRANSLATIONS.md) shows what's done and what's needed. **Everything translated so
far is machine-generated and unreviewed by native speakers**, so corrections to existing languages
are just as welcome as new ones.

## The rules

1. Keep every placeholder (`{0}`) exactly as written. A placeholder may move if your grammar needs it.
2. Don't translate `SimpleLang`, `Minecraft`, the commands `/lang`, `/lang auto`, `/lang list`,
   or the example codes `english`, `portugues`, `pt_br`.
3. In the `usage` message, translate the word inside `<language>` but keep the angle brackets.
4. Short, friendly, game-UI wording. Plain text: no markdown, emoji or line breaks.
5. Use the quotation marks normal in your language around `“{0}”` in the unknown-language message.
6. `simplelang.lang.name` is filled in for you from the catalogue (the language's own name).

The language code is the one in
[`minecraft_languages.json`](simplelang/defaults/minecraft_languages.json), like `fr_fr` or `pt_pt`.

## Option 1: translate by hand

1. Copy `simplelang/defaults/en_us.json` somewhere and translate the values.
2. Check it and add it to the repo in one step:

   ```bash
   python3 tools/translations.py import my_translation.json --code fr_fr
   ```

   It rejects the file with a clear reason if a key is missing, a placeholder was lost or
   changed, or a value contains a line break. When it accepts, it writes
   `simplelang/defaults/fr_fr.json` and registers it in the Rust code for you.
3. Open a pull request. CI re-checks everything.

## Option 2: translate with a local model (or any LLM)

The tool builds one ready-to-paste prompt per language, then validates the replies:

```bash
# 1. one prompt per language that still needs a translation
python3 tools/translations.py prompt --missing --out tools/work/prompts

# 2. run your model on each prompt and save its reply as <code>.json.
#    Example with Ollama (any model or runner works, as long as the reply lands in the file):
mkdir -p tools/work/answers
for f in tools/work/prompts/*.txt; do
  code=$(basename "$f" .txt)
  ollama run YOUR_MODEL < "$f" > "tools/work/answers/$code.json"
done

# 3. validate and add everything that passes
python3 tools/translations.py import tools/work/answers
```

- The prompt tells the model to reply `{"skip": true}` if it can't do a language reliably.
  `import` then skips it instead of adding a bad file. Small models usually can't handle rare or
  constructed languages (Klingon, Quenya, Lojban, Talossan, Viossa, ...), so expect skips there.
- `import` extracts the JSON even if the model wraps it in code fences or chatter, and rejects
  anything with broken placeholders or missing keys. It lists every rejection with the reason.
- **It can check the format, not the quality.** Skim results for languages you can read, and
  prefer big models for languages with few speakers. Existing translations are never replaced
  unless you pass `--overwrite`.
- `tools/work/` is git-ignored; nothing in it ends up in the repo.

To redo only some languages, pass codes: `python3 tools/translations.py prompt nl_nl pt_pt`.

## Regional variants

Variants such as `es_mx`, `de_at` or `en_gb` are marked `variant_of` in the catalogue and don't
need their own file: SimpleLang serves them from the main language (`es_es`, `de_de`, `en_us`). Add
a file for one only if its wording really differs (for example Argentine voseo for `es_ar`). The
tools never ask for these.

## Tool reference

| Command | What it does |
|---|---|
| `python3 tools/translations.py status [--write]` | Coverage summary (`--write` refreshes TRANSLATIONS.md) |
| `python3 tools/translations.py prompt CODE... \| --missing [--out DIR]` | Build LLM prompts |
| `python3 tools/translations.py import PATH... [--code CODE] [--overwrite]` | Validate and add translations |
| `python3 tools/translations.py sync [--check]` | Update the Rust list of bundled languages (`--check` is what CI runs) |
| `python3 .github/scripts/check_lang_files.py` | Validate every language file against English |

After adding or editing files by hand, run `sync` (the list in `simplelang/src/storage.rs` is
generated, don't edit it directly) and `status --write` (to refresh TRANSLATIONS.md).
