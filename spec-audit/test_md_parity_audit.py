#!/usr/bin/env python3
"""Regression fixtures for the field-aware MobilityData parity audit."""

from __future__ import annotations

import importlib.util
import json
import re
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

    def test_every_ledger_is_reachable_through_one_call(self):
        """A parity consumer must be able to ask one question and see all three ledgers.

        The full-catalog benchmark read only `MAP` and `AGG_RULES`, never `BY_DESIGN`,
        and re-reported 74% of an already-adjudicated backlog as fresh blindness (#146).
        This pins each ledger to a representative code, so moving or dropping one is a
        test failure rather than a silently inflated finding count.
        """
        for code, expected_source in [
            ("non_ascii_or_non_printable_char", "by-design"),
            ("missing_required_file", "mapped-divergence"),
            ("fast_travel_between_far_stops", "unmapped"),
        ]:
            with self.subTest(code=code):
                source, rationale = mapping.classify_divergence(code)
                self.assertTrue(
                    source.startswith(expected_source),
                    f"{code} should resolve via {expected_source}, got {source}",
                )
                self.assertTrue(rationale)
                self.assertTrue(mapping.is_adjudicated(code))

    def test_an_undecided_code_is_not_reported_as_adjudicated(self):
        source, rationale = mapping.classify_divergence("a_code_no_ledger_has_seen")
        self.assertEqual(source, "unreviewed")
        self.assertIn("No decision recorded", rationale)
        self.assertFalse(mapping.is_adjudicated("a_code_no_ledger_has_seen"))

    def test_by_design_lives_with_the_other_ledgers(self):
        """BY_DESIGN must stay in the mapping module, next to the tables it belongs with.

        It sat in `md_parity_audit.py` while the mapping module held the other two, which
        is why a new consumer that imported the mapping module still missed it.
        """
        self.assertGreater(len(mapping.BY_DESIGN), 20)
        # The fixtures load the mapping module twice under two names, so compare content
        # rather than identity; the point is that the audit reads this module's ledger.
        self.assertEqual(audit.BY_DESIGN, mapping.BY_DESIGN)
        for reason in mapping.BY_DESIGN.values():
            self.assertTrue(reason.strip(), "a BY_DESIGN entry without reasoning is a blind spot")

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
        # MobilityData counts two states under one code -- already expired and expiring
        # soon -- while we keep them apart (FIN_010 / FIN_019), so the mapping goes to
        # both. Pinning it to FIN_019 alone turned 841 feeds into a false MISS: 837 of
        # them emit FIN_010 and none emits FIN_019, and MD's own samples show 839 of the
        # 841 are already past their feed_end_date (#146).
        self.assertEqual(
            audit.MAP["feed_expiration_date7_days"], ["FIN_010", "FIN_019"]
        )
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


class LedgerDriftGate(unittest.TestCase):
    """Defter ve eşleme değişiklikleri yayımlanan sayıları SESSİZCE oynatabilir.

    Kural setinin kart tutarlılık testi, drift kapısı ve mutasyon sınanmış rozet
    kapısı var; onu yargılayan araçta hiçbiri yoktu (#159). Bu sınıf, bir kararın
    ya da eşlemenin şeklini SABİTLER — değiştirmek serbest, ama sessizce
    değiştirmek değil.

    🔴 Bu kapının varlık sebebi somut: `transfer_with_invalid_trip_and_stop` ve
    `transfer_with_invalid_trip_and_route` defterde `genuine-gap` olarak
    kayıtlıydı ve karar YANLIŞTI — kurallar (`XFL_021`, `TRF_017`) zaten vardı ve
    ateşliyordu. Yanlış karar bir issue'ya, oradan da yol haritasına taşındı.
    """

    # Sözlük UYDURULMAZ, kullanımdan DONDURULUR. Yeni bir etiket eklemek serbest —
    # ama bu listeyi de güncellemek gerekir, yani karar bilinçli olur. Bir etiketin
    # buradan düşmesi de sinyaldir: o kararı taşıyan kod silinmiş demektir.
    UNMAPPED_LABELS = {
        "genuine-gap", "deprecated-md-only", "md-implementation-limit",
        "config-dependent", "intentional-difference", "context-dependent",
    }
    MAPPED_LABELS = {"tolerance-by-design", "structural-fault-owns-it"}

    def test_ledger_labels_stay_within_the_frozen_vocabulary(self):
        """Etiketsiz ya da tanınmayan karar sayımlardan sessizce düşer."""
        for table, allowed, name in (
            (mapping.UNMAPPED_DECISIONS, self.UNMAPPED_LABELS, "UNMAPPED_DECISIONS"),
            (mapping.MAPPED_DIVERGENCE_DECISIONS, self.MAPPED_LABELS, "MAPPED_DIVERGENCE_DECISIONS"),
        ):
            used = set()
            for code, entry in table.items():
                with self.subTest(table=name, code=code):
                    self.assertIsInstance(entry, tuple, "karar (etiket, gerekçe) olmalı")
                    self.assertIn(entry[0], allowed, f"{code}: tanınmayan etiket {entry[0]!r}")
                    self.assertGreater(len(entry[1]), 40, f"{code}: gerekçe fazla kısa")
                    used.add(entry[0])
            self.assertEqual(used, allowed,
                             f"{name}: sözlükte artık kullanılmayan etiket var: {allowed - used}")

    def test_a_code_is_never_both_mapped_and_declared_a_gap(self):
        """Kapsam boşluğu ilan edilen bir kodun eşlemesi OLAMAZ — ikisi çelişir.

        Tam olarak bu çelişki iki kod için aylarca sürdü ve #158'i yanlış açtırdı.
        """
        for code, entry in mapping.UNMAPPED_DECISIONS.items():
            if entry[0] != "genuine-gap":
                continue
            with self.subTest(code=code):
                self.assertNotIn(
                    code, audit.MAP,
                    f"{code} hem 'genuine-gap' hem eşlemeli: kural varsa boşluk değildir",
                )
                self.assertFalse(
                    any(c.md_code == code for c in mapping.CONTEXT_MAPPINGS),
                    f"{code} hem 'genuine-gap' hem bağlamsal eşlemeli",
                )

    def test_classify_divergence_is_stable_for_known_codes(self):
        """Bu üç kod üç AYRI deftere düşer; kaynak etiketi karışırsa sayım kayar."""
        for code, expected_source in (
            ("non_ascii_or_non_printable_char", "by-design"),
            ("fast_travel_between_far_stops", "unmapped:genuine-gap"),
            ("this_code_does_not_exist_anywhere", "unreviewed"),
        ):
            with self.subTest(code=code):
                self.assertEqual(mapping.classify_divergence(code)[0], expected_source)

    def test_the_two_transfer_consistency_codes_are_mapped_not_gaps(self):
        """#158 regresyonu: bu ikisi bir daha 'kapsam boşluğu' olarak geri dönmesin."""
        for code, rule in (
            ("transfer_with_invalid_trip_and_stop", "XFL_021"),
            ("transfer_with_invalid_trip_and_route", "TRF_017"),
            ("pathway_dangling_generic_node", "PTH_019"),
        ):
            with self.subTest(code=code):
                self.assertIn(rule, audit.MAP.get(code, []),
                              f"{code} → {rule} eşlemesi kayboldu")

    def test_every_mapped_rule_exists_in_the_registry(self):
        """Var olmayan bir kurala eşlenen kod SESSİZ bir MISS üretir.

        Eşleme "bu MD kodunun bizdeki karşılığı X" der; X yoksa benchmark sonsuza
        kadar "MD raporladı, biz susuyoruz" yazar ve hiçbir şey itiraz etmez.
        """
        registry = (Path(__file__).resolve().parents[1]
                    / "crates" / "rules" / "src" / "registry.rs").read_text(encoding="utf-8")
        known = set(re.findall(r'r!\("([A-Z]{2,4}_\d{3}[a-z]?)"', registry))
        self.assertGreater(len(known), 500, "registry ayrıştırması boş döndü — ÖNCE SORGUYU şüphelen")
        for code, rules in audit.MAP.items():
            for rule in rules:
                with self.subTest(code=code, rule=rule):
                    self.assertIn(rule, known, f"{code} → {rule}: registry'de böyle bir kural yok")
        for entry in mapping.CONTEXT_MAPPINGS:
            for rule in entry.analyzer_rules:
                with self.subTest(code=entry.md_code, rule=rule):
                    self.assertIn(rule, known, f"{entry.md_code} → {rule}: registry'de yok")


if __name__ == "__main__":
    unittest.main()
