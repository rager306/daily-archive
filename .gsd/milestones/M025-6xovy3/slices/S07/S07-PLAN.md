# S07: Assets Tables Links Identity Replay

**Goal:** Separate assets, tables, links, identity, and evidence metadata from chunk text while replaying the same fixed corpus.
**Demo:** After this: assets, tables, links, and identity evidence are separated from chunks and replayed on the same corpus with metadata-safe provenance-bearing outputs.

## Must-Haves

- Assets, tables, links, and identity are represented as separate metadata-safe artifacts.
- Outputs preserve provenance to article/source/element/chunk identifiers.
- Missing or unsupported evidence types are reported as diagnostics, not silent absence.
- Diagnostics do not embed raw payload text or binary data.
- No artifact marks production import or graph readiness true.

## Threat Surface

## Q3 abuse and exposure analysis

### Abuse scenarios
- **Parameter tampering / path traversal:** `--catalog`, `--index`, `--selection`, `--chunks`, `--evidence`, `--events`, `--write-summary`, and `--write-report` are all filesystem parameters; the implementation should constrain expected artifact shapes, fail on missing S06 chunking inputs, avoid following unsafe symlinks when writing outputs, and never infer trust merely from a user-provided directory.
- **Replay/staleness attacks:** Because the slice replays fixed-corpus artifacts, stale S06 chunks or mismatched catalog/index/selection files could produce apparently valid evidence reports for the wrong corpus. Validation should tie outputs to article/source/chunk identifiers, input hashes or run metadata where available, and explicit event provenance.
- **Flag drift / privilege escalation into KG readiness:** Artifact fields such as production import, graph readiness, LadybugDB writes, trusted fact promotion, or import eligibility must remain false/zero even if input artifacts contain conflicting flags.
- **Silent unsupported evidence:** Unsupported assets/tables/links/identity types must generate diagnostics rather than empty success outputs, otherwise missing evidence could be hidden and later consumed as complete.

### Data exposure risks
- Evidence artifacts and diagnostics may reference article, source, element, and chunk identifiers but must not embed raw article text, table cell text, image bytes, base64 payloads, vectors, embeddings, tokens, secrets, or raw LLM/provider responses.
- Identity/dedup evidence is especially sensitive to accidental over-capture: it should preserve stable IDs and safe metadata only, not author/contact payloads beyond existing safe article metadata contracts.

### Trust boundaries
- Inputs are local files, but they are still untrusted at the validation boundary: catalog/index/selection/chunking/evidence artifacts can be stale, malformed, or maliciously edited.
- Outputs cross from parsing/replay code into committed or reviewable filesystem artifacts; redaction and no-import/no-write assertions must be enforced at write time and again in the final report verification.

### Required mitigations before/while implementing
- Validate schemas for each evidence artifact family and reject forbidden payload keys/large raw fields.
- Emit diagnostics for missing/unsupported evidence types with stable codes and provenance pointers.
- Require no-import/no-write flags in every artifact and in summary/report validation.
- Record counts, provenance coverage, redaction checks, and event lineage sufficient to detect stale or mismatched replay inputs.

## Requirement Impact

## Q4 requirement impact analysis

The milestone-specific requirements artifact requested in the prompt (`.gsd/milestones/M025-6xovy3/REQUIREMENTS.md`) is not present, so this assessment uses the milestone context's "Relevant Requirements" section plus the root `.gsd/REQUIREMENTS.md` definitions.

### Touched requirements
- **R024** — S07 advances staged real-corpus validation indirectly by proving separated evidence artifacts on the fixed smoke corpus, but it must not be treated as 20-document, one-week, or graph-quality validation.
- **R027** — Directly touched: table/figure handling, section/chunk provenance, evidence boundaries, and diagnostics are part of the graph-readiness quality contract, while S07 remains pre-readiness.
- **R029** — Directly touched: chunk-adjacent evidence separation must preserve stable article/source/element/chunk identifiers and avoid mixing assets/tables/links/identity into chunk text.
- **R030** — Directly touched and should be regression-tested: source artifact preservation, extracted figures/tables/image assets, hashes/provenance, and redacted asset manifests must remain metadata-safe.
- **R036** — Directly touched by replay/event/report outputs: evidence replay should produce enough command/input/output provenance to detect stale artifacts and mismatched corpus lineage.
- **R040** — Touched as a safety constraint: S07 must remain local-first and must not enable new infrastructure, external services, production writes, or graph import.
- **R050** — Directly touched: S07 is a deterministic pre-KG artifact/evidence replay step for article structure artifacts, links, reviewable manifests, provenance, and explicit no-import state.
- **R051** — Retest as a negative/gating requirement if any MiniMax-derived fields, prior outputs, or helper-compatible contracts are referenced: outputs must remain non-authoritative, locally validated, and redacted; S07 should not invoke MiniMax.
- **R052** — Retest as a negative/gating requirement if metrics or optimizer wording appears: S07 must not activate DSPy/prompt optimization or imply optimizer readiness.

### Must re-test after shipping S07
- Contract tests for separated assets/tables/links/identity artifacts, including schema, provenance pointers, forbidden-payload rejection, and no-import/no-write flags.
- Smoke replay command against the fixed M025 corpus with catalog/index/selection/chunking inputs and evidence/event outputs.
- Final validation/report mode with `--require-redaction` and `--require-no-import-flags`.
- Regression checks for R030 asset/table/link preservation and redaction, especially no raw article text, table payloads, binary/image data, base64, vectors, embeddings, tokens, secrets, or raw responses in diagnostics/reports.
- Provenance/staleness checks tying evidence artifacts to the selected article corpus and S06 chunk identifiers.
- Negative checks that no artifact claims KG import, graph readiness, production LadybugDB writes, trusted fact promotion, MiniMax authority, or DSPy optimizer activation.

### Decisions to revisit
- No architectural decision requires reversal, but the M025 decisions "Same Corpus Replay At Every Step" and "Preprocessing Only, No Graph Import" must be explicitly upheld in S07 evidence and report outputs.
- If S07 introduces broader artifact detection semantics beyond metadata-safe replay, revisit the pre-KG artifact detection boundary under R050/R051/R052 before expanding scope.

## Proof Level

- This slice proves: Same-corpus evidence-boundary replay with metadata-safe artifact inspection.

## Integration Closure

S08 consumes chunk outputs plus separated evidence artifacts to run the full local preprocessing replay without crossing into KG import.

## Verification

- Records asset/table/link/identity counts, extraction outcomes, provenance pointers, missing evidence diagnostics, and redaction checks.

## Tasks

- [x] **T01: Defined metadata-safe separated evidence boundary fixtures for assets, tables, links, and identity with executable contract tests.** `est:medium`
  Define the separated metadata-safe evidence artifact contracts for assets, tables, links, and identity. The contracts must reference article/source/element/chunk identifiers without embedding raw payload text or binary data, and must keep graph import and production write flags false. At execution time this task consumes S01 catalog/index/selection outputs and S06 chunking outputs, but those future artifacts are intentionally not listed as static inputs for pre-execution validation.
  - Files: `tests/test_article_evidence_boundaries.py`, `tests/fixtures/article_evidence_boundaries_v00_01/`
  - Verify: uv run pytest tests/test_article_evidence_boundaries.py -q
uv run ruff check tests/test_article_evidence_boundaries.py

- [x] **T02: Replayed separated metadata-safe evidence artifacts for the fixed five-article corpus with per-article assets, tables, links, identity, and event logs.** `est:medium`
  Implement or adapt a local evidence replay command that reads the fixed corpus outputs from S06 and writes separate assets, tables, links, and identity artifacts per article. Unsupported evidence types must produce diagnostics rather than silent empty outputs. The command must read the catalog index and corpus selection at runtime and fail clearly if expected S06 chunking artifacts are absent.
  - Files: `src/arxiv_archive/`, `scripts/verify_m025_evidence_boundaries.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_evidence_boundaries.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --chunks data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/chunking --evidence data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence --write-events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-events.jsonl

- [x] **T03: Finalized the S07 evidence boundary report and machine-readable summary with metadata-safe counts, provenance, redaction, diagnostics, and fail-closed safety validation.** `est:small`
  Validate the separated evidence artifacts and write the S07 report. The report must summarize per-article counts, missing/unsupported evidence diagnostics, provenance coverage, redaction checks, and no-import/no-write safety state.
  - Files: `scripts/verify_m025_evidence_boundaries.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_evidence_boundaries.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --evidence data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence --events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-events.jsonl --require-redaction --require-no-import-flags --write-summary data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-summary.json --write-report data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-report.md

## Files Likely Touched

- tests/test_article_evidence_boundaries.py
- tests/fixtures/article_evidence_boundaries_v00_01/
- src/arxiv_archive/
- scripts/verify_m025_evidence_boundaries.py
- data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/
