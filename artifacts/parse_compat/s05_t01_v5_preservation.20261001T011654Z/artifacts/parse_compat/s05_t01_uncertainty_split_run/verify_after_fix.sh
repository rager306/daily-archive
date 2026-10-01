#!/usr/bin/env bash
# S05/T01 uncertainty-split diagnostic — post-format-fix verification.
#
# WHY THIS EXISTS: the frozen capture recorded `07_fmt_check` = rc 1 (rustfmt
# drift in three files this task added/edited). That failure is preserved as-is
# in `07_fmt_check.{argv,meta,rc,stdout,stderr}` and must never be overwritten or
# deleted. This script does not touch it: it applies the formatting fix, then
# records fresh, differently-named steps that prove (a) the check is now clean,
# (b) the fix is semantically neutral for the measurement (the artifact bytes are
# unchanged versus the frozen replay), and (c) rustfmt --check is the LAST Rust
# action.
set -u

cd /root/daily-archive || exit 90
RUN=artifacts/parse_compat/s05_t01_uncertainty_split_run
ARTIFACT=artifacts/parse_compat/native_body_uncertainty_split.json
mkdir -p "$RUN"

printf 'step\targv\tstart_utc\tstart_epoch_ns\tend_utc\tend_epoch_ns\telapsed_ms\texit_code\tstdout_bytes\tstderr_bytes\n' > "$RUN/steps.after_fix.tsv"

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
    "$(wc -c < "$stdout")" "$(wc -c < "$stderr")" >> "$RUN/steps.after_fix.tsv"
  echo "[after-fix] $name rc=$rc elapsed_ms=$elapsed_ms"
  return 0
}

START_NS=$(date +%s%N)
{
  echo "phase=after_format_fix"
  echo "start_utc=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"
  echo "start_epoch_ns=$START_NS"
  echo "frozen_failure_preserved=$RUN/07_fmt_check.rc -> $(cat "$RUN/07_fmt_check.rc" 2>/dev/null)"
  echo "frozen_artifact_probe1=$(cat "$RUN/hashes.artifact.probe1.txt" 2>/dev/null)"
  echo "frozen_artifact_probe2=$(cat "$RUN/hashes.artifact.probe2.txt" 2>/dev/null)"
} > "$RUN/RUN.identity.after_fix.txt"

FMT_ARGS=(-p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline
          -p da-domain -p da-ports -p da-application -p da-adapters -p da-parse -p da-cli
          -p da-mcp -- --check)

run_step 08_fmt_check_after_fix cargo fmt "${FMT_ARGS[@]}"
run_step 09_uncertainty_split_contract_after_fix cargo test -p da-parse --test native_body_uncertainty_split_contract
run_step 10_native_body_character_oracle_after_fix cargo test -p da-parse --test native_body_character_oracle
run_step 11_architecture_contract_after_fix cargo test -p da-parse --test architecture_contract
run_step 12_probe_after_fmt_fix cargo run -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe
sha256sum "$ARTIFACT" > "$RUN/hashes.artifact.after_fmt_fix.txt"

if diff -q "$RUN/hashes.artifact.after_fmt_fix.txt" "$RUN/hashes.artifact.probe2.txt" > /dev/null 2>&1; then
  echo "SAME  artifact-after-fmt-fix (formatting did not change the measured bytes)" \
    > "$RUN/hash_compare.after_fix.txt"
else
  {
    echo "CHANGED  artifact-after-fmt-fix"
    echo "before=$(cat "$RUN/hashes.artifact.probe2.txt")"
    echo "after=$(cat "$RUN/hashes.artifact.after_fmt_fix.txt")"
  } > "$RUN/hash_compare.after_fix.txt"
fi

# rustfmt --check is the LAST Rust action.
run_step 13_fmt_check_final cargo fmt "${FMT_ARGS[@]}"

sha256sum crates/da-parse/src/native_body_text.rs \
         crates/da-parse/src/lib.rs \
         crates/da-parse/tests/native_body_uncertainty_split_contract.rs \
         crates/da-cli/examples/native_body_uncertainty_split_probe.rs \
         crates/da-cli/Cargo.toml \
  > "$RUN/hashes.source.after_fmt_fix.txt"

END_NS=$(date +%s%N)
{
  echo "end_utc=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)"
  echo "end_epoch_ns=$END_NS"
  echo "elapsed_ms=$(( (END_NS - START_NS) / 1000000 ))"
} >> "$RUN/RUN.identity.after_fix.txt"

echo "=========== AFTER-FIX SUMMARY ==========="
cat "$RUN/steps.after_fix.tsv"
echo "---------- artifact vs frozen ----------"
cat "$RUN/hash_compare.after_fix.txt"
echo "================ DONE ==================="
