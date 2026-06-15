# M062 fd Actual vs Required v2

Status meanings: MET = at least one mapped contract test passed; PARTIAL = endpoint responded but header/body/status contract is incomplete; UNKNOWN = endpoint unavailable, skipped fixture, or no live evidence.

## Expected requirement coverage

- P0: 19/19 requirements represented in the contract matrix.
- P1: 9/9 requirements represented in the contract matrix.
- P2: 6/6 requirements represented in the contract matrix.

## Summary

| Priority | Met | Partial | Unknown | Total |
|---|---:|---:|---:|---:|
| P0 | 0 | 0 | 19 | 19 |
| P1 | 1 | 0 | 8 | 9 |
| P2 | 0 | 0 | 6 | 6 |

## Per-requirement detail

| Requirement | Priority | Description | Status | Tests | Evidence |
|---|---|---|---|---|---|
| R-P0-1 | P0 | Input length validation | UNKNOWN | T-E-7 | T-E-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-2 | P0 | Batch size validation | UNKNOWN | T-H-4, T-E-6 | T-H-4:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-3 | P0 | 503 for model not loaded or overloaded | UNKNOWN | T-E-12 | T-E-12:SKIP:note=model-unloaded fixture is disabled by default; actual fd is expected to be warm or externally managed |
| R-P0-4 | P0 | Warmup and readiness | UNKNOWN | T-H-9 | T-H-9:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-5 | P0 | Graceful shutdown | UNKNOWN | T-E-11 | T-E-11:SKIP:note=shutdown mutation is disabled by default for this read-only daily-archive contract harness |
| R-P0-6 | P0 | Performance baseline | UNKNOWN | T-P-1, T-P-2, T-P-3, T-P-4, T-P-5 | T-P-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-P-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-P-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-7 | P0 | GET /version | UNKNOWN | T-H-10, T-EX-1 | T-H-10:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-EX-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-8 | P0 | GET /info or GET /v1/models | UNKNOWN | T-EX-2 | T-EX-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-9 | P0 | GET /metrics Prometheus text | UNKNOWN | T-EX-3 | T-EX-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-10 | P0 | GET /v1/healthcheck alias | UNKNOWN | n/a | No section 5 case exists for /v1/healthcheck; fd v1 observed missing in fd-v2.md section 1.3. |
| R-P0-11 | P0 | X-Request-Id | UNKNOWN | T-E-15, T-HDR-2, T-HDR-3 | T-E-15:SKIP:note=forced internal-error injection is disabled by default; fd exposes no safe fixture endpoint; T-HDR-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-HDR-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-12 | P0 | Server: fd/<version> | UNKNOWN | T-HDR-1 | T-HDR-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-13 | P0 | X-Model-Id header | UNKNOWN | T-HDR-4 | T-HDR-4:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-14 | P0 | X-Dimensions header | UNKNOWN | T-HDR-5 | T-HDR-5:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-15 | P0 | X-Cache header | UNKNOWN | T-HDR-6, T-HDR-7 | T-HDR-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-HDR-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-16 | P0 | Retry-After on 429/503 | UNKNOWN | T-E-11, T-E-12, T-E-13, T-HDR-8 | T-E-11:SKIP:note=shutdown mutation is disabled by default for this read-only daily-archive contract harness; T-E-12:SKIP:note=model-unloaded fixture is disabled by default; actual fd is expected to be warm or externally managed; T-E-13:SKIP:note=rate-limit hammering is disabled by default to avoid mutating local fd state |
| R-P0-17 | P0 | Connection keep-alive | UNKNOWN | T-HDR-9 | T-HDR-9:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-18 | P0 | OpenAI-style error envelope | UNKNOWN | T-E-1, T-E-2, T-E-3, T-E-4, T-E-5, T-E-6, T-E-7, T-E-10, T-E-15 | T-E-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P0-19 | P0 | HTTP status code mapping | UNKNOWN | T-E-1, T-E-2, T-E-3, T-E-4, T-E-5, T-E-6, T-E-7, T-E-8, T-E-10, T-E-11, T-E-12, T-E-13, T-E-14, T-E-15 | T-E-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-2:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-E-3:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P1-1 | P1 | GET /health deep check | UNKNOWN | T-H-7 | T-H-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P1-2 | P1 | GET /warmup status | UNKNOWN | n/a | No section 5 case exists for GET /warmup; feature is missing from fd v1. |
| R-P1-3 | P1 | POST /warmup trigger | UNKNOWN | n/a | No section 5 case exists for POST /warmup; feature is missing from fd v1. |
| R-P1-4 | P1 | Cache with X-Cache header | UNKNOWN | T-HDR-6, T-HDR-7, T-P-1, T-P-2, T-P-3 | T-HDR-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-HDR-7:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-P-1:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P1-5 | P1 | encoding_format option | UNKNOWN | T-H-5 | T-H-5:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P1-6 | P1 | user field | UNKNOWN | n/a | No section 5 case exists for user field; feature is not evidenced. |
| R-P1-7 | P1 | priority option | UNKNOWN | T-H-6 | T-H-6:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P1-8 | P1 | API key auth | MET | T-E-9 | T-E-9:PASS:status=401; latency_ms=3.9; headers={Connection=keep-alive, Content-Type=application/json; charset=utf-8, Server=fd/dev, X-Request-Id=2c3a752a-ca1a-437e-a337-eb55b2463153}; body='{"error":{"code":"unauthorized","type":"authentication_error","param":"authorization","message":"invalid bearer token"}}'; note=expected_status=401, code=unauthorized |
| R-P1-9 | P1 | CORS headers | UNKNOWN | n/a | No section 5 case exists for CORS; feature is not evidenced. |
| R-P2-1 | P2 | OpenAPI schema and Swagger UI | UNKNOWN | T-EX-4, T-EX-5 | T-EX-4:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification; T-EX-5:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P2-2 | P2 | ETag and Cache-Control | UNKNOWN | T-HDR-10 | T-HDR-10:SKIP:note=FD_API_KEY is not configured; protected fd v2 request is not authorized for verification |
| R-P2-3 | P2 | /v1/batch endpoint | UNKNOWN | n/a | No section 5 case exists for /v1/batch; fd-v2.md section 1.3 says it is absent. |
| R-P2-4 | P2 | Streaming response | UNKNOWN | n/a | No section 5 case exists for streaming response; feature is not evidenced. |
| R-P2-5 | P2 | Rate limiting | UNKNOWN | T-E-13 | T-E-13:SKIP:note=rate-limit hammering is disabled by default to avoid mutating local fd state |
| R-P2-6 | P2 | /v1/traces debugging | UNKNOWN | n/a | No section 5 case exists for /v1/traces; feature is not evidenced. |
