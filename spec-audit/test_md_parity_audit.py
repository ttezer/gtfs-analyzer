#!/usr/bin/env python3
"""Regression fixtures for the field-aware MobilityData parity audit."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent


def load_module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"{filename} yüklenemedi")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


mapping = load_module("md_parity_mapping_test", "md_parity_mapping.py")
audit = load_module("md_parity_audit_test", "md_parity_audit.py")


class ContextMappingFixtures(unittest.TestCase):
    def test_same_generic_code_maps_by_file_and_field(self):
        fixtures = [
            (
                {"filename": "trips.txt", "fieldName": "direction_id", "fieldValue": "9"},
                ("TRP_005",),
            ),
            (
                {"filename": "transfers.txt", "fieldName": "transfer_type", "fieldValue": "9"},
                ("TRF_004",),
            ),
        ]
        for sample, expected in fixtures:
            with self.subTest(sample=sample):
                result = mapping.resolve_mapping(
                    "unexpected_enum_value",
                    {"sampleNotices": [sample]},
                    audit.MAP["unexpected_enum_value"],
                )
                self.assertEqual(result.analyzer_rules, expected)
                self.assertEqual(result.kind, "context-dependent")

    def test_route_type_value_range_selects_interop_rule(self):
        result = mapping.resolve_mapping(
            "unexpected_enum_value",
            {"sampleNotices": [{"filename": "routes.txt", "fieldName": "route_type", "fieldValue": "1501"}]},
            audit.MAP["unexpected_enum_value"],
        )
        self.assertEqual(result.analyzer_rules, ("RTS_030",))

        result = mapping.resolve_mapping(
            "unexpected_enum_value",
            {"sampleNotices": [{"filename": "routes.txt", "fieldName": "route_type", "fieldValue": "99"}]},
            audit.MAP["unexpected_enum_value"],
        )
        self.assertEqual(result.analyzer_rules, ("RTS_004",))

    def test_stm_056_is_no_longer_md_only(self):
        result = mapping.resolve_mapping(
            "decreasing_or_equal_stop_time_distance",
            {"sampleNotices": [{"filename": "stop_times.txt", "fieldName": "shape_dist_traveled"}]},
            audit.MAP["decreasing_or_equal_stop_time_distance"],
        )
        self.assertEqual(result.analyzer_rules, ("STM_056",))

    def test_unknown_context_is_visible(self):
        result = mapping.resolve_mapping(
            "unexpected_enum_value",
            {"sampleNotices": [{"filename": "new_extension.txt", "fieldName": "new_enum"}]},
            audit.MAP["unexpected_enum_value"],
        )
        self.assertEqual(result.analyzer_rules, ())
        self.assertEqual(result.kind, "unresolved-context")


class AuditFixture(unittest.TestCase):
    def test_fixture_run_keeps_context_and_aggregation_explicit(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            feed = base / "fixture"
            feed.mkdir()
            (feed / "md.json").write_text(
                json.dumps(
                    {
                        "notices": [
                            {
                                "code": "unexpected_enum_value",
                                "totalNotices": 2,
                                "severity": "ERROR",
                                "sampleNotices": [
                                    {"filename": "trips.txt", "fieldName": "direction_id", "fieldValue": "9"},
                                    {"filename": "transfers.txt", "fieldName": "transfer_type", "fieldValue": "9"},
                                ],
                            },
                            {
                                "code": "decreasing_or_equal_stop_time_distance",
                                "totalNotices": 1,
                                "severity": "ERROR",
                                "sampleNotices": [{"filename": "stop_times.txt", "fieldName": "shape_dist_traveled"}],
                            },
                        ]
                    }
                ),
                encoding="utf-8",
            )
            (feed / "golden.json").write_text(
                json.dumps(
                    {
                        "validation": {
                            "rule_counts": {
                                "TRP_005": {"count": 1, "severity": "LOW", "rule_class": "SPEC"},
                                "TRF_004": {"count": 1, "severity": "LOW", "rule_class": "SPEC"},
                                "STM_056": {"count": 1, "severity": "CRITICAL", "rule_class": "SPEC"},
                            },
                            "scores": {"overall": 1, "publish": 1},
                        }
                    }
                ),
                encoding="utf-8",
            )

            old_argv = sys.argv
            try:
                sys.argv = ["md_parity_audit.py", str(base)]
                audit.main()
            finally:
                sys.argv = old_argv

            rows = (base / "parity_all.csv").read_text(encoding="utf-8")
            self.assertIn("CONTEXT", rows)
            self.assertIn("context-mixed", rows)
            self.assertIn("STM_056", rows)
            self.assertNotIn(
                "decreasing_or_equal_stop_time_distance",
                (base / "parity_md_only.csv").read_text(encoding="utf-8"),
            )


if __name__ == "__main__":
    unittest.main()
