---
id: T01
parent: S02
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-grobid-probe-v1/grobid-runtime-readiness.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-runtime-runbook.md
  - data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl
key_decisions:
  - Use `grobid/grobid:0.9.0-crf` Docker for the first bounded S02 TEI/API probe instead of native build or full DL image.
duration: 
verification_result: passed
completed_at: 2026-06-05T10:13:21.378Z
blocker_discovered: false
---

# T01: Documented GROBID runtime requirements and confirmed the CRF Docker image is ready for the bounded S02 probe.

**Documented GROBID runtime requirements and confirmed the CRF Docker image is ready for the bounded S02 probe.**

## What Happened

Recorded local Java/Docker facts and the GROBID native-vs-Docker tradeoff. Vendored GROBID source requires OpenJDK 21+ for native builds, while the local runtime is Java 17, so S02 uses the recommended Docker service path instead of changing host Java. Pulled and verified `grobid/grobid:0.9.0-crf`, documented CRF vs full image tradeoffs, service health endpoints, and fail-closed safety boundaries. Artifacts were written under `data/article_corpora/m033-grobid-probe-v1/`.

## Verification

Fresh T01 verification passed: `grobid-runtime-readiness.json`, `grobid-runtime-runbook.md`, and `grobid-events.jsonl` exist; readiness records `OpenJDK 21+` native build requirement, Docker daemon availability, selected image `grobid/grobid:0.9.0-crf`, verified image presence after pull, and all safety flags false. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 inline verifier updating `grobid-runtime-readiness.json` after `docker pull grobid/grobid:0.9.0-crf`` | 0 | ✅ pass | 125ms |

## Deviations

None.

## Known Issues

Native GROBID source build is not used in this slice because local Java is 17 while GROBID build requires JDK 21. The CRF image is sufficient for API/TEI contract-shape research but not a best-quality DL bibliography/citation benchmark.

## Files Created/Modified

- `data/article_corpora/m033-grobid-probe-v1/grobid-runtime-readiness.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-runtime-runbook.md`
- `data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl`
