#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../website"
deno check main.ts
