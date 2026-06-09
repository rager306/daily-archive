# Universal KB Synthesis Package — M033 to M045

> **Milestone:** M046-3b7gp0
> **Scope:** synthesis of M033-732r1t, M034-kuei9y, M035-xr6jjf, M036-k2s2pk, M037-7lwenr, M038-hdx112, M039-7o4yf1, M040-4flhk6, M041-8k3kv4, M042-m64cj9, M043-cqiqeq, M044-qq02k8, M045-4s8e44
> **Status:** active (in synthesis)
> **Auditor:** synthesis package, not a new ADR, not a new graph import authorization

## TL;DR

daily-archive is a **local-first universal knowledge base** with **scientific articles as the primary first domain** and proving ground. M033-M045 built, in order: a research pass over external parsers, an ADR package, an evidence pipeline prototype, a no-write smoke on real corpus, a unified control surface, a governance memory bridge, a typed governance graph projection, normalized real-corpus continuity, mixed connectivity smoke, linked metadata repair, a combined sidecar probe, a live GROBID probe with architecture guardrail, and a unified trajectory check. This package is the **single entry point for future agents** to understand what was decided, what exists, what is safe, and what is forbidden.

## Table of Contents

| # | Artifact | Purpose | Read when you need to... |
|---|---|---|---|
| 00 | `00-INDEX.md` | this file | orient in the package |
| 01 | `01-north-star.md` | idea, north star, value, capability contract mapping | understand what the project is and why |
| 02 | `02-architecture-layers.md` | C4-style layered architecture with bounded contexts | navigate the system at the right level of detail |
| 03 | `03-adr-decisions.md` | 8 ADRs, D072-D080, traceability matrix | know which decision binds what |
| 04 | `04-module-map.md` | real modules in `src/`, `scripts/`, `doc/`, `artifacts/` | find the actual code/contract/test that implements a decision |
| 05 | `05-evidence-safety.md` | evidence pipeline, fail-closed invariants, gate findings, safety defaults | verify safety boundaries hold and understand prohibited claims |
| 06 | `06-trajectory-ops.md` | M045 trajectory check as ops layer, governance mirror, drift detection | run preflight/closeout and interpret drift signals |
| 07 | `07-2026-assessment.md` | 2026 best-practices assessment and actionable recommendations | plan the next milestone or audit the package itself |

Each artifact is self-contained. **Start with 01**, then jump to whichever artifact answers your current question.

## Non-Authorization Reminder

This synthesis package does **not** authorize:

- production graph import into any GraphDB;
- final GraphDB selection (LadybugDB / FalkorDB / HelixDB / other);
- LadybugDB / FalkorDB / HelixDB writes;
- parser output, sidecar output, or LLM output as accepted truth;
- agentic orchestration of the evidence pipeline;
- bypassing validators, review packets, or the trajectory check;
- treating any codebase-memory mirror as canonical.

The full safety contract lives in `01-north-star.md` (safety defaults section) and `05-evidence-safety.md` (fail-closed invariants and prohibited-claim scan).

## Source Traceability

Every claim in this synthesis package is anchored to a primary source. Source categories:

- **M034 ADR files** at `doc/adr/m034/ADR-*.md` — 8 ADRs
- **GSD decisions** in `.gsd/DECISIONS.md` — D072 to D080 in scope here
- **GSD requirements** in `.gsd/REQUIREMENTS.md` — R001 to R065 in scope here
- **Milestone summaries** in `.gsd/milestones/M{033..045}-*/M{033..045}-*-SUMMARY.md`
- **Architecture guardrail** in `artifacts/m044-grobid-architecture-guardrail/architecture-context-pack.json`
- **Trajectory report** in `artifacts/m045-project-trajectory/current/trajectory-report.json`

The traceability matrix in `03-adr-decisions.md` makes the ADR/D/verifier/artifact chains explicit.

## Verification

This package is verified by:

1. `uv run python scripts/check_project_trajectory.py --output-dir artifacts/m046-synthesis/current` (M045 trajectory check rerun, expect `on_track`)
2. `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py` (M044 architecture guardrail, expect pass)
3. `grep "graph_import_allowed=false" artifacts/m046-synthesis/*.md` (safety default presence)
4. `ls artifacts/m046-synthesis/*.md | wc -l` (≥ 8 files: INDEX + 7 artifacts)
5. Manual self-review: every claim traceable to source

## LLM Reading Notes

- Read `01-north-star.md` first. Do not skip it.
- Use this package as **navigation**, not as the only source of truth. Canonical truth is in `.gsd/` (requirements, decisions), `doc/adr/m034/` (ADRs), and verified milestone summaries.
- Treat this package as **non-canonical and non-authoritative** for graph import, parser adoption, or production orchestration. The ADRs are the binding layer for those.
- If you find a contradiction between this package and an ADR, **the ADR wins**; file a follow-up to correct this package.
