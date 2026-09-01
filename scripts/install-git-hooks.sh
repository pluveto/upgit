#!/bin/sh
# Point this clone at the tracked hooks in .githooks/
set -eu
cd "$(git rev-parse --show-toplevel)"
git config core.hooksPath .githooks
echo "core.hooksPath=$(git config --get core.hooksPath)"
echo "pre-commit and pre-push run cargo fmt --check and cargo clippy (-D warnings)."
