#!/usr/bin/env python3
"""Regression tests for the evidence staleness fingerprint gate."""

from __future__ import annotations

import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("badge_status_test", ROOT / "badge_status.py")
assert spec is not None and spec.loader is not None
badge_status = importlib.util.module_from_spec(spec)
spec.loader.exec_module(badge_status)


class EvidenceFingerprintTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.provisions = {
            row["id"]: row
            for row in json.loads((ROOT / "spec_provisions.json").read_text(encoding="utf-8"))["provisions"]
        }
        triage = (ROOT / "PROVISION_TRIAGE.md").read_text(encoding="utf-8")
        cls.verdict = badge_status.extract_verdicts(cls.provisions, triage)
        cls.evidence = badge_status.load_provision_evidence(ROOT / "provision_evidence.tsv")
        cls.classes = badge_status.extract_rule_classes(
            (ROOT.parent / "crates/rules/src/registry.rs").read_text(encoding="utf-8")
        )

    def test_current_reviewed_lock_is_fresh(self):
        stale = badge_status.check_evidence_fingerprints(
            {
                pid: status
                for pid, status in self.verdict.items()
                if self.provisions[pid]["strength"] == "strong"
            },
            self.evidence,
            self.classes,
            ROOT / "provision_evidence.lock",
        )
        self.assertEqual(stale, [])

    def test_rule_class_change_invalidates_fingerprint(self):
        active = {
            pid: status
            for pid, status in self.verdict.items()
            if self.provisions[pid]["strength"] == "strong"
            and status in ("KANITLI", "DOLAYLI")
        }
        pid = next(pid for pid in sorted(active) if self.evidence.get(pid))
        mutated_classes = dict(self.classes)
        rule_id = self.evidence[pid][0]
        mutated_classes[rule_id] = "Quality" if mutated_classes[rule_id] == "Spec" else "Spec"

        stale = badge_status.check_evidence_fingerprints(
            active,
            self.evidence,
            mutated_classes,
            ROOT / "provision_evidence.lock",
        )
        self.assertTrue(any(pid in message and "fingerprint" in message for message in stale))


if __name__ == "__main__":
    unittest.main()
