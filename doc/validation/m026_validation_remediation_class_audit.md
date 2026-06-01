# M026 Validation Remediation Class Audit

> Source of truth: `doc/validation/m026_validation_remediation_class_audit.json`

## Remediation Target

- Validation report: `.gsd/milestones/M026-3rvvgp/M026-3rvvgp-VALIDATION.md`
- Current validation verdict: `needs-remediation`
- Target: Resolve validation rerun inputs by reconciling requirement coverage with S05 semantics and explicitly auditing Contract, Integration, Operational, and UAT verification classes.

## Criteria Source

- Canonical success criteria source: `.gsd/milestones/M026-3rvvgp/M026-3rvvgp-ROADMAP.md`
- Decision: Use the milestone roadmap as the canonical success-criteria source for validation rerun; no separate CONTEXT artifact is restored by this metadata-only remediation task.
- Mismatch rule: When validation prose conflicts with S05 matrix semantics, trust the S05 matrix and this remediation interpretation.

### Roadmap Success Criteria
- source-capability matrix exists for arXiv API, abstract pages, PDF download, and practical source paths
- Hermes-agent digest is specified separately from loader raw evidence bundle
- universal loader contract exists with input kinds, provenance fields, fallback chain, and quality statuses
- contract distinguishes discovery metadata from content acquisition and downstream digest projection
- small real-source validation confirms research and identifies implementation gaps before implementation

## Scope Boundary

M026 is a loader research, Hermes digest contract, universal loader contract, and real-source validation milestone. It supplies conservative evidence and boundary language only; it does not implement the loader, batch selection, KG import/readiness, graph validation, corpus validation, or AI helper activation.

### In Scope
- arXiv source-capability research and sample validation evidence.
- Hermes digest contract boundaries and metadata-only redaction requirements.
- Universal loader contract boundaries, compatibility notes, failure modes, and future implementation gaps.
- Requirement-scope reconciliation for all active requirements plus touched validated/context requirements.

### Out of Scope
- loader implementation
- batch selection implementation
- KG import or import readiness
- graph validation or graph readiness acceptance
- import-ready chunks
- Scientific KG corpus validation
- DSPy, RLM, MiniMax, or optimizer activation
- raw article text, PDF bytes, binary payloads, vector payloads, secrets, or production connection details
- production LadybugDB writes

## Requirement Coverage Interpretation

| Requirement | Status | Classification | S05 verdict | Remediation interpretation |
|---|---|---|---|---|
| R001 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R002 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R003 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R004 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R005 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R006 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R007 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R008 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R009 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R010 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R014 | validated | `existing_validated_compatibility_context` | `existing_coverage_supported_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |
| R019 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R022 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R023 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R024 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R027 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R029 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R031 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R032 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R033 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R035 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R036 | active | `adjacent_evidence_not_full_requirement` | `advanced_not_validated` | Adjacent artifact-level traceability was advanced, but full validation CLI provenance coverage remains active and unvalidated. |
| R040 | active | `in_scope_constraint_followed` | `satisfied_for_m026_not_globally_validated` | R040 was followed as a milestone-local research/probe/safety-wrap constraint for M026 only, not globally validated for all infrastructure. |
| R050 | active | `out_of_scope_future_consumer` | `not_implemented_not_validated` | R050 is a future consumer of M026 contracts; no deterministic artifact-detection CLI, scaffold links, review state, or import-disallowed output was implemented. |
| R051 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R052 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | Active and out of scope for M026; not missing touched coverage. |
| R030 | validated | `existing_validated_supporting_context` | `existing_coverage_supported_not_revalidated` | Historical validated or compatibility context only; not newly validated by M026. |

## Canonical Verification Classes

| Class | Verdict | Scope | Planned check | Evidence paths |
|---|---|---|---|---|
| Contract | PASS | M026 research, contract, and metadata-only evidence only | S01 source-capability matrix, S02 Hermes digest contract, S03 universal loader contract, and S04 real-source validation all exist and preserve loader/digest/selection boundaries. | `.gsd/reports/article-artifact-loader-research.md`<br>`.gsd/reports/article-artifact-loader-digest-contract.md`<br>`.gsd/reports/article-artifact-universal-loader-contract.md`<br>`.gsd/reports/article-artifact-loader-validation.md`<br>`.gsd/milestones/M026-3rvvgp/M026-3rvvgp-ROADMAP.md` |
| Integration | PASS | Boundary integration between research artifacts only; no runtime integration or production import path. | Discovery metadata, PDF/content acquisition, Hermes digest projection, universal loader evidence, and future batch selection remain separable. | `.gsd/reports/article-artifact-loader-digest-contract.md`<br>`.gsd/reports/article-artifact-universal-loader-contract.md`<br>`.gsd/reports/article-artifact-loader-validation.md`<br>`.gsd/milestones/M026-3rvvgp/M026-3rvvgp-VALIDATION.md` |
| Operational | PASS | Local-first documentation, reports, and verifier evidence only; no service runtime load or production operation. | Artifacts are local, metadata-only, provenance-supporting, and research-first; failure modes and load boundaries are documented without introducing shared services. | `.gsd/reports/article-artifact-loader-research.md`<br>`.gsd/reports/article-artifact-loader-validation.md`<br>`.gsd/milestones/M026-3rvvgp/slices/S05/S05-COVERAGE.md`<br>`doc/validation/m026_requirement_scope_matrix.json` |
| UAT | PASS | Small real-source sample and human-reviewable contract validation only; no corpus-scale acceptance. | Small real arXiv records were checked against source capability, PDF metadata-only handling, digest separation, and universal-loader contract assumptions. | `.gsd/reports/article-artifact-loader-validation.md`<br>`.gsd/reports/article-artifact-loader-research.md`<br>`.gsd/milestones/M026-3rvvgp/M026-3rvvgp-VALIDATION.md` |

## Rerun-Ready Validation Inputs

- Success criteria source: `.gsd/milestones/M026-3rvvgp/M026-3rvvgp-ROADMAP.md`
- Requirement coverage source: `doc/validation/m026_requirement_scope_matrix.json`
- Expected requirement rows checked: `27`

### Commands

```bash
uv run python -m json.tool doc/validation/m026_validation_remediation_class_audit.json
uv run python scripts/verify_m026_requirement_scope_reconciliation.py --matrix doc/validation/m026_requirement_scope_matrix.json --rendered doc/validation/m026_requirement_scope_matrix.md --coverage .gsd/milestones/M026-3rvvgp/slices/S05/S05-COVERAGE.md --require-active-scope --reject-unsafe-claims
uv run pytest tests/test_m026_requirement_scope_reconciliation.py -q
```

## Safe Validation Wording
- M026 researched arXiv source capabilities and loader-boundary assumptions.
- M026 separated Hermes digest projection from universal loader evidence.
- M026 froze metadata-only contract boundaries and implementation gaps before enabling loader infrastructure.
- M026 performed a small real-source validation pass against contract assumptions.
- M026 followed R040 for this milestone only as a research/probe/safety-wrap constraint.
- M026 advanced R036-adjacent artifact traceability but did not fully validate R036.
- R050 remains a future consumer of M026 contracts and was not implemented.
- Unrelated active requirements remain active and out of scope rather than missing touched coverage.

## Forbidden Claims
- M026 implements the loader
- M026 implements batch selection
- M026 authorizes KG import
- M026 validates KG import readiness
- M026 validates graph readiness
- M026 validates Scientific KG corpus behavior
- M026 validates import-ready chunks
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
- M026 embeds raw article text, PDFs, binary payloads, vector payloads, secrets, or production connection details
- M026 writes to production LadybugDB
- M026 fully validates R036
- M026 globally validates R040
- M026 implements R050
- M026 newly validates R001-R010, R014, or R030
- M026 closes unrelated active requirements as validated

## Safety Flags
- `loader_implementation_claimed`: `false`
- `batch_selection_implementation_claimed`: `false`
- `kg_import_or_readiness_claimed`: `false`
- `graph_validation_claimed`: `false`
- `import_ready_chunks_claimed`: `false`
- `scientific_kg_corpus_validation_claimed`: `false`
- `dspy_rlm_minimax_activation_claimed`: `false`
- `raw_payloads_embedded`: `false`
- `binary_payloads_embedded`: `false`
- `vector_payloads_embedded`: `false`
- `secrets_embedded`: `false`
- `production_ladybugdb_writes_claimed`: `false`
- `metadata_only`: `true`
- `runtime_surface_changed`: `false`
- `loader_implementation_added`: `false`
- `graph_or_kg_import_authorized`: `false`
- `raw_article_text_embedded`: `false`
- `pdf_bytes_embedded`: `false`
- `production_connection_details_embedded`: `false`

## Failure Modes
- Filesystem dependency: required input reports may be missing; rerun validation must fail with named missing paths rather than infer coverage.
- JSON dependency: malformed remediation JSON must fail `python -m json.tool` before any class-audit input is trusted.
- Markdown render dependency: stale rendered Markdown must be detected by comparing class names, requirement IDs, criteria source, safety flags, and forbidden language with JSON.
- No network, API, database, production service, raw article payload, PDF byte, vector payload, or secret dependency is introduced by this static artifact.

## Load Profile
- There is no runtime service load dimension; the artifact is static JSON/Markdown.
- At 10x requirement/class rows, human review attention saturates before compute; protection is machine-readable row structure, exact class names, source paths, and deterministic verifier inputs.

## Negative Tests
- Future verifier must reject a missing class, wrong class name, positive R050 implementation claim, global R040 validation claim, full R036 provenance claim, stale Markdown, unsafe safety-flag drift, and unsafe evidence paths.
- This task enables those tests by providing canonical JSON fields and expected forbidden language; verifier implementation belongs to a later S06 task.

## Remaining Work
- Run the S06 verifier once implemented to check JSON paths, class names, unsafe phrases, stale Markdown, criteria-source mismatches, and safety-flag drift.
- Rerun milestone validation using this remediation artifact as the class-audit and requirement-coverage input.
- Future requirements remain governed by the S05 matrix remaining-work rows.

## Observability Impact
This remediation package creates deterministic validation diagnostics for requirement classifications, class-audit rows, evidence paths, criteria-source choice, safety flags, forbidden claims, and rerun commands without changing runtime behavior.
