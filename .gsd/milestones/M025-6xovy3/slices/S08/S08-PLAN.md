# S08: End to End Preprocessing Replay

**Goal:** Run the complete refactored preprocessing pipeline end to end on the fixed 5 article local corpus and produce final comparison, readiness, and blocker reports.
**Demo:** After this: the full refactored preprocessing pipeline runs on the same smoke corpus, writes final per-article artifacts, compares against baseline, and states whether larger preprocessing validation is ready or blocked.

## Must-Haves

- Full refactored preprocessing pipeline runs locally from catalog/corpus inputs.
- Final per-article artifacts and metrics are persisted.
- Comparison report explains preserved, improved, regressed, and blocked behaviors.
- No network fetch occurs during replay.
- No graph import, production LadybugDB write, or graph readiness acceptance is claimed.

## Threat Surface

## Q3 exploit/abuse assessment

### Trust boundaries
- CLI parameters (`--catalog`, `--index`, `--selection`, `--baseline`, `--final`, `--events`, `--write-summary`, `--write-report`, `--write-decision`) cross from operator-controlled input into filesystem reads/writes.
- Catalog/index/selection JSON are local artifacts, but should be treated as untrusted because tampered article IDs, relative paths, symlinks, or stale references could redirect replay to unintended files.
- Baseline and final replay directories are evidence sources/sinks; stale or pre-populated outputs can create false readiness conclusions if the command does not verify provenance/freshness.

### Abuse scenarios
- **Parameter/path tampering:** Supplying `../`, absolute paths, or symlinked output paths could overwrite unrelated project files or read unintended local artifacts if path containment is not enforced.
- **Replay/stale evidence attack:** Reusing an old `final-replay-events.jsonl`, baseline, or report could make a blocked pipeline appear preserved/improved unless run IDs, input hashes, article set, and output hashes are checked.
- **Network bypass:** Missing or weak `--no-network`/`--require-no-network` enforcement could let the replay silently fetch missing article data, invalidating the fixed local-corpus proof and potentially leaking identifiers or environment metadata externally.
- **Unsafe readiness escalation:** Report or decision generation could accidentally set graph/import readiness flags or omit explicit `production_import_attempted=false`, `ladybugdb_written=false`, and `kg_import_allowed=false` evidence.
- **Payload leakage:** Per-article diagnostics and reports could include raw article text, binary asset payloads, tokens, or environment details if summaries are not constrained to metadata, IDs, counts, hashes, and diagnostics.

### Required mitigations/re-test focus
- Enforce path containment under the expected project/corpus directories for writable outputs; reject path traversal and unsafe symlinks.
- Hash/validate catalog, index, selection, baseline, and final artifacts; include run ID, command, cwd, git commit if available, input hashes, output hashes, and article IDs in events/summary.
- Make no-network fail closed; record no-network proof in events and final summary.
- Make import/write flags fail closed and require explicit false values for graph import, production LadybugDB write, fact promotion, and graph readiness acceptance.
- Keep diagnostics metadata-only and forbid raw text/binary payloads/secrets in persisted summaries and reports.

## Requirement Impact

## Q4 requirement impact assessment

Note: `.gsd/milestones/M025-6xovy3/REQUIREMENTS.md` was not present, so this assessment uses the active root requirement artifact `.gsd/REQUIREMENTS.md`.

### R-IDs touched
- **R024** — staged real-article KG behavior validation remains active. S08 provides a fixed 5-article preprocessing replay input to milestone validation, but must not claim broader 10/20/one-week KG validation.
- **R027** — graph-readiness quality contract for converted paper data/chunks. S08 must compare conversion/preprocessing outputs and blockers without making a positive graph-readiness acceptance claim.
- **R029** — import-ready typed chunk package remains blocked. S08 must preserve no graph import, no fact promotion, and no production write flags while producing final replay evidence.
- **R030** — source artifact preservation. S08 reuses local catalog/corpus artifacts and should re-test that per-article references remain metadata-only and do not embed raw PDFs/images/text payloads in summaries.
- **R036** — replay/audit provenance logs. S08 writes final events, summary, report, and decision artifacts, so it should re-test command/input/output provenance, freshness, hashes, and active milestone/corpus context.
- **R040** — infrastructure/process safety constraint. S08 is the final refactored pipeline replay gate and must preserve compatibility/safety evidence before any larger validation activation.
- **R050** — deterministic article structure/scaffold preprocessing without KG import. S08 is an end-to-end local replay over the article pipeline and should re-test stable per-article artifact references, metrics, diagnostics, and `kg_import_allowed=false` style safety outputs.
- **R052** — benchmark/metrics before optimizer or readiness claims. S08 compares baseline/final behavior and must ensure no DSPy/RLM/optimizer or broader quality claims are inferred from the 5-article smoke corpus.

### Must be re-tested after shipping
- Contract test for final replay schema and required fields: catalog/index/selection inputs, per-article artifact refs, comparison categories, no-network proof, no-import/no-write flags.
- End-to-end replay command over the fixed 5-article local corpus with `--no-network` and fresh events.
- Final report/decision generation with preserved/improved/regressed/blocked classifications and explicit no graph-readiness claim.
- Safety assertions: no network fetch, no production LadybugDB write, no graph import, no trusted fact promotion, no raw payload/secret leakage in persisted summaries.
- Provenance/freshness assertions tying generated artifacts to the current command, inputs, hashes, article set, and output paths.

### Decisions to revisit
- No existing decision appears to need scope expansion for S08; the slice should preserve the prior no-import/no-readiness/no-optimizer boundaries and defer larger validation readiness to milestone validation.

## Proof Level

- This slice proves: End to end local replay proof over the fixed 5 article corpus.

## Integration Closure

Milestone validation consumes this final replay report to decide whether preprocessing readiness is sufficient for a later larger validation milestone.

## Verification

- Writes final run summary, baseline comparison, diagnostics, readiness blockers, no-network proof, and no-write safety evidence.

## Tasks

- [x] **T01: Defined the final preprocessing replay contract fixture and executable contract tests for no-network, per-article final replay artifacts.** `est:medium`
  Define and test the final end-to-end replay contract for the fixed 5 article corpus. The contract must require catalog/index/selection inputs, no-network execution, per-article final artifact references, baseline comparison categories, and no graph import or production write flags.
  - Files: `tests/test_article_preprocessing_replay_contract.py`, `tests/fixtures/article_preprocessing_replay_v00_01/`
  - Verify: uv run pytest tests/test_article_preprocessing_replay_contract.py -q
uv run ruff check tests/test_article_preprocessing_replay_contract.py

- [x] **T02: Ran the final local preprocessing replay over all five selected articles and persisted per-article final artifacts plus replay events.** `est:medium`
  Implement or adapt the final local preprocessing replay command. It must read `data/article_catalog/catalog.json`, `data/article_catalog/index.json`, and the M025 corpus selection; it must reuse local artifacts from earlier slices; it must fail if a network fetch would be required during replay; and it must write final per-article artifacts and metrics.
  - Files: `src/arxiv_archive/`, `scripts/verify_m025_final_preprocessing_replay.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_final_preprocessing_replay.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --baseline data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline --final data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay --write-events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-events.jsonl --no-network

- [x] **T03: Finalized the S08 replay report and readiness decision, classifying all five articles as blocked by missing baseline while preserving no-network and no-write safety evidence.** `est:small`
  Write the final S08 report and machine-readable readiness decision. The report must compare final outputs against the baseline, classify behaviors as preserved/improved/regressed/blocked, summarize diagnostics, and explicitly state that M025 makes no graph readiness claim.
  - Files: `scripts/verify_m025_final_preprocessing_replay.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_final_preprocessing_replay.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --baseline data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline --final data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay --events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-events.jsonl --require-no-network --require-no-import-flags --write-summary data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json --write-report data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-report.md --write-decision data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json

## Files Likely Touched

- tests/test_article_preprocessing_replay_contract.py
- tests/fixtures/article_preprocessing_replay_v00_01/
- src/arxiv_archive/
- scripts/verify_m025_final_preprocessing_replay.py
- data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/
