# M024 Requirement Coverage Matrix

- Milestone: `M024-0xjwh9`
- Slice: `S08`
- Schema: `m024-requirement-coverage-matrix.v1`

## Scope Boundary

M024 is a metadata-only, review-only article evidence validation milestone over a clean 10-document fixture and related local contracts.

### In scope
- S01-S07 evidence for local article loader, evidence bridge, PageIndex, assets, links/dedup, retrieval/table benchmark summaries, and 10-document batch validation.
- Requirement applicability classification for milestone validation rerun.
- S09 handoff gaps for weak or missing validation evidence, including riskratchet.

### Out of scope
- 20-document or one-week corpus validation execution.
- Positive graph-readiness acceptance, KG import, production LadybugDB writes, promoted scientific facts, embeddings, vector payloads, DSPy/RLM activation, optimizer behavior, MiniMax helper activation, or heavy multimodal extraction.
- 30-paper, 100-paper, or future M023 artifact-detection workflow validation.

Safe next step: Use this matrix as S09 validation input; do not widen M024 scope to satisfy unrelated active requirements.

## Requirement Rows

| Requirement | Status | Applicability | Verdict | Evidence | S09 Follow-up |
|---|---|---|---|---|---|
| R019 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R022 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R023 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R024 | active | in_scope_advanced_partial | advanced_not_validated | `.gsd/milestones/M024-0xjwh9/slices/S01/S01-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S02/S02-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S03/S03-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S04/S04-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S05/S05-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S06/S06-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S07/S07-SUMMARY.md`<br>`tests/fixtures/article_batch_validation/ten_document_manifest.json`<br>`src/arxiv_archive/article_batch_validation.py` | During milestone validation, cite the 10-document report as partial advancement only and leave R024 active for 20-document, one-week, and graph-quality evidence. |
| R027 | active | in_scope_advanced_partial | advanced_not_validated | `.gsd/milestones/M024-0xjwh9/slices/S01/S01-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S02/S02-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S03/S03-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S04/S04-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S05/S05-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S06/S06-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S07/S07-SUMMARY.md` | Milestone validation should describe R027 as advanced by diagnostics/metadata contracts, not validated as a positive graph-readiness gate. |
| R029 | active | in_scope_advanced_partial | advanced_not_validated | `.gsd/milestones/M024-0xjwh9/slices/S03/S03-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S04/S04-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S05/S05-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S06/S06-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S07/S07-SUMMARY.md` | Keep R029 active and cite M024 only as partial lineage/provenance advancement. |
| R030 | validated | already_validated_covered_by_m024_s04 | covered_by_existing_validation | `.gsd/REQUIREMENTS.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S04/S04-SUMMARY.md`<br>`src/arxiv_archive/article_assets.py`<br>`tests/test_article_assets.py`<br>`tests/test_property_article_assets.py` | S09 can cite this row directly as existing requirement coverage. |
| R031 | active | out_of_scope_future | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R032 | active | out_of_scope_future | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R033 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R035 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R036 | active | in_scope_evidence_backed_candidate | covered_by_existing_validation | `.gsd/REQUIREMENTS.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S01/S01-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S02/S02-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S06/S06-SUMMARY.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S07/S07-SUMMARY.md`<br>`src/arxiv_archive/validation_batch_provenance.py`<br>`src/arxiv_archive/article_batch_validation.py`<br>`src/arxiv_archive/cli.py`<br>`tests/test_validation_batch_provenance.py`<br>`tests/test_validation_batch_cli_article_report.py`<br>`tests/fixtures/article_batch_validation/ten_document_manifest.json` | No implementation clause remains open for the M024 article-report provenance surface; if milestone validation requires canonical requirement status parity, update R036 through the GSD requirement tool rather than editing REQUIREMENTS.md directly. |
| R040 | active | out_of_scope_project_constraint_followed | constraint_respected_not_validated | `.gsd/REQUIREMENTS.md`<br>`.gsd/milestones/M024-0xjwh9/slices/S07/S07-SUMMARY.md` | — |
| R050 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R051 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |
| R052 | active | out_of_scope_other_milestone | not_applicable_to_m024 | `.gsd/REQUIREMENTS.md` | — |

## Detailed Rationale

### R019

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: Hybrid retrieval with vector/graph/fusion score metadata is owned by M003 S06/S10. M024 S06/S07 only produced metadata-only article retrieval/table benchmark summaries and did not make hybrid retrieval-quality claims.
- Allowed claims:
  - M024 preserved the no-embedding/no-vector safety boundary.
- Forbidden claims:
  - M024 validates hybrid retrieval quality
  - M024 validates vector/graph fusion retrieval

### R022

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: RLM document navigation and workflow-in-code prototypes remain M003 S09 work. M024 explicitly avoided RLM/tool trajectory behavior.
- Allowed claims:
  - M024 did not activate RLM behavior.
- Forbidden claims:
  - M024 validates RLM document navigation
  - M024 validates workflow-in-code prototypes

### R023

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: Adaptive RLM graph traversal benchmarking is owned by M003 S10. M024 has no traversal benchmark, tool usage, cost/latency, or candidate-set comparison surface.
- Allowed claims:
  - M024 preserved no-RLM/no-optimizer boundaries.
- Forbidden claims:
  - M024 validates adaptive graph traversal
  - M024 benchmarks RLM traversal

### R024

- Current status: `active`
- M024 applicability: `in_scope_advanced_partial`
- Coverage verdict: `advanced_not_validated`
- Rationale: M024 delivered a deterministic 10-document metadata-only article evidence validation loop with clean fixture semantics and a review-only recommendation for 20-document scale. It does not execute the 20-document stage, one-week corpus stage, or graph-quality analysis requested by the full requirement.
- Allowed claims:
  - M024 advances the 10-document review-only validation surface for R024.
  - M024 recommends a safe review-only 20-document next step.
- Forbidden claims:
  - M024 fully validates R024
  - M024 validates 20-document graph quality
  - M024 validates one-week corpus graph quality
  - M024 authorizes KG import

### R027

- Current status: `active`
- M024 applicability: `in_scope_advanced_partial`
- Coverage verdict: `advanced_not_validated`
- Rationale: M024 adds conversion/source provenance, PageIndex hierarchy, asset/table/link/retrieval metadata, diagnostics, and 10-document aggregation. It remains review-only and does not provide positive graph-readiness quality acceptance before KG validation or scaling resumes.
- Allowed claims:
  - M024 advances graph-readiness evidence diagnostics for source, structure, assets, links, retrieval, and table candidates.
- Forbidden claims:
  - M024 validates positive graph readiness
  - M024 authorizes KG validation to resume
  - M024 authorizes scaling beyond review-only evidence

### R029

- Current status: `active`
- M024 applicability: `in_scope_advanced_partial`
- Coverage verdict: `advanced_not_validated`
- Rationale: M024 contributes stable IDs, source spans, PageIndex anchors, asset/link/retrieval/table provenance, and review-only status rows. It does not produce an import-ready typed chunk package with independent semantic acceptance for KG import.
- Allowed claims:
  - M024 advances R029 lineage/provenance evidence for later import-readiness work.
- Forbidden claims:
  - M024 validates import-ready chunks
  - M024 validates positive KG import readiness
  - M024 promotes chunks to trusted KG facts

### R030

- Current status: `validated`
- M024 applicability: `already_validated_covered_by_m024_s04`
- Coverage verdict: `covered_by_existing_validation`
- Rationale: R030 is already marked validated with M024 S04 evidence for metadata-only asset preservation, bridge integration, stable IDs/provenance/status summaries, and forbidden payload validation. S08 should cite this coverage rather than reopen it.
- Allowed claims:
  - R030 is validated by M024 S04 metadata-only asset preservation evidence.
- Forbidden claims:
  - R030 implies raw image bytes are preserved in the bridge
  - R030 implies multimodal retrieval is implemented

### R031

- Current status: `active`
- M024 applicability: `out_of_scope_future`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: R031 concerns a 30-paper deviation scan. M024 stops at a 10-document review-only validation loop and recommendation for later 20-document review.
- Allowed claims:
  - M024 does not draw 30-paper conclusions.
- Forbidden claims:
  - M024 validates the 30-paper deviation scan

### R032

- Current status: `active`
- M024 applicability: `out_of_scope_future`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: R032 requires automated +10-paper iterative loop support toward 100 papers. M024 did not implement resumable multi-batch automation.
- Allowed claims:
  - M024 recommends one future review-only scale step but does not automate iterative 100-paper loops.
- Forbidden claims:
  - M024 validates 100-paper iterative automation

### R033

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: R033 is the deterministic iterative +10-paper validation batch workflow owned by M007. M024 consumes local fixture manifest/report patterns but does not replace that workflow.
- Allowed claims:
  - M024 does not change R033 status.
- Forbidden claims:
  - M024 validates the M007 iterative validation-batch workflow

### R035

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: R035 is about deterministic replacement candidates for accepted-paper quota filling. M024 uses a fixed 10-document fixture and does not implement acquisition/top-up integration.
- Allowed claims:
  - M024 fixture validation does not satisfy replacement materialization.
- Forbidden claims:
  - M024 validates bounded top-up acquisition integration

### R036

- Current status: `active`
- M024 applicability: `in_scope_evidence_backed_candidate`
- Coverage verdict: `covered_by_existing_validation`
- Rationale: M024 S07/S08 evidence satisfies the R036 executable provenance contract for the article-report validation CLI surface: the real command emits provenance and freshness artifacts tying generated report/diagnostics outputs to the redacted argv/command, input and output fingerprints, cwd, git commit, start/completion/duration, exit code, batch/run context, expected artifact metadata, and JSONL round trip/verification behavior. Focused provenance and article-report CLI tests cover secret redaction, unchanged/missing/mutated/unsafe artifacts, metadata mismatch detection, malformed JSON handling, blocked report artifacts, and fresh CLI output. The canonical requirement row still shows active because this execution runtime did not expose the DB-backed gsd_requirement_update tool; S08 did not manually edit REQUIREMENTS.md.
- Allowed claims:
  - M024 provides direct R036 validation evidence for the metadata-only article-report validation CLI provenance surface.
  - The article-report CLI automatically emits provenance and freshness artifacts for generated report/diagnostics outputs.
  - R036 status reconciliation should be performed only through the GSD requirement tool when that tool is available.
- Forbidden claims:
  - S08 manually edited REQUIREMENTS.md
  - R036 validates unrelated future validation-batch commands outside M024 evidence
  - R036 authorizes KG import, production writes, embeddings, vectors, or unattended scaling

### R040

- Current status: `active`
- M024 applicability: `out_of_scope_project_constraint_followed`
- Coverage verdict: `constraint_respected_not_validated`
- Rationale: R040 is a project-wide infrastructure safety principle. M024 followed the principle by not enabling new infrastructure or production import paths; it did not introduce a new infrastructure activation that would validate R040 itself.
- Allowed claims:
  - M024 respects R040 by keeping new infrastructure and unsafe runtime activation out of scope.
- Forbidden claims:
  - M024 validates all future infrastructure safety wrapping

### R050

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: R050 belongs to M023 article artifact detection CLI work. M024 preserves metadata-only article evidence but does not implement the artifact detection CLI for KG scaffold links.
- Allowed claims:
  - M024 remains compatible with future artifact-detection work by preserving review-only boundaries.
- Forbidden claims:
  - M024 validates R050 artifact detection CLI

### R051

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: R051 is bounded MiniMax helper integration behind explicit flags. M024 does not use MiniMax or any external helper in the article validation path.
- Allowed claims:
  - M024 did not activate MiniMax helper behavior.
- Forbidden claims:
  - M024 validates MiniMax artifact detection integration

### R052

- Current status: `active`
- M024 applicability: `out_of_scope_other_milestone`
- Coverage verdict: `not_applicable_to_m024`
- Rationale: R052 is the M023 DSPy optimizer/benchmark gating requirement. M024 preserves the no-DSPy/no-optimizer boundary and does not create artifact-detection benchmark fixtures for DSPy activation.
- Allowed claims:
  - M024 does not activate DSPy or optimizer behavior.
- Forbidden claims:
  - M024 validates DSPy prompt optimization readiness for artifact detection

## S09 Handoff Gaps

### S09-GAP-riskratchet-direct-evidence: riskratchet diagnostic evidence is absent from code/test artifacts

- Severity: `validation_evidence_gap`
- Required before validation rerun: `True`
- Description: S08 research found no repo/dependency references to riskratchet beyond roadmap/activity text. S09 should either provide direct diagnostic evidence for the non-blocking riskratchet criterion or explicitly mark it not applicable/non-blocking in milestone validation.
- Evidence:
  - `.gsd/milestones/M024-0xjwh9/slices/S08/S08-RESEARCH.md`

### S09-GAP-r036-requirement-update-decision: R036 canonical requirement status reconciliation

- Severity: `requirement_status_reconciliation`
- Required before validation rerun: `False`
- Description: S08 verified that M024 article-report CLI provenance satisfies R036 for the in-scope executable surface, but REQUIREMENTS.md remains active because the DB-backed gsd_requirement_update tool was not exposed in this execution runtime. Milestone validation may cite the matrix as implementation evidence, and any canonical status/note change must be made through the GSD requirement tool, not by manual file edit.
- Evidence:
  - `src/arxiv_archive/validation_batch_provenance.py`
  - `src/arxiv_archive/article_batch_validation.py`
  - `src/arxiv_archive/cli.py`
  - `tests/test_validation_batch_provenance.py`
  - `tests/test_validation_batch_cli_article_report.py`

## Review Notes

- R024, R027, and R029 are intentionally partial/advanced, not validated by M024.
- R030 is already validated by M024 S04 and should be cited as covered.
- R036 is covered by M024 article-report provenance evidence; REQUIREMENTS.md remains active only because this runtime did not expose DB-backed requirement-update tooling.
- M003, M023, 30-paper, and 100-paper requirements are explicitly out of M024 validation scope.
