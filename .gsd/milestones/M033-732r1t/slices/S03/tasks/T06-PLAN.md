---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T06: Map OpenDataLoader probe results to daily-archive contracts

Create a contract mapping matrix from OpenDataLoader hybrid outputs, Java-only fallback outputs, or blockers to daily-archive SourceRef, EvidencePath, PageIndex, SemanticChunk, table artifact, refusal diagnostic, and graph-readiness packet boundaries. Classify the bounded tool verdict as `hybrid-sidecar-candidate`, `java-only-candidate`, `needs-larger-hybrid-probe`, `blocked-by-runtime`, or `reject-for-now`. The verdict must remain bounded research only and must not claim graph readiness, production import eligibility, or LadybugDB write readiness. Include backend/cache operational requirements in the verdict: Python 3.13 venv, hybrid extras, server lifecycle, Hugging Face cache paths, cache size, and network dependency if cache is absent.

## Inputs

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-report.md`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json`
- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json`
- `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.json`

## Expected Output

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-contract-mapping.md`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-probe-verdict.json`

## Verification

Verify `opendataloader-probe-verdict.json` parses as JSON, contains one of the allowed bounded verdict values, includes `graph_import_allowed:false`, `ladybugdb_written:false`, `production_import_attempted:false`, references the quality summary and model-cache inventory, and states backend/runtime/cache cost and remaining evidence gaps. Verify the contract mapping markdown is non-empty and covers SourceRef, EvidencePath, PageIndex, SemanticChunk, table artifact, refusal diagnostic, and graph-readiness packet boundaries.

## Observability Impact

Verdict explains bounded hybrid candidate status, fallback role, backend/cache/runtime cost, and any need for future larger probe without weakening fail-closed graph safety.
