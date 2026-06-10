# 05 — Vendor-Source Convention

> **Source:** established daily-archive practice (28 repos already in `/root/vendor-source`, 12 indexed in GitNexus); formalized here for future agents
> **Scope:** how and when to clone external repositories for study, without adopting them as runtime dependencies
> **Status:** convention, not a binding ADR (per ADR-007 quant-mind pattern source; consistent with M033 research pattern)

## 0. Why This Convention

daily-archive's M033-M045 evolution studied several external libraries (GROBID, OpenDataLoader, Adaptix, quant-mind) without adopting them as runtime dependencies. The same pattern is now needed for M048 patterns-review (ActiveGraph, SkillGenome, FalkorDB).

**Without vendoring, we have:**
- README-quality understanding only
- No code-level evidence for architectural decisions
- Future agents must re-read external docs to verify claims
- Pattern recommendations cannot be anchored to actual symbols

**With vendoring + indexing, we have:**
- Local clone pinned to specific commit
- GitNexus knowledge graph for symbol-level queries
- `gitnexus_context`, `gitnexus_impact`, `gitnexus_query` tools available to future agents
- code-level evidence for ADR/M-claims (e.g., "ActiveGraph's CONTRACT.md defines X")

## 1. Where

`/root/vendor-source/<repo-name>/`

The directory already exists with 28 repos. Each repo is a **flat** clone (not submodule, not symlink).

## 2. When to Vendor

Vendor a repo when **at least one** of these is true:

- A milestone plans to study its architecture, code, or contracts
- A pattern-review identifies a candidate pattern source
- An ADR cites an external project as pattern source (e.g., ADR-007 quant-mind)
- A roadmap item (M-series) requires code-level evidence from external

**Do not vendor:**

- Projects mentioned only in passing in chat/PR
- Libraries we already use as runtime dependencies (those are in `pyproject.toml` / `uv.lock`)
- Repos that duplicate functionality we already have

## 3. How to Vendor

```bash
# 1. Clone (shallow is OK; full is preferred for symbol-level indexing)
git clone https://github.com/<owner>/<repo>.git /root/vendor-source/<repo-name>

# 2. Pin to a specific commit/tag for reproducibility
cd /root/vendor-source/<repo-name>
git checkout <commit-sha-or-tag>
# Record the commit in patterns-review or ADR for traceability

# 3. Index via GitNexus
gitnexus analyze /root/vendor-source/<repo-name>

# 4. (Optional) Mirror to codebase-memory MCP for fast recall
# Requires user approval of the MCP server first; then via mcp_call
```

**Note on depth:** `--depth 1` is faster but loses history. For projects where symbol history matters (e.g., when an ADR cites "version X had feature Y, version Z removed it"), use full clone. For pure pattern study, shallow is fine.

## 4. Constraints

| Constraint | Why |
|---|---|
| **Local clone only** | Vendored repos are read-only references. No network calls into them at runtime. |
| **Pin to specific commit** | Reproducibility. ADR-007 quant-mind study cited specific commit; same convention. |
| **No runtime adoption** | Vendored repos do not enter `pyproject.toml`. They are **pattern sources**, not dependencies. |
| **Document the why** | Each vendored repo should be referenced in a milestone, ADR, or patterns-review with the rationale. |
| **No auto-sync** | Vendored clones are frozen at clone time. Updates require explicit re-clone + re-index + documentation. |

## 5. GitNexus Indexing

GitNexus builds a knowledge graph:

- symbols (functions, classes, methods)
- edges (calls, imports, type refs)
- clusters (semantic groupings)
- processes (call chains, execution flows)

After `gitnexus analyze`, the repo is registered and available for:
- `gitnexus_context <symbol>` — 360° view of a symbol
- `gitnexus_impact <symbol>` — blast radius of changes
- `gitnexus_query <concept>` — execution flows related to a concept
- `gitnexus_detect_changes --scope all|unstaged|staged|compare`

**Current state of `/root/vendor-source` and GitNexus:**

| Repo | Path | Indexed | Symbols | Edges | Anchor |
|---|---|---|---:|---:|---|
| gsd-pi | `/root/vendor-source/gsd-pi` | ✅ | 90 583 | 176 595 | the GSD-pi engine itself |
| daily-archive | `/root/daily-archive` | ✅ | 23 514 | 37 523 | our project |
| grobid | `/root/vendor-source/grobid` | ✅ | 31 613 | 58 016 | M033 sidecar research |
| opendataloader-pdf | `/root/vendor-source/opendataloader-pdf` | ✅ | 7 654 | 17 963 | M033 sidecar research |
| codegraph | `/root/vendor-source/codegraph` | ✅ | 6 666 | 14 381 | related to indexing |
| code-index-mcp | `/root/vendor-source/code-index-mcp` | ✅ | 2 808 | 6 780 | related to indexing |
| quant-mind | `/root/vendor-source/quant-mind` | ✅ | 1 327 | 2 497 | M033 pattern source (ADR-007) |
| code-ontology-spec | `/root/vendor-source/code-ontology-spec` | ✅ | 84 | 80 | related to indexing |
| (others) | `/root/vendor-source/*` | not indexed | — | — | other projects (law-nexus, postiz-app, fd, etc.) |

**Note:** Grobid is large (13 359 files, 31 613 symbols) because it includes the source. Index quality is best when the cloned repo has been pruned of build artifacts.

## 6. M048 Patterns-Review: New Vendors Required

Per the M048 patterns-review (`artifacts/m048-patterns-review/`), three external projects were studied:

| Project | URL | Currently vendored? | Currently indexed? |
|---|---|---|---|
| ActiveGraph | https://github.com/yoheinakajima/activegraph | No | No |
| SkillGenome | https://github.com/jscheiber78-droid/skillgenome | No | No |
| FalkorDB | https://github.com/FalkorDB/FalkorDB | No | No |

**Recommendation:** vendor + index all three to support M049-M058 roadmap with code-level evidence.

After vendoring:

- `gitnexus_context production` in FalkorDB → evidence for M056 GraphDB comparison
- `gitnexus_context work_id` in ActiveGraph/SkillGenome → pattern reference for M050
- `gitnexus_query score UDF` in FalkorDB → evidence for M058 tier structure

## 7. Codebase-Memory MCP Integration

`codebase-memory-mcp` is configured (per `~/.gsd/mcp.json`) and indexes code for fast ADR/R/D recall mirror (per D075). After vendoring:

1. User approval of the MCP server is required (per daily-archive `.gsd/PREFERENCES.md` MCP rules)
2. Once approved, use `mcp_call(server="codebase-memory-mcp", tool="...", args=...)` to mirror vendored ADR-equivalents
3. ADR content from `doc/adr/` in each vendored repo (if present) can be mirrored to daily-archive's `.codebase-memory/` mirror

**Caveat (per D076 + M039 lesson):** `ingest_traces` accepts calls but does NOT implement runtime edge creation. The MCP graph is non-canonical and non-authoritative.

## 8. Re-Sync Policy

Vendored clones are **frozen at clone time** by default. Re-sync requires:

1. Explicit decision (per milestone or ADR)
2. Re-clone at new commit
3. Re-index via GitNexus
4. Update patterns-review or ADR with new commit hash
5. Commit + push

**Do not auto-sync.** Stale clones are a feature, not a bug — they preserve the exact code version that was studied.

## 9. What NOT to Do

| Anti-pattern | Why not |
|---|---|
| Add vendored repo to `pyproject.toml` | No runtime adoption (would violate ADR-007 + M033 research pattern) |
| Symlink vendored repo | Loses commit pinning; harder to delete cleanly |
| Auto-sync via cron | Stale clones are valuable; updates are explicit decisions |
| Vendor duplicates of existing repos | `/root/vendor-source/forx` and `/root/vendor-source/forxq` are different projects but similar name; check before cloning |
| Vendor without documenting why | Convention requires linkage to milestone, ADR, or patterns-review |

## 10. Worked Example: Vendoring ActiveGraph (M048)

```bash
# 1. Clone to /root/vendor-source
git clone https://github.com/yoheinakajima/activegraph.git /root/vendor-source/activegraph

# 2. Pin to specific commit
cd /root/vendor-source/activegraph
git checkout <commit-sha>  # record in 01-activegraph-patterns.md

# 3. Index via GitNexus
gitnexus analyze /root/vendor-source/activegraph
# Output: stats (files, symbols, edges), clusters, processes

# 4. Update patterns-review
# Edit 01-activegraph-patterns.md: add "Vendored at <commit> per this convention"
# Edit 00-INDEX.md: add cross-reference to indexed repo

# 5. (Optional) Mirror ADR-equivalents to codebase-memory
# Requires user approval; not required for daily-archive workflow

# 6. Commit + push the documentation update
```

## 11. LLM Reading Notes

- **Vendor-source is for pattern study, not adoption.** Per ADR-007 (quant-mind) and M033 research pattern.
- **Each vendored repo needs a "why" link** in a milestone, ADR, or patterns-review.
- **Re-sync is explicit, not automatic.** Stale clones are valuable.
- **Index via GitNexus, not via MCP runtime.** The MCP mirror is for fast recall, not symbol-level impact.
- **Today's 28 repos in `/root/vendor-source` are already a working practice** — this convention formalizes it.

## 12. Cross-References

- INDEX: `00-INDEX.md`
- ActiveGraph patterns: `01-activegraph-patterns.md`
- SkillGenome patterns: `02-skillgenome-patterns.md`
- FalkorDB evaluation: `03-falkordb-evaluation.md`
- Applicability matrix: `04-applicability-matrix.md`
- ADR-007 (quant-mind pattern source): `doc/adr/m034/ADR-007-quantmind-pattern-source-not-runtime-dependency.md`
- D075 (hybrid governance memory): `.gsd/DECISIONS.md`
- D076 (typed graph projection): `.gsd/DECISIONS.md`
