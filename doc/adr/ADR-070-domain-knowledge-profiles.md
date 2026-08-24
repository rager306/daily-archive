# ADR-070: Domain Knowledge Profiles — Per-Domain Extraction Norms

- Status: Accepted
- Date: 2026-08-24
- Deciders: repo owner + agent (M451)
- Tags: domains, extraction, data-driven, multi-domain, routing

## 0. One-line Decision

**Every scientific domain carries its own SCHEMA.yaml (entity types, structural types, quality thresholds) selected per-paper by multi-domain routing over `coarse_topic_code`; cross-domain papers apply several profiles as layers (primary sets thresholds, secondaries add entity types).**

## 1. Context

Rule-based extraction found only CS entities: a math paper yielded 7 entities
from 244 sections because a single flat `extraction_patterns.yaml` cannot
serve mathematics (Theorem/Lemma/Definition/Axiom), biology, physics and CS
simultaneously. Thresholds also differ: math text has lower entity density
and lower grounding ceilings than CS papers.

User decisions driving this ADR:
- "статья или иной источник могут принадлежать нескольким доменам" —
  papers live at intersections (ML+bio, math+CS) and need BOTH profiles.
- "нужно ловить регрессию" — routing and loader regressions must be
  covered by tests (they were caught live twice).

## 2. Decision

### 2.1 Structure (ADR-068 roles)

```
data/domains/
  _shared/SCHEMA.yaml          # base layer: Method/Dataset/Model/Metric,
                               # IMRaD structural types, default thresholds
  mathematics/SCHEMA.yaml      # MathConcept/Theorem/Lemma/Proof/Axiom,
                               # structural: theorem/definition/proof/exercise
  computer_science/SCHEMA.yaml # corpus-dominant profile (~88%)
  physics/ biology/ economics/
```

Each domain also carries `MANIFEST.yaml` — a derived cache recomputed on
every `build-extracted`: papers_processed, per-type entity_coverage, gaps
(SCHEMA entity_types with zero coverage). Stale = harmless.

### 2.2 Multi-domain routing

`data/path_config.yaml → paths.domain_routing`: `coarse_topic_code →
[domain, ...]` where the LIST is ordered:

- first domain = PRIMARY — its quality thresholds apply;
- remaining = SECONDARY — add entity_types + structural_types, never
  override thresholds.

Examples: `stat-ml → [computer_science, mathematics]`,
`html → [computer_science, mathematics]` (GNN textbook),
`q-bio-nc → [biology, computer_science]`.

Fallback chain when routing misses: `detect_domain_from_text` counts
Theorem/Definition/Lemma titles in navigation.json (>10% → mathematics).
Default: computer_science.

### 2.3 Resolution is tested, not assumed

Live-caught regressions now guarded by tests:
- `domain_routing_single_and_multi_domain` (parse single-string AND list
  values; the routing table was once silently emptied by a scope mistake)
- `domains_unique_across_codes`, `domain_routing_empty_when_absent`
- `fresh_acquisition_without_manifest_starts_at_raw` (status crashed on
  fresh acquisitions — legacy loader)
- `collect_pdfs_finds_pdf_in_source_subdir` (canonical layout
  `<id>/source/<id>.pdf` broke the flat PDF scan)

### 2.4 Genre interacts with domain (ADR-069 §2.4)

Position/vision papers (essayistic headings, no IMRaD) fall back to global
structural passes regardless of domain — genre detection and domain
profiles compose: domain says WHAT types exist, genre says WHERE to look.

## 3. Consequences

- New domain = new directory + routing entry, zero code.
- Domain MANIFESTs expose corpus heterogeneity (per-domain coverage,
  gaps), feeding acquisition decisions (math corpus is 4 papers — thin).
- Extraction thresholds stop being corpus-global constants.

## 4. Status of adoption

Shipped: profiles, routing, manifests, resolution, tests. The extractor
consumes domain entity_types through the shared structural layer; math
detectors (Theorem/Lemma regex) live in `mathematics/SCHEMA.yaml`
`extraction_rules` and are wired via ADR-069 structural patterns.
