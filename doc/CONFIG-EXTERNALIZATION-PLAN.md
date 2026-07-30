# Configuration Externalization Plan

**Status:** Design (2026-07-29)
**Principle:** No hardcoded reference data in Rust. All configurable vocabularies,
codes, and taxonomies live in YAML files under `data/`.

## Current state

287 hardcoded reference constants across 5 files:

| File | Constants | Category |
|------|-----------|----------|
| domain.rs | 166 | arXiv category codes |
| relation.rs | 68 | Edge type label strings |
| process.rs | 30 | Failure taxonomy + origin/stage |
| source.rs | 16 | Source codes + types |
| rule_extractor.rs | 7 | Entity whitelists (duplicated with JSON) |

Already externalized: `data/extraction_patterns.json` (35 lines).
Problem: KNOWN_* const arrays in rule_extractor.rs DUPLICATE the JSON.

## Target architecture

```
data/
  arxiv_categories.yaml        ← 154 arXiv codes + groups + descriptions
  extension_domains.yaml       ← da.* extension namespace codes
  extraction_patterns.yaml     ← entity whitelists (replaces .json)
  source_codes.yaml            ← Source node codes + types
  failure_taxonomy.yaml        ← FailureEvent stage/class vocabulary
  edge_types.yaml              ← Edge type label strings (reference)
  domain_packs/                ← per-domain templates (ADR-043)
    cs.LG/
    da.medicine/
    ...

crates/da-domain/src/
  config.rs                    ← ConfigProvider trait (port)
  domain.rs                    ← DomainRegistry (logic only, data from config)
  source.rs                    ← SourceRegistry (logic only)
  process.rs                   ← FailureRegistry (logic only)
```

## Wave plan

### Wave A: Infrastructure (config loading)
- Add `serde_yaml` to workspace deps
- Create `data/` YAML files for each category
- Create `DomainRegistry`, `SourceRegistry` structs
- Load at startup, cache in `OnceCell`
- Keep logic (canonicalize, is_known, validate) in Rust

### Wave B: Domain codes (biggest impact)
- `data/arxiv_categories.yaml` + `data/extension_domains.yaml`
- DomainRegistry loads from YAML
- `is_known()` / `canonicalize()` query the registry
- Bundled fallback for first boot (minimal: cs.LG, cs.AI, da.general)

### Wave C: Extraction patterns (dedup)
- Migrate `extraction_patterns.json` → `.yaml`
- Remove KNOWN_* const arrays from rule_extractor.rs
- Single source of truth: YAML file
- ExtractionConfig::from_yaml()

### Wave D: Source codes, failure taxonomy
- Source codes → `data/source_codes.yaml`
- Failure taxonomy → `data/failure_taxonomy.yaml`
- Constants become enum variants (logic) + YAML strings (data)

### Wave E: Edge types (optional)
- Edge type strings → `data/edge_types.yaml`
- Keep Rust const wrappers for compile-time checking in hot paths
- YAML is the canonical reference, Rust consts are generated from it

## Design rules

1. **Logic stays in Rust**: canonicalization, validation, matching algorithms.
2. **Data goes to YAML**: codes, labels, descriptions, vocabularies.
3. **Bundled fallback**: minimal YAML embedded via `include_str!` for first boot.
4. **No network dependency**: YAML is read from disk, not fetched.
5. **Single source of truth**: no duplication between const arrays and YAML.
6. **Backward compatible**: existing public API (`is_known()`, `canonicalize()`) preserved.
