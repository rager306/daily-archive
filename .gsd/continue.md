# Handoff — 2026-08-11 (M374 sealed)

## Sealed

| ID | Status | Product |
|----|--------|---------|
| M371-pr59ty | complete | pipeline semantic acceptance |
| M372-sqj6hc | complete | universal article load plane |
| M373-2d52vo | complete | reliable cached 28-unit textbook hub |
| M374-fvqtlt | complete | exact version-faithful arXiv acquisition |

## M374 proof

- Commit `12939518`: typed arXiv revision selector, exact idvN assets, no bare fallback, versioned cache/report/snapshot identity.
- `2506.07853v1`: PDF `bd601ac0…`, HTML `0e11dbd…`, 20 sections, 3 entities.
- `2506.07853v5`: PDF `aa6e247b…`, HTML `4dc4abdd…`, 13 sections, 2 entities.
- Both match independent provider fetches, coexist under canonical unit `2506.07853`, and replay offline with parse/embed hits and stable hashes.
- D127 false; full architecture PASS; governor 32/32, YAML valid, 0 violations; logs clean.
- D222 recorded. ADR-056 clarified. ADR-058 + `data/ontology/drafts/article_revision_contracts.yaml` remain Proposed/draft-only.
- Governor blocks `PaperRevision`, `SourceManifestation`, `HAS_REVISION`, and `HAS_MANIFESTATION` leakage into production ontology before acceptance.

## Ontology decision pending

Recommended ADR-058 model:

```text
Paper (one canonical work)
  -> HAS_REVISION -> PaperRevision(v1..vN)
      -> HAS_MANIFESTATION -> PDF/HTML
      -> hasPart -> revision-scoped Section
      -> SUPERSEDES -> prior observed revision
```

Do not change `paper_vid` (GitNexus CRITICAL: 74 symbols, 8 flows). Do not materialize revision nodes until explicit human acceptance.

## Next after decision

- If ADR-058 accepted: DATA schemas/edges, revision VIDs, revision-scoped Sections, one-Paper/two-revision Samyama runtime proof, current/as-of queries, replay idempotency.
- If deferred: continue with extraction-stage PipelineCache; keep revision data catalog/cache-only.
- ADR-057 remains independently Proposed.

## Locks

D127 false; ADR-057 Proposed; ADR-058 Proposed; cache/catalog evidence is not authoritative graph truth.
