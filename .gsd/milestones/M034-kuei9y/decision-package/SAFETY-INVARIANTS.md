# M034 Safety Invariants

These invariants are binding for all future implementation work unless superseded by an explicit graph-promotion ADR and authorized milestone.

## Default Flags

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

## Non-authorization Rules

- Parser, sidecar, adapter, and LLM outputs are candidate evidence only.
- No direct extractor/parser/sidecar/LLM to GraphDB write path is allowed.
- GraphDB selection remains deferred.
- Agentic orchestration remains deferred.
- No review packet means no readiness handoff.
- No readiness handoff means no import recommendation.
- No explicit future graph-promotion milestone means no GraphDB write.

## Redaction Rules

Diagnostics may expose IDs, counts, hashes, paths, status names, error codes, timings, and backend/cache health. Diagnostics must not expose raw paper text, raw chunk text, embeddings, secrets, credentials, or unredacted LLM payloads.

## Review Boundary

Candidate evidence must pass contract validation and review packet generation before graph-readiness review. Review packets must be post-checked before any manifest or handoff claims readiness.
