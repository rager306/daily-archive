# ADR-068: Schema/Manifest Hierarchy — Meta-Prompt Data Architecture

- Status: Proposed
- Date: 2026-08-23
- Deciders: repo owner + agent (M451)
- Tags: data-architecture, meta-prompt, schema, manifest, versioning, no-hardcode

## 0. One-line Decision

Every level of the data tree carries a **SCHEMA** (what is possible) and a **MANIFEST** (what exists now). Both are DATA files — the code reads them and never hardcodes structure.

## 1. Context

The project accumulated structural chaos because data layout lived in code (hardcoded paths), in ADRs (prose only), or in nobody's head. The M445 audit found double-nested catalogs, orphan dictionaries, and 66% unresolvable citations. The M450 restructure fixed paths but didn't establish the **principle** that structure itself must be data.

The law-nexus project demonstrated the pattern: artifact graph + lifecycle ceilings + authority stack + forbidden edges — all in DATA, checked by Governor.

## 2. Decision

### 2.1 The Schema/Manifest pair

**Every top-level directory in the repository** carries a SCHEMA.yaml + MANIFEST.yaml pair. Not just `data/` — also `governor/`, `crates/`, `scripts/`, `doc/`, `artifacts/`, `archive/`, `logs/`. The repo root itself has a SCHEMA + MANIFEST defining what top-level directories are valid.

| File | Role | Mutability |
|---|---|---|
| `SCHEMA.yaml` | What CAN exist: types, structure, rules, lifecycle values, versioning model | Versioned (v1 → v2), changes require schema_version bump |
| `MANIFEST.yaml` | What EXISTS now: counts, statuses, coverage, timestamps, active items | Updated automatically by pipeline stages |

Neither is hand-edited during normal operation. Schemas change through ADR; manifests change through pipeline execution.

### 2.2 Hierarchy (top-down)

```
daily-archive/                   ← REPO ROOT
├── SCHEMA.yaml                  ← what top-level directories are valid
├── MANIFEST.yaml                ← repo state: project phase, active milestone, counts
│
├── data/                        ← scientific data
│   ├── SCHEMA.yaml              ← what types of data can live here
│   ├── MANIFEST.yaml            ← what data exists, sizes, last_updated
│   ├── sources/
│   │   ├── SCHEMA.yaml          ← what source types are valid
│   │   ├── MANIFEST.yaml        ← which sources connected, coverage
│   │   ├── arxiv/
│   │   │   ├── SCHEMA.yaml      ← arxiv article structure + versioning
│   │   │   ├── MANIFEST.yaml    ← 801 articles, 31 categories
│   │   │   └── <category>/<id>/ ← articles follow arxiv SCHEMA
│   │   ├── textbook/
│   │   │   ├── SCHEMA.yaml      ← DIFFERENT structure (chapters, not articles)
│   │   │   ├── MANIFEST.yaml    ← 2 textbooks, 29 chapters
│   │   │   └── <textbook_id>/   ← chapters follow textbook SCHEMA
│   │   └── …each source has its own SCHEMA + MANIFEST
│   ├── processed/
│   │   ├── SCHEMA.yaml          ← processing layers (lifecycle model)
│   │   ├── MANIFEST.yaml        ← articles at each stage
│   │   └── <paper_id>/          ← follows processed SCHEMA
│   ├── ontology/
│   │   ├── SCHEMA.yaml          ← what dictionary types are valid
│   │   └── MANIFEST.yaml        ← which dictionaries, versions
│   ├── graphs/
│   │   ├── SCHEMA.yaml
│   │   └── MANIFEST.yaml        ← snapshots, sizes, D127 status
│   └── gold_standard/
│       ├── SCHEMA.yaml
│       └── MANIFEST.yaml
│
├── governor/                    ← control-plane data (separated from scientific)
│   ├── SCHEMA.yaml              ← what fixture types are valid
│   ├── MANIFEST.yaml            ← which contracts, which fixtures
│   └── fixtures/
│       └── <contract>/          ← follows governor SCHEMA
│
├── crates/                      ← Rust code
│   ├── SCHEMA.yaml              ← crate naming, dependency rules, hexagonal
│   └── MANIFEST.yaml            ← which crates exist, versions, test counts
│
├── scripts/                     ← build/check scripts
│   ├── SCHEMA.yaml              ← what script types are valid
│   └── MANIFEST.yaml            ← which scripts, entry points
│
├── doc/                         ← documentation
│   ├── SCHEMA.yaml              ← doc types (ADR, review, …)
│   └── MANIFEST.yaml            ← which docs, dates, statuses
│
├── artifacts/                   ← scratch space (current wave)
│   ├── SCHEMA.yaml              ← what artifact types are valid + GC policy
│   └── MANIFEST.yaml            ← current artifacts, age, readers
│
├── archive/                     ← cold storage (D237)
│   ├── SCHEMA.yaml              ← what can be archived, retention policy
│   └── MANIFEST.yaml            ← what's archived, from when, sizes
│
└── logs/                        ← structured logs (ADR-052)
    ├── SCHEMA.yaml              ← log format, rotation
    └── MANIFEST.yaml            ← current log files, dates
```

### 2.3 Schema composition rules

1. **Child schema inherits from parent.** `sources/arxiv/SCHEMA.yaml` extends `sources/SCHEMA.yaml`.
2. **Each source type has its own schema.** An arxiv article and a textbook chapter have different structures, different parsers, different lifecycles.
3. **Schemas are versioned.** `schema_version: v1` → `v2` when structure changes. Old manifests remain readable.
4. **A schema can reference other schemas.** `processed/SCHEMA.yaml` says "input follows sources/arxiv/SCHEMA.yaml".

### 2.4 Manifest composition rules

1. **Manifests are machine-generated.** Pipeline stages update them. No hand-editing.
2. **Manifests track lifecycle per item.** Each article in `processed/MANIFEST.yaml` has a lifecycle value.
3. **Manifests are the Governor's source of truth.** Governor checks manifest invariants, not raw file existence.
4. **Manifests record provenance.** Every entry has timestamps, source references, checksums.

### 2.5 Versioning (per-source)

```
sources/arxiv/<category>/<paper_id>/
├── v1/                      ← version 1 (immutable after v2 exists)
│   ├── source/*.pdf
│   ├── parsed/article.json
│   └── manifest.yaml        ← manifest for v1
├── v2/                      ← version 2
│   ├── source/*.pdf
│   ├── parsed/article.json
│   └── manifest.yaml        ← manifest for v2
└── revision_graph.json      ← links between versions
```

- New version → new directory, old untouched (append-only).
- `revision_graph.json` tracks `supersedes`, `same_as`, `derived_from`.
- Active version = latest with lifecycle ≥ parsed.

### 2.6 Meta-prompt principle (binding)

```
RULE: Code reads schemas. Code does not define structure.

This applies to ALL data in the repository, not just data/:
- No hardcoded paths in crates/ → read from path_config.yaml
- No hardcoded lifecycle values → read from SCHEMA.yaml
- No hardcoded source structures → read from source SCHEMA.yaml
- No hardcoded processing stages → read from processed SCHEMA.yaml
- No hardcoded artifact types → read from artifacts/SCHEMA.yaml
- No hardcoded log formats → read from logs/SCHEMA.yaml
- No hardcoded crate dependency rules → read from crates/SCHEMA.yaml

Governor enforces:
  - SCHEMA.yaml + MANIFEST.yaml exists for every top-level directory
  - Child SCHEMA references parent SCHEMA (inheritance chain)
  - MANIFEST.yaml content matches actual directory state
  - Code does not reference paths not in path_config.yaml
  - Schema changes require schema_version bump + ADR reference
  - No ad-hoc directories without SCHEMA (anywhere in repo)
```

### 2.7 Governor integration

New Governor dimension: `schema_hierarchy`

```yaml
rules:
  - id: schema_manifest_pair_exists
    description: "Every data/ subdirectory has both SCHEMA.yaml and MANIFEST.yaml"
    
  - id: manifest_current
    description: "MANIFEST.yaml checksum matches actual directory contents"
    
  - id: schema_versioned
    description: "Every SCHEMA.yaml has schema_version field"
    
  - id: no_unschemaed_directories
    description: "No ad-hoc directories in data/ without schema"
    
  - id: lifecycle_values_valid
    description: "All lifecycle values in manifests exist in parent SCHEMA"
```

## 3. Architecture

```
┌─────────────────────────────────────────────┐
│ data/SCHEMA.yaml + MANIFEST.yaml            │ ← root
├─────────────────────────────────────────────┤
│ sources/SCHEMA.yaml + MANIFEST.yaml         │ ← source types
├──────────┬──────────┬───────────────────────┤
│ arxiv/   │ textbook/│ blog/                 │ ← per-source
│ SCHEMA   │ SCHEMA   │ SCHEMA                │
│ MANIFEST │ MANIFEST │ MANIFEST              │
├──────────┴──────────┴───────────────────────┤
│ <articles follow source SCHEMA>             │
├─────────────────────────────────────────────┤
│ processed/SCHEMA.yaml + MANIFEST.yaml       │ ← processing layers
│ <processed articles follow processed SCHEMA>│
├─────────────────────────────────────────────┤
│ ontology/, graphs/, gold_standard/          │
│ each with SCHEMA + MANIFEST                 │
└─────────────────────────────────────────────┘
```

## 4. Consequences

+ One place to understand what data exists and how it's structured.
+ Governor can mechanically validate structure (no prose-only rules).
+ Adding a new source = creating SCHEMA + MANIFEST, zero code changes.
+ Versioning built into the structure, not bolted on.
+ Meta-prompt principle enforced: code adapts to data, not vice versa.

− Initial cost: create schemas for all existing directories.
− Manifests need automatic update hooks in pipeline stages.
− Schema changes require discipline (version bump + ADR).

## 5. Implementation waves

| Wave | Content | Status |
|---|---|---|
| W1 (this ADR) | Root + sources + processed schemas | Proposed |
| W2 | Create all SCHEMA.yaml + MANIFEST.yaml files | Next |
| W3 | Governor schema_hierarchy rules | Next |
| W4 | Manifest auto-update in pipeline | Later |
| W5 | Versioned article directories | Later |

## 6. Testing requirements (ADR-051)

- Governor contract: schema_manifest_pair_exists, manifest_current
- Integration: pipeline stages update manifests
- Unit: schema versioning round-trip

## 7. Acceptance criteria

- **Every top-level directory** in the repo has SCHEMA.yaml + MANIFEST.yaml
- Every data/ subdirectory (depth ≥ 1) has SCHEMA.yaml + MANIFEST.yaml
- Repo root has SCHEMA.yaml defining valid top-level directories
- No hardcoded lifecycle values or path structures in Rust code
- Governor self-test includes schema_hierarchy checks
- Adding a new source type requires only: new SCHEMA + MANIFEST + source_codes.yaml entry
- Adding a new top-level directory requires: root SCHEMA update + ADR

## 8. Supersession / amendments

- Supersedes the implicit "data lives wherever" practice from ADR-067.
- ADR-067 defined normalized layout; this ADR defines the schema/manifest principle on top.

## 9. Open points

- Should `governor/fixtures/` also carry SCHEMA + MANIFEST? (Probably yes.)
- How deep does the hierarchy go? (Per-article manifests are leaves; no deeper.)
- Automatic schema migration tool when schema_version bumps?

---

## LLM Reading Notes

- **Core principle:** SCHEMA = what's possible (static, versioned), MANIFEST = what exists (dynamic, auto-generated). Both are DATA files that the code reads.
- **Anti-pattern prevented:** hardcoding structure in code. If you want to add a new source type, you create a SCHEMA.yaml, not a Rust struct.
- **Key constraint from law-nexus:** artifact graph with lifecycle ceilings and authority stack — each layer can only go up, never down.
- **The "meta-prompt" in practice:** the system's structure is described by data files that both humans and code read. The code is a generic engine; the data tells it what to do.
