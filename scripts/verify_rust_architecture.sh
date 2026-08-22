#!/usr/bin/env bash
# Local hexagonal dependency-direction check for daily-archive v2.
# Mirrors the CI job in .github/workflows/architecture-guardrail.yml.
# Exit 0 = pass, 1 = fail.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# kg-* crates are universal (ADR-050). They MUST NOT depend on da-* crates.
KG_CRATES="kg-ontology kg-storage kg-embeddings kg-algorithms kg-pipeline"

# Packages included in fmt/clippy/test guardrails.
KG_FMT_PKGS="-p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline"
DA_FMT_PKGS="-p da-domain -p da-ports -p da-application -p da-adapters -p da-cli -p da-mcp"

echo "=== cargo fmt --check (our crates only, kg-* + da-*) ==="
cargo fmt $KG_FMT_PKGS $DA_FMT_PKGS -- --check

# NOTE: no `cargo check [full-workspace]` here — it triggers librocksdb-sys
# rebuild (5-10 min, MEM468). Per-crate clippy below already compiles
# all our crates incrementally. Full workspace check is CI-only.

echo "=== cargo clippy (our crates incl kg-*, no-deps, deny warnings) ==="
cargo clippy \
  $KG_FMT_PKGS \
  -p da-domain -p da-ports -p da-application \
  -p da-adapters -p da-cli -p da-mcp \
  --all-targets --no-deps -- -D warnings

# M441 lesson (M442 pin): documented CLI feature combinations must compile.
# pdf-classifier stayed broken for a month because the gate never built it.
# Keep every documented feature compiling; add new features here.
echo "=== cargo check feature matrix (da-cli pdf-classifier) ==="
cargo check -p da-cli --features pdf-classifier --examples

# Helper: grep inside [dependencies] only (prod deps). Use for hexagonal
# checks where dev-dependencies are allowed to violate the rule (tests
# legitimately import adapters for wiring).
grep_deps_prod() {
  local crate_toml="$1"
  shift
  awk '
    /^\[dependencies\]$/ { in_deps=1; next }
    /^\[/ && $0 != "[dependencies]" { in_deps=0 }
    in_deps { print }
  ' "$crate_toml" | grep -E "$*"
}

# Helper: grep inside [dependencies] AND [dev-dependencies]. Use for
# universal-crate invariants (kg-* must have zero da-* deps, even in tests).
grep_deps() {
  local crate_toml="$1"
  shift
  awk '
    /^\[dependencies\]$/ { in_deps=1; next }
    /^\[dev-dependencies\]$/ { in_deps=1; next }
    /^\[/ && $0 != "[dependencies]" && $0 != "[dev-dependencies]" { in_deps=0 }
    in_deps { print }
  ' "$crate_toml" | grep -E "$*"
}

echo "=== hexagonal dependency direction ==="
fail=0

echo "  -- ADR-050 invariant: kg-* must not depend on da-* (universal crates) --"
for kg in $KG_CRATES; do
  toml="crates/$kg/Cargo.toml"
  if [ ! -f "$toml" ]; then
    echo "ERROR: $toml missing"
    fail=1
    continue
  fi
  if grep_deps "$toml" 'da-domain|da-ports|da-application|da-adapters|da-cli'; then
    echo "ERROR: $kg must not depend on da-* crates (ADR-050 universal invariant)"
    grep_deps "$toml" 'da-domain|da-ports|da-application|da-adapters|da-cli'
    fail=1
  else
    echo "  $kg: OK (no da-* deps)"
  fi
done

if grep_deps_prod crates/da-domain/Cargo.toml 'reqwest|tokio|samyama|ruvector|hyper|axum|actix'; then
  echo "ERROR: da-domain must not depend on infrastructure crates"
  fail=1
else
  echo "  da-domain: OK (no infra deps)"
fi

if grep_deps_prod crates/da-ports/Cargo.toml 'reqwest|tokio|samyama|ruvector|hyper|axum|actix'; then
  echo "ERROR: da-ports must not depend on infrastructure crates"
  fail=1
else
  echo "  da-ports: OK (no infra deps)"
fi

if grep_deps_prod crates/da-application/Cargo.toml 'da-adapters'; then
  echo "ERROR: da-application must not depend on da-adapters (hexagonal violation)"
  fail=1
else
  echo "  da-application: OK (no da-adapters in prod deps; dev-deps allowed for tests)"
fi

echo "=== unit tests (per-package, avoids rocksdb test-profile rebuild hang) ==="
cargo test -p kg-ontology --lib
cargo test -p kg-storage --lib
cargo test -p kg-embeddings --lib
cargo test -p kg-algorithms --lib
cargo test -p kg-pipeline --lib
cargo test -p da-domain --lib
cargo test -p da-application --tests
# M432: integration suites — acquisition contracts + pipeline cache
# (hub_ocr_probe compiles empty without the pdf-classifier feature).
cargo test -p da-adapters --tests
# M432: da-cli integration tests incl. the cross-process ADR-061
# contract (remote_read_plane_test) and load_journey suite.
cargo test -p da-cli --tests
# M432: da-mcp real-engine tool tests (M425-M428 sweep).
cargo test -p da-mcp --lib
# ADR-051 P1: TemporalEdge property suite (M353)
cargo test -p kg-ontology --lib proptest
# ADR-051 P1: EntityType / RelationType wire stability (M354)
cargo test -p da-domain --lib proptest

# M352/M353: BatchIngestResult must keep pdf_triage (batch triage shape lock)
if ! grep -q 'pdf_triage' crates/da-application/src/batch_ingest.rs; then
  echo "ERROR: BatchIngestResult.pdf_triage missing from batch_ingest.rs (M352 shape lock)"
  fail=1
else
  echo "  batch_ingest.pdf_triage: OK"
fi

if [ "$fail" -ne 0 ]; then
  echo "FAIL: architecture guardrail failed"
  exit 1
fi
echo "PASS: architecture guardrail"
