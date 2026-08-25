#!/usr/bin/env python3
"""`/usr/bin/time -v` çıktısının ayrıştırılması — #149'un regresyonu.

Bu dosyanın varlık sebebi tek bir satır: run-31934698855'te 4.259 feed'in
4.259'unda süre kolonu BOŞTU ve bu fark edilmedi, çünkü aynı dosyadan okunan
`max_rss_kb` çalışıyordu. Ölçümün yarısının doğru olması diğer yarısının
sessizce None dönmesini gizledi.
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


class StateClassification(unittest.TestCase):
    """`classify_analyzer` bir koşumu bitmiş sayarken ÇIKIŞ KODUNU okumalıdır.

    Kusurun kendisi: rapor dosyası diskte diye "completed" dönülüyordu. `timeout
    --signal=TERM` süreci 124 ile öldürür (takip SIGKILL'de 137) ve süreç o ana
    kadar çıktısının bir kısmını ZATEN yazmıştır. run-32145833613'te `mdb-2014`
    300 sn'de öldürüldü ve 1.551.740 bulguyla TEMİZ sayıldı; `aggregate.py` o kesik
    çıktıdan 11 sapma satırı türetti.

    Kusur bilgi eksikliği değil ASİMETRİYDİ: aynı dosyadaki `classify_md` bu ayrımı
    (`partial_timeout`) başından beri yapıyordu. Bu sınıf bu yüzden İKİ tarafı da
    aynı matrise karşı koşar — biri diğerinin eksiğini gösterir.
    """

    def setUp(self):
        self.present = HERE / "test_timing.py"          # var olan bir dosya
        self.absent = HERE / "does-not-exist-report.json"

    # ── kesilmiş koşum: rapor VAR ama süreç öldürülmüş ──────────────────────
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

    # ── temiz koşum: analyzer'ın 1'i "bulgu var" demektir, arıza değil ──────
    def test_analyzer_normal_exit_codes_are_completed(self):
        for code in (0, 1, 2):
            with self.subTest(exit_code=code):
                self.assertEqual(
                    shard.classify_analyzer(code, self.present, ""), "completed"
                )

    # ── rapor yoksa zaman aşımı zaten timeout'tur ───────────────────────────
    def test_no_report_after_sigterm_is_timeout(self):
        self.assertEqual(shard.classify_analyzer(124, self.absent, ""), "timeout")
        self.assertEqual(shard.classify_md(124, self.absent, None, ""), "timeout")

    def test_symmetric_states_for_the_same_inputs(self):
        """İki sınıflandırıcı aynı girdide aynı hükmü vermeli.

        MD'nin `partial_oom`/`partial_internal` gibi FAZLADAN durumları vardır ve
        bunlar Java'ya özgüdür; burada kıyaslanan yalnız ORTAK eksen: rapor
        varlığı × çıkış kodu.
        """
        for code in (0, 124, 137):
            for report in (self.present, self.absent):
                with self.subTest(exit_code=code, report=report.name):
                    self.assertEqual(
                        shard.classify_analyzer(code, report, ""),
                        shard.classify_md(code, report, None, ""),
                    )


class StateConsistencyGuard(unittest.TestCase):
    """`completed` + anormal çıkış kodu = kesik koşum temiz gibi sunulmuş."""

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
        """Analyzer'ın 1'i "bulgu var" demektir; arıza sanılırsa korpusun çoğu patlar."""
        agg.require_consistent_states(
            [{"feed_id": "f", "analyzer_state": "completed", "analyzer_exit": c}
             for c in (0, 1, 2)]
        )

    def test_partial_timeout_row_is_not_challenged(self):
        """Durum zaten dürüstse çıkış kodu ne olursa olsun sorun yok."""
        agg.require_consistent_states(
            [{"feed_id": "f", "analyzer_state": "partial_timeout", "analyzer_exit": 124}]
        )

    def test_missing_exit_code_is_not_invented(self):
        agg.require_consistent_states(
            [{"feed_id": "f", "analyzer_state": "completed", "analyzer_exit": None}]
        )


class AdjudicationLedgerReachable(unittest.TestCase):
    """Run 32290410755'in defteri hiç okuyamamasının regresyonu.

    `aggregate.py` hükümleri `md_parity_mapping` üzerinden okur ve benchmark
    onu `benchmark/audit_all/` köprüsünden yükler. Köprü kanonik dosyayı
    `exec` ettiği için `__file__` köprünün yolunda kalıyordu; defter
    `benchmark/audit_all/fp_adjudication.tsv` diye aranıp bulunamıyor,
    `_fp_verdicts()` de eksik dosyada SESSİZCE boş sözlük dönüyordu. Koşum
    hata vermedi — yalnız yargılanmış 14.980 satırı taze sapma diye yayımladı.

    Bu test köprüyü aracın kullandığı yoldan yükler; kanonik modülü doğrudan
    import etmek hatayı GÖREMEZ, çünkü orada `__file__` zaten doğrudur.
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
        """STP_033 defterde SCOPE_DIFFERENCE; koşum onu 3.134 kez taze saydı."""
        decision, _ = self.bridge.classify_analyzer_divergence("STP_033")
        self.assertTrue(decision.startswith("adjudicated:"),
                        f"STP_033 hükmü çözülmedi: {decision}")

    def test_aggregate_sees_the_same_ledger_as_the_bridge(self):
        """Kapı `aggregate.py`'nin gerçekten kullandığı bağlantıyı ölçer."""
        decision, _ = agg.classify_analyzer_divergence("STP_033")
        self.assertTrue(decision.startswith("adjudicated:"),
                        f"aggregate.py defteri okuyamıyor: {decision}")


if __name__ == "__main__":
    unittest.main()
