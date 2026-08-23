# M176 Baseline

## Verdict

**Baseline status: PASS.** No M176 scanner edits have been made.

## Baseline inventory

```text
total_records=341
script-only=265
unknown=0
shared-state=0
by_root.scripts=265
by_root.src=76
```

Generated artifacts:

- `data/architecture-assessment/m176-write-path-inventory-baseline.json`
- `data/architecture-assessment/m176-write-path-inventory-baseline.md`

## Top script-only files

| Count | File |
|---:|---|
| 6 | `scripts/m061_anchor_pilot.py` |
| 5 | `scripts/m061_full_5_anchors.py` |
| 4 | `scripts/m058_plotextractor_extract.py` |
| 4 | `scripts/verify_m029_unified_source_acquisition.py` |
| 3 | `scripts/audit_test_architecture.py` |
| 3 | `scripts/build_m028_pdf_acquisition_diagnostics.py` |
| 3 | `scripts/build_m028_universal_loader_evidence_bundles.py` |
| 3 | `scripts/build_r024_20_document_corpus_selection.py` |
| 3 | `scripts/build_r024_53_document_corpus_selection.py` |
| 3 | `scripts/build_r024_entity_networkx_probe.py` |
| 3 | `scripts/convert_r024_53_pdf_to_text.py` |
| 3 | `scripts/extract_r024_entity_scale_entities.py` |
| 3 | `scripts/inventory_write_paths.py` |
| 3 | `scripts/m057_marker_extract.py` |
| 3 | `scripts/m058_compare_v2_vs_m057.py` |
| 3 | `scripts/m058_marker_compare_5.py` |

## Top script-only target names

```text
path=76
tmp_path=20
output_path=19
summary_path=8
fd=7
json_path=6
output=6
report_path=6
```

Target names are too generic for classification. M176 must use exact script-family source paths.

## Evidence

- Baseline scanner: `gsd_exec[ec5794c3-8674-4041-8049-f3a2e5be0896]`
- Script-only summary: `gsd_exec[9fef7782-faad-4dab-b5b5-020df3ab7c66]`
