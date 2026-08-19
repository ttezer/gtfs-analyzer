"""Compatibility bridge for the canonical MobilityData parity mapping.

`aggregate.py` loads `spec-audit/md_parity_audit.py` by file path. That module
imports `md_parity_mapping` by module name, while Python starts the benchmark
with `benchmark/audit_all` on `sys.path`. Execute the canonical sibling module
here so the audit uses the repository's single mapping implementation rather
than a duplicated copy.

🔴 `__file__` MUST be repointed at the canonical file before the exec. The
canonical module anchors `fp_adjudication.tsv` on `Path(__file__).parent`, and
an `exec` inherits the *bridge's* `__file__`, not the compiled filename. Run
32290410755 aggregated with the ledger resolving to `benchmark/audit_all/`,
where no such file exists — `_fp_verdicts()` returns an empty dict when the
path is missing, so every adjudicated rule silently came back "No verdict
recorded" and 14,980 already-settled rows were republished as fresh
divergences. The failure produced no error, only wrong numbers.
"""

from pathlib import Path

_CANONICAL = Path(__file__).resolve().parents[2] / "spec-audit" / "md_parity_mapping.py"
__file__ = str(_CANONICAL)
exec(compile(_CANONICAL.read_bytes(), str(_CANONICAL), "exec"), globals(), globals())
