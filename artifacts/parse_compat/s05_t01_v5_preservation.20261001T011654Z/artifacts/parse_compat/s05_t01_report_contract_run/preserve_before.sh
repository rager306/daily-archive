#!/usr/bin/env bash
# S05/T01 uncertainty-split REPORT-CONTRACT repair — preservation step.
#
# Run BEFORE any edit or overwrite. Two jobs, both read-only:
#
#   1. Copy the files this repair will touch (pre-edit source snapshots), the
#      prior diagnostic artifact, the prior evidence documents and the failure
#      records of the previous run into preserved_before/, then hash the copies.
#   2. Record a byte manifest of every pre-existing durable run directory, so it
#      is provable that none of them was mutated by this repair.
#
# The previous run's rustfmt failure (`s05_t01_uncertainty_split_run/07_fmt_check`
# = rc 1) and its after-fix records are preserved by both jobs: copied by name in
# (1) and covered by the manifest in (2). Nothing here edits an original.
set -euo pipefail
cd /root/daily-archive

RUN=artifacts/parse_compat/s05_t01_report_contract_run
PRES="$RUN/preserved_before"
mkdir -p "$PRES"

kept=(
  # this repair's live targets (pre-edit snapshots)
  crates/da-cli/examples/native_body_uncertainty_split_probe.rs
  crates/da-parse/tests/native_body_uncertainty_split_contract.rs
  # pinned-but-unedited sources, so before == after is provable
  crates/da-parse/tests/architecture_contract.rs
  crates/da-parse/src/native_body_text.rs
  crates/da-parse/src/lib.rs
  crates/da-cli/Cargo.toml
  # prior diagnostic artifact and its evidence (this repair rewrites the artifact)
  artifacts/parse_compat/native_body_uncertainty_split.json
  artifacts/parse_compat/native_body_uncertainty_split.evidence.md
  # accepted sibling artifacts that must stay byte-identical
  artifacts/parse_compat/native_body_character_scope_bound.json
  artifacts/parse_compat/native_body_character_scope_bound.evidence.md
  artifacts/parse_compat/native_body_character_shadow.json
  artifacts/parse_compat/native_body_order_lineage.json
  artifacts/parse_compat/native_body_order_lineage.evidence.md
  artifacts/parse_compat/native_body_order_lineage.addendum.md
  # previous run identity and failure records (the original rustfmt rc 1)
  artifacts/parse_compat/s05_t01_uncertainty_split_run/RUN.identity.txt
  artifacts/parse_compat/s05_t01_uncertainty_split_run/RUN.identity.after_fix.txt
  artifacts/parse_compat/s05_t01_uncertainty_split_run/steps.tsv
  artifacts/parse_compat/s05_t01_uncertainty_split_run/steps.after_fix.tsv
  artifacts/parse_compat/s05_t01_uncertainty_split_run/07_fmt_check.argv
  artifacts/parse_compat/s05_t01_uncertainty_split_run/07_fmt_check.meta
  artifacts/parse_compat/s05_t01_uncertainty_split_run/07_fmt_check.rc
  artifacts/parse_compat/s05_t01_uncertainty_split_run/07_fmt_check.stdout
  artifacts/parse_compat/s05_t01_uncertainty_split_run/07_fmt_check.stderr
  artifacts/parse_compat/s05_t01_uncertainty_split_run/capture.sh
  artifacts/parse_compat/s05_t01_uncertainty_split_run/preserve_before.sh
  artifacts/parse_compat/s05_t01_uncertainty_split_run/verify_after_fix.sh
  artifacts/parse_compat/s05_t01_uncertainty_split_run/hash_compare.txt
  artifacts/parse_compat/s05_t01_uncertainty_split_run/live_sources_vs_accepted.txt
)

for f in "${kept[@]}"; do
  if [ ! -e "$f" ]; then
    echo "MISSING (refusing to invent): $f" >&2
    exit 91
  fi
  mkdir -p "$PRES/$(dirname "$f")"
  cp -p "$f" "$PRES/$f"
done

( cd "$PRES" && find . -type f | sort | xargs sha256sum ) > "$RUN/preserved_before.sha256"

# Byte manifest of every pre-existing run directory (my new run dir excluded).
find \
  artifacts/parse_compat/s05_t01_repair2_run \
  artifacts/parse_compat/s05_t01_repair2_cleanup_run \
  artifacts/parse_compat/s05_t01_order_lineage_run \
  artifacts/parse_compat/s05_t01_p1_rung_checked_run \
  artifacts/parse_compat/s05_t01_uncertainty_split_run \
  -type f -print0 | sort -z | xargs -0 sha256sum > "$RUN/preserved_run_dirs.sha256"

echo "preserved $(wc -l < "$RUN/preserved_before.sha256") files -> $PRES"
echo "run-dir manifest lines: $(wc -l < "$RUN/preserved_run_dirs.sha256")"
