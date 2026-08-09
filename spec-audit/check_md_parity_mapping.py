#!/usr/bin/env python3
"""Fail closed when registry/docs parity declarations drift from the audit map.

Rule cards opt into the machine-readable contract with a comment such as::

    <!-- md-parity: code=decreasing_or_equal_stop_time_distance -->

The prose around the card remains human-readable; the comment gives CI an
unambiguous declaration to compare with the mapping implementation.
"""

from __future__ import annotations

import importlib.util
import re
from pathlib import Path

from md_parity_mapping import CONTEXT_MAPPINGS, context_rules


ROOT = Path(__file__).resolve().parent.parent
AUDIT_PATH = ROOT / "spec-audit" / "md_parity_audit.py"
REGISTRY_PATH = ROOT / "crates" / "rules" / "src" / "registry.rs"
DOCS_PATH = ROOT / "docs" / "rules"
MARKER = re.compile(r"<!--\s*md-parity:\s*code=([a-z0-9_]+)\s*-->")
CARD_ID = re.compile(r"^#\s+([A-Z]{2,3}_[0-9]{3}[a-z]?)\b", re.MULTILINE)
REGISTRY_ID = re.compile(r'r!\("([A-Z]{2,3}_[0-9]{3}[a-z]?)"')


def load_audit():
    spec = importlib.util.spec_from_file_location("md_parity_audit", AUDIT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"audit modülü yüklenemedi: {AUDIT_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    audit = load_audit()
    registry_ids = set(REGISTRY_ID.findall(REGISTRY_PATH.read_text(encoding="utf-8-sig")))
    mapped_ids = {rule for rules in audit.MAP.values() for rule in rules}
    mapped_ids.update(context_rules())

    problems: list[str] = []
    for code, rules in audit.MAP.items():
        unknown = sorted(set(rules) - registry_ids)
        if unknown:
            problems.append(f"MAP {code}: registry'de olmayan kural(lar): {', '.join(unknown)}")

    for entry in CONTEXT_MAPPINGS:
        unknown = sorted(set(entry.analyzer_rules) - registry_ids)
        if unknown:
            problems.append(
                f"CONTEXT {entry.md_code} ({entry.label}): registry'de olmayan kural(lar): "
                f"{', '.join(unknown)}"
            )

    declared = 0
    for card in sorted(DOCS_PATH.glob("**/*.md")):
        text = card.read_text(encoding="utf-8")
        codes = MARKER.findall(text)
        if not codes:
            continue
        match = CARD_ID.search(text)
        if not match:
            problems.append(f"{card.relative_to(ROOT)}: md-parity kart ID'si bulunamadı")
            continue
        rule_id = match.group(1)
        for code in codes:
            declared += 1
            mapped_for_code = set(audit.MAP.get(code, ()))
            mapped_for_code.update(
                rule
                for entry in CONTEXT_MAPPINGS
                if entry.md_code == code
                for rule in entry.analyzer_rules
            )
            if rule_id not in mapped_for_code:
                problems.append(
                    f"{card.relative_to(ROOT)}: {rule_id} {code} paritesini belgeliyor, "
                    "ancak parity mapping'de yok"
                )

    if "STM_056" not in mapped_ids:
        problems.append("STM_056 parity mapping'de yok")
    if not declared:
        problems.append("Hiç md-parity kart bildirimi bulunamadı")

    if problems:
        print("MD parity mapping drift — FAIL")
        for problem in problems:
            print(f"  - {problem}")
        return 1

    print(
        f"MD parity mapping OK — {len(audit.MAP)} MD kodu, "
        f"{len(mapped_ids)} Analyzer kuralı, {declared} kart bildirimi."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
