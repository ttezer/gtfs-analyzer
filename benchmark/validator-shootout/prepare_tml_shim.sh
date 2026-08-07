#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

TML_BIN="$(node --input-type=module <<'NODE'
import { getValidatorInfo } from '@tmlmobilidade/gtfs-validator';
const info = await getValidatorInfo();
if (!info.isAvailable) process.exit(2);
console.log(info.binaryPath);
NODE
)"

if [[ -x "${TML_BIN}.real" ]]; then
  echo "TML shim already prepared: $TML_BIN"
  exit 0
fi

mv "$TML_BIN" "${TML_BIN}.real"
cat > "$TML_BIN" <<'SHIM'
#!/usr/bin/env bash
set -u
SELF="$(readlink -f "$0")"
REAL="${SELF}.real"
"$REAL" "$@"
status=$?

# On some process-level panics the published SDK's binary creates a zero-byte
# out_file before terminating. Preserve that fact as *.empty, but remove the
# original pathname so the benchmark does not mistake an empty file for JSON.
if [[ -n "${GITHUB_WORKSPACE:-}" ]]; then
  result_dir="$GITHUB_WORKSPACE/benchmark/validator-shootout/results/tml"
  if [[ -d "$result_dir" ]]; then
    while IFS= read -r -d '' f; do
      mv "$f" "${f}.empty"
    done < <(find "$result_dir" -maxdepth 1 -type f -name '*.report.json' -size 0 -print0 2>/dev/null)
  fi
fi
exit "$status"
SHIM
chmod +x "$TML_BIN"
echo "Wrapped TML binary: $TML_BIN"
