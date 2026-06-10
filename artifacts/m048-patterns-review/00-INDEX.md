# M048 Patterns Review — ActiveGraph, SkillGenome, FalkorDB

> **Source:** external architecture analyses (yoheinakajima/activegraph, jscheiber78/skillgenome, falkordb/falkordb)
> **Audience:** future agents and the user; input for M049-M058 milestone plans
> **Scope:** daily-archive-relevant patterns; explicit "what we adopt", "what we don't", and "what we defer"
> **Status:** research artifact, not a new ADR, not a graph import authorization

## TL;DR

Three external architectures were studied. **None is adopted wholesale.** Patterns are extracted and mapped to the existing roadmap (M049-M058, Phase 1A and 1B):

- **ActiveGraph** — runtime not adopted (we don't have long-running agents); patterns (serial audit + async workers, content-addressed artifacts, deterministic work_id, cascaded gates) applied to Track A (M050-M053, agent layer) where bounded LLM/eval work happens.
- **SkillGenome** — genome model not adopted (we don't have skill fragments); patterns (cascaded gates, semantic prefilter, race/successive halving, fingerprint dedupe) applied to M051 (eval fixtures) and M053 (RLM benchmark) where eval is bounded.
- **FalkorDB** — added as a serious candidate in **M056 (GraphDB comparison matrix)** alongside LadybugDB and HelixDB. ADR-002 still deferred. The matrix now has explicit criteria: license, locality, GraphBLAS support, vector index, UDF support, write semantics, cluster mode, single-graph shard limit.

## Table of Contents

| # | File | Purpose | Read when you need to... |
|---|---|---|---|
| 00 | `00-INDEX.md` | this file | orient in the patterns review |

## Vendored Repos (M048 patterns-review)

Per `05-vendor-source-convention.md`, three external projects were vendored to `/root/vendor-source` and indexed via GitNexus:

| Repo | Path | Commit | Size | Anchor |
|---|---|---|---:|---|
| ActiveGraph | `/root/vendor-source/activegraph` | `f3ed033` | 4.1 MB | 01-activegraph-patterns.md |
| SkillGenome | `/root/vendor-source/skillgenome` | `e9f79eb` | 3.2 MB | 02-skillgenome-patterns.md |
| FalkorDB | `/root/vendor-source/falkordb` | `1a172b1` | 102 MB | 03-falkordb-evaluation.md |

**GitNexus index status:**
- `activegraph`: ✅ indexed (6 447 nodes, 11 857 edges, 242 clusters, 300 flows)
- `skillgenome`: ✅ indexed (2 198 nodes, 2 956 edges, 51 clusters, 63 flows)
- `falkordb`: ⚠️ cloned but NOT indexed (vendored deps/GraphBLAS at 100 MB+ with non-Python scope annotations break the analyzer; clone preserved as code reference, M056 evidence still complete from public docs)

Symbol-level queries (gitnexus_context/impact/query) available for activegraph and skillgenome. For falkordb, use `read` tool on vendored source files directly.
| 01 | `01-activegraph-patterns.md` | ActiveGraph deep-dive + Track A adaptation | plan or review agent-layer work (M050-M053) |
| 02 | `02-skillgenome-patterns.md` | SkillGenome patterns (cascaded gates, race, semantic prefilter, fingerprint) | design eval pipeline (M051, M053) |
| 03 | `03-falkordb-evaluation.md` | FalkorDB as M056 candidate with explicit criteria | compare GraphDB options (M056) |
| 04 | `04-applicability-matrix.md` | pattern × milestone matrix; Track A/B columns; non-applicable rationale | decide what to apply when |
| 05 | `05-vendor-source-convention.md` | how to clone + index external pattern sources in `/root/vendor-source` | vendor a new external repo for pattern study |

## Non-Authorization Reminder

This is a research artifact, not a new ADR, not a graph import authorization. It does **not**:

- authorize graph import into any GraphDB;
- select FalkorDB or any other GraphDB (M056 will produce evidence, not selection);
- enable agentic orchestration (ADR-006 binding);
- modify M045 trajectory check or M044 architecture guardrail;
- bypass any safety contract (5× false defaults).

## Source Traceability

External sources cited in this review:

- ActiveGraph: yoheinakajima/activegraph on GitHub; production guide at docs.activegraph.ai
- SkillGenome: jscheiber78/skillgenome on GitHub; specific modules `fragment.py`, `dag_recombine.py`, `coding_chain_llm.py`, `eval_harness.py`, `behavioral_runner.py`, `canalization.py`, `plausibility.py`, `goal_driven.py`
- FalkorDB: falkordb/falkordb on GitHub; docs at docs.falkordb.com; ar5iv 1905.01294 for GraphBLAS background

Daily-archive sources cross-referenced:

- `artifacts/m046-synthesis/` for current state synthesis
- `.gsd/REQUIREMENTS.md` for R001-R065 mapping
- `doc/adr/m034/ADR-*.md` for binding ADRs
- `.gsd/milestones/M033-M048/` for pattern deployment history

## LLM Reading Notes

- Treat this review as **pattern reference, not adoption decision**. Adoptions are gated by the existing roadmap milestones (M049-M058).
- The ActiveGraph file (01) is **deep-dive** by user request — read it fully if planning Track A work.
- The FalkorDB file (03) is **candidate input** for M056 — it does not pre-select FalkorDB.
- The applicability matrix (04) is the **single source of truth** for "what applies where" in our roadmap.
