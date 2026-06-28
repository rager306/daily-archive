# M190 S03 Local Validator Results

## Verdict

**PASS: bounded M027 replay, M027 source boundary verification, and M030 validate-only gates passed.**

## Evidence

| Gate | Result | Evidence |
|---|---|---|
| M027 current pipeline replay | PASS: command exited 0 and wrote 6 per-article baseline JSON files | `gsd_exec[7a0b464d-f0a2-4598-b7de-981960cf6136]` |
| M027 replay output assertions | PASS: output directory exists, file_count=6, unsafe flags absent | `gsd_exec[7b1a3891-e9da-4c0f-8ba3-965204dd6e1b]` |
| M027 source boundary verifier | PASS: six selected articles, terminal variant states, local artifact hashes, redaction constraints, fail-closed graph/production flags valid | `gsd_exec[fe4fab8a-deb0-4d4d-9795-2756730e9423]` |
| M030 validate-only | PASS: 4 refs, 3 cataloged, 1 typed catalog blocker, graph/import claims fail-closed | `gsd_exec[b3cf5c22-e972-4225-8b01-9f3eefead372]` |

## Observed outputs against expected contract

- `source_quality_labels_present`: observed through M027 source boundary verifier and replay baseline files.
- `low_quality_source_fail_closed`: preserved; no low-quality source was reclassified as success in T01.
- `parser_ready_scope`: bounded to M027 replay scope only.
- `chunk_ready_scope`: bounded to M027 replay output scope only.
- `graph_import_ready=false`: preserved.
- `production_persistence_ready=false`: preserved.
- `optimizer_enabled=false`: preserved.
- `direct_extractor_to_graph_write=false`: preserved.

## Generated outputs

- `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2603.04448/baseline.json`
- `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2604.18478/baseline.json`
- `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.20897/baseline.json`
- `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.21401/baseline.json`
- `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.25522/baseline.json`
- `data/architecture-assessment/m190-m027-current-pipeline-replay/nature_mixed-source_s44387-025-00019-5/baseline.json`

## Scope boundary

T01 did not claim broad parser readiness, graph import readiness, production persistence readiness, or optimizer readiness.
