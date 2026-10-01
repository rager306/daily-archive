#!/usr/bin/env bash
# M514-1lss2s / S05 / T01 — additive native Body extraction/assignment repair.
#
# Sequential producing run, no execution timeout, no mid-run script change:
# every step's full stdout/stderr, original exit code, argv, start/end UTC and
# epoch-ns are written to this directory, and the source/input bytes are hashed
# before and after the run. `cargo fmt --all -- --check` is the LAST Rust action.
#
# This script is written before the run and hashed at run time; it must not be
# edited while the run is in flight.
set -u
RUN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$RUN_DIR/../../../.." && pwd)"
cd "$REPO" || exit 2

SELF_SHA="$(sha256sum "$RUN_DIR/capture.sh" | awk '{print $1}')"

SOURCES=(
  crates/da-parse/src/native_body_text.rs
  crates/da-parse/src/lib.rs
  crates/da-parse/tests/native_body_character_oracle.rs
  crates/da-parse/tests/native_body_uncertainty_split_contract.rs
  crates/da-parse/tests/architecture_contract.rs
  crates/da-cli/examples/native_body_character_probe.rs
  crates/da-cli/Cargo.toml
)

INPUTS=(
  data/parse/layout_bakeoff.yaml
  crates/da-parse/tests/fixtures/preserved/provenance.yaml
  crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml
  crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml
  crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml
  data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf
  data/sources/arxiv/cs-ai/2512.24601/source/original.pdf
  data/sources/arxiv/cs-cv/2605.26525v1/source/2605.26525v1.pdf
)

ARTIFACT="artifacts/parse_compat/native_body_character_scope_bound.json"
ACCEPTED="artifacts/parse_compat/native_body_character_scope_bound.accepted.v3.json"

hash_set() {
  local out="$1"; shift
  : > "$out"
  local path
  for path in "$@"; do
    if [ -f "$path" ]; then
      printf '%s  %s\n' "$(sha256sum "$path" | awk '{print $1}')" "$path" >> "$out"
    else
      printf '%s  %s\n' "MISSING" "$path" >> "$out"
    fi
  done
}

hash_set "$RUN_DIR/hashes.source.before.txt" "${SOURCES[@]}"
hash_set "$RUN_DIR/hashes.input.before.txt" "${INPUTS[@]}"
hash_set "$RUN_DIR/hashes.artifact.before.txt" "$ARTIFACT" "$ACCEPTED"

printf 'step\tcommand\tstart_utc\tend_utc\tstart_epoch_ns\tend_epoch_ns\telapsed_ms\trc\tstdout_bytes\tstderr_bytes\n' > "$RUN_DIR/steps.tsv"

STEP=0
run_step() {
  local name="$1"; shift
  STEP=$((STEP + 1))
  local prefix
  prefix="$(printf '%02d' "$STEP")_${name}"

  printf '%s\n' "$@" > "$RUN_DIR/${prefix}.argv"
  local start_utc end_utc start_ns end_ns rc elapsed out err
  start_utc="$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"
  start_ns="$(date +%s%N)"
  "$@" > "$RUN_DIR/${prefix}.stdout" 2> "$RUN_DIR/${prefix}.stderr"
  rc=$?
  end_ns="$(date +%s%N)"
  end_utc="$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"
  elapsed=$(( (end_ns - start_ns) / 1000000 ))
  out="$(wc -c < "$RUN_DIR/${prefix}.stdout" | tr -d ' ')"
  err="$(wc -c < "$RUN_DIR/${prefix}.stderr" | tr -d ' ')"
  printf '%s\n' "$rc" > "$RUN_DIR/${prefix}.rc"
  {
    printf 'argv: %s\n' "$*"
    printf 'start_utc: %s\n' "$start_utc"
    printf 'end_utc: %s\n' "$end_utc"
    printf 'start_epoch_ns: %s\n' "$start_ns"
    printf 'end_epoch_ns: %s\n' "$end_ns"
    printf 'elapsed_ms: %s\n' "$elapsed"
    printf 'rc: %s\n' "$rc"
    printf 'stdout_bytes: %s\n' "$out"
    printf 'stderr_bytes: %s\n' "$err"
  } > "$RUN_DIR/${prefix}.meta"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$prefix" "$*" "$start_utc" "$end_utc" "$start_ns" "$end_ns" "$elapsed" "$rc" "$out" "$err" \
    >> "$RUN_DIR/steps.tsv"
  printf 'step %s rc=%s elapsed_ms=%s\n' "$prefix" "$rc" "$elapsed" >&2
  return 0
}

RUN_START_UTC="$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"
GIT_HEAD="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
CARGO_V="$(cargo --version 2>/dev/null || echo unknown)"
RUSTC_V="$(rustc --version 2>/dev/null || echo unknown)"
{
  printf 'script: %s\n' "$RUN_DIR/capture.sh"
  printf 'script_sha256: %s\n' "$SELF_SHA"
  printf 'repo: %s\n' "$REPO"
  printf 'git_head: %s\n' "$GIT_HEAD"
  printf 'cargo: %s\n' "$CARGO_V"
  printf 'rustc: %s\n' "$RUSTC_V"
  printf 'run_start_utc: %s\n' "$RUN_START_UTC"
} > "$RUN_DIR/RUN.identity.txt"

run_step native_body_character_oracle cargo test -p da-parse --test native_body_character_oracle
run_step native_body_uncertainty_split_contract cargo test -p da-parse --test native_body_uncertainty_split_contract
run_step architecture_contract cargo test -p da-parse --test architecture_contract
run_step clippy_da_parse cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings
run_step clippy_da_cli cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings
run_step clippy_probe cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings
run_step probe cargo run -p da-cli --features pdf-layout --example native_body_character_probe
run_step fmt_check cargo fmt --all -- --check

RUN_END_UTC="$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"

hash_set "$RUN_DIR/hashes.source.after.txt" "${SOURCES[@]}"
hash_set "$RUN_DIR/hashes.input.after.txt" "${INPUTS[@]}"
hash_set "$RUN_DIR/hashes.artifact.after.txt" "$ARTIFACT" "$ACCEPTED"

compare_set() {
  local label="$1" before="$2" after="$3"
  if diff -q "$before" "$after" > /dev/null; then
    printf 'SAME %s\n' "$label"
  else
    printf 'CHANGED %s\n' "$label"
    diff "$before" "$after" || true
  fi
}
{
  compare_set source "$RUN_DIR/hashes.source.before.txt" "$RUN_DIR/hashes.source.after.txt"
  compare_set input "$RUN_DIR/hashes.input.before.txt" "$RUN_DIR/hashes.input.after.txt"
  compare_set artifact "$RUN_DIR/hashes.artifact.before.txt" "$RUN_DIR/hashes.artifact.after.txt"
} > "$RUN_DIR/hash_compare.txt"

{
  printf 'run_end_utc: %s\n' "$RUN_END_UTC"
  printf 'script_sha256_after: %s\n' "$(sha256sum "$RUN_DIR/capture.sh" | awk '{print $1}')"
} >> "$RUN_DIR/RUN.identity.txt"

printf 'capture run complete: %s\n' "$RUN_DIR"
