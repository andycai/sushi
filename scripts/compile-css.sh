#!/usr/bin/env bash
set -euo pipefail

if ! command -v pnpm >/dev/null 2>&1; then
  echo "pnpm is required. Install pnpm and run: pnpm install" >&2
  exit 1
fi

pnpm exec tailwindcss -i ./web/static/css/input.css -o ./web/static/css/style.css --minify
