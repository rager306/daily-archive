#!/usr/bin/env bash
# S05/T01 second bounded repair — frozen-revision verification runbook.
# Detached, sequential, no execution timeout. Full durable stdout/stderr and
# original exit codes are captured per step. fmt check is the LAST Rust action.
set -u
cd /root/daily-archive || exit 90
LOG=artifacts/parse_compat/logs
mkdir -p "$LOG"

SOURCE_FILES=(
  crates/da-parse/src/native_body_text.rs
  crates/da-parse/src/lib.rs
  crates/da-parse/tests/native_body_character_oracle.rs
  crates/da-parse/tests/architecture_contract.rs
  crates/da-cli/examples/native_body_character_probe.rs
  crates/da-cli/Cargo.toml
)
INPUT_FILES=(
  data/parse/layout_bakeoff.yaml
  crates/da-parse/tests/fixtures/preserved/provenance.yaml
  crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml
  crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml
  crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml
  data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf
  data/sources/arxiv/cs-ai/2512.24601/source/original.pdf
  data/sources/arxiv/cs-cv/2605.26525v1/source/2605.26525v1.pdf
  artifacts/parse_compat/native_body_character_shadow.json
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json
)

sha256sum "${SOURCE_FILES[@]}" > "$LOG/hashes.source.before.txt"
sha256sum "${INPUT_FILES[@]}" > "$LOG/hashes.input.before.txt"
sha256sum artifacts/parse_compat/native_body_character_scope_bound.json > "$LOG/hashes.artifact.before.txt"

run_step() {
  local name="$1"
  shift
  echo "[runbook] start $name"
  "$@" > "$LOG/$name.stdout" 2> "$LOG/$name.stderr"
  local rc=$?
  echo "$rc" > "$LOG/$name.rc"
  echo "[runbook] $name rc=$rc"
  return 0
}

run_step 01_oracle cargo test -p da-parse --test native_body_character_oracle
run_step 02_architecture_contract cargo test -p da-parse --test architecture_contract
run_step 03a_clippy_da_parse cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings
run_step 03b_clippy_da_cli cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings
run_step 03c_clippy_probe cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings
run_step 04_probe cargo run -p da-cli --features pdf-layout --example native_body_character_probe

sha256sum "${SOURCE_FILES[@]}" > "$LOG/hashes.source.after.txt"
sha256sum "${INPUT_FILES[@]}" > "$LOG/hashes.input.after.txt"
sha256sum artifacts/parse_compat/native_body_character_scope_bound.json > "$LOG/hashes.artifact.after.txt"

run_step 05_fmt_check cargo fmt -p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline -p da-domain -p da-ports -p da-application -p da-adapters -p da-parse -p da-cli -p da-mcp -- --check

echo "================ RUNBOOK SUMMARY ================"
for f in "$LOG"/*.rc; do
  printf 'rc %-26s %s\n' "$(basename "$f" .rc)" "$(cat "$f")"
done
echo "------------ stdout tails ------------"
for f in "$LOG"/*.stdout; do
  echo "== $(basename "$f") =="
  tail -4 "$f"
done
echo "------------ any stderr non-empty ------------"
for f in "$LOG"/*.stderr; do
  if [ -s "$f" ]; then
    echo "== $(basename "$f") ($(wc -c < "$f") bytes) =="
    tail -6 "$f"
  fi
done
echo "================ RUNBOOK DONE ================"
