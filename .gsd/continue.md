# Handoff — 2026-08-11 (M373 sealed)

## Sealed

| ID | Status | Product |
|----|--------|---------|
| M371-pr59ty | complete | pipeline semantic acceptance |
| M372-sqj6hc | complete | universal article load plane |
| M373-2d52vo | complete | reliable cached 28-unit textbook hub |

## M373 proof

- Commit `57ccc3ab`: atomic fail-closed LoadReport writes; semantic embed-cache checks; ACAP missing replay fail-closed; Rust 1.94 canary lint cleanup.
- First `da load --batch-toc ... --stages ingest`: 28/28, 771 sections, parse miss=28, embed miss=28, D127 false.
- Exact replay: 28/28, parse hit=28, embed hit=28, stable payload hashes, no network/GROBID.
- Warm embed hit works without `FD_API_KEY`; miss/bypass still requires credentials.
- Full architecture gate passed; governor closeout: 0 violations (2 operational drift flags); app logs: no ERROR/panic.
- UAT/validation/closeout PASS. GSD auto-commit `f0481aef` was followed by a hygiene commit removing generated runtime/cache files from Git only; local evidence remains available.

## Operator

```bash
da load --batch-toc data/source_catalogs/gnn_textbook_toc.yaml \
  --stages ingest \
  --cache-root artifacts/pipeline_cache/gnn_textbook \
  --batch-report-dir artifacts/loads/gnn_textbook \
  --output artifacts/article_context/gnn_textbook_hub.sgsnap
```

## Next product wave

1. Review queued M363-ma2kmw relevance (article preparation/agent readiness; four unplanned slices).
2. Next reusable load gap: extraction-stage PipelineCache with exact pattern/config/output-schema identity; no provider/model hardcode.
3. ADR-057 remains Proposed. Collection/KnowledgeUnit materialization requires explicit human acceptance first.
4. Related-article expansion remains a separate future milestone.

## Locks

D127 false; ADR-057 Proposed; no Collection/KnowledgeUnit/DISCUSSES_PAPER production writes; cache is derived evidence, not graph truth.
