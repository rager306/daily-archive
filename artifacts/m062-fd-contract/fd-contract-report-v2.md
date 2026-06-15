# M062 fd Contract Report v2

Source contract: `/root/fd/docs/fd-v2.md` (the requested `/root/fd-v2.md` path was not present in this environment).
Configuration: fd v2 env uses `FD_API_KEY`, `TEI_URL`, `MODEL_ID`, `REDIS_HOST`, and `REDIS_PORT`.

## Summary

total=52, passed=8, failed=0, skipped=44

## Per-category breakdown

| Category | Total | Passed | Failed | Skipped | Pass rate |
|---|---:|---:|---:|---:|---:|
| endpoints | 5 | 0 | 0 | 5 | 0.0% |
| env | 4 | 4 | 0 | 0 | 100.0% |
| error | 15 | 1 | 0 | 14 | 6.7% |
| happy | 10 | 0 | 0 | 10 | 0.0% |
| headers | 10 | 0 | 0 | 10 | 0.0% |
| performance | 5 | 0 | 0 | 5 | 0.0% |
| wrapper | 3 | 3 | 0 | 0 | 100.0% |

## Per-test detail

| Test ID | Category | Description | Expected | Observed | Status | Evidence |
|---|---|---|---|---|---|---|
| T-H-1 | happy | single 1024-d embedding | 200, dimensions=1024 in response | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-2 | happy | single 512-d embedding | 200, dimensions=512 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-3 | happy | three embeddings | 200, 3 embeddings | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-4 | happy | max batch of 32 embeddings | 200, 32 embeddings | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-5 | happy | base64 encoding_format | 200, base64 string | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-6 | happy | priority option | 200 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-7 | happy | health deep check | 200, body contains model_loaded: true | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-8 | happy | liveness endpoint | 200 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-9 | happy | readiness endpoint | 200 after warmup | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-H-10 | happy | version endpoint | 200, version field | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-1 | error | missing input | 400, code=input_required | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-2 | error | empty input | 400, code=input_required | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-3 | error | invalid dimensions | 400, code=dimensions_invalid | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-4 | error | invalid non-string input | 400, code=invalid_request_error, no legacy unmarshal | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-5 | error | malformed JSON | 400, code=invalid_json | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-6 | error | batch too large | 413, code=batch_too_large | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-7 | error | input too long | 413, code=input_too_long | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-8 | error | GET embeddings method not allowed | 405, not 404 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-9 | error | auth rejects invalid bearer token | 401, code=unauthorized or is not authorized | status=401; latency_ms=3.9; headers={Connection=keep-alive, Content-Type=application/json; charset=utf-8, Server=fd/dev, X-Request-Id=2c3a752a-ca1a-437e-a337-eb55b2463153}; body='{"error":{"code":"unauthorized","type":"authentication_error","param":"authorization","message":"invalid bearer token"}}'; note=expected_status=401, code=unauthorized | PASS | status=401; latency_ms=3.9; headers={Connection=keep-alive, Content-Type=application/json; charset=utf-8, Server=fd/dev, X-Request-Id=2c3a752a-ca1a-437e-a337-eb55b2463153}; body='{"error":{"code":"unauthorized","type":"authentication_error","param":"authorization","message":"invalid bearer token"}}'; note=expected_status=401, code=unauthorized |
| T-E-10 | error | unknown route | 404, code=not_found | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-E-11 | error | during graceful shutdown | 503, code=shutting_down, Retry-After: 30 | note=shutdown mutation is disabled by default for this read-only daily-archive contract harness | SKIP | note=shutdown mutation is disabled by default for this read-only daily-archive contract harness |
| T-E-12 | error | model not loaded | 503, code=model_not_loaded, Retry-After: 5 | note=model-unloaded fixture is disabled by default; actual fd is expected to be warm or externally managed | SKIP | note=model-unloaded fixture is disabled by default; actual fd is expected to be warm or externally managed |
| T-E-13 | error | rate limit hit | 429, code=rate_limit_exceeded, Retry-After: 60 | note=rate-limit hammering is disabled by default to avoid mutating local fd state | SKIP | note=rate-limit hammering is disabled by default to avoid mutating local fd state |
| T-E-14 | error | oversized 50MB body | 413, code=payload_too_large | note=50MB payload test is disabled by default to avoid excessive local resource usage | SKIP | note=50MB payload test is disabled by default to avoid excessive local resource usage |
| T-E-15 | error | forced internal error | 500, code=internal_error, X-Request-Id in body | note=forced internal-error injection is disabled by default; fd exposes no safe fixture endpoint | SKIP | note=forced internal-error injection is disabled by default; fd exposes no safe fixture endpoint |
| T-HDR-1 | headers | server version header | Server: fd/2.0.0 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-2 | headers | request id echo | response echoes X-Request-Id: my-id | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-3 | headers | generated request id | response has X-Request-Id | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-4 | headers | model id header | X-Model-Id matches MODEL_ID | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-5 | headers | dimensions header | X-Dimensions: 1024 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-6 | headers | cache hit header on repeat | X-Cache: HIT | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-7 | headers | cache miss header on first request | X-Cache: MISS | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-8 | headers | Retry-After on temporary failure | 429/503 response has Retry-After | note=temporary-failure fixture is disabled by default; no safe fd trigger exists | SKIP | note=temporary-failure fixture is disabled by default; no safe fd trigger exists |
| T-HDR-9 | headers | keep-alive connection | Connection: keep-alive | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-HDR-10 | headers | cache hit ETag | ETag: <hash> | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-P-1 | performance | 1 input cache-hot p95 target | p95 < 50ms and X-Cache: HIT | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-P-2 | performance | 10 inputs cache-hot p95 target | p95 < 200ms and X-Cache: HIT | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-P-3 | performance | 32 inputs cache-hot p95 target | p95 < 1000ms and X-Cache: HIT | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-EX-1 | endpoints | version endpoint exists | 200, not 404 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-EX-2 | endpoints | info endpoint exists | 200, not 404 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-EX-3 | endpoints | metrics endpoint exists | 200, Content-Type: text/plain | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-EX-4 | endpoints | OpenAPI schema exists | 200, not 404 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-EX-5 | endpoints | Swagger UI exists | 200, not 404 | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-P-4 | performance | 100 sequential cache-hot requests | p95 < 50ms, all X-Cache=HIT | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-P-5 | performance | concurrency 32 cache-hot requests | p95 < 50ms, all X-Cache=HIT | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification | SKIP | note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| T-W-1 | wrapper | force 3 failures, fourth call returns zero embedding | circuit open and zero embedding | note=state=open, first_errors=['500', '500'], fourth=[[0.0, 0.0, 0.0, 0.0]], calls=3 | PASS | note=state=open, first_errors=['500', '500'], fourth=[[0.0, 0.0, 0.0, 0.0]], calls=3 |
| T-W-2 | wrapper | circuit opens then recovers after 60s cooldown | half-open probe succeeds and circuit closes | note=opened=True, final_state=closed, recovered=[[0.25, 0.25, 0.25, 0.25]] | PASS | note=opened=True, final_state=closed, recovered=[[0.25, 0.25, 0.25, 0.25]] |
| T-W-3 | wrapper | 5xx response returns zero embedding and records warning path | zero embedding plus error_count=1 | note=embeddings=[[0.0, 0.0, 0.0, 0.0]], metrics={'request_count': 1, 'error_count': 1, 'latency': {'count': 1, 'p50': 0.00014367606490850449, 'p95': 0.00014367606490850449, 'p99': 0.00014367606490850449}, 'cache_hit_rate': 0.0, 'circuit_state': 'open', 'circuit_state_gauge': 2} | PASS | note=embeddings=[[0.0, 0.0, 0.0, 0.0]], metrics={'request_count': 1, 'error_count': 1, 'latency': {'count': 1, 'p50': 0.00014367606490850449, 'p95': 0.00014367606490850449, 'p99': 0.00014367606490850449}, 'cache_hit_rate': 0.0, 'circuit_state': 'open', 'circuit_state_gauge': 2} |
| T-ENV-1 | env | FD_API_KEY supplies bearer auth header | Authorization bearer header is set from FD_API_KEY without logging the key | note=authorization_header_present=True | PASS | note=authorization_header_present=True |
| T-ENV-2 | env | TEI_URL overrides fd base URL and derived endpoint | TEI_URL base derives /v1/embeddings endpoint | note=base_host=fd-test.internal, endpoint_suffix=/v1/embeddings, ok=True | PASS | note=base_host=fd-test.internal, endpoint_suffix=/v1/embeddings, ok=True |
| T-ENV-3 | env | MODEL_ID overrides advertised model id | get_model_id() returns test/model-v2 | note=observed_model_id=test/model-v2 | PASS | note=observed_model_id=test/model-v2 |
| T-ENV-4 | env | REDIS_HOST and REDIS_PORT override cache target | Redis cache target env resolves to fd-cache.internal:6380 | note=observed_host=fd-cache.internal, observed_port=6380 | PASS | note=observed_host=fd-cache.internal, observed_port=6380 |

## v1 -> v2 comparison

Prior v1 statuses loaded: yes.
Now passing after v1 failure or skip: 1.
Still passing from v1: 7.
Regressed from v1 PASS: 33.

### Tests now passing

- **T-E-9** — v1=SKIP, v2=PASS; auth rejects invalid bearer token

### Regressions from v1 PASS

- **T-H-1** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-2** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-3** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-4** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-5** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-6** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-7** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-8** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-9** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-H-10** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-E-1** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-E-2** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-E-3** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-E-5** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-E-7** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-E-8** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-E-10** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-2** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-3** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-4** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-5** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-6** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-7** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-9** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-HDR-10** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-P-1** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-P-2** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-EX-1** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-EX-2** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-EX-3** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-EX-4** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-EX-5** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **T-P-4** — v1=PASS, v2=SKIP; note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification

## Gaps prioritized

### P0

- **R-P0-1 UNKNOWN** — Input length validation: T-E-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-2 UNKNOWN** — Batch size validation: T-H-4:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-3 UNKNOWN** — 503 for model not loaded or overloaded: T-E-12:SKIP:note=model-unloaded fixture is disabled by default; actual fd is expected to be warm or externally managed
- **R-P0-4 UNKNOWN** — Warmup and readiness: T-H-9:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-5 UNKNOWN** — Graceful shutdown: T-E-11:SKIP:note=shutdown mutation is disabled by default for this read-only daily-archive contract harness
- **R-P0-6 UNKNOWN** — Performance baseline: T-P-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-P-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-P-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-7 UNKNOWN** — GET /version: T-H-10:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-EX-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-8 UNKNOWN** — GET /info or GET /v1/models: T-EX-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-9 UNKNOWN** — GET /metrics Prometheus text: T-EX-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-10 UNKNOWN** — GET /v1/healthcheck alias: No section 5 case exists for /v1/healthcheck; fd v1 observed missing in fd-v2.md section 1.3.
- **R-P0-11 UNKNOWN** — X-Request-Id: T-E-15:SKIP:note=forced internal-error injection is disabled by default; fd exposes no safe fixture endpoint; T-HDR-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-HDR-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-12 UNKNOWN** — Server: fd/<version>: T-HDR-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-13 UNKNOWN** — X-Model-Id header: T-HDR-4:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-14 UNKNOWN** — X-Dimensions header: T-HDR-5:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-15 UNKNOWN** — X-Cache header: T-HDR-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-HDR-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-16 UNKNOWN** — Retry-After on 429/503: T-E-11:SKIP:note=shutdown mutation is disabled by default for this read-only daily-archive contract harness; T-E-12:SKIP:note=model-unloaded fixture is disabled by default; actual fd is expected to be warm or externally managed; T-E-13:SKIP:note=rate-limit hammering is disabled by default to avoid mutating local fd state
- **R-P0-17 UNKNOWN** — Connection keep-alive: T-HDR-9:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-18 UNKNOWN** — OpenAI-style error envelope: T-E-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P0-19 UNKNOWN** — HTTP status code mapping: T-E-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification

### P1

- **R-P1-1 UNKNOWN** — GET /health deep check: T-H-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P1-2 UNKNOWN** — GET /warmup status: No section 5 case exists for GET /warmup; feature is missing from fd v1.
- **R-P1-3 UNKNOWN** — POST /warmup trigger: No section 5 case exists for POST /warmup; feature is missing from fd v1.
- **R-P1-4 UNKNOWN** — Cache with X-Cache header: T-HDR-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-HDR-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-P-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P1-5 UNKNOWN** — encoding_format option: T-H-5:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P1-6 UNKNOWN** — user field: No section 5 case exists for user field; feature is not evidenced.
- **R-P1-7 UNKNOWN** — priority option: T-H-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P1-9 UNKNOWN** — CORS headers: No section 5 case exists for CORS; feature is not evidenced.

### P2

- **R-P2-1 UNKNOWN** — OpenAPI schema and Swagger UI: T-EX-4:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-EX-5:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P2-2 UNKNOWN** — ETag and Cache-Control: T-HDR-10:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification
- **R-P2-3 UNKNOWN** — /v1/batch endpoint: No section 5 case exists for /v1/batch; fd-v2.md section 1.3 says it is absent.
- **R-P2-4 UNKNOWN** — Streaming response: No section 5 case exists for streaming response; feature is not evidenced.
- **R-P2-5 UNKNOWN** — Rate limiting: T-E-13:SKIP:note=rate-limit hammering is disabled by default to avoid mutating local fd state
- **R-P2-6 UNKNOWN** — /v1/traces debugging: No section 5 case exists for /v1/traces; feature is not evidenced.
