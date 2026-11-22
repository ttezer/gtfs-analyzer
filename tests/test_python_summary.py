"""Tests for ``python -m gtfs_analyzer.summary`` (used by the Claude plugin's skill).

The demo feed has deliberate faults: stop_times go backwards on trip T1 (STM_008), and
stop S4 duplicates S3's coordinates and serves no trip.
"""

import json
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

import gtfs_analyzer

TODAY = "20261015"

DEMO_FEED = {
    "agency.txt": "agency_id,agency_name,agency_url,agency_timezone\n"
    "A1,Demo Transit,https://example.com,Europe/Istanbul\n",
    "calendar.txt": "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n"
    "WK,1,1,1,1,1,0,0,20261001,20261231\n",
    "routes.txt": "route_id,agency_id,route_short_name,route_long_name,route_type\n"
    "R1,A1,1,Merkez - Otogar,3\n",
    "stop_times.txt": "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n"
    "T1,08:00:00,08:00:00,S1,1\n"
    "T1,08:10:00,08:10:00,S2,2\n"
    "T1,08:05:00,08:05:00,S3,3\n"
    "T2,09:00:00,09:00:00,S1,1\n"
    "T2,09:10:00,09:10:00,S2,2\n"
    "T2,09:20:00,09:20:00,S3,3\n",
    "stops.txt": "stop_id,stop_name,stop_lat,stop_lon\n"
    "S1,Merkez,37.2150,28.3630\n"
    "S2,Hastane,37.2200,28.3700\n"
    "S3,Otogar,37.2250,28.3800\n"
    "S4,Kopya Durak,37.2250,28.3800\n",
    "trips.txt": "route_id,service_id,trip_id\nR1,WK,T1\nR1,WK,T2\n",
}


def run(*args):
    proc = subprocess.run(
        [sys.executable, "-m", "gtfs_analyzer.summary", *args],
        capture_output=True, text=True, check=False,
    )
    return proc.returncode, json.loads(proc.stdout)


class SummaryCommandTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)
        self.feed = self.build_zip("feed.zip")

    def tearDown(self):
        self._tmp.cleanup()

    def build_zip(self, name, drop=()):
        path = self.tmp / name
        with zipfile.ZipFile(path, "w") as archive:
            for file, text in sorted(DEMO_FEED.items()):
                if file not in drop:
                    archive.writestr(file, text)
        return str(path)

    def test_summary_keeps_engine_verdict_scores_and_order(self):
        code, out = run(self.feed, "--today", TODAY, "--max-priorities", "100")
        engine = gtfs_analyzer.validate_gtfs(self.feed, today=TODAY)

        self.assertEqual(code, 0)
        self.assertEqual(out["validation_status"], "COMPLETE")
        self.assertEqual(out["publishability"], engine["reports"]["r1"])
        self.assertEqual(out["scores"], engine["reports"]["r5"])
        self.assertEqual(
            [p["rule_id"] for p in out["r9_priorities"]],
            [i["rule_id"] for i in engine["reports"]["r9"]["items"]],
        )
        self.assertEqual(out["total_notices"], len(engine["notices"]))
        self.assertEqual(sum(out["severity_counts"].values()), out["total_notices"])
        self.assertEqual(out["engine_version"], gtfs_analyzer.__version__)
        self.assertNotIn("file_stats", out["metrics"])
        self.assertNotIn("version_warning", out)

    def test_known_faults_are_reported_with_examples(self):
        _, out = run(self.feed, "--today", TODAY, "--max-priorities", "100")
        by_rule = {p["rule_id"]: p for p in out["r9_priorities"]}

        self.assertIn("STM_008", by_rule)
        self.assertEqual(by_rule["STM_008"]["examples"][0]["file"], "stop_times.txt")
        self.assertLessEqual({"STP_016", "STP_020"}, set(by_rule))

    def test_limits_cap_priorities_and_examples(self):
        _, out = run(self.feed, "--today", TODAY, "--max-priorities", "2", "--examples", "1")

        self.assertEqual(len(out["r9_priorities"]), 2)
        self.assertTrue(all(len(p["examples"]) <= 1 for p in out["r9_priorities"]))
        self.assertGreater(out["r9_total"], 2)

    def test_language_switches_notice_text(self):
        _, en = run(self.feed, "--today", TODAY, "--lang", "en")
        _, tr = run(self.feed, "--today", TODAY, "--lang", "tr")

        self.assertEqual(en["r9_priorities"][0]["rule_id"], tr["r9_priorities"][0]["rule_id"])
        self.assertNotEqual(en["r9_priorities"][0]["title"], tr["r9_priorities"][0]["title"])

    def test_partial_result_is_marked_and_not_publishable(self):
        feed = self.build_zip("partial.zip", drop=("routes.txt",))
        code, out = run(feed, "--today", TODAY)

        self.assertEqual(code, 0)
        self.assertEqual(out["validation_status"], "PARTIAL")
        self.assertIn("routes.txt", out["partial"]["unavailable_files"])
        self.assertIs(out["publishability"]["publishable"], False)
        self.assertGreaterEqual(out["blocker_total"], 1)
        self.assertEqual(out["blocker_total"], len(out["publishability"]["blocker_notice_ids"]))

    def test_unreadable_zip_is_fatal(self):
        bad = self.tmp / "bad.zip"
        bad.write_bytes(b"not a zip")
        code, out = run(str(bad))

        self.assertEqual(code, 2)
        self.assertEqual(out["error"], "FATAL")

    def test_input_errors_use_exit_code_4(self):
        cases = [
            ("missing.zip",),
            (self.feed, "--today", "20261301"),
            (self.feed, "--examples", "-1"),
            (self.feed, "--lang", "de"),
            (),
        ]
        for args in cases:
            with self.subTest(args=args):
                code, out = run(*args)
                self.assertEqual(code, 4)
                self.assertEqual(out["error"], "INPUT")

    def test_version_mismatch_adds_warning(self):
        _, out = run(self.feed, "--today", TODAY, "--expect-version", "0.0.0")
        self.assertEqual(out["version_warning"], "expected gtfs-analyzer 0.0.0")

        _, same = run(self.feed, "--today", TODAY, "--expect-version", gtfs_analyzer.__version__)
        self.assertNotIn("version_warning", same)


if __name__ == "__main__":
    unittest.main()
