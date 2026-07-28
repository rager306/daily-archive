#!/usr/bin/env bash
# Local hexagonal dependency-direction check for daily-archive v2.
# Mirrors the CI job in .github/workflows/architecture-guardrail.yml.
# Exit 0 = pass, 1 = fail.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "=== cargo fmt --check (our crates only) ==="
cargo fmt -p da-domain -p da-ports -p da-application -p da-graph -p da-adapters -p da-cli -- --check

echo "=== cargo check --workspace ==="
cargo check --workspace

echo "=== cargo clippy (our crates, no-deps, deny warnings) ==="
cargo clippy \
  -p da-domain -p da-ports -p da-application \
  -p da-graph -p da-adapters -p da-cli \
  --all-targets --no-deps -- -D warnings

# Helper: grep only inside [dependencies], not [dev-dependencies].
grep_deps() {
  local crate_toml="$1"
  shift
  awk '
    /^\[dependencies\]$/ { in_deps=1; next }
    /^\[/ && $0 != "[dependencies]" { in_deps=0 }
    in_deps { print }
  ' "$crate_toml" | grep -E "$*"
}

echo "=== hexagonal dependency direction ==="
fail=0

if grep_deps crates/da-domain/Cargo.toml 'reqwest|tokio|samyama|ruvector|hyper|axum|actix'; then
  echo "ERROR: da-domain must not depend on infrastructure crates"
  fail=1
else
  echo "  da-domain: OK (no infra deps)"
fi

if grep_deps crates/da-ports/Cargo.toml 'reqwest|tokio|samyama|ruvector|hyper|axum|actix'; then
  echo "ERROR: da-ports must not depend on infrastructure crates"
  fail=1
else
  echo "  da-ports: OK (no infra deps)"
fi

if grep_deps crates/da-application/Cargo.toml 'da-adapters'; then
  echo "ERROR: da-application must not depend on da-adapters (hexagonal violation)"
  fail=1
else
  echo "  da-application: OK (no da-adapters in dependencies)"
fi

if grep_deps crates/da-graph/Cargo.toml 'da-adapters'; then
  echo "ERROR: da-graph must not depend on da-adapters (hexagonal violation)"
  fail=1
else
  echo "  da-graph: OK (no da-adapters in dependencies)"
fi

echo "=== unit tests (per-package, avoids rocksdb test-profile rebuild hang) ==="
cargo test -p da-domain --lib
cargo test -p da-graph --lib
cargo test -p da-application --tests
cargo test -p da-adapters --lib

if [ "$fail" -ne 0 ]; then
  echo "FAIL: architecture guardrail failed"
  exit 1
fi
echo "PASS: architecture guardrail"
