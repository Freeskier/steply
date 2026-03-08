#!/usr/bin/env bash
# Usage:
#   1. From this repo, without installing Steply system-wide:
#      bash tools/examples/flow-builder.sh
#   2. After installing the binary:
#      cargo install --path crates/steply-cli
#      bash tools/examples/flow-builder.sh
#
# If `steply` is not available in PATH, this script falls back to
# `cargo run -q -p steply-cli --` from the workspace root.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"

if command -v steply >/dev/null 2>&1; then
  STEPLY_CMD=(steply)
  STEPLY_CWD="$PWD"
else
  STEPLY_CMD=(cargo run -q -p steply-cli --)
  STEPLY_CWD="$REPO_ROOT"
fi

steply_cmd() {
  (
    cd "$STEPLY_CWD"
    "${STEPLY_CMD[@]}" "$@"
  )
}

cleanup() {
  if [[ -n "${FLOW_ID:-}" ]]; then
    steply_cmd flow drop "$FLOW_ID" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT

FLOW_ID="$(steply_cmd flow create --decorate)"

steply_cmd flow step "$FLOW_ID" --title "Basic user data" --id basic
steply_cmd text-input \
  --flow "$FLOW_ID" \
  --target user.name \
  --label "Name" \
  --placeholder "Ada Lovelace"

steply_cmd text-input \
  --flow "$FLOW_ID" \
  --target user.email \
  --label "Email" \
  --placeholder "ada@example.com"

steply_cmd select \
  --flow "$FLOW_ID" \
  --target user.role \
  --label "Role" \
  --options developer \
  --options designer \
  --options founder

steply_cmd flow step "$FLOW_ID" --title "Project setup"
steply_cmd file-browser \
  --flow "$FLOW_ID" \
  --target project.directory \
  --label "Project directory" \
  --browser-mode tree \
  --cwd "$PWD"

steply_cmd choice-input \
  --flow "$FLOW_ID" \
  --target project.package_manager \
  --label "Package manager" \
  --options cargo \
  --options npm \
  --options bun

EXPORT_PATH="$REPO_ROOT/tools/examples/generated-flow.yaml"
steply_cmd flow export "$FLOW_ID" --out "$EXPORT_PATH"
printf 'Exported flow YAML to %s\n' "$EXPORT_PATH"

steply_cmd flow run "$FLOW_ID"
