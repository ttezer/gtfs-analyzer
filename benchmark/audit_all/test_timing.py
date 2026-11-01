#!/usr/bin/env python3
"""Parsing `/usr/bin/time -v` output—a regression test for #149.

This file exists for one reason: in run-31934698855 the time column was EMPTY in
all 4,259 feeds, and nobody noticed because `max_rss_kb` from the same file worked.
 Having half the measurement correct hid the other half
 silently returning None.
"""
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


shard = load("run_shard_test", "run_shard.py")
agg = load("aggregate_test", "aggregate.py")
probe = load("platform_probe_test", "platform_probe.py")

# Real GNU time -v output; the label contains a colon INSIDE its parentheses.
TIME_V = """\tCommand being timed: "gtfs-analyzer validate feed.zip"
\tUser time (seconds): 1.20
\tSystem time (seconds): 0.30
\tPercent of CPU this job got: 95%
\tElapsed (wall clock) time (h:mm:ss or m:ss): 0:01.23
\tMaximum resident set size (kbytes): 14228
\tExit status: 0
"""


class TimingParse(unittest.TestCase):
    def _timing(self, text):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "time.txt"
            p.write_text(text, encoding="utf-8")
            return shard.timing(p)

    def test_elapsed_is_the_value_not_part_of_the_label(self):
        """The lazy `.*?:` stopped at the colon inside the label and captured `mm:ss or m:ss): 0:01.23`."""
        t = self._timing(TIME_V)
        self.assertEqual(t["elapsed"], "0:01.23")
        self.assertEqual(t["max_rss_kb"], 14228)

    def test_elapsed_reaches_a_number_end_to_end(self):
        """Contract between parsing and aggregation: `timing()` output must convert to seconds."""
        t = self._timing(TIME_V)
        self.assertAlmostEqual(agg.elapsed_seconds(t["elapsed"]), 1.23, places=2)

    def test_hour_form_is_parsed(self):
        t = self._timing(TIME_V.replace("0:01.23", "1:02:03"))
        self.assertEqual(agg.elapsed_seconds(t["elapsed"]), 3723.0)

    def test_the_old_capture_would_have_been_dropped(self):
        """Locks in the defect itself: text produced by the old pattern silently became None."""
        self.assertIsNone(agg.elapsed_seconds("mm:ss or m:ss): 0:01.23"))

    def test_missing_file_is_empty_not_an_exception(self):
        self.assertEqual(shard.timing(HERE / "does-not-exist.txt"), {})


class DownloadMetadata(unittest.TestCase):
    def test_curl_trailer_preserves_http_payload_metadata(self):
        meta=shard.parse_download_metadata(
            "https://example.test/archive.zip\t200\tapplication/zip\t1234\n"
        )
        self.assertEqual(meta["effective_url"], "https://example.test/archive.zip")
        self.assertEqual(meta["http_status"], 200)
        self.assertEqual(meta["content_type"], "application/zip")
        self.assertEqual(meta["response_bytes"], 1234)

    def test_malformed_curl_trailer_keeps_raw_url(self):
        self.assertEqual(
            shard.parse_download_metadata("https://example.test/error"),
            {"effective_url": "https://example.test/error"},
        )


class AnalyzerProfiles(unittest.TestCase):
    def test_profiles_are_deduplicated_in_requested_order(self):
        self.assertEqual(
            shard.parse_profiles("auto,v3,auto,v4"), ["auto", "v3", "v4"]
        )

    def test_unknown_profile_is_rejected(self):
        with self.assertRaises(ValueError):
            shard.parse_profiles("auto,v5")

    def test_profile_summary_preserves_rule_counts(self):
        result = agg.summarize_analyzer_profiles([
            {
                "feed": {"feed_id": "feed-1"},
                "analyzer_profiles": {
                    "v3": {
                        "state": "completed",
                        "notice_count": 3,
                        "by_rule": {"JPN_028": 2, "JPN_030": 1},
                    },
                    "v4": {
                        "state": "partial_timeout",
                        "notice_count": 4,
                        "by_rule": {"JPN_029": 4},
                    },
                },
            },
        ])
        self.assertEqual(result["profiles"]["v3"]["completed"], 1)
        self.assertEqual(result["profiles"]["v3"]["notice_total"], 3)
        self.assertEqual(result["profiles"]["v3"]["rules_seen"], 2)
        self.assertEqual(result["rules"][0]["profile"], "v3")

    def test_platform_probe_extracts_current_profile_counts(self):
        result = probe.extract_probe([
            {
                "feed": {"feed_id": "jbda-nantocity-nanbus"},
                "analyzer_profiles": {
                    "auto": {
                        "state": "completed",
                        "exit_code": 1,
                        "timing": {"elapsed": "0:00.08"},
                        "by_rule": {"STM_014": 4},
                    }
                },
            },
            {
                "feed": {"feed_id": "jbda-kagaminotown-kagaminotownbus"},
                "analyzer_profiles": {
                    "auto": {"state": "completed", "by_rule": {"SHP_017": 11}}
                },
            },
        ])
        counts = {
            (case["feed_id"], case["rule_id"]): case["notice_count"]
            for case in result["cases"]
        }
        self.assertEqual(counts[("jbda-nantocity-nanbus", "STM_014")], 4)
        self.assertEqual(
            counts[("jbda-kagaminotown-kagaminotownbus", "SHP_017")], 11
        )
        self.assertEqual(len(result["missing"]), 3)


class SourceDrift(unittest.TestCase):
    def test_baseline_comparison_reports_changed_payload(self):
        with tempfile.TemporaryDirectory() as d:
            baseline=Path(d) / "baseline.json"
            baseline.write_text(json.dumps([{
                "feed":{"feed_id":"mdb-1"},
                "download":{"sha256":"old","bytes":10,"effective_url":"https://old"},
            }]))
            current=[{
                "feed":{"feed_id":"mdb-1"},
                "download":{"sha256":"new","bytes":11,"effective_url":"https://new"},
            }]
            result=agg.compare_source_drift(current, baseline)
            self.assertTrue(result["baseline_checked"])
            self.assertEqual(result["changed_feed_count"], 1)
            self.assertEqual(result["changed"][0]["feed_id"], "mdb-1")

    def test_without_baseline_current_hashes_are_not_called_stable(self):
        result=agg.compare_source_drift([], None)
        self.assertFalse(result["baseline_checked"])
        self.assertIn("hashes are still recorded", result["reason"])


class EmptyColumnGuard(unittest.TestCase):
    def test_a_wholly_empty_measured_column_is_an_error(self):
        """`n=0` is not a median but an ERROR; the summary must not print `null` and move on."""
        with self.assertRaises(SystemExit) as cm:
            agg.require_measured({"analyzer_wall_s": [], "md_wall_s": [1.0]}, attempted=10)
        self.assertIn("analyzer_wall_s", str(cm.exception))
        self.assertNotIn("md_wall_s", str(cm.exception))

    def test_a_populated_column_passes(self):
        agg.require_measured({"analyzer_wall_s": [1.0, 2.0]}, attempted=2)

    def test_nothing_attempted_is_not_a_measurement_failure(self):
        """If no feed was attempted, an empty column is expected; do not confuse failure with an empty corpus."""
        agg.require_measured({"analyzer_wall_s": []}, attempted=0)


class StateClassification(unittest.TestCase):
    """`classify_analyzer` must read the EXIT CODE when deciding that a run finished.

    The defect: the function returned "completed" because the report existed on disk. `timeout
    --signal=TERM` kills the process with 124 (the follow-up SIGKILL uses 137), after it has
    already written part of its output. In run-32145833613, `mdb-2014` was killed at 300 s
    and counted as CLEAN with 1,551,740 findings; `aggregate.py` derived 11 divergence rows
    from that truncated output.

    The defect was not missing information but ASYMMETRY: `classify_md` in the same file
    had made this distinction (`partial_timeout`) from the start. This class therefore runs
    BOTH sides against the same matrix—one exposes what the other lacks.
    """

    def setUp(self):
        self.present = HERE / "test_timing.py"          # an existing file
        self.absent = HERE / "does-not-exist-report.json"

    # ── truncated run: the report EXISTS but the process was killed ──────────
    def test_analyzer_sigterm_with_report_is_not_completed(self):
        for code in (124, 137):
            with self.subTest(exit_code=code):
                self.assertEqual(
                    shard.classify_analyzer(code, self.present, ""),
                    "partial_timeout",
                    "rapor diskte olsa bile 124/137 bitmiş koşum DEĞİLDİR",
                )

    def test_md_sigterm_with_report_is_not_completed(self):
        for code in (124, 137):
            with self.subTest(exit_code=code):
                self.assertEqual(
                    shard.classify_md(code, self.present, None, ""),
                    "partial_timeout",
                )

    # ── clean run: analyzer exit 1 means "findings", not failure ─────────────
    def test_analyzer_normal_exit_codes_are_completed(self):
        for code in (0, 1, 2):
            with self.subTest(exit_code=code):
                self.assertEqual(
                    shard.classify_analyzer(code, self.present, ""), "completed"
                )

    # ── without a report, a timeout is simply a timeout ───────────────────────
    def test_no_report_after_sigterm_is_timeout(self):
        self.assertEqual(shard.classify_analyzer(124, self.absent, ""), "timeout")
        self.assertEqual(shard.classify_md(124, self.absent, None, ""), "timeout")

    def test_symmetric_states_for_the_same_inputs(self):
        """Both classifiers must reach the same decision for the same input.

        MD has EXTRA states such as `partial_oom`/`partial_internal`, which are
        Java-specific;
        this comparison uses only the SHARED axis: report existence × exit code.
        """
        for code in (0, 124, 137):
            for report in (self.present, self.absent):
                with self.subTest(exit_code=code, report=report.name):
                    self.assertEqual(
                        shard.classify_analyzer(code, report, ""),
                        shard.classify_md(code, report, None, ""),
                    )


class StateConsistencyGuard(unittest.TestCase):
    """`completed` + an abnormal exit code means a truncated run was presented as clean."""

    def test_completed_with_sigterm_exit_is_an_error(self):
        rows = [
            {"feed_id": "mdb-2014", "analyzer_state": "completed", "analyzer_exit": 124},
            {"feed_id": "mdb-1", "analyzer_state": "completed", "analyzer_exit": 1},
        ]
        with self.assertRaises(SystemExit) as cm:
            agg.require_consistent_states(rows)
        self.assertIn("mdb-2014", str(cm.exception))
        self.assertNotIn("mdb-1", str(cm.exception))

    def test_findings_exit_code_is_clean(self):
        """Analyzer exit 1 means "findings"; treating it as failure breaks most of the corpus."""
        agg.require_consistent_states(
            [{"feed_id": "f", "analyzer_state": "completed", "analyzer_exit": c}
             for c in (0, 1, 2)]
        )

    def test_partial_timeout_row_is_not_challenged(self):
        """If the state is already honest, the exit code does not matter."""
        agg.require_consistent_states(
            [{"feed_id": "f", "analyzer_state": "partial_timeout", "analyzer_exit": 124}]
        )

    def test_missing_exit_code_is_not_invented(self):
        agg.require_consistent_states(
            [{"feed_id": "f", "analyzer_state": "completed", "analyzer_exit": None}]
        )


class AdjudicationLedgerReachable(unittest.TestCase):
    """Regression test for run 32290410755 failing to read the ledger at all.

    `aggregate.py` reads rulings through `md_parity_mapping`, and the benchmark loads it
    through the `benchmark/audit_all/` bridge. Because the bridge `exec`s the canonical file,
    `__file__` remained the bridge path; the ledger was sought as
    `benchmark/audit_all/fp_adjudication.tsv` and not found, while `_fp_verdicts()` silently
    returned an empty dictionary. The run did not fail—it published 14,980 adjudicated rows
    as fresh divergences.

    This test loads the bridge through the path used by the tool; importing the canonical
    module directly would not SEE the defect because `__file__` is already correct there.
    """

    def setUp(self):
        self.bridge = load("md_parity_bridge_test", "md_parity_mapping.py")

    def test_ledger_path_resolves_to_the_canonical_file(self):
        self.assertTrue(
            self.bridge._FP_LEDGER.exists(),
            f"defter köprüden görünmüyor: {self.bridge._FP_LEDGER}",
        )

    def test_recorded_verdicts_are_actually_read(self):
        self.assertGreater(len(self.bridge._fp_verdicts()), 0,
                           "fp_adjudication.tsv boş okundu")

    def test_a_settled_rule_classifies_as_adjudicated(self):
        """STP_033 has SCOPE_DIFFERENCE in the ledger; the run counted it as fresh 3,134 times."""
        decision, _ = self.bridge.classify_analyzer_divergence("STP_033")
        self.assertTrue(decision.startswith("adjudicated:"),
                        f"STP_033 hükmü çözülmedi: {decision}")

    def test_aggregate_sees_the_same_ledger_as_the_bridge(self):
        """The gate measures the bridge actually used by `aggregate.py`."""
        decision, _ = agg.classify_analyzer_divergence("STP_033")
        self.assertTrue(decision.startswith("adjudicated:"),
                        f"aggregate.py defteri okuyamıyor: {decision}")


if __name__ == "__main__":
    unittest.main()
