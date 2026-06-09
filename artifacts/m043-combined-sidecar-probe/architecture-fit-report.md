# M043 Combined Sidecar Architecture Fit Report

- Verdict: `candidate_only_combined_sidecar_architecture_fit_with_blockers`
- Target article count: 6
- Target category counts: {'baseline': 1, 'reference_linked': 5}
- Candidate only: true
- No graph import authorized
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled

## Fit summary

| System | Fit |
|---|---|
| current_baseline | ready as comparison/refusal contract for all target articles |
| grobid | architecturally fit as scholarly TEI sidecar, but target-specific run blocked by missing GROBID_URL and missing local PDFs for linked articles |
| opendataloader_pdf | architecturally fit as layout/OCR/table/coordinate sidecar; one baseline record is ready for bounded live PDF probe, five linked records are blocked by missing local PDFs |
| adaptix | fit as typed adapter after OpenDataLoader fixed JSON exists; currently ready for the baseline path and blocked for five linked records |
| quant_mind_patterns | fit as pattern source for PageIndex/tree/card/provenance only; runtime dependency adoption remains unauthorized |
| combined_architecture | M033 bounded combined sidecar architecture remains the right shape for M043, with blockers made explicit |

## Blockers before target-specific sidecar evidence

- `configure_or_start_GROBID_service_before_target_TEI_claims`
- `perform_bounded_local_pdf_acquisition_for_five_linked_articles_before_pdf_parser_claims`
- `produce_OpenDataLoader_fixed_JSON_before_Adaptix_target_adapter_claims`
- `keep_quant_mind_as_pattern_source_not_runtime_dependency`

## Packet status counts

| Status | Count |
|---|---:|
| adaptix:blocked_waiting_for_target_opendataloader_fixed_json | 5 |
| adaptix:ready_after_opendataloader_fixed_json | 1 |
| combined_architecture:ready_recommendation_mapping | 6 |
| current_baseline:ready_contract_reference | 6 |
| grobid:blocked_target_specific_run_replayable_prior_evidence | 6 |
| opendataloader_pdf:blocked_target_specific_run_replayable_prior_evidence | 5 |
| opendataloader_pdf:ready_for_bounded_live_pdf_probe | 1 |
| quant_mind_patterns:ready_pattern_mapping_only | 6 |

## Next gate

bounded_pdf_acquisition_and_live_sidecar_probe_or_ADR_gated_parser_quality_plan
