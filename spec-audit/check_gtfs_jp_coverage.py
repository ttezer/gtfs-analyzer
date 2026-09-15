#!/usr/bin/env python3
"""Validate the machine-readable GTFS-JP V3/V4 coverage contract.

The provision inventory is the denominator for the product's scoped 100%
claim.  This check deliberately fails closed when a strong provision is left
unmapped, a rule id is removed from the registry, or the published contract
document stops reflecting the inventory.
"""

from __future__ import annotations

import csv
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INVENTORY = ROOT / "spec-audit" / "gtfs_jp_provisions.tsv"
REGISTRY = ROOT / "crates" / "rules" / "src" / "registry.rs"
CONTRACT = ROOT / "docs" / "gtfs-jp-automated-coverage.md"
REGISTRY_ID = re.compile(r'r!\("([A-Z]{2,3}_\d{3}[a-z]?)"')
JPN_ID = re.compile(r"^JPN_\d{3}[a-z]?$")
REQUIRED_COLUMNS = {
    "provision_id",
    "profile",
    "strength",
    "automation",
    "rule_ids",
    "source",
    "note",
    "source_document",
    "source_version",
    "audited_on",
    "page_anchor",
}
ALLOWED_STRENGTHS = {"strong", "soft", "manual"}
ALLOWED_AUTOMATION = {"rule", "manual", "excluded_recommendation"}
ALLOWED_PROFILES = {"v3", "v4"}


def load_rows() -> list[dict[str, str]]:
    with INVENTORY.open(encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if reader.fieldnames is None or set(reader.fieldnames) != REQUIRED_COLUMNS:
            raise ValueError(
                "inventory columns must be exactly: "
                + ", ".join(sorted(REQUIRED_COLUMNS))
            )
        return list(reader)


def rule_ids(row: dict[str, str]) -> set[str]:
    return {
        value.strip()
        for value in row["rule_ids"].split(";")
        if value.strip()
    }


def main() -> int:
    problems: list[str] = []
    try:
        rows = load_rows()
    except (OSError, ValueError) as exc:
        print(f"GTFS-JP coverage inventory — FAIL: {exc}")
        return 1

    provision_ids = [row["provision_id"].strip() for row in rows]
    duplicates = sorted({value for value in provision_ids if provision_ids.count(value) > 1})
    if not all(provision_ids):
        problems.append("boş provision_id bulundu")
    if duplicates:
        problems.append("tekrarlı provision_id: " + ", ".join(duplicates))

    registry_ids = set(REGISTRY_ID.findall(REGISTRY.read_text(encoding="utf-8")))
    inventory_rules = set().union(*(rule_ids(row) for row in rows)) if rows else set()
    unknown_rules = sorted(inventory_rules - registry_ids)
    if unknown_rules:
        problems.append("registry'de olmayan rule id: " + ", ".join(unknown_rules))

    for row in rows:
        provision = row["provision_id"]
        strength = row["strength"]
        automation = row["automation"]
        mapped = rule_ids(row)
        profiles = {profile.strip() for profile in row["profile"].split(",") if profile.strip()}
        if not profiles or not profiles <= ALLOWED_PROFILES:
            problems.append(f"{provision}: bilinmeyen profile={row['profile']!r}")
        for provenance_field in ("source_document", "source_version", "audited_on", "page_anchor"):
            if not row[provenance_field].strip():
                problems.append(f"{provision}: provenance alanı boş: {provenance_field}")
        if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", row["audited_on"].strip()):
            problems.append(f"{provision}: audited_on YYYY-MM-DD olmalı")
        if strength not in ALLOWED_STRENGTHS:
            problems.append(f"{provision}: bilinmeyen strength={strength!r}")
        if automation not in ALLOWED_AUTOMATION:
            problems.append(f"{provision}: bilinmeyen automation={automation!r}")
        if strength == "manual" and automation != "manual":
            problems.append(f"{provision}: manual hüküm manual otomasyonuna sahip olmalı")
        if automation == "excluded_recommendation" and strength != "soft":
            problems.append(f"{provision}: excluded_recommendation yalnız soft hükümlerde kullanılabilir")
        if strength == "strong" and (automation != "rule" or not mapped):
            problems.append(
                f"{provision}: strong hüküm rule otomasyonuna ve en az bir rule id'ye sahip olmalı"
            )
        if automation == "rule" and not mapped:
            problems.append(f"{provision}: rule otomasyonu boş rule_ids taşıyor")

    registry_jpn = {value for value in registry_ids if JPN_ID.fullmatch(value)}
    inventory_jpn = {value for value in inventory_rules if JPN_ID.fullmatch(value)}
    if missing := sorted(registry_jpn - inventory_jpn):
        problems.append("envanterde eksik JPN kuralı: " + ", ".join(missing))
    if extra := sorted(inventory_jpn - registry_jpn):
        problems.append("registry dışı JPN kuralı: " + ", ".join(extra))

    contract_text = CONTRACT.read_text(encoding="utf-8")
    missing_contract_ids = sorted(set(provision_ids) - set(re.findall(r"`([A-Z][A-Z0-9-]+)`", contract_text)))
    if missing_contract_ids:
        problems.append(
            "sözleşme belgesinde olmayan provision_id: " + ", ".join(missing_contract_ids)
        )
    if "Eşleşmemiş güçlü makine hükmü: 0" not in contract_text:
        problems.append("sözleşme belgesinde 'Eşleşmemiş güçlü makine hükmü: 0' kanıtı yok")

    strong = [row for row in rows if row["strength"] == "strong"]
    strong_rules = [row for row in strong if row["automation"] == "rule" and rule_ids(row)]
    unmatched = [row["provision_id"] for row in strong if row["automation"] != "rule" or not rule_ids(row)]
    if unmatched:
        problems.append("eşleşmemiş güçlü makine hükmü: " + ", ".join(unmatched))

    if problems:
        print("GTFS-JP coverage inventory — FAIL")
        for problem in problems:
            print(f"  - {problem}")
        return 1

    print(
        "GTFS-JP coverage inventory OK — "
        f"{len(rows)} provision, {len(strong_rules)} strong machine mappings, "
        f"{len(inventory_jpn)} JPN rules, unmatched strong machine provisions: 0."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
