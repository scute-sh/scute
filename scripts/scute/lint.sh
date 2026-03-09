#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../crates"
cargo clippy --all-targets "$@"
