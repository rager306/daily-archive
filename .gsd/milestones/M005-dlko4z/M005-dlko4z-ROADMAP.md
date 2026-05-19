# M005-dlko4z: Chunking Import Model Deepening

**Vision:** Deepen chunking from research into an import-ready data model so future scientific KG import consumes typed, traceable, reviewed chunks instead of generic text fragments.

## Success Criteria

- Import-ready chunk model is defined, implemented, and versioned.
- Representative real-paper benchmark proves improved chunk quality over baseline or documents blockers.
- Only chunks/routes passing the contract are considered eligible for future KG import.
- Independent review confirms artifacts are semantically meaningful and not count-only.
- Production KG writes remain blocked until dry-run import evidence passes.

## Slices

- [x] **S01: S01** `risk:high` `depends:[]`
  > After this: After this slice, there is a versioned import-ready chunk package contract and a representative benchmark corpus selection with review rubric.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: After this slice, current chunking has measured import-readiness failures and a baseline report.

- [x] **S03: S03** `risk:high` `depends:[]`
  > After this: After this slice, a structure-aware chunk model emits typed parent-child chunks with provenance and route eligibility.

- [ ] **S04: Chunk annotation sidecars** `risk:medium` `depends:[S03]`
  > After this: After this slice, chunks have deterministic sidecar annotations useful for routing and review, without becoming KG facts.

- [ ] **S05: Source asset preservation and multimodal manifest** `risk:high` `depends:[S04]`
  > After this: After this slice, source PDFs, normalized Markdown, extracted figures, tables, and image assets are preserved with redacted asset manifests for future multimodal retrieval.

- [ ] **S06: Benchmark chunking methods and independent review** `risk:high` `depends:[S05]`
  > After this: After this slice, current, structure-aware, and selected real chunking candidates are compared on real papers, including asset-linkage quality, and reviewed independently.

- [ ] **S07: Isolated import rehearsal** `risk:high` `depends:[S06]`
  > After this: After this slice, an isolated import rehearsal proves approved package records can be validated and loaded without production KG writes.

## Boundary Map

```text
In M005:
  Deep chunking quality implementation and benchmark
  Versioned import-ready chunk package contract
  Typed GraphReadyChunk model with source spans, hierarchy, routes, and quality states
  Deterministic chunk annotations as sidecars
  Representative real-paper benchmark and independent artifact review
  Isolated import rehearsal from package only

Out of M005:
  Production LadybugDB KG writes
  Broad corpus scaling
  Semantic/vector retrieval production claims
  LLM-based chunking as default
  DSPy optimizer adoption
  Treating annotations as KG facts without extraction validation
```
