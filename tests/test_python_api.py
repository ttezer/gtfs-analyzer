import io
import unittest
import zipfile

import gtfs_analyzer


def make_feed() -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        archive.writestr(
            "agency.txt",
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n"
            "A,Demo,https://example.com,Europe/Istanbul,tr\n",
        )
    return buffer.getvalue()


class PythonApiTests(unittest.TestCase):
    def test_version_matches_package_metadata(self):
        from importlib.metadata import version

        self.assertEqual(gtfs_analyzer.__version__, version("gtfs-analyzer"))

    def test_validate_returns_shared_result_shape(self):
        result = gtfs_analyzer.validate_gtfs(make_feed(), today="2026-09-24")

        self.assertIn(result["validation_status"], {"COMPLETE", "PARTIAL"})
        self.assertIn("notices", result)
        self.assertIn("reports", result)
        self.assertIn("metrics", result)

    def test_messages_default_to_english(self):
        result = gtfs_analyzer.validate_gtfs(make_feed(), today="2026-09-24")
        notice = next(n for n in result["notices"] if n["rule_id"] == "ARC_004")
        self.assertTrue(notice["message"].isascii(), notice["message"])
        self.assertTrue(notice["title"].isascii(), notice["title"])

    def test_turkish_keeps_the_engine_text(self):
        english = gtfs_analyzer.validate_gtfs(make_feed(), today="2026-09-24")
        turkish = gtfs_analyzer.validate_gtfs(make_feed(), today="2026-09-24", lang="tr")
        pick = lambda r: next(n for n in r["notices"] if n["rule_id"] == "ARC_004")
        self.assertNotEqual(pick(english)["message"], pick(turkish)["message"])
        self.assertEqual(
            [n["rule_id"] for n in english["notices"]],
            [n["rule_id"] for n in turkish["notices"]],
        )

    def test_unknown_language_is_rejected(self):
        with self.assertRaises(ValueError):
            gtfs_analyzer.validate_gtfs(make_feed(), today="2026-09-24", lang="de")
        with self.assertRaises(TypeError):
            gtfs_analyzer.validate_gtfs(make_feed(), today="2026-09-24", lang=None)

    def test_fatal_error_is_translated(self):
        with self.assertRaises(gtfs_analyzer.ValidationError) as english:
            gtfs_analyzer.validate_gtfs(b"not a zip", today=20260924)
        with self.assertRaises(gtfs_analyzer.ValidationError) as turkish:
            gtfs_analyzer.validate_gtfs(b"not a zip", today=20260924, lang="tr")
        self.assertTrue(str(english.exception).isascii(), str(english.exception))
        self.assertNotEqual(str(english.exception), str(turkish.exception))

    def test_invalid_zip_is_a_validation_error(self):
        with self.assertRaises(gtfs_analyzer.ValidationError):
            gtfs_analyzer.validate_gtfs(b"not a zip", today=20260924)

    def test_unknown_config_is_rejected(self):
        with self.assertRaises(ValueError):
            gtfs_analyzer.validate_gtfs(
                make_feed(),
                config={"unknown_python_config": True},
            )

    def test_invalid_today_is_rejected(self):
        with self.assertRaises(ValueError):
            gtfs_analyzer.validate_gtfs(make_feed(), today="2026-02-30")


if __name__ == "__main__":
    unittest.main()
