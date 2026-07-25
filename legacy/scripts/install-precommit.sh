#!/usr/bin/env bash
# install-precommit.sh — one-step installer for pre-commit hooks.
#
# What it does:
#   1. Verifies uv is installed (project uses uv for Python deps).
#   2. Installs pre-commit into the active uv environment if absent.
#   3. Installs git hooks via `pre-commit install`.
#
# Idempotent: safe to re-run.
# Flags:
#   --check-only  print status without mutating; exits 0 if hooks installed.
#   --uninstall   run `pre-commit uninstall` and exit.
#
# Standard usage:
#   bash scripts/install-precommit.sh
#   bash scripts/install-precommit.sh --check-only
#   bash scripts/install-precommit.sh --uninstall

set -euo pipefail

CHECK_ONLY=0
UNINSTALL=0
for arg in "$@"; do
  case "$arg" in
    --check-only) CHECK_ONLY=1 ;;
    --uninstall)  UNINSTALL=1 ;;
    -h|--help)
      sed -n '2,17p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

# Resolve project root (parent of scripts/).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

# Step 0: --uninstall fast path.
if [ "$UNINSTALL" = "1" ]; then
  if command -v pre-commit >/dev/null 2>&1; then
    pre-commit uninstall
    echo "pre-commit hooks uninstalled."
  else
    echo "pre-commit not installed; nothing to uninstall."
  fi
  exit 0
fi

# Step 1: ensure uv exists.
if ! command -v uv >/dev/null 2>&1; then
  echo "Error: 'uv' is required but not found in PATH." >&2
  echo "Install uv: https://docs.astral.sh/uv/getting-started/installation/" >&2
  exit 1
fi

# Step 2: ensure pre-commit is installed (via uv tool or system).
if ! command -v pre-commit >/dev/null 2>&1; then
  if [ "$CHECK_ONLY" = "1" ]; then
    echo "pre-commit: not installed (--check-only mode, no install)" >&2
    exit 1
  fi
  echo "Installing pre-commit via uv tool install ..."
  uv tool install pre-commit
  # Re-check.
  if ! command -v pre-commit >/dev/null 2>&1; then
    echo "Error: pre-commit install succeeded but binary not in PATH." >&2
    echo "Try: uv tool run pre-commit --version" >&2
    exit 1
  fi
fi

# Step 3: verify .pre-commit-config.yaml exists.
if [ ! -f "$ROOT_DIR/.pre-commit-config.yaml" ]; then
  echo "Error: .pre-commit-config.yaml not found at $ROOT_DIR" >&2
  exit 1
fi

if [ "$CHECK_ONLY" = "1" ]; then
  # In check-only mode, just report status.
  if [ -f "$ROOT_DIR/.git/hooks/pre-commit" ]; then
    echo "pre-commit: installed and configured."
    exit 0
  else
    echo "pre-commit: configured but not installed (run without --check-only)." >&2
    exit 1
  fi
fi

# Step 4: install git hooks.
pre-commit install

echo "pre-commit installed. Hooks will run on every commit."
echo "Run manually: pre-commit run --all-files"
echo "Bypass in emergency: git commit --no-verify"
