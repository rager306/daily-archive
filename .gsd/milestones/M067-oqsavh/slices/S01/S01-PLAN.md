# S01: Re-score FalkorDB with SSPLv1 + distribution model assumption

**Goal:** Re-score FalkorDB with corrected SSPLv1 license + explicit distribution model assumption (self-hosted research). Document why FalkorDB 68/90 wins for self-hosted daily-archive.
**Demo:** Updated 5 candidate reports with SSPLv1 license correction + distribution model explicit + re-scored scoring matrix.

## Must-Haves

- FalkorDB license score corrected to 4/5 (SSPLv1 OK self-hosted)
- Distribution model assumption explicit
- Self-hosted FalkorDB viable without source disclosure
- 5+ tests pass
- 5 safety defaults stay false
- 127.0.0.1 NOT 127.0.0.1
- M045/M044 ok
- 1 commit

## Proof Level

- This slice proves: operational

## Integration Closure

Corrected license analysis. Foundation for S02 ADR-022 binding FalkorDB.

## Verification

- Updated scoring matrix + distribution model assumption document.

## Tasks

- [x] **T01: Re-scored FalkorDB to 70/90 under corrected SSPLv1 self-hosted distribution model.** `est:60m`
  Step 1: Update artifacts/m066-graphdb-reselection/candidates/falkordb-report.md:
  - License section: SSPLv1 (corrected from AGPLv3/AGPLv3 confusion)
  - Distribution model analysis: self-hosted OK without disclosure, SaaS triggers Section 13
  - FalkorDB Cloud pricing: Free / Startup $73/1GB-mo / Pro $350/8GB-mo / Enterprise tailored
  - Self-hosting: supported (Docker, K8s, standalone, requires Redis 8.0+)
  - License score: 4/5 (was 3/5 in M066 — too generous for SSPLv1, but more accurate for self-hosted use)
  - Files: `artifacts/m066-graphdb-reselection/candidates/falkordb-report.md`, `artifacts/m066-graphdb-reselection/distribution-model.md`, `artifacts/m066-graphdb-reselection/scoring-matrix.md`, `tests/test_m067_s01.py`
  - Verify: test -f artifacts/m066-graphdb-reselection/distribution-model.md

## Files Likely Touched

- artifacts/m066-graphdb-reselection/candidates/falkordb-report.md
- artifacts/m066-graphdb-reselection/distribution-model.md
- artifacts/m066-graphdb-reselection/scoring-matrix.md
- tests/test_m067_s01.py
