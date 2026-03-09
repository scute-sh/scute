#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(dirname "$0")"

echo "==> fmt (check)"
"$SCRIPT_DIR/fmt.sh" --check

echo "==> lint"
"$SCRIPT_DIR/lint.sh"

echo "==> test"
"$SCRIPT_DIR/test.sh"
