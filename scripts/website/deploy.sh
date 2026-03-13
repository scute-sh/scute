#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $(basename "$0") [--prod]"
  exit 1
}

prod_flag=""
for arg in "$@"; do
  case "$arg" in
    --prod) prod_flag="--prod" ;;
    --help|-h) usage ;;
    *) echo "Unknown option: $arg"; usage ;;
  esac
done

cd "$(dirname "$0")/../../website"

# shellcheck disable=SC2086
deployctl deploy \
  --org=scute \
  --project=scute-website \
  $prod_flag \
  main.ts
