# daily-archive

Local-first **Universal Knowledge Base** pipeline for scientific papers (first domain), with a hexagonal/onion package layout and **fail-closed import**.

```text
Source → Parser → Structure → Extraction → Graph → Review → Agents
```

Runtime package: **`research_graph`** (`src/research_graph/`).  
Historical rename shims: `archive/` (not on the import path).

---

## Current state (2026-07-24)

| Area | Status |
|------|--------|
| Hybrid body coverage | **81 / 230 ≈ 35.2%** (residual target 0.35 met) |
| Deploy extract path | **`header_priority`** constrained select (no free invent) |
| GEPA / LLM | Offline/staged compare only; **not** promoted |
| Structure continuous gate | **pass** on hybrid body sample; signal `ready_for_structure_review` |
| Import / graph write | **`import_eligible=false`** (D127 — needs explicit user go) |
| Import-hold inventory | **0** `= True` enablements |
| Tests | `uv run pytest` (~3500 collected) |
| Package layout | domain / application / infrastructure / workflows / cli |

Authoritative residual dashboard:

```text
artifacts/etl/ETL-READINESS-MATRIX-ROADMAP.md
artifacts/etl/continuity-pack.json
artifacts/etl/fleet-report.json
```

Hygiene policy: `doc/REPO-HYGIENE.md`.

---

## Safety invariants (always)

- `import_eligible=false` / `graph_writes_allowed=false` on operator paths unless a future milestone + **explicit human go** say otherwise.
- Hybrid claimed success requires **body evidence**, not “container up”.
- Constrained extract selects **`candidate_id` only** — no free-form label invent.
- Ship/promote for GEPA/LLM requires **same-n** dual F1 beat header **and** val-gap guard.

Binding ADRs: `doc/adr/ADR-INDEX.md` (ADR-008/009 hybrid parser, ADR-023 layered pipeline, ADR-034 onion, ADR-035 write governance, ADR-036 preprocess stack).

---

## Architecture

```text
src/research_graph/
├── domain/           # pure types, ports, schema
├── application/      # use-cases (corpus, graph, extraction) — pure where possible
├── infrastructure/   # parsers, LLM clients, graph drivers, retrieval
├── workflows/        # composition roots (sidecars, batch gates)
└── cli/              # typer entrypoints
```

Onion guard: `uv run python scripts/verify_onion_layering.py`.

---

## Daily operators (ETL / Wave B)

Prefer these over historical `verify_m0xx_*.py` unless a test/pre-commit pin requires the old verifier.

```bash
# Dashboard: hybrid fraction, multi_root, closeout, PDF queue
uv run python scripts/verify_etl_continuity_pack.py

# Fleet glue: pack + ship matrix + import-hold + quality n
uv run python scripts/verify_etl_fleet.py
uv run python scripts/verify_etl_fleet.py --rescore-quality   # live same-n header/matrix/grounding

# Gated hybrid expand (pack refresh default ON; --no-refresh-continuity-pack to skip)
uv run python scripts/verify_hybrid_expand_batch.py --help

# Wave B ship matrix (header deploy vs LLM/GEPA compare)
uv run python scripts/verify_wave_b_ship_gate_matrix.py

# GEPA vs header same-n (promote only dual F1 + val_gap)
uv run python scripts/verify_wave_b_gepa_vs_header.py

# Structure continuous chunk quality + readiness
uv run python scripts/verify_structure_chunk_quality_gate.py
uv run python scripts/verify_structure_readiness_package.py

# Import hold (must stay pass / 0 hits)
uv run python scripts/verify_import_hold_inventory.py
```

### Hybrid sidecars (GROBID / OpenDataLoader)

```bash
# GROBID CRF (pilots)
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid
curl -sS http://127.0.0.1:8070/api/isalive

# Host ODL library for hybrid body
uv sync --extra hybrid

# Single article (live hybrid when ports available)
uv run python -m research_graph article run path/to/paper.pdf --mode hybrid -o artifacts/single-article/demo
```

Details: `.docker/README.md`, ADR-008/009.  
`hybrid_claimed_success` still requires body evidence (not merely container up).

---

## Development

```bash
uv sync
uv run pytest tests/ -q
uv run ruff check src/ tests/
uv run pyrefly check   # via pre-commit / project config
uv run python scripts/verify_onion_layering.py
uv run python scripts/verify_import_hold_inventory.py
```

Pre-commit enforces import-hold, onion, and type ratchet on relevant paths (see `.pre-commit-config.yaml`).

---

## Project layout (top-level)

```text
src/research_graph/     # runtime package
scripts/                # verify_* operators (ETL/Wave B + historical milestone pins)
tests/                  # pytest suite
doc/adr/                # binding ADRs + index
artifacts/etl/          # continuity pack, fleet, structure gate, readiness matrix
artifacts/wave-b/       # ship matrix, GEPA, grounding, human_go stamp
data/article_catalog/   # canonical catalog + PDFs
archive/                # package rename shims (historical only)
.docker/                # GROBID/ODL compose
```

**Not source of truth / local scratch (gitignored):** `mutants/`, `tmp/`, `.gsd-backups/`, hybrid `runs-live*` workdirs, coverage caches. See `doc/REPO-HYGIENE.md`.

---

## What is intentionally deferred

| Item | Status |
|------|--------|
| Production graph import / Falkor write | Locked without explicit user go |
| GEPA/LLM as deploy select | Staged offline only; header wins |
| SymFSM agents as production brain | Directional ADR-026; not current ETL critical path |
| Stretch hybrid 0.50, YAKE default-on | Optional residual scale/quality |

---

## Further reading

- Residual ETL matrix: `artifacts/etl/ETL-READINESS-MATRIX-ROADMAP.md`
- Hygiene: `doc/REPO-HYGIENE.md`
- ADR index: `doc/adr/ADR-INDEX.md`
- Spec: `doc/SPEC.md`
- Changelog: `CHANGELOG.md`
