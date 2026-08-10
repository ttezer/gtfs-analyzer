#!/usr/bin/env python3
"""Regenerate the reviewed evidence fingerprint lock.

Run after deliberately re-adjudicating a changed hard-provision evidence set:
    python3 spec-audit/update_provision_evidence_lock.py
"""

from __future__ import annotations

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
import badge_status  # noqa: E402


def main() -> int:
    provisions = {
        row["id"]: row
        for row in json.loads((ROOT / "spec_provisions.json").read_text(encoding="utf-8"))["provisions"]
    }
    triage = (ROOT / "PROVISION_TRIAGE.md").read_text(encoding="utf-8")
    verdict = badge_status.extract_verdicts(provisions, triage)
    evidence = badge_status.load_provision_evidence(ROOT / "provision_evidence.tsv")
    classes = badge_status.extract_rule_classes(
        (ROOT.parent / "crates/rules/src/registry.rs").read_text(encoding="utf-8")
    )

    active = sorted(
        pid
        for pid, status in verdict.items()
        if provisions[pid]["strength"] == "strong"
        and status in ("KANITLI", "KISMİ", "DOLAYLI")
        and pid in evidence
    )
    missing = [pid for pid in active if pid not in evidence]
    unknown = sorted(
        {rule_id for pid in active for rule_id in evidence.get(pid, []) if rule_id not in classes}
    )
    if missing or unknown:
        if missing:
            print(f"Eksik kanıt satırları: {', '.join(missing)}", file=sys.stderr)
        if unknown:
            print(f"Registry'de olmayan kanıt kuralları: {', '.join(unknown)}", file=sys.stderr)
        return 1

    lines = [
        "# provision_id<TAB>sha256(pid + canonical sorted (rule_id, class) pairs)",
        "# Severity intentionally excluded; this lock detects evidence staleness, not semantic coverage.",
    ]
    for pid in active:
        lines.append(f"{pid}\t{badge_status.evidence_fingerprint(pid, evidence[pid], classes)}")
    (ROOT / "provision_evidence.lock").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Yazıldı: provision_evidence.lock ({len(active)} hüküm)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
