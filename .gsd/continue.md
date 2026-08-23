# continue — session state

## Active direction (M451): schema/manifest skeleton BEFORE code

ADR-068 accepted (revised after elegance review): four roles —
SCHEMA = norm (machine-readable), FS = truth, Governor = direct
validator (FS vs SCHEMA), MANIFEST = derived cache (~7 aggregate
levels only). Meta/prompt: no hardcoded structure anywhere; rules
live in schema data, checker is a generic engine.

Shipped (commit HEAD):
- 19 SCHEMA.yaml (structural dirs, v2, adr_ref: ADR-068)
- 7 MANIFEST.yaml caches (root, data, sources, arxiv, textbook, processed, governor)
- data/schemas/article_schema.yaml — SINGLE definition site: layers
  (A0-A7), lifecycle_order, invariants, forbidden edges, bi-temporal
  lifecycle_events, flat-default versioning
- Governor dimension schema_hierarchy: contract YAML + generic engine
  (valid_subdirs/valid_files/forbidden/ignore/tolerated/subdir_pattern)
- Gap closed: old_artifacts_graphs/ -> archive/m451-cleanup (byte-dupes);
  data/{index,catalog,index-rebuild-*} -> artifacts/m450-build-reports
- Pre-existing bugs fixed: doctor (ctor kwargs, report.flags, args.quick),
  test_prompt_eval path (M448 migration miss)
- Tests: 140 passed. Real repo: 0 schema_hierarchy flags.

## Next actions
1. W4: per-article lowercase manifest.yaml with lifecycle_events —
   migrate 604 processed articles from manifest.json (M450 format).
   Design first (fields per article_schema.yaml manifest section),
   then generator script, then Governor content-invariants
   (ast_covers_sections etc.) as W3 extension.
2. Governor dimension status: reverse_adr_audit still 269 violations
   + graph gaps (section_text_incomplete, enrichment_missing,
   revision_binding_low) — honest, clear when pipeline fixed.
3. HTML debug view (debug/ layer, derived_from: [manifest]) — after W4.

## Operators
PYTHONPATH=scripts python3 -m governor doctor      # full: trajectory+arch+docs
PYTHONPATH=scripts python3 -c "from pathlib import Path; from governor.adapters.schema_hierarchy_checker import check_schema_hierarchy as c; print(c(Path('.')))"

## Do not forget
- D127: import_eligible stays false
- Destructive ops: only moves with preservation, never recursive deletion
- gsd-pi updates wipe hotfixes (markdown-renderer, doctor-engine-checks)
- Russian for user-facing replies
