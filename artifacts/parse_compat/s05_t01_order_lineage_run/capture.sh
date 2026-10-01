#!/usr/bin/env bash
# S05/T01 order-lineage diagnostic — durable capture script.
#
# Written into the durable run directory BEFORE the run and hashed at run time.
# It is the actual script that produced every artifact in this directory.
#
# Properties required by the task:
#   * detached, sequential, one cargo process at a time, NO execution timeout;
#   * before/after sha256 of every source, evidence and frozen input, so a frozen
#     input is provable rather than asserted;
#   * per-step argv, start/end timestamp, elapsed time, real exit code, and final
#     log sizes beside the full stdout/stderr;
#   * if the cargo output wrapper compresses an oversized log, the uncompressed
#     log it names is copied into this directory as `<step>.full.log` so the
#     durable record is never a summary;
#   * preserved prior artifacts are re-verified by re-hashing them after the run;
#   * rustfmt --check is the LAST Rust action.
set -u

cd /root/daily-archive || exit 90

RUN=artifacts/parse_compat/s05_t01_order_lineage_run
ARTIFACT=artifacts/parse_compat/native_body_order_lineage.json
mkdir -p "$RUN"

SOURCE_FILES=(
  crates/da-parse/src/native_body_order_lineage.rs
  crates/da-parse/src/lib.rs
  crates/da-parse/tests/native_body_order_lineage_contract.rs
  crates/da-parse/tests/architecture_contract.rs
  crates/da-cli/examples/native_body_order_lineage_probe.rs
  crates/da-cli/Cargo.toml
)
EVIDENCE_FILES=(
  "$RUN/preserve_before.sh"
  "$RUN/preserved_before.sha256"
  "$RUN/preserved_run_dirs.sha256"
  "$RUN/preserved_accepted_sources.txt"
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
  artifacts/parse_compat/native_body_character_scope_bound.json
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json
  artifacts/parse_compat/native_body_character_scope_bound.repair2_artifact.json
  "$RUN/preserved_before.sha256"
  "$RUN/preserved_run_dirs.sha256"
)

# ── identity of this run, recorded before any step ──────────────────────────────
RUN_START_NS=$(date +%s%N)
RUN_START_UTC=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)
{
  echo "run_dir=$RUN"
  echo "capture_script=$RUN/capture.sh"
  echo "capture_script_sha256=$(sha256sum "$RUN/capture.sh" | cut -d' ' -f1)"
  echo "preserve_script=$RUN/preserve_before.sh"
  echo "preserve_script_sha256=$(sha256sum "$RUN/preserve_before.sh" | cut -d' ' -f1)"
  echo "start_utc=$RUN_START_UTC"
  echo "start_epoch_ns=$RUN_START_NS"
  echo "cwd=$(pwd)"
  echo "git_head=$(git rev-parse HEAD 2>/dev/null)"
  echo "git_branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
  echo "cargo_version=$(cargo --version 2>/dev/null)"
  echo "rustc_version=$(rustc --version 2>/dev/null)"
  echo "host_date=$(date)"
} > "$RUN/RUN.identity.txt"

# ── frozen-source / evidence / input hashes, captured BEFORE the run ────────────
sha256sum "${SOURCE_FILES[@]}" > "$RUN/hashes.source.before.txt"
sha256sum "${EVIDENCE_FILES[@]}" > "$RUN/hashes.evidence.before.txt"
sha256sum "${INPUT_FILES[@]}" > "$RUN/hashes.input.before.txt"
if [ -f "$ARTIFACT" ]; then
  sha256sum "$ARTIFACT" > "$RUN/hashes.artifact.before.txt"
else
  echo "ABSENT  $ARTIFACT" > "$RUN/hashes.artifact.before.txt"
fi

# ── per-step capture ───────────────────────────────────────────────────────────
printf 'step\targv\tstart_utc\tstart_epoch_ns\tend_utc\tend_epoch_ns\telapsed_ms\texit_code\tstdout_bytes\tstderr_bytes\n' > "$RUN/steps.tsv"

# Copy the uncompressed log the cargo wrapper names, if it compressed a step.
stash_full_log() {
  local name="$1"
  local marker
  marker=$(grep -ho '\[full output: [^]]*\]' "$RUN/$name.stdout" "$RUN/$name.stderr" 2>/dev/null \
    | head -1 | sed 's/^\[full output: //; s/\]$//')
  case "$marker" in
    "~/"*) marker="$HOME/${marker#\~/}" ;;
  esac
  if [ -n "$marker" ] && [ -f "$marker" ]; then
    cp "$marker" "$RUN/$name.full.log"
    echo "[capture] $name full log stashed from $marker"
  fi
}

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
  stash_full_log "$name"
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
    "$(wc -c < "$stdout")" "$(wc -c < "$stderr")" >> "$RUN/steps.tsv"
  echo "[capture] $name rc=$rc elapsed_ms=$elapsed_ms"
  return 0
}

# Sequential. A failure is recorded, not masked, and the run continues so the
# whole evidence set exists for a reader.
run_step 01_order_lineage_contract cargo test -p da-parse --test native_body_order_lineage_contract
run_step 02_architecture_contract cargo test -p da-parse --test architecture_contract
run_step 03a_clippy_da_parse cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings
run_step 03b_clippy_da_cli cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings
run_step 03c_clippy_probe cargo clippy -p da-cli --features pdf-layout --example native_body_order_lineage_probe --no-deps -- -D warnings
run_step 04_probe cargo run -p da-cli --features pdf-layout --example native_body_order_lineage_probe

# ── frozen-source / evidence / input hashes, captured AFTER the probe ───────────
sha256sum "${SOURCE_FILES[@]}" > "$RUN/hashes.source.after.txt"
sha256sum "${EVIDENCE_FILES[@]}" > "$RUN/hashes.evidence.after.txt"
sha256sum "${INPUT_FILES[@]}" > "$RUN/hashes.input.after.txt"
if [ -f "$ARTIFACT" ]; then
  sha256sum "$ARTIFACT" > "$RUN/hashes.artifact.after.txt"
else
  echo "ABSENT  $ARTIFACT" > "$RUN/hashes.artifact.after.txt"
fi

{
  for pair in "source hashes.source.before.txt hashes.source.after.txt" \
              "evidence hashes.evidence.before.txt hashes.evidence.after.txt" \
              "input hashes.input.before.txt hashes.input.after.txt" \
              "artifact hashes.artifact.before.txt hashes.artifact.after.txt"; do
    set -- $pair
    if diff -q "$RUN/$2" "$RUN/$3" > /dev/null 2>&1; then
      echo "SAME  $1"
    else
      echo "CHANGED  $1"
    fi
  done
} > "$RUN/hash_compare.txt"

# ── re-verify that every preserved prior artifact is still byte-identical ───────
run_step 05_preserved_artifacts_check sha256sum -c "$RUN/preserved_before.sha256"
run_step 06_preserved_run_dirs_check sha256sum -c "$RUN/preserved_run_dirs.sha256"

# ── rustfmt --check is the LAST Rust action ─────────────────────────────────────
run_step 07_fmt_check cargo fmt -p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline -p da-domain -p da-ports -p da-application -p da-adapters -p da-parse -p da-cli -p da-mcp -- --check

# ── log manifest: bytes and sha256 for every captured log, from the bytes ───────
: > "$RUN/logs.manifest.tsv"
for f in "$RUN"/*.stdout "$RUN"/*.stderr "$RUN"/*.rc "$RUN"/*.full.log; do
  [ -e "$f" ] || continue
  printf '%s\t%s\t%s\n' "$(basename "$f")" "$(wc -c < "$f")" "$(sha256sum "$f" | cut -d' ' -f1)" \
    >> "$RUN/logs.manifest.tsv"
done

RUN_END_NS=$(date +%s%N)
RUN_END_UTC=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)
{
  echo "end_utc=$RUN_END_UTC"
  echo "end_epoch_ns=$RUN_END_NS"
  echo "elapsed_ms=$(( (RUN_END_NS - RUN_START_NS) / 1000000 ))"
  echo "elapsed_seconds=$(( (RUN_END_NS - RUN_START_NS) / 1000000000 ))"
} >> "$RUN/RUN.identity.txt"

echo "================ CAPTURE SUMMARY ================"
cat "$RUN/steps.tsv"
echo "------------ hash stability ------------"
cat "$RUN/hash_compare.txt"
echo "=============== CAPTURE DONE ===================="
