"""Tests for scripts/issue_progress.py.  Run: python3 -m unittest scripts/test_issue_progress.py"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
from issue_progress import update  # noqa: E402

BODY = """Intro.

## Review progress

<details><summary><b>STM</b> · 3 rules · 11.4M notices</summary>

- [x] `STM_001` a
- [X] `STM_002` b
- [ ] `STM_003` c
</details>

<details><summary><b>DQ</b> · 2 rules</summary>

- [ ] `DQ_001` a
- [ ] `DQ_002` b
</details>
"""


class IssueProgressTest(unittest.TestCase):
    def test_writes_overall_and_group_progress(self) -> None:
        out = update(BODY)
        self.assertIn("## Review progress\n\n<!-- progress -->**Progress: 2 / 5 rules (40.0%)**\n\n", out)
        self.assertIn("<b>STM</b> · 3 rules · 11.4M notices · 2/3 (67%)</summary>", out)
        self.assertIn("<b>DQ</b> · 2 rules · 0/2 (0%)</summary>", out)

    def test_is_idempotent(self) -> None:
        once = update(BODY)
        self.assertEqual(update(once), once)

    def test_follows_a_new_tick(self) -> None:
        once = update(BODY)
        ticked = once.replace("- [ ] `DQ_001`", "- [x] `DQ_001`")
        out = update(ticked)
        self.assertIn("**Progress: 3 / 5 rules (60.0%)**", out)
        self.assertIn("<b>DQ</b> · 2 rules · 1/2 (50%)</summary>", out)
        self.assertEqual(out.count("<!-- progress -->"), 1)

    def test_keeps_crlf_line_endings(self) -> None:
        out = update(BODY.replace("\n", "\r\n"))
        self.assertNotIn("\n", out.replace("\r\n", ""))
        self.assertIn("**Progress: 2 / 5 rules (40.0%)**", out)


if __name__ == "__main__":
    unittest.main()
