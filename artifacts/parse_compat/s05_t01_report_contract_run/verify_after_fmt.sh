#!/usr/bin/env bash
# S05/T01 uncertainty-split REPORT-CONTRACT repair — post-format verification.
#
# WHY THIS EXISTS (the previous run's finding): the prior run recorded clippy
# BEFORE the formatting fix and never re-ran it afterwards, so its clippy result
# was not a statement about the frozen final sources. This script is the
# independent replay that closes that gap: it re-runs the WHOLE check set,
# including the report-contract clippy, against the already-formatted frozen
# sources, and leaves `cargo fmt --check` as the LAST Rust action.
#
# It never writes into the capture's step namespace: every record here is named
# `v<NN>_*` and the timeline is `steps.after_fmt.tsv`. Nothing in `capture.sh`'s
# output is overwritten or deleted.
#
# It first proves there was no formatting drift to fix at all (the capture's own
# `08_fmt_check` must be rc 0) and fails loudly if that is not the case, because
# then the ordering premise of this replay would be false.
set -u

cd /root/daily-archive || exit 90

RUN=artifacts/parse_compat/s05_t01_report_contract_run
ARTIFACT=artifacts/parse_compat/native_body_uncertainty_split.json
mkdir -p "$RUN"

{
  echo "phase=after_format_verification"
  echo "start_utc=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"
  echo "capture_fmt_check_rc=$(cat "$RUN/08_fmt_check.rc" 2>/dev/null || echo MISSING)"
  echo "capture_clippy_after_format_rc=$(cat "$RUN/05c_clippy_probe.rc" 2>/dev/null || echo MISSING)"
  echo "capture_artifact=$(cat "$RUN/hashes.artifact.after.txt" 2>/dev/null || echo MISSING)"
} > "$RUN/RUN.identity.after_fmt.txt"

# Premise check: the capture must have found NO formatting drift. If it did, this
# replay's ordering claim is wrong and the repair must not be reported as done.
if [ "$(cat "$RUN/08_fmt_check.rc" 2>/dev/null || echo MISSING)" != "0" ]; then
  echo "PREMISE FAILED: capture 08_fmt_check rc != 0; refusing to run the replay" >&2
  exit 92
fi

printf 'step\targv\tstart_utc\tstart_epoch_ns\tend_utc\tend_epoch_ns\telapsed_ms\texit_code\tstdout_bytes\tstderr_bytes\n' > "$RUN/steps.after_fmt.tsv"

run_step() {
  local name="$1"
  shift
  local stdout="$RUN/$name.stdout"
  local stderr="$RUN/$name.stderr"
  local start_ns start_utc end_ns end_utc elapsed_ms rc
  start_ns=$(date +%s%N)
  start_utc=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)
  printf '%s\n' "$*" > "$RUN/$name.argv"
  "$@" > "$stdout" 2> "$stderr"
  rc=$?
  end_ns=$(date +%s%N)
  end_utc=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)
  elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
  printf '%s\n' "$rc" > "$RUN/$name.rc"
  {
    echo "step=$name"
    echo "argv=$*"
    echo "start_utc=$start_utc"
    echo "start_epoch_ns=$start_ns"
    echo "end_utc=$end_utc"
    echo "end_epoch_ns=$end_ns"
    echo "elapsed_ms=$elapsed_ms"
    echo "exit_code=$rc"
    echo "stdout_path=$stdout"
    echo "stderr_path=$stderr"
    echo "stdout_bytes=$(wc -c < "$stdout")"
    echo "stderr_bytes=$(wc -c < "$stderr")"
  } > "$RUN/$name.meta"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$name" "$*" "$start_utc" "$start_ns" "$end_utc" "$end_ns" "$elapsed_ms" "$rc" \
    "$(wc -c < "$stdout")" "$(wc -c < "$stderr")" >> "$RUN/steps.after_fmt.tsv"
  echo "[after-fmt] $name rc=$rc elapsed_ms=$elapsed_ms"
  return 0
}

# Replay of the whole check set on the frozen, already-formatted sources.
run_step v01_uncertainty_split_contract cargo test -p da-parse --test native_body_uncertainty_split_contract
run_step v02_native_body_character_oracle cargo test -p da-parse --test native_body_character_oracle
run_step v03_architecture_contract cargo test -p da-parse --test architecture_contract
run_step v04_report_contract_example cargo test -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe
run_step v05a_clippy_da_parse cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings
run_step v05b_clippy_da_cli cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings
run_step v05c_clippy_probe cargo clippy -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe --no-deps -- -D warnings
run_step v05d_clippy_probe_tests cargo clippy -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe --tests --no-deps -- -D warnings
run_step v06_probe cargo run -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe
sha256sum "$ARTIFACT" > "$RUN/hashes.artifact.after_fmt.txt"
{
  if diff -q "$RUN/hashes.artifact.after.txt" "$RUN/hashes.artifact.after_fmt.txt" > /dev/null 2>&1; then
    echo "SAME artifact-after-fmt (capture == after-fmt replay)"
  else
    echo "CHANGED artifact-after-fmt (capture != after-fmt replay)"
  fi
  cat "$RUN/hashes.artifact.after_fmt.txt"
} > "$RUN/hash_compare.after_fmt.txt"

# LAST Rust action.
run_step v07_fmt_check cargo fmt -p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline -p da-domain -p da-ports -p da-application -p da-adapters -p da-parse -p da-cli -p da-mcp -- --check

{
  echo "end_utc=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"
} >> "$RUN/RUN.identity.after_fmt.txt"

echo "================ AFTER-FMT SUMMARY ================"
cat "$RUN/steps.after_fmt.tsv"
echo "------------ artifact replay ------------"
cat "$RUN/hash_compare.after_fmt.txt"
echo "================ AFTER-FMT DONE ================"
