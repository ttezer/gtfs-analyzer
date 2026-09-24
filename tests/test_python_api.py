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
    def test_validate_returns_shared_result_shape(self):
        result = gtfs_analyzer.validate_gtfs(make_feed(), today="2026-09-24")

        self.assertIn(result["validation_status"], {"COMPLETE", "PARTIAL"})
        self.assertIn("notices", result)
        self.assertIn("reports", result)
        self.assertIn("metrics", result)

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
