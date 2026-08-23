# ADR-067: Normalized Data Layout

- Status: Accepted (2026-08-22)
- Deciders: repo owner + agent (M447)
- Tags: data-layout, hygiene, governor, migration

## 0. One-line Decision
One canonical home per data class under `data/`; the article catalog is flattened one level; reader-less legacy bundles move to `archive/` as cold storage; Governor enforces the layout.

## 1. Context
The data tree grew without a contract:
- `data/article_catalog/article_catalog/<source>/<category>/<id>/` — double nesting from an extraction bug; 3.7G, referenced by 18 literals in 8 prod files.
- `article_corpora/m0xx-*` — milestone-era snapshot corpora; only `m033-grobid-probe-v1` is read (grobid_parser tests).
- `r024-*`, `architecture-assessment`, `test-architecture-alignment` — zero code readers.
- `data/edge_contracts.yaml` — orphan duplicate of the ontology contract (removed in M446 S02).
- D237 freezes python-era evidence: physical deletion of catalog/archive content is forbidden; moves preserve bytes.

## 2. Decision
Normalized roots (one home per class):

| Root | Contents | Notes |
|---|---|---|
| `data/articles/` | article assets: `<source>/<category>/<id>/source/*` | flattened from `article_catalog/article_catalog`; service buckets (`mixed-source`, `unknown`) allowed inside but never canonical |
| `data/ontology/` | dictionaries and contracts (29+ YAML) | single source of truth; generated exports live here |
| `data/gold/` | F1 fixture corpus (`gold_standard` renamed at usage sites gradually) | frozen fixture only |
| `data/graph_tasks/` | semantic task suites | unchanged |
| `data/corpora/` | bounded snapshot corpora; only actively-read ones stay | `m033-grobid-probe-v1` stays until grobid tests migrate |
| `archive/data-legacy/…` | reader-less legacy bundles (r024-*, architecture-assessment, test-architecture-alignment, superseded m0xx corpora) | D237 semantics preserved |
| `artifacts/reports/` (gradual) | regression/drift/comparison reports | current flat artifacts stay until a follow-up |

Compatibility policy:
1. Moves are same-FS renames (bytes preserved).
2. After each move, a compatibility symlink may exist for one wave max (e.g. `data/articles/article_catalog -> .` is NOT needed because all 18 prod literals are rewritten in the same commit; tests/examples/docs get a follow-up sweep).
3. Every move lands with its verification in one atomic commit.

## 3. Architecture (onion)
No crate changes. Path constants live at composition roots (CLI) and test fixtures; domain/adapters take paths as parameters. Governor (scripts/) owns enforcement as DATA policy + Python check.

## 4. Consequences
+ One obvious place for every artifact class; no double nesting.
+ Governor flags future violations mechanically (not by memory).
− One-time migration cost (renames + 18 literal rewrites).
− Old absolute paths in external tooling break → compatibility symlink covers the transition.

## 5. Implementation waves
| Wave | Content | Verification |
|---|---|---|
| W1 (this ADR) | flatten article_catalog + rewrite 18 literals + shim | gate both modes; da-cli/da-adapters/da-application tests; eval_batch manifest; regression check clean |
| W2 | legacy moves to archive/data-legacy/ | rg zero readers broken; governor green |
| W3 | governor data_layout_policy (DATA + Python check + tests) | self-test incl. negative case |

## 6. Testing requirements (ADR-051)
- Contract: path resolution tests updated to canonical layout.
- Integration: eval_batch manifest build; corpus_regression_check clean.
- Governor negative test: resurrected legacy dir or nested service bucket must flag.

## 7. Acceptance criteria
- `rg "article_catalog/article_catalog" crates --type rust -g '!*tests*' -g '!*examples*'` → 0 hits.
- `test -d data/article_catalog/article_catalog` → false (flattened), symlink shim optional.
- `ls data/r024-* data/architecture-assessment data/test-architecture-alignment` → absent; present under archive/data-legacy/.
- Governor self-test ≥ previous count + new cases, all green.

## 8. Supersession / amendments
Supersedes the implicit "catalog lives wherever unpacking put it" practice. Amendments via new ADRs only.

## 9. Open points
- `data/gold_standard` rename deferred (20 readers) — cosmetic, later wave.
- Physical dedup of byte-identical assets across categories remains forbidden (lookup-surface unification only, M061 lesson).
