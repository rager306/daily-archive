---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Document GROBID runtime requirements and service readiness

Confirm local Java/Docker/runtime facts, record native-vs-Docker requirements from vendored GROBID docs, and attempt to prepare the recommended CRF Docker service path. If Docker image pull/start is blocked, record a typed blocker rather than weakening scope. Read-only research context: `/root/vendor-source/grobid/doc/Install-Grobid.md`, `/root/vendor-source/grobid/doc/Grobid-docker.md`, `/root/vendor-source/grobid/Readme.md`.

## Inputs

- None specified.

## Expected Output

- `data/article_corpora/m033-grobid-probe-v1/grobid-runtime-readiness.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-runtime-runbook.md`
- `data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl`

## Verification

Fresh command verifies runtime readiness artifacts exist, include Java version, Docker daemon status, selected image, native JDK21 requirement, and fail-closed safety flags.

## Observability Impact

Records service readiness/blocker facts and command-level diagnostics without secrets.
