#!/usr/bin/env python3
"""Regression tests for the GTFS-JP provision denominator and rule mapping."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("gtfs_jp_coverage", HERE / "check_gtfs_jp_coverage.py")
assert spec is not None and spec.loader is not None
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


class GtfsJpCoverageTests(unittest.TestCase):
    def test_inventory_is_closed_and_machine_rows_are_mapped(self):
        rows = coverage.load_rows()
        self.assertEqual(len(rows), 38)
        self.assertTrue(all(row["source_document"] and row["source_version"] for row in rows))
        self.assertTrue(all(row["audited_on"] == "2026-09-16" for row in rows))
        self.assertTrue(all(row["strength"] != "strong" or row["automation"] == "rule" for row in rows))
        machine = [row for row in rows if row["automation"] == "rule"]
        self.assertEqual(len(machine), 31)
        self.assertEqual(sum(row["strength"] == "soft" for row in machine), 2)
        self.assertTrue(all(coverage.rule_ids(row) for row in machine))
        self.assertEqual(
            sum(row["automation"] == "excluded_recommendation" for row in rows), 2
        )
        mapped = set().union(*(coverage.rule_ids(row) for row in rows))
        jpn = {rule for rule in mapped if coverage.JPN_ID.fullmatch(rule)}
        registry = set(coverage.REGISTRY_ID.findall(coverage.REGISTRY.read_text(encoding="utf-8")))
        self.assertEqual(
            jpn,
            {f"JPN_{number:03d}" for number in range(1, 34)}
            - coverage.NON_NORMATIVE_JPN_RULES,
        )
        self.assertTrue(jpn <= registry)

    def test_checker_passes_current_contract(self):
        self.assertEqual(coverage.main(), 0)

    def test_machine_gate_includes_soft_rule_rows(self):
        rows = [
            {"provision_id": "SOFT-MAPPED", "automation": "rule", "rule_ids": "JPN_013"},
            {"provision_id": "SOFT-UNMAPPED", "automation": "rule", "rule_ids": ""},
            {"provision_id": "RECOMMENDATION", "automation": "excluded_recommendation", "rule_ids": ""},
        ]
        self.assertEqual(coverage.unmatched_machine_provisions(rows), ["SOFT-UNMAPPED"])


if __name__ == "__main__":
    unittest.main()
