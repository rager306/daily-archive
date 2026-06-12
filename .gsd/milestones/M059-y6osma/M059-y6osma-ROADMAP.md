# M059-y6osma: M060 Manifest Driven PDF Ingest Architecture

**Vision:** Establish manifest-driven PDF ingest architecture for daily-archive: unified batch manifest schema, parser output schemas (jsonschema), retroactive manifests for M054-M058 batches, validation + replay tooling, ADR-013 binding the architecture. After M059, every PDF batch has versioned, replayable, validatable processing contract.

## Slices

- [x] **S01: Schemas + retroactive manifests + ADR-013** `risk:medium` `depends:[]`
  > After this: 6 JSON schemas in schemas/, 5 retroactive manifests for M054-M058, jsonschema validation works on 1 batch, ADR-013 binding emitted

- [ ] **S02: Validation + replay tooling + decision** `risk:medium` `depends:[S01]`
  > After this: validate_pdf_batch.py runs end-to-end on M054 (5 PDF, 2 parsers), replay_ingest.py produces identical output, decision: scale to M061 2-hop BFS with manifests

## Boundary Map

Not provided.
