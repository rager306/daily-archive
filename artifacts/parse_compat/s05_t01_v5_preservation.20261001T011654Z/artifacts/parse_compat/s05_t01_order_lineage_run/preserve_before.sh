#!/usr/bin/env bash
# S05/T01 order-lineage diagnostic — preservation snapshot.
#
# Taken before the producing run. Records the bytes of every prior artifact,
# evidence document and run directory, and cross-checks the accepted T01 sources
# against the bytes the accepted P1 run recorded, so "the accepted API was not
# touched" is a byte fact rather than a claim.
set -u

cd /root/daily-archive || exit 90
RUN=artifacts/parse_compat/s05_t01_order_lineage_run
mkdir -p "$RUN"

PRESERVED=(
  artifacts/parse_compat/native_body_character_shadow.json
  artifacts/parse_compat/native_body_character_scope_bound.json
  artifacts/parse_compat/native_body_character_scope_bound.evidence.md
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
  artifacts/parse_compat/native_body_character_scope_bound.repair2_artifact.json
  artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
  artifacts/parse_compat/s05_t01_p1_rung_checked_run/EVIDENCE.md
  crates/da-parse/src/native_body_text.rs
  crates/da-parse/tests/native_body_character_oracle.rs
  crates/da-cli/examples/native_body_character_probe.rs
)
sha256sum "${PRESERVED[@]}" > "$RUN/preserved_before.sha256"

find artifacts/parse_compat/s05_t01_repair2_run \
     artifacts/parse_compat/s05_t01_repair2_cleanup_run \
     artifacts/parse_compat/s05_t01_p1_rung_checked_run \
     -type f -print0 2>/dev/null \
  | sort -z \
  | xargs -0 -r sha256sum > "$RUN/preserved_run_dirs.sha256"

ACCEPTED_INDEX=artifacts/parse_compat/s05_t01_p1_rung_checked_run/hashes.source.before.txt
{
  echo "# accepted T01 sources vs the accepted P1 run's own record ($ACCEPTED_INDEX)"
  for f in crates/da-parse/src/native_body_text.rs \
           crates/da-parse/tests/native_body_character_oracle.rs \
           crates/da-cli/examples/native_body_character_probe.rs; do
    now=$(sha256sum "$f" | cut -d' ' -f1)
    was=$(grep -F "  $f" "$ACCEPTED_INDEX" | cut -d' ' -f1)
    if [ -n "$was" ] && [ "$now" = "$was" ]; then
      echo "SAME  $f"
    else
      echo "CHANGED  $f now=$now was=$was"
    fi
  done
} > "$RUN/preserved_accepted_sources.txt"

cat "$RUN/preserved_accepted_sources.txt"
echo "[preserve] wrote $RUN/preserved_before.sha256 ($(wc -l < "$RUN/preserved_before.sha256") files)"
echo "[preserve] wrote $RUN/preserved_run_dirs.sha256 ($(wc -l < "$RUN/preserved_run_dirs.sha256") files)"
