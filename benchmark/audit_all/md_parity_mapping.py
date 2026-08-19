"""Compatibility bridge for the canonical MobilityData parity mapping.

`aggregate.py` loads `spec-audit/md_parity_audit.py` by file path. That module
imports `md_parity_mapping` by module name, while Python starts the benchmark
with `benchmark/audit_all` on `sys.path`. Load the canonical sibling module here
so the audit uses the repository's single mapping implementation rather than a
duplicated copy.

🔴 This loads the canonical file AS A MODULE and does not `exec` it. The
difference is not stylistic. An `exec` leaves `__file__` pointing at THIS file,
and the canonical module anchors `fp_adjudication.tsv` on `Path(__file__).parent`
— so the ledger resolved to `benchmark/audit_all/`, where it does not exist, and
run 32290410755 republished 14,980 adjudicated rows as fresh divergences with no
error raised. `module_from_spec` sets `__file__` from the spec's origin, so every
path the canonical module derives from `__file__` — the ones written today and
the ones written later — resolves against `spec-audit/` where the data lives.
Patching `__file__` before an `exec` fixes the one path that exists now; this
fixes the class.
"""

import importlib.util
import sys
from pathlib import Path

_CANONICAL = Path(__file__).resolve().parents[2] / "spec-audit" / "md_parity_mapping.py"
_NAME = "_canonical_md_parity_mapping"

_spec = importlib.util.spec_from_file_location(_NAME, _CANONICAL)
if _spec is None or _spec.loader is None:  # pragma: no cover - defensive
    raise ImportError(f"Kanonik parite modülü yüklenemedi: {_CANONICAL}")
_module = importlib.util.module_from_spec(_spec)
sys.modules[_NAME] = _module
_spec.loader.exec_module(_module)

globals().update({k: v for k, v in vars(_module).items() if k not in
                  ("__name__", "__file__", "__spec__", "__loader__", "__package__", "__doc__")})
