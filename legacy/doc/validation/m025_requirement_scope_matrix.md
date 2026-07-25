# M025 Requirement Scope Matrix

**Milestone:** M025-6xovy3  
**Slice:** S11 Requirements Scope Reconciliation  
**Source JSON:** `doc/validation/m025_requirement_scope_matrix.json`  
**Mode:** metadata-only coverage artifact; no raw article text, binary payloads, base64 payloads, vectors, secrets, or production connection details.

## Scope Boundary

M025 is a local article preprocessing/refactor milestone over a fixed five-article smoke corpus. It can claim preprocessing replay, boundary completion, metadata-safe evidence, provenance-oriented replay artifacts, no-network behavior, no-import/no-write flags, and readiness for larger **preprocessing** validation.

M025 must not claim scientific KG graph readiness, KG import readiness, production LadybugDB writes, 20-document validation, one-week validation, DSPy/RLM activation, optimizer behavior, semantic KG quality acceptance, or raw payload exposure.

## Requirement Rows

| Requirement | Canonical Status | M025 Applicability | S11 Verdict | Recommended Action |
|---|---:|---|---|---|
| R024 | active | `m025_advanced_preprocessing_only` | `advanced_not_validated` | `remain_active` |
| R027 | active | `m025_advanced_preprocessing_diagnostics` | `advanced_not_validated` | `remain_active` |
| R029 | active | `m025_advanced_traceable_chunks` | `advanced_not_validated` | `remain_active` |
| R030 | validated | `already_validated_supported_by_m025` | `covered_by_existing_validation_or_supported` | `stay_already_validated` |
| R036 | active | `m025_advanced_audit_provenance` | `advanced_not_fully_validated` | `remain_active` |
| R040 | active | `constraint_followed_not_validated` | `satisfied_as_constraint` | `treat_as_followed_constraint` |

## R024 — staged real-corpus validation

- **Canonical status:** active
- **M025 applicability:** `m025_advanced_preprocessing_only`
- **S11 verdict:** `advanced_not_validated`
- **Recommended action:** `remain_active`
- **Allowed claims:**
  - M025 advances R024 with fixed five-article local preprocessing smoke replay evidence.
  - M025 shows the fixed smoke corpus is ready for a larger preprocessing validation step.
  - M025 preserves no-network, no-import, no-production-write, and no-graph-readiness boundaries while advancing preprocessing evidence.
- **Forbidden claims:**
  - M025 fully validates R024.
  - M025 validates 10-document, 20-document, or one-week scientific KG behavior.
  - M025 validates scientific KG graph quality or graph readiness.
  - M025 authorizes KG import readiness or production LadybugDB writes.
  - M025 activates DSPy, RLM, MiniMax, or optimizer behavior.
- **Remaining work:** run staged real-article scientific KG behavior validation at the requirement's 10-document, 20-document, and one-week corpus scopes, with graph-quality analysis before validation.
- **Evidence paths:**
  - `.gsd/REQUIREMENTS.md`
  - `.gsd/milestones/M025-6xovy3/M025-6xovy3-CONTEXT.md`
  - `.gsd/milestones/M025-6xovy3/M025-6xovy3-ROADMAP.md`
  - `.gsd/milestones/M025-6xovy3/slices/S10/S10-SUMMARY.md`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/run-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline-recovery-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md`

## R027 — graph-readiness quality contract

- **Canonical status:** active
- **M025 applicability:** `m025_advanced_preprocessing_diagnostics`
- **S11 verdict:** `advanced_not_validated`
- **Recommended action:** `remain_active`
- **Allowed claims:**
  - M025 advances R027 with explicit preprocessing diagnostics, replay boundaries, redaction checks, and quality classifications.
  - M025 improves traceability for converted/preprocessed article data before graph-readiness acceptance.
- **Forbidden claims:**
  - M025 fully validates R027.
  - M025 accepts or validates positive graph readiness.
  - M025 authorizes KG validation/scaling, semantic KG import readiness, or production LadybugDB writes.
  - M025 activates DSPy or RLM behavior.
- **Remaining work:** run a dedicated graph-readiness quality benchmark or acceptance pass before KG validation or scaling resumes.
- **Evidence paths:**
  - `.gsd/REQUIREMENTS.md`
  - `.gsd/milestones/M025-6xovy3/M025-6xovy3-CONTEXT.md`
  - `.gsd/milestones/M025-6xovy3/slices/S10/S10-SUMMARY.md`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md`

## R029 — import-ready typed chunk package

- **Canonical status:** active
- **M025 applicability:** `m025_advanced_traceable_chunks`
- **S11 verdict:** `advanced_not_validated`
- **Recommended action:** `remain_active`
- **Allowed claims:**
  - M025 advances R029 with traceable chunk outputs for the fixed smoke corpus.
  - M025 records PageIndex/source-provenance-oriented preprocessing evidence and diagnostics for later import-readiness work.
- **Forbidden claims:**
  - M025 fully validates R029.
  - M025 validates import-ready typed chunk packages or KG import readiness.
  - M025 authorizes production KG import or production LadybugDB writes.
  - M025 promotes chunks to trusted KG facts.
  - M025 completes independent semantic review for import readiness.
- **Remaining work:** produce an import-ready typed chunk package only after independent semantic review and explicit import-readiness gates are satisfied.
- **Evidence paths:**
  - `.gsd/REQUIREMENTS.md`
  - `.gsd/milestones/M025-6xovy3/M025-6xovy3-CONTEXT.md`
  - `.gsd/milestones/M025-6xovy3/slices/S10/S10-SUMMARY.md`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md`

## R030 — source artifacts and derived metadata preservation

- **Canonical status:** validated
- **M025 applicability:** `already_validated_supported_by_m025`
- **S11 verdict:** `covered_by_existing_validation_or_supported`
- **Recommended action:** `stay_already_validated`
- **Allowed claims:**
  - R030 is already validated by prior M024 asset-preservation evidence.
  - M025 supports R030 by preserving source/artifact metadata and derived metadata through local preprocessing replay artifacts.
  - M025 keeps asset/table/link/identity evidence metadata-safe and review-oriented.
- **Forbidden claims:**
  - M025 newly validates R030 beyond the existing canonical validation.
  - M025 embeds raw article text, raw binary assets, base64 payloads, vectors, embeddings, tokens, secrets, or production connection details in coverage reports.
  - M025 authorizes graph import of assets or implements full multimodal retrieval.
- **Remaining work:** no M025 status change is needed; cite the existing validated status and supporting M025 metadata-only evidence.
- **Evidence paths:**
  - `.gsd/REQUIREMENTS.md`
  - `.gsd/milestones/M025-6xovy3/M025-6xovy3-CONTEXT.md`
  - `.gsd/milestones/M025-6xovy3/slices/S10/S10-SUMMARY.md`
  - `.gsd/milestones/M024-0xjwh9/slices/S08/S08-COVERAGE.md`
  - `doc/validation/m024_requirement_coverage_matrix.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/run-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md`

## R036 — replay/audit provenance logs

- **Canonical status:** active
- **M025 applicability:** `m025_advanced_audit_provenance`
- **S11 verdict:** `advanced_not_fully_validated`
- **Recommended action:** `remain_active`
- **Allowed claims:**
  - M025 advances R036 with replay/audit summaries, readiness metadata, reports, and boundary evidence for this corpus.
  - M025 provides provenance-oriented closeout evidence for local preprocessing replay.
- **Forbidden claims:**
  - M025 fully validates R036 for every validation CLI run in the project.
  - M025 proves all future validation CLI commands automatically emit exact command, inputs, output hashes, exit code, cwd, git commit, and active milestone/batch context.
  - M025 authorizes production LadybugDB writes, KG import readiness, graph readiness, DSPy behavior, or RLM behavior.
- **Remaining work:** verify full field-level provenance for relevant CLI surfaces before marking R036 fully validated.
- **Evidence paths:**
  - `.gsd/REQUIREMENTS.md`
  - `.gsd/milestones/M025-6xovy3/M025-6xovy3-CONTEXT.md`
  - `.gsd/milestones/M025-6xovy3/slices/S10/S10-SUMMARY.md`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline-recovery-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md`

## R040 — new infrastructure safety constraint

- **Canonical status:** active
- **M025 applicability:** `constraint_followed_not_validated`
- **S11 verdict:** `satisfied_as_constraint`
- **Recommended action:** `treat_as_followed_constraint`
- **Allowed claims:**
  - M025 followed R040 as a project constraint by remaining local-first, metadata-only, no-network, no-import, no-write, and redacted.
  - M025 did not enable new infrastructure in the main Scientific KG process.
- **Forbidden claims:**
  - M025 fully validates all future infrastructure safety wrapping.
  - M025 enabled a new production integration.
  - M025 authorizes production LadybugDB writes, KG import readiness, graph readiness, DSPy/RLM/MiniMax activation, or optimizer behavior.
- **Remaining work:** continue applying R040 to future infrastructure via research, compatibility probes, and safety wrappers before activation.
- **Evidence paths:**
  - `.gsd/REQUIREMENTS.md`
  - `.gsd/milestones/M025-6xovy3/M025-6xovy3-CONTEXT.md`
  - `.gsd/milestones/M025-6xovy3/slices/S10/S10-SUMMARY.md`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/run-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json`
  - `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md`

## Global Forbidden Claims Checklist

The JSON matrix explicitly forbids the unsafe closeout claims required by S11:

- graph readiness validation
- KG import readiness authorization
- production LadybugDB writes
- 20-document validation
- one-week validation
- DSPy activation
- RLM activation
- raw article payload exposure

## Negative Tests for Later Verifier

A later verifier should reject malformed JSON, missing required IDs, duplicate IDs, missing evidence paths, non-existent evidence paths, unsafe positive graph-readiness/KG-import/scale/DSPy/RLM/raw-payload claims, full-validation verdicts for R024/R027/R029, R030 downgrades, and R040 global-validation claims.
