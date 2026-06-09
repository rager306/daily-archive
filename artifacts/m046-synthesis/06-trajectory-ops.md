# 06 — Trajectory and Operations Layer

> **Source:** `scripts/check_project_trajectory.py`, M045 SUMMARY, `.codebase-memory/adr.md`, `.codebase-memory/governance-graph.json`
> **Synthesis layer:** 6 of 7
> **Decision:** D075 (governance memory), D076 (typed graph projection), D080 (trajectory check)

## 0. The Operations Loop

```
  preflight (M045 trajectory check)
        ↓
  plan (gsd_plan_milestone, gsd_plan_slice, gsd_plan_task)
        ↓
  execute (T01, T02, ... per slice)
        ↓
  closeout (gsd_task_complete, gsd_slice_complete, gsd_milestone_complete)
        ↓
  validate (gsd_validate_milestone)
        ↓
  preflight (M045 trajectory check again)
        ↓
  next milestone
```

The M045 trajectory check is the **single point of project-direction drift detection**. It runs at the start of every milestone (preflight) and at the end (closeout verification), and is the **prohibited replacement for per-topic guardrails** (D080).

## 1. Trajectory Check — 7 Dimensions

`scripts/check_project_trajectory.py` reads existing canonical artifacts and produces a JSON + Markdown trajectory report. The report covers 7 dimensions:

| Dimension | Sources | Status signals |
|---|---|---|
| **architecture** | `.gsd/DECISIONS.md`, `doc/adr/`, `.codebase-memory/governance-graph.json` | tracked / missing / drift |
| **functionality** | `.gsd/REQUIREMENTS.md` (count + statuses) | tracked / missing |
| **module_code** | `git status --short` | tracked / unknown / uncommitted |
| **evidence** | recent 6 milestone SUMMARY.md (by mtime) | tracked / missing |
| **safety** | prohibited-claim scan over README + recent summaries | clear / blocked |
| **operations** | artifact-derived state, no live process management | tracked |
| **next_gate** | README + recent milestones (look for "Next gate" / "Next safe milestone") | clear / needs_attention |

Each dimension carries: `status`, `evidence` (list of source files / counts), `flags` (drift signals).

## 2. Drift Flags (3 severity levels)

| Severity | Examples | Effect on verdict |
|---|---|---|
| **high** | `governance_mirror_missing`, `prohibited_claim_*` | verdict = `blocked` |
| **medium** | `latest_milestone_missing_readme_reference`, `missing_next_gate` | verdict = `drift_risk` |
| **info** | `uncommitted_changes_present` | verdict = `on_track` (still passes) |

The latest report (`artifacts/m045-project-trajectory/current/trajectory-report.md`) shows `verdict: on_track flags=0` after the M046 chore-commit (the 9 uncommitted files flag is gone).

## 3. Codebase-Memory MCP — Non-Canonical Mirror

D075 records the hybrid governance memory model:

```text
GSD          = canonical source of truth (requirements, decisions)
GitNexus     = mandatory for code impact/change safety
codebase-memory MCP = fast ADR/R/D recall mirror (NON-canonical)
```

The mirror is **generated** by `scripts/sync_codebase_memory_governance.py` from canonical artifacts. It is **never** a source of truth. The mirror:

- States the source-of-truth hierarchy explicitly
- Includes requirement and decision indexes
- Includes ADR relationship graph notes
- Rejects secret-shaped or raw-payload-like content
- Is regenerated after every GSD status change

### 3.1 Read-only usage

The mirror is consumed via MCP `manage_adr` readback and `search_graph` queries. It is **not** mutated by daily-archive code; only the sync script writes to it, and only from canonical sources.

### 3.2 Known limitation

`codebase-memory-mcp ingest_traces` currently reports that **runtime edge creation is not implemented**. The M039 lesson learned confirms this. Until that changes, the typed governance graph projection is **artifact-first** (D076), not native MCP graph ingestion.

## 4. Typed Governance Graph Projection

`scripts/sync_codebase_memory_governance.py` produces `.codebase-memory/governance-graph.json` with typed nodes and edges:

- **Node types:** Requirement, Decision, ADR, Milestone, SafetyBoundary, Artifact
- **Edge types:** extends, implements, owned_by, blocks, provides, depends_on
- **Example edges:**
  - `D076 extends D075`
  - `D076 implements R063`
  - `R063 owned_by M039`
  - `ADR-005 blocks the no-direct-GraphDB safety boundary`
  - `M038/M039 provide generated artifacts`

The graph is non-canonical and regenerated after every GSD status change. M039 validation confirmed `ingest_traces` is the wrong tool for this; use `manage_adr` readback and `search_graph` instead.

## 5. Next Gate Management

The trajectory check's `next_gate` dimension looks for explicit next-step text in `README.md` and recent milestone summaries. Patterns accepted:

- `Next safe milestone`
- `Next gate`
- `next gate` (lowercase)

If absent, the check flags `missing_next_gate` (medium severity). Current state: clear (next gate text present in recent summaries).

### 5.1 Next gate content (current)

The most recent next-gate text is in M045 SUMMARY follow-ups:

> Use the trajectory checker before planning and closeout. Next project gate remains bounded local PDF acquisition for the five linked target records, then rerun live GROBID/OpenDataLoader/Adaptix candidate packets under the trajectory preflight.

This text is the **single canonical next-gate** for the post-M045 era. M046 (this synthesis) does not change it; it **operates under it**.

## 6. Recommended Operations Procedure

For any future milestone:

1. **Preflight** — `uv run python scripts/check_project_trajectory.py --output-dir artifacts/m###/current`
2. **Verify** — `verdict=on_track` (no high-severity drift)
3. **If drift_risk** — fix medium-severity flags before planning
4. **If blocked** — fix high-severity flags (governance mirror, prohibited claims) before anything else
5. **Plan** — `gsd_plan_milestone` → `gsd_plan_slice` → `gsd_plan_task`
6. **Execute** — T01, T02, ... per slice
7. **Closeout** — `gsd_task_complete` → `gsd_slice_complete` → `gsd_milestone_complete` → `gsd_validate_milestone`
8. **Re-preflight** — re-run trajectory check
9. **Commit** — atomic commit per slice or per milestone
10. **Update governance mirror** — `uv run python scripts/sync_codebase_memory_governance.py` if R/D changed

This is the **D080 procedure** in code form. Future agents must follow it; deviation requires explicit ADR/decision.

## 7. Re-running the Trajectory Check on This Synthesis Package

```bash
uv run python scripts/check_project_trajectory.py --output-dir artifacts/m046-synthesis/current
```

Expected output: `verdict=on_track flags=0` (assuming M046 artifacts follow the same safety contract as M033-M045). If any flag appears, fix the synthesis artifact before milestone closeout.

## 8. Cross-References

- North star: `01-north-star.md`
- Architecture: `02-architecture-layers.md`
- Decisions: `03-adr-decisions.md`
- Modules: `04-module-map.md`
- Safety: `05-evidence-safety.md`
- Assessment: `07-2026-assessment.md`

## 9. LLM Reading Notes (binding)

- **The trajectory check is the single preflight.** Do not add per-topic guardrails (D080).
- **codebase-memory MCP is non-canonical.** Never treat its contents as authoritative.
- **`ingest_traces` is not the right tool** for typed graph edges today. Use artifact-first projection (D076).
- **Always regenerate the governance mirror** after R/D changes; otherwise M045 trajectory check flags `governance_mirror_missing` (high severity).
- **The trajectory report is derived, non-canonical, and never authorizes graph import.** (Same as M045 SUMMARY.)
