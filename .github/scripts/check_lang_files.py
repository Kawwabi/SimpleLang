#!/usr/bin/env python3
"""Sanity-check SimpleLang's bundled language files.

Fails (exit 1) if, for any file in simplelang/defaults/:
  - it isn't valid JSON, or isn't a flat {"key": "text"} object of strings
  - it has keys en_us.json doesn't have, or is missing keys en_us.json has
  - a message uses different {0}, {1}, ... placeholders than the English one

English (en_us.json) is the reference, because it's the fallback language.
"""
import json
import re
import sys
from pathlib import Path

DEFAULTS = Path(__file__).resolve().parents[2] / "simplelang" / "defaults"
PLACEHOLDER = re.compile(r"\{(\d+)\}")


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


def main() -> int:
    errors: list[str] = []
    files = sorted(DEFAULTS.glob("*.json"))
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
    print(f"OK: {len(files)} language file(s) checked against en_us.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
