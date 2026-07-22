# M027 Requirement Scope Matrix

Source JSON: `doc/validation/m027_requirement_scope_matrix.json`
Schema: `m027-requirement-scope-matrix.v1`

This fresh-reader twin is metadata-only. It distinguishes M027-advanced but not globally validated requirements from future/out-of-scope active requirements and preserves the S07 closeout validation chain.

## Closeout Boundary
M027-aakeky advanced a local six-article mixed-source preprocessing cycle and validation-ready synthesis. This matrix is metadata-only remediation evidence for S08; it prevents active broad requirements from being mistaken for failed M027 deliverables.

### In Scope
- Classify R024, R027, and R029 as M027-advanced preprocessing evidence only.
- Classify R036 as preserving the S07 closeout validation chain.
- Classify R019, R022, R023, R031, R032, and R033 as future/out-of-scope active requirements.
- Validate local source paths, safety flags, forbidden claim language, raw payload guardrails, and markdown freshness.

### Out of Scope
- KG graph import, graph readiness, trusted fact promotion, import-ready chunk approval, production LadybugDB writes, DSPy/RLM/MiniMax activation, unattended scaling, runtime services, dashboards, network calls, raw article text, PDF bytes, vectors, binary payloads, base64 payloads, or secrets.

## Requirement Coverage Summary
| Requirement | Status | Classification | Verdict | Action |
|---|---|---|---|---|
| R019 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R022 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R023 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R024 | active | `m027_advanced_preprocessing_only` | `advanced_not_globally_validated` | remain_active_with_m027_evidence_note |
| R027 | active | `m027_advanced_preprocessing_only` | `advanced_not_globally_validated` | remain_active_with_m027_evidence_note |
| R029 | active | `m027_advanced_preprocessing_only` | `advanced_not_globally_validated` | remain_active_with_m027_evidence_note |
| R031 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R032 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R033 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R036 | active | `s07_closeout_validation_chain_preserved` | `validation_chain_preserved_not_globally_closed` | preserve_s07_closeout_chain_and_keep_active |

## Evidence Sources
- `.gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-ROADMAP.md`
- `.gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-07-SUMMARY.md`
- `data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-summary.json`
- `data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-report.md`
- `data/article_corpora/m027-mixed-source-corpus-v1/provenance-riskratchet-gate/provenance-riskratchet-gate-summary.json`

## Allowed Validation Language
### R019
- M027 leaves R019 as a future/out-of-scope active requirement.
- Validation should not count broad active requirements as failed M027 deliverables when the M027 boundary excludes them.
### R022
- M027 leaves R022 as a future/out-of-scope active requirement.
- Validation should not count broad active requirements as failed M027 deliverables when the M027 boundary excludes them.
### R023
- M027 leaves R023 as a future/out-of-scope active requirement.
- Validation should not count broad active requirements as failed M027 deliverables when the M027 boundary excludes them.
### R024
- M027 advanced R024 for the six-article preprocessing cycle only; it is M027-advanced but not globally validated.
- Validation may cite local metadata-only S01-S07 evidence and must keep graph/import/trusted-fact readiness blocked.
### R027
- M027 advanced R027 for the six-article preprocessing cycle only; it is M027-advanced but not globally validated.
- Validation may cite local metadata-only S01-S07 evidence and must keep graph/import/trusted-fact readiness blocked.
### R029
- M027 advanced R029 for the six-article preprocessing cycle only; it is M027-advanced but not globally validated.
- Validation may cite local metadata-only S01-S07 evidence and must keep graph/import/trusted-fact readiness blocked.
### R031
- M027 leaves R031 as a future/out-of-scope active requirement.
- Validation should not count broad active requirements as failed M027 deliverables when the M027 boundary excludes them.
### R032
- M027 leaves R032 as a future/out-of-scope active requirement.
- Validation should not count broad active requirements as failed M027 deliverables when the M027 boundary excludes them.
### R033
- M027 leaves R033 as a future/out-of-scope active requirement.
- Validation should not count broad active requirements as failed M027 deliverables when the M027 boundary excludes them.
### R036
- M027 preserves the R036 S07 closeout validation chain for generated metadata artifacts.
- Validation may use S07 provenance hashes and safety flags as M027 closeout evidence while keeping R036 active.

## Forbidden Validation Language
### Global
- M027 validates graph readiness
- M027 authorizes KG import
- M027 validates KG import readiness
- M027 validates Scientific KG corpus behavior
- M027 validates import-ready chunks
- M027 promotes trusted facts
- M027 writes to production LadybugDB
- M027 activates DSPy, RLM, MiniMax, or optimizer behavior
- M027 proves unattended scaling
- M027 embeds raw article text, PDFs, binary payloads, base64 payloads, vector payloads, secrets, or production connection details
### R019
- M027 fully validates R019
- M027 advances R019 as a deliverable
- M027 validates graph readiness
- M027 proves unattended scaling
- M027 promotes trusted facts
### R022
- M027 fully validates R022
- M027 advances R022 as a deliverable
- M027 validates graph readiness
- M027 proves unattended scaling
- M027 promotes trusted facts
### R023
- M027 fully validates R023
- M027 advances R023 as a deliverable
- M027 validates graph readiness
- M027 proves unattended scaling
- M027 promotes trusted facts
### R024
- M027 globally validates R024
- M027 fully validates R024
- M027 validates graph readiness
- M027 authorizes KG import readiness
- M027 validates import-ready chunks
### R027
- M027 globally validates R027
- M027 fully validates R027
- M027 validates graph readiness
- M027 authorizes KG import readiness
- M027 validates import-ready chunks
### R029
- M027 globally validates R029
- M027 fully validates R029
- M027 validates graph readiness
- M027 authorizes KG import readiness
- M027 validates import-ready chunks
### R031
- M027 fully validates R031
- M027 advances R031 as a deliverable
- M027 validates graph readiness
- M027 proves unattended scaling
- M027 promotes trusted facts
### R032
- M027 fully validates R032
- M027 advances R032 as a deliverable
- M027 validates graph readiness
- M027 proves unattended scaling
- M027 promotes trusted facts
### R033
- M027 fully validates R033
- M027 advances R033 as a deliverable
- M027 validates graph readiness
- M027 proves unattended scaling
- M027 promotes trusted facts
### R036
- M027 fully validates R036 for every future artifact workflow
- M027 authorizes production LadybugDB writes
- M027 closes all provenance validation obligations globally

## Requirement Notes
### R019
- Rationale: hybrid retrieval and trusted graph readiness remains future work
- Evidence: M027 S07 explicitly lists R019 as future/out-of-scope active requirement evidence.
- Evidence: No graph import, trusted fact promotion, production write, operational monitoring, unattended scaling, or scientific KG validation is claimed.
- Remaining work: Keep this requirement active until a future milestone targets it directly.
- Remaining work: Do not expand M027 validation beyond the six-article preprocessing-only evidence boundary.
### R022
- Rationale: production graph import and KG write workflows remain future work
- Evidence: M027 S07 explicitly lists R022 as future/out-of-scope active requirement evidence.
- Evidence: No graph import, trusted fact promotion, production write, operational monitoring, unattended scaling, or scientific KG validation is claimed.
- Remaining work: Keep this requirement active until a future milestone targets it directly.
- Remaining work: Do not expand M027 validation beyond the six-article preprocessing-only evidence boundary.
### R023
- Rationale: trusted fact promotion and raw article payload handling remain future work
- Evidence: M027 S07 explicitly lists R023 as future/out-of-scope active requirement evidence.
- Evidence: No graph import, trusted fact promotion, production write, operational monitoring, unattended scaling, or scientific KG validation is claimed.
- Remaining work: Keep this requirement active until a future milestone targets it directly.
- Remaining work: Do not expand M027 validation beyond the six-article preprocessing-only evidence boundary.
### R024
- Rationale: six-article staged preprocessing cycle evidence was advanced
- Evidence: M027 advanced R024 through local six-article preprocessing evidence only.
- Evidence: S07 reports ready_with_blockers_conditions and preserves not_import_ready_validate_only boundaries.
- Remaining work: A future milestone must supply canonical validation evidence before status can change from active.
- Remaining work: Graph import, trusted facts, production writes, and unattended scaling require separate explicit gates.
### R027
- Rationale: conversion and replay diagnostics were advanced without graph-readiness acceptance
- Evidence: M027 advanced R027 through local six-article preprocessing evidence only.
- Evidence: S07 reports ready_with_blockers_conditions and preserves not_import_ready_validate_only boundaries.
- Remaining work: A future milestone must supply canonical validation evidence before status can change from active.
- Remaining work: Graph import, trusted facts, production writes, and unattended scaling require separate explicit gates.
### R029
- Rationale: pipeline integration gaps and typed-output readiness diagnostics were advanced without import-ready approval
- Evidence: M027 advanced R029 through local six-article preprocessing evidence only.
- Evidence: S07 reports ready_with_blockers_conditions and preserves not_import_ready_validate_only boundaries.
- Remaining work: A future milestone must supply canonical validation evidence before status can change from active.
- Remaining work: Graph import, trusted facts, production writes, and unattended scaling require separate explicit gates.
### R031
- Rationale: unattended deviation scans and scaling remain future work
- Evidence: M027 S07 explicitly lists R031 as future/out-of-scope active requirement evidence.
- Evidence: No graph import, trusted fact promotion, production write, operational monitoring, unattended scaling, or scientific KG validation is claimed.
- Remaining work: Keep this requirement active until a future milestone targets it directly.
- Remaining work: Do not expand M027 validation beyond the six-article preprocessing-only evidence boundary.
### R032
- Rationale: operational monitoring, paging, and automated expansion remain future work
- Evidence: M027 S07 explicitly lists R032 as future/out-of-scope active requirement evidence.
- Evidence: No graph import, trusted fact promotion, production write, operational monitoring, unattended scaling, or scientific KG validation is claimed.
- Remaining work: Keep this requirement active until a future milestone targets it directly.
- Remaining work: Do not expand M027 validation beyond the six-article preprocessing-only evidence boundary.
### R033
- Rationale: scientific KG validation policy and graph/import promotion remain future work
- Evidence: M027 S07 explicitly lists R033 as future/out-of-scope active requirement evidence.
- Evidence: No graph import, trusted fact promotion, production write, operational monitoring, unattended scaling, or scientific KG validation is claimed.
- Remaining work: Keep this requirement active until a future milestone targets it directly.
- Remaining work: Do not expand M027 validation beyond the six-article preprocessing-only evidence boundary.
### R036
- Rationale: S07 closeout validation chain with provenance hashes and safety flags is preserved
- Evidence: S07 closeout validation chain includes command, cwd, milestone/slice context, input and output hashes, diagnostics, and safe false flags.
- Evidence: S08 preserves the S07 closeout validation chain as remediation evidence without claiming all future provenance validation is globally closed.
- Remaining work: A separate canonical requirement update is needed if R036 is to move from active to validated.
- Remaining work: Future generated artifact classes must continue to record command, inputs, outputs, hashes, exit status, and milestone context.

## Validation Recommendation
Accept M027 validation evidence for preprocessing-only advancement of R024/R027/R029 and R036 closeout-chain preservation, while leaving all listed active requirements active unless separately validated.
M027-advanced does not mean globally validated; future/out-of-scope active requirements are intentionally excluded from M027 success criteria.

## Safety Flags
- `metadata_only`: `true`
- `network_fetch_attempted`: `false`
- `runtime_surface_changed`: `false`
- `kg_import_or_readiness_claimed`: `false`
- `graph_validation_claimed`: `false`
- `import_ready_chunks_claimed`: `false`
- `trusted_fact_promotion_claimed`: `false`
- `scientific_kg_corpus_validation_claimed`: `false`
- `dspy_rlm_minimax_activation_claimed`: `false`
- `unattended_scaling_claimed`: `false`
- `raw_payloads_embedded`: `false`
- `binary_payloads_embedded`: `false`
- `base64_payloads_embedded`: `false`
- `vector_payloads_embedded`: `false`
- `secrets_embedded`: `false`
- `production_ladybugdb_writes_claimed`: `false`

## Failure Modes
- Missing/malformed JSON or markdown inputs bubble as exit 2 for unreadable verifier inputs or exit 1 for contract diagnostics.
- Malformed evidence paths, unsupported suffixes, URL references, path traversal, and absent tracked evidence emit stable diagnostics.
- Planning-path existence checks are opt-in so default validation does not depend on gitignored .gsd/.planning/.audits files.

## Load Profile
- Expected load is one local matrix, one markdown twin, and ten requirement rows.
- At 10x rows, local filesystem reads and string scans saturate first and remain linear; no network, subprocess, database pool, or runtime service is introduced.

## Negative Tests
- Focused tests cover missing rows, duplicate rows, malformed evidence paths, skipped planning paths, required planning evidence opt-in, unsafe positive claims, unsafe true booleans, raw-payload field names, false validation of future requirements, false global validation of advanced requirements, R036 chain loss, stale markdown, and malformed JSON.

## Observability Impact
The verifier emits local validate-only diagnostics for requirement-scope ambiguity, safety-flag violations, stale renders, unsafe claims, raw-payload leakage markers, and evidence-path failures without touching runtime services.

## Verifier
- Command: `uv run python scripts/verify_m027_requirement_scope_reconciliation.py --validate-only`
- Planning evidence checks: opt-in via --require-planning-evidence for .gsd/.planning/.audits paths
