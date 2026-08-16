"""Compatibility bridge for the canonical MobilityData parity mapping.

`aggregate.py` loads `spec-audit/md_parity_audit.py` by file path. That module
imports `md_parity_mapping` by module name, while Python starts the benchmark
with `benchmark/audit_all` on `sys.path`. Execute the canonical sibling module
here so the audit uses the repository's single mapping implementation rather
than a duplicated copy.
"""

from pathlib import Path

_CANONICAL = Path(__file__).resolve().parents[2] / "spec-audit" / "md_parity_mapping.py"
exec(compile(_CANONICAL.read_bytes(), str(_CANONICAL), "exec"), globals(), globals())
