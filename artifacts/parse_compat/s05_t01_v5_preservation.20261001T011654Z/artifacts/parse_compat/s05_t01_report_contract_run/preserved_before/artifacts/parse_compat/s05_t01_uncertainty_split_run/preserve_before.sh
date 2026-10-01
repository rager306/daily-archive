#!/usr/bin/env bash
# S05/T01 uncertainty-split diagnostic — preservation snapshot.
#
# Taken BEFORE any edit. It records the bytes of every prior artifact, evidence
# document, accepted-run manifest and frozen source, and copies the actual
# originals (with their repo-relative paths) into preserved_before/ so an edit
# cannot destroy what it replaced. It also cross-checks the six accepted T01
# sources against the accepted P1 run's own manifest, so "which source moved
# relative to the accepted run" is a byte fact rather than a claim.
set -u

cd /root/daily-archive || exit 90
RUN=artifacts/parse_compat/s05_t01_uncertainty_split_run
mkdir -p "$RUN/preserved_before"

PRESERVED=(
  # prior S05/T01 artifacts
  artifacts/parse_compat/native_body_character_shadow.json
  artifacts/parse_compat/native_body_character_scope_bound.json
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json
  artifacts/parse_compat/native_body_character_scope_bound.repair2_artifact.json
  artifacts/parse_compat/native_body_order_lineage.json
  # prior S05/T01 evidence
  artifacts/parse_compat/native_body_character_scope_bound.evidence.md
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
  artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
  artifacts/parse_compat/native_body_order_lineage.evidence.md
  artifacts/parse_compat/s05_t01_p1_rung_checked_run/EVIDENCE.md
  # accepted-run manifests (the actual original records)
  artifacts/parse_compat/s05_t01_p1_rung_checked_run/hashes.source.before.txt
  artifacts/parse_compat/s05_t01_order_lineage_run/hashes.source.before.txt
  artifacts/parse_compat/s05_t01_order_lineage_run/RUN.identity.txt
  # frozen / to-be-edited sources
  crates/da-parse/src/native_body_text.rs
  crates/da-parse/src/lib.rs
  crates/da-parse/tests/native_body_character_oracle.rs
  crates/da-parse/tests/architecture_contract.rs
  crates/da-parse/src/native_body_order_lineage.rs
  crates/da-parse/tests/native_body_order_lineage_contract.rs
  crates/da-cli/examples/native_body_character_probe.rs
  crates/da-cli/examples/native_body_order_lineage_probe.rs
  crates/da-cli/Cargo.toml
)
sha256sum "${PRESERVED[@]}" > "$RUN/preserved_before.sha256"

# Copy the actual originals, keeping their repo-relative directory structure.
cp --parents "${PRESERVED[@]}" "$RUN/preserved_before/" 2>/dev/null \
  || { mkdir -p "$RUN/preserved_before"; for f in "${PRESERVED[@]}"; do
         mkdir -p "$RUN/preserved_before/$(dirname "$f")"; cp -p "$f" "$RUN/preserved_before/$f"; done; }

# Cross-check the six accepted T01 sources against the accepted P1 run record.
ACCEPTED_INDEX=artifacts/parse_compat/s05_t01_p1_rung_checked_run/hashes.source.before.txt
{
  echo "# accepted T01 sources vs the accepted P1 run's own record ($ACCEPTED_INDEX)"
  for f in crates/da-parse/src/native_body_text.rs \
           crates/da-parse/src/lib.rs \
           crates/da-parse/tests/native_body_character_oracle.rs \
           crates/da-parse/tests/architecture_contract.rs \
           crates/da-cli/examples/native_body_character_probe.rs \
           crates/da-cli/Cargo.toml; do
    now=$(sha256sum "$f" | cut -d' ' -f1)
    was=$(grep -F "  $f" "$ACCEPTED_INDEX" | cut -d' ' -f1)
    if [ -n "$was" ] && [ "$now" = "$was" ]; then
      echo "SAME  $f"
    else
      echo "CHANGED  $f now=$now accepted=$was"
    fi
  done
} > "$RUN/preserved_accepted_sources.txt"

# Re-hash the copies and prove they equal the recorded originals.
( cd "$RUN/preserved_before" && sha256sum -c "../preserved_before.sha256" ) \
  > "$RUN/preserved_copies_check.txt" 2>&1
echo "preserved_copies_rc=$?" >> "$RUN/preserved_copies_check.txt"

cat "$RUN/preserved_accepted_sources.txt"
echo "[preserve] wrote $RUN/preserved_before.sha256 ($(wc -l < "$RUN/preserved_before.sha256") files)"
echo "[preserve] copies under $RUN/preserved_before/"
