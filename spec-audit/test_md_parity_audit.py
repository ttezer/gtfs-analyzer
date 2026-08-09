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

    def test_partial_samples_do_not_claim_exact_generic_parity(self):
        result = mapping.resolve_mapping(
            "unexpected_enum_value",
            {
                "totalNotices": 10,
                "sampleNotices": [
                    {"filename": "trips.txt", "fieldName": "direction_id", "fieldValue": "9"}
                ],
            },
            audit.MAP["unexpected_enum_value"],
        )
        self.assertEqual(result.analyzer_rules, ("TRP_005",))
        self.assertEqual(result.kind, "context-mixed")
        self.assertFalse(result.context_complete)

    def test_unmapped_corpus_headings_have_explicit_adjudication(self):
        for code in [
            "fast_travel_between_far_stops",
            "feed_expiration_date30_days",
            "feed_valid_beyond_total_service_window",
            "start_and_end_range_equal",
            "unused_trip",
        ]:
            with self.subTest(code=code):
                classification, rationale = mapping.classify_unmapped(code)
                self.assertNotEqual(classification, "unreviewed")
                self.assertTrue(rationale)

    def test_far_stop_speed_gap_is_not_aliased_to_consecutive_speed(self):
        classification, rationale = mapping.classify_unmapped(
            "fast_travel_between_far_stops"
        )
        self.assertEqual(classification, "genuine-gap")
        self.assertIn("non-consecutive", rationale)
        self.assertNotIn(
            "fast_travel_between_far_stops",
            audit.MAP,
            "far-stop MD notice must remain an explicit, unaffiliated coverage gap",
        )

    def test_feed_expiration_30_day_parity_is_config_dependent(self):
        classification, rationale = mapping.classify_unmapped(
            "feed_expiration_date30_days"
        )
        self.assertEqual(classification, "config-dependent")
        self.assertIn("feed_info_expiry_warning_days=30", rationale)
        self.assertIn("7-day default", rationale)

        self.assertEqual(audit.MAP["same_stop_and_agency_url"], ["STP_034"])
        self.assertEqual(audit.MAP["same_stop_and_route_url"], ["STP_035"])
        self.assertEqual(
            audit.MAP["trip_with_shape_dist_traveled_but_no_shape_distances"], ["SHP_030"]
        )
        self.assertEqual(audit.MAP["single_shape_point"], ["SHP_006"])
        self.assertEqual(audit.MAP["feed_expiration_date7_days"], ["FIN_019"])
        for code, rule in {
            "invalid_row_length": "ARC_012",
            "missing_required_file": "ARC_004",
            "missing_calendar_and_calendar_date_files": "ARC_008",
            "invalid_input_files_in_subfolder": "ARC_024",
            "stop_time_with_arrival_before_previous_departure_time": "STM_008",
            "stop_time_timepoint_without_times": "STM_047",
            "overlapping_frequency": "FRQ_011",
            "pathway_unreachable_location": "PTH_012",
            "missing_feed_info_date": "FIN_014",
            "more_than_one_entity": "FIN_015",
        }.items():
            with self.subTest(code=code):
                self.assertIn(rule, audit.MAP[code])

    def test_generic_context_mappings_do_not_collapse_to_one_rule(self):
        fixtures = [
            ("invalid_url", {"filename": "agency.txt", "fieldName": "agency_url"}, ("AGN_003",)),
            ("invalid_url", {"filename": "routes.txt", "fieldName": "route_url"}, ("RTS_005",)),
            ("invalid_date", {"filename": "calendar.txt", "fieldName": "end_date"}, ("CAL_004",)),
            ("number_out_of_range", {"filename": "frequencies.txt", "fieldName": "headway_secs", "fieldValue": 0}, ("FRQ_008",)),
        ]
        for code, sample, expected in fixtures:
            with self.subTest(code=code, sample=sample):
                result = mapping.resolve_mapping(code, {"sampleNotices": [sample]}, audit.MAP[code])
                self.assertEqual(result.analyzer_rules, expected)
                self.assertEqual(result.kind, "context-dependent")


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
