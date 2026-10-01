#!/usr/bin/env bash
# M514-1lss2s / S05 / T01 — BODY-IDENTITY-INTERSECTION capture harness.
#
# One invocation = one fresh run directory (never reused). The harness records,
# durably and per step, the command argv, elapsed time, original rc and the full
# stdout/stderr; it records the source/input sha256 set before and after the run
# and the regenerated report artifact sha256 before and after. Nothing is staged,
# committed, reset or stashed; no DB/WAL is touched; no Java is activated; no
# graph write, import or promotion happens; no threshold is accepted.
#
# Usage: bash capture.sh <run-dir>
set -u

OUT="${1:?usage: capture.sh <run-dir>}"
if [ -e "$OUT/RUN.identity.txt" ]; then
  echo "run dir already used (refusing to reuse): $OUT" >&2
  exit 98
fi
mkdir -p "$OUT"
cd /root/daily-archive || exit 99

SELF="$OUT/capture.sh"
cp -p "$0" "$SELF"

REPORT="artifacts/parse_compat/native_body_character_scope_bound.json"

INPUTS=(
  crates/da-parse/src/native_body_text.rs
  crates/da-parse/src/lib.rs
  crates/da-parse/tests/native_body_character_oracle.rs
  crates/da-parse/tests/native_body_uncertainty_split_contract.rs
  crates/da-parse/tests/architecture_contract.rs
  crates/da-cli/examples/native_body_character_probe.rs
  crates/da-cli/Cargo.toml
  data/parse/layout_bakeoff.yaml
  crates/da-parse/tests/fixtures/preserved/provenance.yaml
  crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml
  crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml
  crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml
  data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf
  data/sources/arxiv/cs-ai/2512.24601/source/original.pdf
  data/sources/arxiv/cs-cv/2605.26525v1/source/2605.26525v1.pdf
  "$REPORT"
  artifacts/parse_compat/native_body_character_scope_bound.accepted.v3.json
  artifacts/parse_compat/native_body_order_lineage.json
  artifacts/parse_compat/native_body_uncertainty_split.json
  artifacts/parse_compat/native_body_character_shadow.json
)

hash_inputs() {
  local out="$1" f
  : > "$out"
  for f in "${INPUTS[@]}"; do
    if [ -f "$f" ]; then
      sha256sum "$f" >> "$out"
    else
      echo "MISSING  $f" >> "$out"
    fi
  done
}

now() { date -u +%Y-%m-%dT%H:%M:%S.%NZ; }

{
  echo "argv: $0 $*"
  echo "cwd: $(pwd)"
  echo "git_head: $(git rev-parse HEAD)"
  echo "cargo: $(cargo --version)"
  echo "rustc: $(rustc --version)"
  echo "script_sha256_before: $(sha256sum "$SELF" | awk '{print $1}')"
  echo "start_utc: $(now)"
} > "$OUT/RUN.identity.txt"

hash_inputs "$OUT/inputs.before.sha256"
if [ -f "$REPORT" ]; then
  sha256sum "$REPORT" > "$OUT/report.before.sha256"
else
  echo "MISSING  $REPORT" > "$OUT/report.before.sha256"
fi
echo "$(now) start" > "$OUT/timing.log"

FAILED=0

run_step() {
  local name="$1" cmd="$2" start end rc
  printf '%s\n' "$cmd" > "$OUT/$name.argv"
  start=$(date +%s%N)
  bash -c "$cmd" > "$OUT/$name.stdout" 2> "$OUT/$name.stderr"
  rc=$?
  end=$(date +%s%N)
  echo "$rc" > "$OUT/$name.rc"
  {
    echo "command: $cmd"
    echo "elapsed_ms: $(( (end - start) / 1000000 ))"
    echo "rc: $rc"
    echo "stdout_bytes: $(wc -c < "$OUT/$name.stdout")"
    echo "stderr_bytes: $(wc -c < "$OUT/$name.stderr")"
    echo "started_utc: $(date -u -d "@$(( start / 1000000000 ))" +%Y-%m-%dT%H:%M:%SZ)"
    echo "ended_utc: $(date -u -d "@$(( end / 1000000000 ))" +%Y-%m-%dT%H:%M:%SZ)"
  } > "$OUT/$name.meta"
  echo "$(now) $name rc=$rc elapsed_ms=$(( (end - start) / 1000000 ))" >> "$OUT/timing.log"
  if [ "$rc" -ne 0 ]; then FAILED=1; fi
}

run_step 01_native_body_character_oracle \
  "cargo test -p da-parse --test native_body_character_oracle"
run_step 02_native_body_uncertainty_split_contract \
  "cargo test -p da-parse --test native_body_uncertainty_split_contract"
run_step 03_architecture_contract \
  "cargo test -p da-parse --test architecture_contract"
run_step 04_clippy_da_parse \
  "cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings"
run_step 05_clippy_da_cli \
  "cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings"
run_step 06_clippy_probe \
  "cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings"
run_step 07_native_body_character_probe \
  "cargo run -p da-cli --features pdf-layout --example native_body_character_probe"
# rustfmt is the LAST Rust action: any later source edit would re-introduce drift.
run_step 08_fmt_check \
  "cargo fmt --all -- --check"

if [ -f "$REPORT" ]; then
  sha256sum "$REPORT" > "$OUT/report.after.sha256"
else
  echo "MISSING  $REPORT" > "$OUT/report.after.sha256"
fi
hash_inputs "$OUT/inputs.after.sha256"

{
  echo "script_sha256_after: $(sha256sum "$SELF" | awk '{print $1}')"
  echo "end_utc: $(now)"
} >> "$OUT/RUN.identity.txt"

{
  echo "=== capture summary: $OUT ==="
  echo "source_inputs: $(cmp -s "$OUT/inputs.before.sha256" "$OUT/inputs.after.sha256" && echo SAME || echo CHANGED)"
  echo "report_before: $(awk '{print $1}' "$OUT/report.before.sha256")"
  echo "report_after:  $(awk '{print $1}' "$OUT/report.after.sha256")"
  echo "report_artifact: $(cmp -s "$OUT/report.before.sha256" "$OUT/report.after.sha256" && echo SAME || echo CHANGED)"
  echo "steps:"
  for rcfile in "$OUT"/*.rc; do
    name=$(basename "$rcfile" .rc)
    echo "  $name rc=$(cat "$rcfile")"
  done
} > "$OUT/SUMMARY.txt"

exit "$FAILED"
