#!/usr/bin/env bash
# Run all tests: Rust unit tests, WASM smoke tests, Python smoke tests
set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== Rust tests ==="
cargo test -p patchlang

echo ""
echo "=== WASM tests ==="
node tests/test_wasm.mjs

echo ""
echo "=== Python tests ==="
# A missing .venv used to `exit 0` here, so "All tests passed!" was printed
# without the Python suite ever running — a skip that reported success is how
# the gap in #36 stayed invisible. Exit non-zero instead.
# Test for the file rather than relying on `source` to fail: `.` is a special
# builtin, so a non-interactive bash exits the moment the file is missing —
# before any `||` or `if !` guard can run, killing the script with no message.
if [ ! -f .venv/bin/activate ]; then
  echo "FAIL: no .venv — run scripts/build-python.sh first." >&2
  echo "      Skipping Python tests is not success; re-run once the venv exists." >&2
  exit 1
fi
source .venv/bin/activate
python tests/test_python.py

echo ""
echo "All tests passed!"
