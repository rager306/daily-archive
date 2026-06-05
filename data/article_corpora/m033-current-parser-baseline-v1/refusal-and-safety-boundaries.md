# M033 S01 T03: Refusal Diagnostics and Safety Boundaries

This document defines the fail-closed model that external parser probes must preserve.

| Boundary | Refusal/blocker states | Why it exists | External parser implication |
|---|---|---|---|
| catalog_intake | `typed_catalog_blocker`<br>`silent_missing_count must remain 0` | A requested ref without a safe catalog row must not silently advance into acquisition. | External tools cannot repair catalog identity gaps; they need a selected local source or typed blocker. |
| source_acquisition | `missing_source`<br>`metadata_only`<br>`external_pdf_without_safe_local_path`<br>`unsafe_path` | Acquisition must prove safe local artifacts with hashes and sizes before loader/parser work. | OpenDataLoader/GROBID probes should use local PDFs with sha256 and no network fetch by default. |
| loader_evidence | `loader_blocker`<br>`not_captured_by_acquisition`<br>`metadata_only_loaded_row` | Only captured local artifacts should become loaded candidates; non-captured rows stay blocked. | Parser probes must preserve which rows were actually loadable versus metadata-only controls. |
| parser_conversion | `low_quality_source`<br>`no_substantive_body`<br>`unsafe_input`<br>`missing_source`<br>`external_only`<br>`catalog_gap` | Non-empty markdown or HTTP/PDF success is insufficient; usable body text and source quality must be checked. | External parser output must be judged for reading order, body substance, section structure, and diagnostic clarity. |
| chunk_evidence | `zero_chunk_refusal`<br>`parser_not_ready`<br>`stale_source_hash`<br>`invalid_source_span` | Chunking only follows parser-ready converted artifacts with stable source linkage. | Layout/table outputs must map to source spans before they can influence chunk/evidence contracts. |
| graph_readiness_review | `pending_review`<br>`missing_completed_review`<br>`output_contract_completed_not_true` | Reviewer packets are not import eligibility without independent completed review. | OpenDataLoader/GROBID artifacts can at most create candidate reviewer inputs in M033. |
| no_write_import | `import_eligible_count=0`<br>`accepted_count=0`<br>`graph_import_allowed=false` | M031 established a no-write import rehearsal; parser improvements do not authorize LadybugDB writes. | All M033 artifacts must keep production import and LadybugDB write flags false. |

## Required false flags

- `graph_import_allowed` must remain `false` for M033 research artifacts.
- `trusted_kg_import_allowed` must remain `false` for M033 research artifacts.
- `production_import_attempted` must remain `false` for M033 research artifacts.
- `production_persistence_attempted` must remain `false` for M033 research artifacts.
- `ladybugdb_written` must remain `false` for M033 research artifacts.
- `graph_write_attempted` must remain `false` for M033 research artifacts.
- `model_call_required_for_parser_probe` must remain `false` for M033 research artifacts.

## Forbidden positive claims

- production graph readiness
- positive KG import eligibility
- LadybugDB write readiness
- trusted KG fact promotion
- parser output is graph-ready by default

## Evaluation rule

Classify failures as implementation defects, formal artifact conflicts, requirement framing issues, or validation policy issues before changing code.
