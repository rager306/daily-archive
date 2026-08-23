# ADR-068: Schema/Manifest Hierarchy — Meta-Prompt Data Architecture

- Status: Accepted
- Date: 2026-08-23 (revised same day after elegance review)
- Deciders: repo owner + agent (M451)
- Tags: data-architecture, meta-prompt, schema, manifest, versioning, no-hardcode

## 0. One-line Decision

**Schema is the norm, the filesystem is the truth, the Governor is the direct validator, and manifests are derived caches.** Four roles, each doing one thing. Structure is DATA — the code reads schemas and never hardcodes layout.

## 1. Context

Structural chaos accumulated because data layout lived in code (hardcoded paths), in ADRs (prose only), or nowhere. The M445 audit found double-nested catalogs, orphan dictionaries, 66% unresolvable citations. M450 fixed paths but did not establish the **principle** that structure itself must be data.

law-nexus demonstrated the pattern: artifact graph + lifecycle ceilings + authority stack + forbidden edges — all in DATA, checked by a governance tool.

An initial draft of this ADR was reviewed for elegance and corrected: manifests at every level bloat metadata; manifest-vs-FS comparison creates a sync-error class; schema history files duplicate git+ADR; bi-temporality is meaningless for directories. This revision fixes those.

## 2. Decision

### 2.1 The four roles (binding)

| Role | Object | Property |
|---|---|---|
| **Norm** | `SCHEMA.yaml` | What CAN and MUST exist. Machine-readable rules. Versioned. |
| **Truth** | the filesystem | What actually exists. Never edited to please a checker. |
| **Validator** | Governor | Compares truth against norm **directly** — FS ↔ SCHEMA. Reports gaps. |
| **Cache** | `MANIFEST.yaml` | Derived aggregates for humans and fast lookups. Stale = harmless; regenerated. |

Corollaries:

- Governor NEVER validates "manifest ↔ filesystem". That comparison class is deleted. A stale manifest costs a recompute, nothing more.
- Work = closing gaps between FS and SCHEMA. Never editing the schema to match a messy present, never hand-writing manifests.
- Definitions live in exactly ONE place (the schema of the object). Checks consume; they do not restate.

### 2.2 Hierarchy

```
daily-archive/                   ← repo root
├── SCHEMA.yaml                  ← valid top-level dirs, ignored tool dirs, forbidden dirs
├── MANIFEST.yaml                ← CACHE: repo aggregates
│
├── data/
│   ├── SCHEMA.yaml              ← NORM: valid data classes
│   ├── MANIFEST.yaml            ← CACHE: data aggregates
│   ├── sources/
│   │   ├── SCHEMA.yaml          ← NORM: valid source types
│   │   ├── MANIFEST.yaml        ← CACHE: source coverage
│   │   ├── arxiv/
│   │   │   ├── SCHEMA.yaml      ← NORM: arxiv article layout + versioning
│   │   │   ├── MANIFEST.yaml    ← CACHE: article counts per category
│   │   │   └── <category>/<paper_id>/…          ← follows arxiv SCHEMA
│   │   ├── textbook/
│   │   │   ├── SCHEMA.yaml      ← NORM: chapters, not articles
│   │   │   └── MANIFEST.yaml    ← CACHE
│   │   └── …                    ← blog/, nature/, course/ (schemas; manifests optional)
│   ├── processed/
│   │   ├── SCHEMA.yaml          ← NORM: processing layers + lifecycle values
│   │   │                           (references data/schemas/article_schema.yaml)
│   │   └── MANIFEST.yaml        ← CACHE: stage coverage
│   │   └── <paper_id>/manifest.yaml  ← per-article ARTIFACT metadata (see 2.5)
│   ├── ontology/SCHEMA.yaml     ← NORM: dictionary types
│   ├── graphs/SCHEMA.yaml       ← NORM: snapshot types, D127 lock
│   ├── gold_standard/SCHEMA.yaml← NORM: fixture rules, immutable
│   ├── graph_tasks/SCHEMA.yaml  ← NORM
│   └── schemas/SCHEMA.yaml      ← NORM: which schema files may exist here
│
├── governor/SCHEMA.yaml + MANIFEST.yaml
├── crates/SCHEMA.yaml
├── scripts/SCHEMA.yaml
├── doc/SCHEMA.yaml
├── artifacts/SCHEMA.yaml
├── archive/SCHEMA.yaml
└── logs/SCHEMA.yaml
```

**Manifests exist ONLY at aggregate levels** (root, data, sources, processed, arxiv, textbook, governor). Leaves (a single article, a single dictionary) carry artifact metadata, not directory manifests. This bounds manifest count at ~7 regardless of corpus size.

### 2.3 Schema composition rules

1. **Child inherits parent.** `sources/arxiv/SCHEMA.yaml` is a refinement of `sources/SCHEMA.yaml`.
2. **Each source type has its own schema.** An arxiv article and a textbook chapter are different shapes with different lifecycles.
3. **Schemas are machine-readable.** Every rule the Governor enforces is declared in the schema itself (`valid_subdirs`, `valid_files`, `forbidden`, `ignore`, `lifecycle_values`, …). New checks require no new checker code — only new schema data.
4. **One definition site.** The article layer/lifecycle/invariant definition lives in `data/schemas/article_schema.yaml`. Everything else (Governor checks, pipeline code, docs) references it.
5. **Schemas are versioned but their history is git+ADR.** `schema_version` + `adr_ref` fields. A version bump cites the ADR that motivated it. No separate SCHEMA.history.yaml — that duplicates git and the ADR-INDEX (both already append-only).

### 2.4 Naming convention (binding)

| Name | Role | Where |
|---|---|---|
| `SCHEMA.yaml` | norm for a structural directory | every structural directory |
| `MANIFEST.yaml` (UPPERCASE) | derived cache of a directory's aggregates | aggregate levels only (~7 files) |
| `manifest.yaml` (lowercase) | metadata of ONE artifact (article, version) | inside the artifact's directory |

Legacy names (`source_manifest.json`, `textbook_manifest.json`, per-article `manifest.json`) are declared legacy; migration folds them into the lowercase artifact manifest. Migration is scheduled, not urgent — the schema tolerates both during transition.

### 2.5 The article artifact (leaf level)

Each processed article directory carries a lowercase `manifest.yaml` — self-sufficient metadata:

- `schema_version` — which article manifest schema version it follows
- `paper_id`, `source` locator (type, category, sha256, acquired_at)
- `lifecycle` — **derived** from the last `lifecycle_events` entry, never hand-set
- `lifecycle_events` — append-only log (see 2.6)
- `artifacts` — the artifact graph: layer → {file, stage, sha256, derived_from}
- `revision` — present only for versioned articles (see 2.7)

A human opening one `manifest.yaml` sees everything about that article without walking schemas.

Layer definitions, authority stack, cross-layer invariants and forbidden edges live in `data/schemas/article_schema.yaml` (the single definition site). The Governor consumes that file; it does not restate the rules.

### 2.6 Temporality

Two axes — **but only where both are meaningful**:

| Level | Axes | Rationale |
|---|---|---|
| Per-article lifecycle events | `observed_at` (valid_time) + `recorded_at` (transaction_time) | A fact about the external world (arXiv published v2) vs when our pipeline recorded it |
| Directory manifests | `recorded_at` only | A directory has no "truth in the external world"; the second axis would be noise |

```yaml
lifecycle_events:            # append-only; regeneration preserves the log
  - stage: parsed
    recorded_at: 2026-08-22T14:03:11Z
    observed_at: 2026-08-01T00:00:00Z   # present only when externally anchored
    actor: grobid-profile-structural
    evidence_sha256: "…"
```

- `lifecycle: extracted` is DERIVED from the last event.
- Manifest regeneration recomputes derived fields and appends new events; it never rewrites or drops history.
- `recorded_at` is monotonic within one article.

### 2.7 Article versioning — flat by default

```
data/sources/arxiv/<category>/<paper_id>/       ← SINGLE version (the default)
    source/<id>.pdf …
    identity.json

data/sources/arxiv/<category>/<paper_id>/       ← MULTI version (appears on demand)
    v1/… v2/…
    revision_graph.json     ← supersedes/same_as/derived_from edges,
                              each edge carrying observed_at + recorded_at
```

- Single-version articles stay FLAT. No empty `v1/` nesting for 800 articles to serve the rare multi-version case.
- When a second version arrives: versions move into `v1/`, `v2/` directories (append-only — old content untouched), `revision_graph.json` appears.
- Active version = latest with lifecycle ≥ parsed.
- `revision_graph` is acyclic; old version layers are immutable once superseded.

### 2.8 Meta-prompt principle (binding)

```
RULE: Code reads schemas. Code does not define structure.

Applies to ALL data in the repository, not just data/:
- paths            → data/path_config.yaml
- lifecycle values → the SCHEMA that owns them
- source shapes    → per-source SCHEMA.yaml
- article layers   → data/schemas/article_schema.yaml
- processing stages→ data/ontology/pipeline_stage_contracts.yaml
- artifact types   → artifacts/SCHEMA.yaml

Governor enforces:
- SCHEMA.yaml exists for every structural directory
- FS conforms to SCHEMA (valid/forbidden rules) — validated DIRECTLY
- invariants declared in schemas actually hold on the FS
- manifest caches exist at aggregate levels and carry generated_at
```

### 2.9 Governor integration

New dimension: `schema_hierarchy`. The checker is a **generic engine**:

1. Reads the list of structural/aggregate directories from its contract YAML (DATA).
2. For each, loads `SCHEMA.yaml` and validates the filesystem against the rules **declared in that schema** — valid subdirs, file patterns, forbidden names, ignore lists. No structural knowledge is hardcoded in the checker.
3. Reports each violation as a drift flag naming the schema rule violated.

Invariants beyond structure (e.g., `ast_nodes == sections_count`) are read from `article_schema.yaml` and checked against artifact contents — same engine, data-driven rules.

## 3. Architecture

```
┌─────────────────┐   declares    ┌──────────────────┐
│   SCHEMA.yaml   │◄──────────────│       ADR        │
│  (norm, data)   │               └──────────────────┘
└────────┬────────┘
         │ validated against
┌────────▼────────┐   reports gaps   ┌──────────────┐
│  FILESYSTEM     │──────────────────►│   GOVERNOR   │
│  (truth)        │                  └──────────────┘
└────────┬────────┘                         │ flags
         │ aggregated by pipeline           ▼
┌────────▼────────┐                  ┌──────────────┐
│  MANIFEST.yaml  │  (cache, stale   │  drift report│
│  (~7 files)     │   is harmless)   └──────────────┘
└─────────────────┘
```

## 4. Consequences

+ One norm per structure, one truth, one validator, cheap caches.
+ Governor checks structure without manifest-sync errors (that class is deleted).
+ Adding a source type = one SCHEMA.yaml, zero code changes.
+ Versioning appears only where versions exist (flat default).
+ Manifest count bounded (~7) regardless of corpus growth.
+ Bi-temporal semantics only where both axes mean something.

− Initial cost: write schemas for existing directories; migrate legacy manifest names on a schedule.
− Schema changes require discipline (version bump + ADR reference).
− Per-article manifest content validation is a later wave (structure first).

## 5. Implementation waves

| Wave | Content | Status |
|---|---|---|
| W1 | This ADR: roles, hierarchy, temporality, versioning | Done |
| W2 | SCHEMA.yaml at structural levels; MANIFEST.yaml at aggregate levels | Done (this milestone) |
| W3 | Governor `schema_hierarchy` generic engine + contract | Done (this milestone) |
| W4 | Per-article lowercase manifest.yaml with lifecycle_events | Next |
| W5 | Manifest auto-refresh hooks in pipeline stages | Later |
| W6 | Legacy manifest name migration | Later |

## 6. Testing requirements (ADR-051)

- Governor contract tests: schema pair existence, FS conformance (positive/negative fixtures in governor/fixtures/schema_hierarchy/)
- Negative fixture: `old_artifacts_graphs/`-style forbidden directory must flip the check
- Unit: schema loading, rule evaluation, gap reporting

## 7. Acceptance criteria

- Every structural directory has SCHEMA.yaml with `schema_version`
- Aggregate levels (~7) have MANIFEST.yaml with `generated_at`; leaves do not
- Governor `schema_hierarchy` dimension validates FS ↔ SCHEMA directly, data-driven
- Existing gap `old_artifacts_graphs/` is flagged until closed
- No hardcoded lifecycle values or path structures in checker code

## 8. Supersession / amendments

- Builds on ADR-067 (normalized layout): 067 defined WHERE things live; 068 defines HOW structure is declared, validated and versioned.
- Replaces the initial M451 draft (manifests everywhere, manifest↔FS checks, SCHEMA.history.yaml) — corrected after elegance review.

## 9. Open points

- Bulk tool for re-versioning manifests when schema_version bumps (lazy migration is default; bulk optional).
- Should per-article `manifest.json` (M450) migrate to `manifest.yaml` in W6 or absorb YAML events first?

---

## LLM Reading Notes

- **Four roles:** SCHEMA = norm (what must/may exist), FS = truth, Governor = direct validator, MANIFEST = derived cache. Never compare manifest to FS; never edit schema to match reality.
- **Direction of fit:** schemas are NORMATIVE. Governor reports gaps; the work is closing them.
- **Manifests are bounded:** only ~7 aggregate levels. An article's metadata is its own lowercase `manifest.yaml`, self-sufficient, with lifecycle DERIVED from an append-only event log.
- **Temporality:** bi-temporal (observed_at/recorded_at) only for facts about articles; directories get recorded_at only.
- **Versioning:** flat layout is the default; v1/v2 directories + revision_graph.json appear only when a second version actually exists. Schema history = git + ADR (schema_version + adr_ref), no duplicate history files.
- **Anti-pattern prevented:** hardcoding structure in code. New source type → new SCHEMA.yaml, not a Rust struct. New check → new schema data, not new checker code.
