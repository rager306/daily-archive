# Handoff — 2026-08-11 (M377 sealed)

## Completed

M371-M377 complete. Latest: **M377-42cdf7 Extraction Stage Pipeline Cache**.

## Universal load cache plane

`da load --stages ingest+extract --cache-root …` now records:

```text
parse   hit|miss|bypass|retryable_failure
embed   hit|miss|bypass|retryable_failure
extract hit|miss|bypass|retryable_failure
```

Extract stage contract:
- depends on parse payload SHA;
- `RuleBasedExtractor` owns implementation/config/output-schema identity;
- configuration SHA hashes actual loaded `ExtractionConfig` values, not a path;
- equivalent bundled/filesystem DATA matches; rule mutation invalidates;
- payload is serialized `Vec<ExtractedEntity>`;
- graph materialization always runs after hit/miss/bypass through revision context;
- cache is derived evidence, not graph truth.

## Real M377 proof

Exact local `2506.07853v1`, fresh `artifacts/pipeline_cache/m377_v1`:

- cold: parse=miss, embed=miss, extract=miss;
- offline replay without FD_API_KEY: parse=hit, embed=hit, extract=hit;
- no-cache: parse/embed/extract=bypass;
- stable parse/embed/extract payload SHA cold→replay;
- each run: `entities_extracted=3 found_in=3`, `revision_graph_ok`, D127=false;
- ingest-only: parse/embed records only; no extract record/finding;
- corrupted payload: FilePipelineCache checksum mismatch hard failure before decode.

Runtime reports remain ignored under `artifacts/loads/m377_v1/`; structured GSD UAT is durable evidence.

## Verification

- fingerprint tests: 3
- adapter rule_extractor: 37
- da-cli load: 8
- extraction: 4
- revision materialization: 9
- all-target clippy PASS
- full architecture guardrail PASS
- governor 33 / YAML valid / violations=0
- logs clean
- UAT/validation/closeout PASS

Commits:
- `053e00fa` product
- `4d7b11f5` GSD closeout auto-commit

## Next product decisions

1. **Review queued M363-ma2kmw relevance** before executing its stale four-slice article-preparation roadmap.
2. Do not cache graph materializers. Cache later stages only where a pure deterministic compute seam exists with complete identity.
3. Remote GraphStore remains future integration.
4. ADR-057 remains Proposed.

## Locks

D127 false; ADR-057 Proposed; ADR-058 Accepted. Existing `paper_vid` unchanged. Current revision explicit. Cache/catalog evidence is provenance, never authoritative graph truth.
