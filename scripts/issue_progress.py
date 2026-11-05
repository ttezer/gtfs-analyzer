#!/usr/bin/env python3
"""Write the review progress of the Planner Beta issue (#199) into the issue body.

The issue lists every rule as a task (`- [x]` / `- [ ]`) inside one `<details>` block per
rule group. GitHub counts the tasks on its own ("33 of 619 tasks") but shows no percentage
and nothing per group. This script adds both:

* one line under "## Review progress":  **Progress: 33 / 619 rules (5.3%)**
* each group summary gains  ` · 26/55 (47%)`

It is idempotent: the lines it writes are recognised and replaced, so running it on its own
output changes nothing. The workflow `.github/workflows/beta-progress.yml` runs it whenever
the issue is edited (ticking a box is an edit).

Usage:
    python3 scripts/issue_progress.py < body.md > new_body.md
"""

from __future__ import annotations

import re
import sys

TASK = re.compile(r"^\s*- \[([ xX])\] ", re.MULTILINE)
PROGRESS_LINE = re.compile(r"^<!-- progress -->.*$\n?", re.MULTILINE)
HEADING = "## Review progress"
# Our suffix at the end of a group summary's text, e.g. " · 26/55 (47%)".
GROUP_SUFFIX = re.compile(r" · \d+/\d+ \(\d+%\)$")
DETAILS = re.compile(r"(<details><summary>)(.*?)(</summary>)(.*?)(</details>)", re.DOTALL)


def _counts(text: str) -> tuple[int, int]:
    marks = TASK.findall(text)
    return sum(1 for m in marks if m in "xX"), len(marks)


def _percent(done: int, total: int, digits: int) -> str:
    value = 100 * done / total if total else 0.0
    return f"{value:.{digits}f}%"


def update(body: str) -> str:
    """Return `body` with the overall progress line and per-group counts refreshed."""
    # Line endings as GitHub stores them may be CRLF; work on LF and restore at the end.
    crlf = "\r\n" in body
    text = body.replace("\r\n", "\n")

    def group(match: re.Match[str]) -> str:
        summary = GROUP_SUFFIX.sub("", match.group(2))
        done, total = _counts(match.group(4))
        if total:
            summary += f" · {done}/{total} ({_percent(done, total, 0)})"
        return match.group(1) + summary + match.group(3) + match.group(4) + match.group(5)

    text = DETAILS.sub(group, text)
    text = PROGRESS_LINE.sub("", text)
    done, total = _counts(text)
    line = f"<!-- progress -->**Progress: {done} / {total} rules ({_percent(done, total, 1)})**\n"
    if HEADING in text:
        head_end = text.index(HEADING) + len(HEADING)
        # Keep exactly one blank line between the heading and the progress line.
        rest = text[head_end:].lstrip("\n")
        text = text[:head_end] + "\n\n" + line + "\n" + rest
    return text.replace("\n", "\r\n") if crlf else text


def main() -> int:
    sys.stdout.write(update(sys.stdin.read()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
