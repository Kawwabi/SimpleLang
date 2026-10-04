#!/usr/bin/env python3
"""Sanity-check SimpleLang's bundled language files.

Fails (exit 1) if, for any file in simplelang/defaults/:
  - it isn't valid JSON, or isn't a flat {"key": "text"} object of strings
  - it has keys en_us.json doesn't have, or is missing keys en_us.json has
  - a message uses different {0}, {1}, ... placeholders than the English one

English (en_us.json) is the reference, because it's the fallback language.

It also validates defaults/minecraft_languages.json, the built-in list of
Minecraft languages: well-formed entries, unique codes, and every bundled
language file must use a code that appears in that list.
"""
import json
import re
import sys
from pathlib import Path

DEFAULTS = Path(__file__).resolve().parents[2] / "simplelang" / "defaults"
CATALOGUE = DEFAULTS / "minecraft_languages.json"
PLACEHOLDER = re.compile(r"\{(\d+)\}")
CODE = re.compile(r"^[a-z]{2,4}(_[a-z0-9]{2,5})?$")


def load(path: Path, errors: list) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        errors.append(f"{path.name}: cannot parse: {e}")
        return {}
    if not isinstance(data, dict) or not all(
        isinstance(k, str) and isinstance(v, str) for k, v in data.items()
    ):
        errors.append(f"{path.name}: must be a flat object of string -> string")
        return {}
    return data


def check_catalogue(errors: list) -> set:
    """Validate minecraft_languages.json. Returns the set of codes it lists."""
    try:
        data = json.loads(CATALOGUE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        errors.append(f"{CATALOGUE.name}: cannot parse: {e}")
        return set()
    if not isinstance(data, list) or not data:
        errors.append(f"{CATALOGUE.name}: must be a non-empty list of objects")
        return set()
    codes: list[str] = []
    for i, entry in enumerate(data):
        where = f"{CATALOGUE.name}[{i}]"
        if not isinstance(entry, dict):
            errors.append(f"{where}: must be an object")
            continue
        code, name, native = entry.get("code"), entry.get("name"), entry.get("native", "")
        if not isinstance(code, str) or not CODE.match(code):
            errors.append(f"{where}: bad code {code!r} (lowercase, like 'pt_br' or 'tok')")
            continue
        if not isinstance(name, str) or not name.strip():
            errors.append(f"{where} ({code}): 'name' must be a non-empty string")
        if not isinstance(native, str):
            errors.append(f"{where} ({code}): 'native' must be a string if present")
        extra = set(entry) - {"code", "name", "native", "variant_of"}
        if extra:
            errors.append(f"{where} ({code}): unknown fields: {', '.join(sorted(extra))}")
        codes.append(code)
    for i, entry in enumerate(data):
        target = entry.get("variant_of") if isinstance(entry, dict) else None
        if target is not None and (target not in codes or target == entry.get("code")):
            errors.append(
                f"{CATALOGUE.name}[{i}] ({entry.get('code')}): variant_of {target!r} must be "
                "a different language in the catalogue"
            )
    dupes = sorted({c for c in codes if codes.count(c) > 1})
    if dupes:
        errors.append(f"{CATALOGUE.name}: duplicate codes: {', '.join(dupes)}")
    return set(codes)


def main() -> int:
    errors: list[str] = []
    catalogue_codes = check_catalogue(errors)
    files = sorted(p for p in DEFAULTS.glob("*.json") if p != CATALOGUE)
    if not files:
        print(f"no language files found in {DEFAULTS}")
        return 1

    reference_path = DEFAULTS / "en_us.json"
    reference = load(reference_path, errors)

    for path in files:
        if path == reference_path:
            continue
        data = load(path, errors)
        if not data:
            continue
        if catalogue_codes and path.stem not in catalogue_codes:
            errors.append(
                f"{path.name}: '{path.stem}' is not in {CATALOGUE.name}; "
                "add it there or fix the file name"
            )
        missing = sorted(set(reference) - set(data))
        extra = sorted(set(data) - set(reference))
        if missing:
            errors.append(f"{path.name}: missing keys: {', '.join(missing)}")
        if extra:
            errors.append(f"{path.name}: keys not in en_us.json: {', '.join(extra)}")
        for key in sorted(set(reference) & set(data)):
            want = sorted(PLACEHOLDER.findall(reference[key]))
            got = sorted(PLACEHOLDER.findall(data[key]))
            if want != got:
                errors.append(
                    f"{path.name}: '{key}' uses placeholders {got}, English uses {want}"
                )

    if errors:
        print("Language file problems:")
        for e in errors:
            print(f"  - {e}")
        return 1
    print(
        f"OK: {len(files)} language file(s) checked against en_us.json; "
        f"{len(catalogue_codes)} Minecraft languages in the catalogue"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
