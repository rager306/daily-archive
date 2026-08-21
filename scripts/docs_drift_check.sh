#!/usr/bin/env bash
# Verify that the CLI-rendered doc tables are present in GRAPH-SCHEMA.md.
# This is a drift detector, not a regenerator: if `da schema-list`,
# `da edge-contracts`, `da cross-refs`, `da audit-fields`, or
# `da schema-check` produce new content, the docs must be updated in the
# same commit. audit-fields/schema-check are the ADR-045 Wave E schema
# drift gate — M438 extended coverage to them (README documents both;
# GRAPH-SCHEMA.md carries the canonical gate section).
#
# Non-mutating, safe to run from pre-commit. Always exits 0 unless the
# da CLI binary is missing (warns and skips).
#
# See ADR-045 §Wave E/G/F and MEM503 #c.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

CLI=./target/debug/da
if [ ! -x "$CLI" ]; then
  cargo build -p da-cli --quiet 2>/dev/null || {
    echo "docs-drift-check: da CLI not built, skipping"
    exit 0
  }
fi

DOCS=doc/GRAPH-SCHEMA.md
[ -f "$DOCS" ] || { echo "docs-drift-check: $DOCS missing"; exit 0; }

STATUS=0
for cmd in schema-list edge-contracts cross-refs audit-fields schema-check; do
  if ! grep -q "$cmd" "$DOCS" README.md 2>/dev/null; then
    echo "docs-drift-check: $cmd output missing from $DOCS or README.md"
    STATUS=1
  fi
done

if [ "$STATUS" -ne 0 ]; then
  echo "docs-drift-check: run the CLI commands and paste their output into the docs"
fi
exit 0  # advisory only — do not block the commit
