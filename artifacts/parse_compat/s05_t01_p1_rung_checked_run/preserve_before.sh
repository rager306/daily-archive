#!/usr/bin/env bash
# S05/T01 P1 rung+localization repair — preservation step.
#
# Run BEFORE any edit or overwrite. Copies every historical shadow / prior-repair
# / repair2 / cleanup artifact and evidence document, plus the pre-edit source
# snapshots of the files this repair touches, into preserved_before/. Then it
# records sha256 manifests of the copies and a byte manifest of the existing
# durable run directories, so it is provable that the originals were never
# mutated by this repair. Nothing here edits an original.
set -euo pipefail
cd /root/daily-archive

RUN=artifacts/parse_compat/s05_t01_p1_rung_checked_run
PRES="$RUN/preserved_before"
mkdir -p "$PRES"

kept=(
  artifacts/parse_compat/native_body_character_shadow.json
  artifacts/parse_compat/native_body_character_scope_bound.json
  artifacts/parse_compat/native_body_character_scope_bound.evidence.md
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json
  artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
  artifacts/parse_compat/native_body_character_scope_bound.repair2_artifact.json
  artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
  crates/da-parse/src/native_body_text.rs
  crates/da-parse/src/lib.rs
  crates/da-parse/tests/native_body_character_oracle.rs
  crates/da-parse/tests/architecture_contract.rs
  crates/da-cli/examples/native_body_character_probe.rs
  crates/da-cli/Cargo.toml
)

for f in "${kept[@]}"; do
  mkdir -p "$PRES/$(dirname "$f")"
  cp -p "$f" "$PRES/$f"
done

( cd "$PRES" && find . -type f | sort | xargs sha256sum ) > "$RUN/preserved_before.sha256"

find artifacts/parse_compat/s05_t01_repair2_run artifacts/parse_compat/s05_t01_repair2_cleanup_run \
  -type f -print0 | sort -z | xargs -0 sha256sum > "$RUN/preserved_run_dirs.sha256"

echo "preserved $(wc -l < "$RUN/preserved_before.sha256") files -> $PRES"
echo "run-dir manifest lines: $(wc -l < "$RUN/preserved_run_dirs.sha256")"
