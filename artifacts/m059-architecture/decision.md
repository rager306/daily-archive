# M059 S02 Decision: M061 manifest-scaled 2-hop BFS ingest

Decision: **go with manifest-gated scale-up for M061**.

M061 should scale the M056 2-hop BFS corpus using versioned PDF batch manifests and the S02 validation/replay tooling before any parser output can feed downstream graph work. The scale-up remains diagnostic-only until a later milestone explicitly authorizes graph writes, production import, fact promotion, external network use, or LLM calls.

## Scope for M061

- Use `daily-archive.pdf-batch-manifest.v1` as the required batch contract for every PDF selected by the 2-hop BFS expansion.
- Require each parser expectation to declare `expected_output_schema`, parser version, mode, and resolvable output path templates.
- Run `scripts/m059_validate_pdf_batch.py` per parser before downstream graph or evidence packaging stages consume parser output.
- Run `scripts/m059_replay_ingest.py` for at least one deterministic parser sample per batch to prove byte-identical replay semantics.
- Preserve the five explicit false safety defaults in manifests and reports:
  - `external_network_authorized: false`
  - `graph_writes_authorized: false`
  - `production_import_authorized: false`
  - `fact_promotion_authorized: false`
  - `llm_calls_authorized: false`

## Evidence from M059 S02

- M054 manifest covers 5 PDFs and two parser expectations: GROBID and OpenDataLoader.
- Validation tooling checks existing per-PDF outputs against `schemas/grobid-tei.v1.json` and `schemas/opendataloader-pdf.v1.json`.
- Replay tooling verifies deterministic parser output with SHA-256 byte identity without invoking external services or mutating graph state.
- End-to-end reports are written under `artifacts/m059-architecture/` for validation, replay, and aggregate S02 status.

## Constraints carried forward

- GROBID endpoints must use `127.0.0.1` when a future live runner is explicitly authorized; `localhost` should not be used in runner defaults.
- Parser sidecars remain evidence artifacts, not graph truth.
- Non-deterministic LLM-call parser output must be labeled non-deterministic and must not receive byte-identical replay claims.
- M061 should fail closed when manifest safety defaults are missing or set to true.

## Consequence

M061 can expand corpus size while retaining reproducibility gates. The cost is extra manifest/report plumbing per batch, but the project avoids repeating pre-M059 ambiguity about which parser outputs are safe, replayable, or eligible for downstream use.
