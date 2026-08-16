#!/usr/bin/env python3
"""`/usr/bin/time -v` çıktısının ayrıştırılması — #149'un regresyonu.

Bu dosyanın varlık sebebi tek bir satır: run-31934698855'te 4.259 feed'in
4.259'unda süre kolonu BOŞTU ve bu fark edilmedi, çünkü aynı dosyadan okunan
`max_rss_kb` çalışıyordu. Ölçümün yarısının doğru olması diğer yarısının
sessizce None dönmesini gizledi.
"""
from __future__ import annotations

import importlib.util
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

# GNU time -v'nin gerçek çıktısı; etiket parantezinin İÇİNDE iki nokta vardır.
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
        """Tembel `.*?:` etiketin içindeki iki noktada durup `mm:ss or m:ss): 0:01.23` yakalıyordu."""
        t = self._timing(TIME_V)
        self.assertEqual(t["elapsed"], "0:01.23")
        self.assertEqual(t["max_rss_kb"], 14228)

    def test_elapsed_reaches_a_number_end_to_end(self):
        """Ayrıştırma ile toplama arasındaki sözleşme: `timing()` çıktısı saniyeye çevrilebilmeli."""
        t = self._timing(TIME_V)
        self.assertAlmostEqual(agg.elapsed_seconds(t["elapsed"]), 1.23, places=2)

    def test_hour_form_is_parsed(self):
        t = self._timing(TIME_V.replace("0:01.23", "1:02:03"))
        self.assertEqual(agg.elapsed_seconds(t["elapsed"]), 3723.0)

    def test_the_old_capture_would_have_been_dropped(self):
        """Kusurun kendisini sabitler: eski desenin ürettiği metin sessizce None olur."""
        self.assertIsNone(agg.elapsed_seconds("mm:ss or m:ss): 0:01.23"))

    def test_missing_file_is_empty_not_an_exception(self):
        self.assertEqual(shard.timing(HERE / "does-not-exist.txt"), {})


class EmptyColumnGuard(unittest.TestCase):
    def test_a_wholly_empty_measured_column_is_an_error(self):
        """`n=0` bir medyan değil bir ARIZADIR; özet onu `null` diye basıp geçmemeli."""
        with self.assertRaises(SystemExit) as cm:
            agg.require_measured({"analyzer_wall_s": [], "md_wall_s": [1.0]}, attempted=10)
        self.assertIn("analyzer_wall_s", str(cm.exception))
        self.assertNotIn("md_wall_s", str(cm.exception))

    def test_a_populated_column_passes(self):
        agg.require_measured({"analyzer_wall_s": [1.0, 2.0]}, attempted=2)

    def test_nothing_attempted_is_not_a_measurement_failure(self):
        """Hiç feed koşulmadıysa boş kolon beklenendir; arıza ile boş korpus karıştırılmaz."""
        agg.require_measured({"analyzer_wall_s": []}, attempted=0)


if __name__ == "__main__":
    unittest.main()
