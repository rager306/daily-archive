# 05 — Evidence and Safety Layer

> **Source:** all M033-M045 milestones, `scripts/check_project_trajectory.py`, `scripts/verify_m044_sidecar_architecture_guardrail.py`, contracts in `doc/contracts/m034-universal-kb/`
> **Synthesis layer:** 5 of 7
> **Status:** 0 high-severity drift in latest trajectory report (M045)

## 0. The Single Safety Contract

Every artifact in this project, regardless of where it is produced, must carry the following five safety defaults **as false**:

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

These five flags are the **single safety contract** for daily-archive. No other safety claim supersedes them. The defaults flip to true **only** when a future explicit graph promotion / import milestone is accepted.

## 1. Safety Defaults Origin and Enforcement

| Default | Source | Enforced by |
|---|---|---|
| `graph_import_allowed=false` | ADR-005, ADR-002 | M035 contracts, M036 audit, M044 guardrail, M045 trajectory |
| `graphdb_written=false` | ADR-002, ADR-005 | M035 substrate rehearsal (no real writes) |
| `ladybugdb_written=false` | ADR-002 | M035 substrate rehearsal, M045 prohibited-claim scan |
| `production_import_attempted=false` | ADR-005 | M035/M036/M037 verifiers |
| `import_eligible=false` | R029, R038 | M022 final gate, M035 contracts |

## 2. Evidence Pipeline (Stage-by-Stage)

The evidence pipeline has nine stages, each producing a durable artifact. Every stage preserves the five safety defaults.

### Stage 1: Catalog & Intake

- **Module:** `data/article_catalog/`, `scripts/select_m036_real_corpus_smoke_batch.py`
- **Inputs:** user request, arXiv list (or local source)
- **Output:** `manifest.json` (5-20 articles), `index.json`
- **Artifact:** `data/article_catalog/catalog.json`, `index.json`
- **Safety invariant:** no raw text, no PDF, only metadata

### Stage 2: Acquisition

- **Module:** `arxiv_archive/article_loader.py`, `scripts/build_m031_catalog_backed_replay_selection.py`
- **Inputs:** manifest
- **Output:** source files, loader evidence
- **Artifact:** per-article source manifest
- **Safety invariant:** no network refresh unless explicitly authorized

### Stage 3: Conversion

- **Module:** `arxiv_archive/md_converter.py`, `arxiv_archive/full_text.py`
- **Inputs:** source files
- **Output:** parser-ready artifact (markdown or full text)
- **Artifact:** per-article parser-ready artifact + diagnostics
- **Safety invariant:** `graph_import_allowed=false` always

### Stage 4: Chunking

- **Module:** `arxiv_archive/chunk_import_contract.py`, `arxiv_archive/structure_aware_chunking.py`
- **Inputs:** parser-ready artifact
- **Output:** chunk packages with stable IDs, source spans, parent-child lineage
- **Artifact:** chunk package + diagnostics
- **Safety invariant:** `import_eligible=false` until reviewed

### Stage 5: Sidecar Probes (optional)

- **Module:** `arxiv_archive/universal_kb_sidecar_boundary.py`, `scripts/probe_m043_*`, `scripts/run_m044_live_grobid_candidate_probe.py`
- **Inputs:** parser-ready artifact
- **Output:** candidate packets (TEI summary hash, OpenDataLoader JSON, Adaptix mapped)
- **Artifact:** `artifacts/m043-combined-sidecar-probe/sidecar-packets.json`, `artifacts/m044-grobid-architecture-guardrail/live-grobid-candidate-packets.json`
- **Safety invariant:** candidates only, never graph truth

### Stage 6: Review & Readiness

- **Module:** `arxiv_archive/universal_kb_review_assistance.py`, `arxiv_archive/reviewer_packet_prototype.py`
- **Inputs:** chunk evidence + sidecar candidates
- **Output:** review packets, readiness handoff
- **Artifact:** `artifacts/m022-*/final-gate.json`, `artifacts/m024-*/` validation closure
- **Safety invariant:** no promotion authority, no LLM approval

### Stage 7: Durable Queue (infrastructure)

- **Module:** `arxiv_archive/universal_kb_queue.py`
- **Inputs:** jobs from any stage
- **Output:** durable job state with leases, heartbeats, retries
- **Artifact:** SQLite queue database
- **Safety invariant:** no writes to GraphDB

### Stage 8: Audit

- **Module:** `scripts/audit_m036_real_corpus_smoke.py`, `scripts/audit_m042_connectivity_groups.py`
- **Inputs:** all stage outputs
- **Output:** audit JSON + Markdown
- **Artifact:** `artifacts/m036-real-corpus-no-write-smoke/audit.{json,md}`, `artifacts/m042-linked-metadata-readiness/connectivity-audit.{json,md}`
- **Safety invariant:** verifies all 5 defaults remain false

### Stage 9: Trajectory & Governance Mirror

- **Module:** `scripts/check_project_trajectory.py`, `scripts/sync_codebase_memory_governance.py`
- **Inputs:** all artifacts + `.gsd/`, `doc/adr/m034/`
- **Output:** `trajectory-report.json`, `.codebase-memory/adr.md`, `.codebase-memory/governance-graph.json`
- **Artifact:** `artifacts/m045-project-trajectory/current/trajectory-report.{json,md}`
- **Safety invariant:** derived, non-canonical, no graph writes

## 3. Prohibited-Claim Scan (M045)

The M045 trajectory check runs a **prohibited-claim scan** over README, recent milestone summaries, and synthesis files. The scan has four regex patterns and eleven counterterms that exempt legitimate safety language.

### 3.1 Four prohibited patterns (regex)

| Pattern | Detects |
|---|---|
| `graph_import_authorized` | "graph import" within 80 chars of "authorized/allowed/enabled" |
| `fact_promotion_allowed` | "fact promotion" or "promoted facts" within 80 chars of "authorized/allowed/enabled" |
| `production_import_authorized` | "production import" within 80 chars of "authorized/allowed/enabled" |
| `raw_payload_promoted` | "raw TEI/text/full text/embedding/vector" within 80 chars of "persisted/promoted/imported" |

### 3.2 Eleven counterterms (false-positive protection)

```
no graph import
not authorized
not persisted
not promoted
not imported
prohibited
disabled
false
blocked
before any
only advance
```

A match is only a real prohibited claim if **none** of the counterterms appear in the matched phrase **and** no local negation (`no`/`not`) precedes the match.

### 3.3 Scan targets

- `README.md`
- Recent milestone summaries (last 6 by mtime)
- Any new file passed via `--codebase-memory-snapshot` (read-only, non-canonical)

This scan is what makes the trajectory check both **strict** (catches overclaim) and **useful** (does not false-positive on legitimate safety language).

## 4. Gate Findings (Q3, Q4 patterns)

The M033+ template embeds gate findings in PLAN files (Q3, Q4 sections). These are derived during slice planning and remain visible after slice completion.

| Gate | Question | Pattern |
|---|---|---|
| Q3 | Quality of work | scope, dependencies, observability requirements, failure modes, handoff constraints |
| Q4 | Requirement coverage | verification commands, observability, failure modes, integration closure |

Findings are `Verdict: pass.` or `Verdict: needs-attention.` M045 trajectory check counts `needs-attention` as drift.

## 5. Architecture Guardrail (M044 / D079)

The M044 sidecar architecture guardrail `scripts/verify_m044_sidecar_architecture_guardrail.py` is the **mandatory preflight** before any future sidecar / graph-readiness work. It enforces:

- ADR-003 (durable lazy async evidence pipeline) is referenced
- ADR-004 (sidecars as candidate evidence) is referenced
- ADR-005 (no direct extractor to GraphDB) is referenced
- ADR-007 (quant-mind pattern source) is referenced
- D078 (candidate-only sidecar packets) is referenced
- D079 (architecture guardrail preflight) is referenced
- All artifacts have `graph_import_allowed=false`
- All artifacts have `import_eligible=false`

The guardrail context pack at `artifacts/m044-grobid-architecture-guardrail/architecture-context-pack.json` is the machine-readable specification. **D079 records this as mandatory.**

## 6. Reverse ADR Audit Result

Performed during S04 (module map) and re-confirmed here:

| ADR | Expected invariant | Code finding |
|---|---|---|
| ADR-002 | no final GraphDB selection | no `falkordb` / `helixdb` imports in src/ — PASS |
| ADR-003 | durable lazy async evidence pipeline | queue with WAL, leases, heartbeats present — PASS |
| ADR-004 | sidecars as candidate evidence only | `universal_kb_sidecar_boundary.py` enforces — PASS |
| ADR-005 | no direct parser to GraphDB | no GraphDB write paths — PASS |
| ADR-006 | agents do not orchestrate | `universal_kb_review_assistance.py` is diagnostic-only — PASS |
| ADR-007 | quant-mind as pattern source | no runtime imports of quant-mind — PASS |

**Result: 0 reverse ADR violations at code level.** The M044 guardrail is the durable runtime enforcement.

## 7. Cross-References

- North star: `01-north-star.md` (capability contract mapping)
- Architecture: `02-architecture-layers.md` (bounded contexts)
- Decisions: `03-adr-decisions.md` (ADRs, traceability)
- Modules: `04-module-map.md` (verifier scripts)
- Trajectory: `06-trajectory-ops.md` (M045 trajectory check)
- Assessment: `07-2026-assessment.md` (2026 best practices)

## 8. LLM Reading Notes (binding)

- **Read this layer before any code that might affect graph state** — parsers, sidecars, queue, substrate rehearsal.
- **Five safety defaults are the single contract.** If a future code change flips one to true, that is a binding violation requiring a future explicit graph promotion / import milestone.
- **The M044 guardrail is mandatory preflight** before any sidecar / graph-readiness work (D079).
- **The M045 trajectory check is mandatory preflight** before any planning or closeout (D080).
- **Prohibited-claim scan is the audit that catches overclaim.** If it triggers, fix the language, do not weaken the scan.
- **Reverse ADR audit is ongoing.** Add it to the M045 trajectory check as a routine dimension (follow-up).
