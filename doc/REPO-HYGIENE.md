# Repository hygiene (current)

**As of:** 2026-07-24 (post M257–M273 ETL residual waves)

## What is source of truth

| Area | Path | Notes |
|------|------|--------|
| Package | `src/research_graph/` | Hexagonal/onion: domain / application / infrastructure / workflows / cli |
| Operators | `scripts/verify_*.py` | Thin CLI over application pure packages |
| ETL readiness | `artifacts/etl/ETL-READINESS-MATRIX-ROADMAP.md` | Live residual matrix + roadmap |
| Continuity pack | `artifacts/etl/continuity-pack.json` | Hybrid/closeout/multi_root dashboard |
| Fleet | `artifacts/etl/fleet-report.json` | Pack + ship matrix + import-hold + quality n |
| Wave B quality | `artifacts/wave-b/` | Ship matrix, GEPA, grounding, stamps |
| ADRs | `doc/adr/` | Binding architecture; import never authorized by ADR alone |
| Tests | `tests/` | ~3500+ collected; prefer `uv run pytest` |
| Historical rename shims | `archive/package-layout-shims/`, `archive/package-rename-waves/` | **Not runtime** — migration archaeology only |

## Local garbage (safe to delete anytime)

Already gitignored; wipe when disk is tight:

| Path | Typical size | Why |
|------|--------------|-----|
| `mutants/` | multi-GB | mutmut / mutation testing workspace |
| `.coverage*` | small | pytest-cov leftovers at repo root |
| `tmp/` | small–med | local scratch (gitignored) |
| `.hypothesis/`, `.gremlins_cache/` | small | property/mutation caches |
| `.gsd-backups/` | large | GSD migration backups (gitignored) |
| `artifacts/m213-hybrid-gate/runs-live*/` | large | hybrid runtime bodies (gitignored; keep if replaying expand) |
| `artifacts/single-article/` | var | smoke workdirs |

## Do not delete without review

| Path | Why |
|------|-----|
| `archive/` | Tracked historical package-rename evidence |
| `artifacts/etl/`, `artifacts/wave-b/*.json` | Operator evidence (many tracked) |
| `data/article_catalog/` | Canonical PDF/catalog corpus |
| `doc/adr/` | Binding decisions |
| `scripts/verify_m*.py` | Historical milestone verifiers still used by tests/pre-commit |

## Docs drift policy

- **README** = current operator map + safety + structure (not full milestone diary).
- **ADR-INDEX** = binding ADRs; amend status notes only when policy changes.
- **ETL readiness matrix** = living residual dashboard after scale/quality waves.
- Milestone narratives live under `.gsd/` (local) and git history — not duplicated as root `m00*.md` briefs.

## Cleanup commands

```bash
# free disk from mutation/coverage scratch
rm -rf mutants .coverage* .hypothesis .gremlins_cache

# optional: large GSD backups (local only)
# rm -rf .gsd-backups

# re-check import hold after any script/src edit
uv run python scripts/verify_import_hold_inventory.py
```

## Script inventory notes

- ~250 scripts under `scripts/`; majority are `verify_*` milestone operators.
- Active ETL/Wave B entrypoints (prefer these):
  - `verify_etl_fleet.py`
  - `verify_etl_continuity_pack.py`
  - `verify_hybrid_expand_batch.py`
  - `verify_wave_b_ship_gate_matrix.py`
  - `verify_wave_b_gepa_vs_header.py`
  - `verify_structure_chunk_quality_gate.py`
  - `verify_structure_readiness_package.py`
  - `verify_import_hold_inventory.py`
- Older `verify_m0xx_*.py` are **not dead** if tests or pre-commit still call them; treat as freeze verifiers, not daily operators.

## Package rename reality

- Runtime package is **`research_graph`** (`python -m research_graph`).
- Legacy name `arxiv_archive` appears only in comments, archives, and historical docs.
- Any README/CLI example using `python -m arxiv_archive` is **stale**.
