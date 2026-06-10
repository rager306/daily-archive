---
id: T02
parent: S05
milestone: M055-kyxuqm
key_files:
  - artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json
  - artifacts/m055deep-parser-benchmark/hybrid-routing-20/per-pdf/
key_decisions:
  - Use hybrid_with_fulltext_grobid_fallback as the aggregate recommendation when any PDF requires single-parser GROBID fallback.
duration: 
verification_result: passed
completed_at: 2026-06-10T12:20:02.247Z
blocker_discovered: false
---

# T02: Ran the 20-PDF hybrid routing comparison and emitted per-PDF plus aggregate artifacts.

**Ran the 20-PDF hybrid routing comparison and emitted per-PDF plus aggregate artifacts.**

## What Happened

Executed the S05 comparator against artifacts/m055deep-parser-benchmark/grobid-fulltext-20/per-pdf and artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf. The run wrote 20 per-PDF comparison packets plus artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json.

## Verification

uv run pytest tests/test_m055deep_hybrid_routing_20.py -q passed with 6 tests and verifies 20 per-PDF packets, summary schema, aggregate route counts, dimension winners, length buckets, safety flags, and idempotence.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_m055deep_hybrid_routing_20.py -q` | 0 | ✅ pass: 6 passed | 9000ms |

## Deviations

Aggregate routing is 95% hybrid, not 100%, because 2605.28617v1 has a low-quality OpenDataLoader packet and routes to GROBID fulltext only.

## Known Issues

None.

## Files Created/Modified

- `artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json`
- `artifacts/m055deep-parser-benchmark/hybrid-routing-20/per-pdf/`
