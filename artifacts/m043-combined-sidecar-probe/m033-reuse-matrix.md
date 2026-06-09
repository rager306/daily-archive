# M043 M033 Reuse Matrix

- Target article count: 6
- Candidate only: true
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled

| System | Role | Reuse | All prior artifacts present | Target-specific gap |
|---|---|---|---:|---|
| current_baseline | comparison_contracts_and_refusal_boundaries | reuse_contracts | true | must compare target subset sidecar outputs against these contracts |
| grobid | scholarly_tei_bibliography_section_candidate | reuse_runtime_contract_and_verdict | true | need target component PDFs or typed unavailable status before claiming target-specific TEI evidence |
| opendataloader_pdf | layout_ocr_table_coordinate_candidate | reuse_backend_contract_and_quality_caveats | true | need target component PDFs or typed unavailable status before claiming target-specific layout/OCR/table evidence |
| adaptix | typed_adapter_over_fixed_parser_json | reuse_adapter_pattern_and_tests | true | requires fixed OpenDataLoader JSON or fixture packet for target-specific adaptation |
| quant_mind_patterns | pattern_source_for_tree_card_provenance_flow | reuse_pattern_mapping_only | true | must not import quant-mind runtime or treat it as extraction proof |
| combined_architecture | bounded_combined_sidecar_architecture | reuse_recommendation_and_safety_boundaries | true | must produce candidate-only comparison packets for M043 target subset |
