# S01: Schemas + retroactive manifests + ADR-013 — UAT

**Milestone:** M059-y6osma
**Written:** 2026-06-12T10:18:17.872Z

# S01 UAT

## Checks

- PASS: Six JSON schemas exist under `schemas/` and validate as draft-07 schemas.
- PASS: Retroactive manifests exist for M054, M055, M055deep, M056, M057, and M058 with counts 5, 5, 20, 166, 166, and 5.
- PASS: M054 GROBID jsonschema validation returns aggregate total=5 passed=5 failed=0 missing=0.
- PASS: ADR-013 is present and accepted as a binding manifest-driven PDF ingest decision.
- PASS: Safety defaults are explicit false in generated manifests; production import is not authorized and graph writes are disabled.
- PASS: M045 trajectory reports on_track and M044 guardrail exits ok.

