# M027 Validation Remediation Class Audit

Source JSON: `doc/validation/m027_validation_remediation_class_audit.json`  
Schema: `m027-validation-remediation-class-audit.v1`

This fresh-reader audit is metadata-only. It supplies canonical validation-class remediation evidence for M027-aakeky/S08 without expanding the preprocessing-only milestone boundary.

## Criteria Source
- `.gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-ROADMAP.md`
- `doc/validation/m027_requirement_scope_matrix.json`
- `doc/validation/m027_requirement_scope_matrix.md`

## Remediation Target
- Round: `1`
- Goal: Provide canonical verification-class evidence for M027 validation without expanding the preprocessing-only milestone boundary.
- Supported remediation:
- Treat R024, R027, and R029 as M027-advanced preprocessing-only evidence, not closed requirements.
- Treat R036 as S07 closeout-chain preservation evidence while leaving the active requirement open for broader future workflows.
- Treat R019, R022, R023, R031, R032, and R033 as intentionally future/out-of-scope active requirements for M027 validation.

## Requirement Coverage Interpretation
M027-advanced but not globally validated rows are separate from future/out-of-scope active requirements. The source of truth is `doc/validation/m027_requirement_scope_matrix.json`.

| Requirement | Status | Classification | S08 verdict | Action |
|---|---|---|---|---|
| R019 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | `remain_active` |
| R022 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | `remain_active` |
| R023 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | `remain_active` |
| R024 | active | `m027_advanced_preprocessing_only` | `advanced_not_globally_validated` | `remain_active_with_m027_evidence_note` |
| R027 | active | `m027_advanced_preprocessing_only` | `advanced_not_globally_validated` | `remain_active_with_m027_evidence_note` |
| R029 | active | `m027_advanced_preprocessing_only` | `advanced_not_globally_validated` | `remain_active_with_m027_evidence_note` |
| R031 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | `remain_active` |
| R032 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | `remain_active` |
| R033 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated` | `remain_active` |
| R036 | active | `s07_closeout_validation_chain_preserved` | `validation_chain_preserved_not_globally_closed` | `preserve_s07_closeout_chain_and_keep_active` |

## Canonical Verification Classes
| Class | Verdict | Scope | Planned check | Evidence paths |
|---|---|---|---|---|
| Contract | PASS | Checks the S08 audit contract, matrix schema, canonical class names, safe flags, path shape, and markdown freshness using metadata-only files. | Run the M027 class-audit verifier and the requirement-scope verifier against local JSON and markdown artifacts. | doc/validation/m027_validation_remediation_class_audit.json, doc/validation/m027_requirement_scope_matrix.json, doc/validation/m027_requirement_scope_matrix.md, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-summary.json, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-report.md, .gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-ROADMAP.md |
| Integration | PASS | Connects roadmap criteria, the S08 matrix, S07 synthesis, and S06 provenance-riskratchet summaries without adding runtime integration. | Confirm referenced local source inputs exist or are accepted planning citations, and preserve S07 not_import_ready_validate_only evidence. | doc/validation/m027_validation_remediation_class_audit.json, doc/validation/m027_requirement_scope_matrix.json, doc/validation/m027_requirement_scope_matrix.md, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-summary.json, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-report.md, .gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-ROADMAP.md |
| Operational | PASS | Covers metadata-only validate-only failure diagnostics, safe false operational flags, no runtime surface, and local filesystem-only load behavior. | Check missing or malformed inputs bubble as stable diagnostics and all operational readiness expansion flags remain false. | doc/validation/m027_validation_remediation_class_audit.json, doc/validation/m027_requirement_scope_matrix.json, doc/validation/m027_requirement_scope_matrix.md, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-summary.json, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-report.md, .gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-ROADMAP.md |
| UAT | PASS | Gives the milestone validator a fresh-reader view that separates M027-advanced preprocessing rows from future active requirements. | Inspect the rendered audit and matrix to confirm supported wording, forbidden wording, and requirement interpretations are discoverable. | doc/validation/m027_validation_remediation_class_audit.json, doc/validation/m027_requirement_scope_matrix.json, doc/validation/m027_requirement_scope_matrix.md, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-summary.json, data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-report.md, .gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-ROADMAP.md |

## Rerun-Ready Validation Inputs
- `uv run python scripts/verify_m027_requirement_scope_reconciliation.py --validate-only`
- `uv run python scripts/verify_m027_validation_remediation.py --validate-only`

## Safe Validation Wording
- M027 S08 supplies metadata-only validation remediation evidence.
- M027-advanced rows remain active until a future milestone supplies direct canonical closure evidence.
- Future/out-of-scope active requirements are intentionally excluded from M027 success criteria.
- Canonical verification classes PASS only for scoped validation-remediation artifacts and rerun-ready local checks.

## Forbidden Claims
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
- M027 globally validates R024, R027, or R029
- M027 fully validates R019, R022, R023, R031, R032, R033, or R036
- M027 converts canonical PASS rows into graph, import, trusted fact, production, optimizer, scientific KG, or scale acceptance

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
- `production_import_attempted`: `false`
- `ladybugdb_written`: `false`
- `trusted_fact_promotion_allowed`: `false`
- `graph_import_allowed`: `false`

## Failure Modes
- Missing or malformed JSON/Markdown inputs return exit 2 for unreadable verifier inputs or exit 1 for contract diagnostics.
- Unsafe paths, unsupported suffixes, URL references, path traversal, stale markdown, and source drift emit stable diagnostics.
- Planning-path existence checks remain opt-in so default validation does not depend on gitignored .gsd files.

## Load Profile
- Expected load is one local class audit, one requirement matrix, one roadmap citation, and S07/S06 metadata evidence.
- At 10x rows or class evidence paths, local filesystem reads and string scans saturate first and remain linear.
- No network calls, services, database pools, background workers, dashboards, or telemetry streams are introduced.

## Negative Tests
- Verifier guards missing/extra canonical classes, non-PASS verdicts, unsafe safety flags, matrix semantic drift, stale markdown, malformed path shape, unsafe positive claims, and raw payload leakage markers.
- Requirement matrix verifier remains the negative contract source for malformed requirement rows, future/out-of-scope false validation, R036 chain loss, and planning-evidence opt-in behavior.

## Remaining Work
- Run milestone validation with these canonical class rows as round-1 remediation evidence.
- Keep broad active requirements open unless later milestones provide direct validation evidence.
- Add separate import eligibility, graph readiness, trusted fact, production write, and unattended scale gates before any future readiness claim.

## Observability Impact
Adds validate-only diagnostics naming exact JSON paths, canonical classes, requirement IDs, stale markdown markers, unsafe flags, and source consistency failures; no runtime telemetry surface changes.
