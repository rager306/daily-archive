# M025-6xovy3: M025-6xovy3: Article Pipeline Modular Refactor

**Vision:** Refactor the article preprocessing pipeline only after freezing a 5 selected real-article smoke corpus and capturing current behavior. Every loader, parser, PageIndex, chunking, assets, identity, and evidence-boundary change must be grounded by replay on the same corpus, so M025 proves practical preprocessing readiness before larger validation or graph readiness claims.

## Success Criteria

- A fixed 5 selected real-article smoke corpus exists before refactor work starts: four arXiv RLM/DSPy-related articles plus the PageIndex company-blog article.
- The reusable article catalog includes an index with titles so CLI lookup does not scan the full catalog tree.
- The current pipeline baseline is captured on that corpus before any module extraction is accepted.
- Every module boundary change is verified by replaying the same corpus and comparing outputs to baseline or the previous accepted run.
- Parser, normalization, PageIndex, and chunking readiness is proven by real per-article artifacts, not only fixtures.
- Expected chunkable articles produce traceable chunks; low-quality or zero-chunk cases have explicit diagnostics.
- Assets, tables, links, and identity outputs are separated from chunks and preserve provenance.
- The final refactored preprocessing pipeline runs end to end locally on the smoke corpus with persisted metrics and diagnostics.
- The milestone remains preprocessing-only and makes no graph import, production write, or graph readiness claim.

## Slices

- [x] **S01: Smoke Corpus Freeze** `risk:high` `depends:[]`
  > After this: After this: a fixed 5 article corpus exists with local source artifacts, catalog index entries, checksums, expected profiles, and an integrity validator so every later slice uses the same real inputs.

- [x] **S02: Current Pipeline Baseline** `risk:high` `depends:[S01]`
  > After this: After this: the current pipeline has been run as-is on the smoke corpus, with per-article intermediate artifacts, metrics, diagnostics, and a baseline report that future refactors compare against.

- [x] **S03: Loader Boundary Replay** `risk:high` `depends:[S02]`
  > After this: After this: article loading is isolated behind a loader boundary and replayed on the same corpus, proving source IDs, checksums, kinds, outcomes, and failure reasons match or intentionally improve over baseline.

- [x] **S04: Parser Normalization Replay** `risk:high` `depends:[S03]`
  > After this: After this: parser and normalization are isolated from loader and downstream indexing, and the same smoke corpus produces typed article elements, source-span diagnostics, and parser quality metrics.

- [x] **S05: PageIndex Replay** `risk:high` `depends:[S04]`
  > After this: After this: PageIndex is built from normalized elements for the smoke corpus, with deterministic nodes, anchors, routes, and provenance references verified against baseline expectations.

- [x] **S06: Chunking Replay** `risk:high` `depends:[S05]`
  > After this: After this: chunking is isolated and proven on the smoke corpus, with traceable chunks for expected articles and explicit diagnostics for zero-chunk or low-quality cases.

- [x] **S07: Assets Tables Links Identity Replay** `risk:high` `depends:[S06]`
  > After this: After this: assets, tables, links, and identity evidence are separated from chunks and replayed on the same corpus with metadata-safe provenance-bearing outputs.

- [x] **S08: End to End Preprocessing Replay** `risk:high` `depends:[S07]`
  > After this: After this: the full refactored preprocessing pipeline runs on the same smoke corpus, writes final per-article artifacts, compares against baseline, and states whether larger preprocessing validation is ready or blocked.

- [x] **S09: Baseline Recovery Replay** `risk:high` `depends:[S08]`
  > After this: Recover or regenerate the current-pipeline baseline for the fixed five-article smoke corpus using local artifacts only, persist per-article baseline artifacts and diagnostics, and prove no network fetches or production writes occurred.

- [x] **S10: Boundary Replay Completion** `risk:high` `depends:[S09]`
  > After this: Replay loader, parser normalization, PageIndex, chunking, and evidence boundaries on the same five-article corpus against the recovered baseline or accepted prior run, with explicit comparisons, traceable chunks, and diagnostics for low-quality or zero-chunk cases.

- [x] **S11: Requirements Scope Reconciliation** `risk:medium` `depends:[S10]`
  > After this: Produce a milestone requirements coverage matrix distinguishing M025 preprocessing requirements from out-of-scope KG/RLM/scale requirements, update requirement notes where appropriate, and rerun final end-to-end replay so validation can close with coherent requirement coverage.

## Boundary Map

## Boundary Map

```text
In M025:
  5 selected real articles with local source artifacts, checksums, expected profiles, and catalog index entries
  Reusable article catalog plus corpus selection rather than milestone-local URL lists
  Current pipeline baseline run before refactor
  Loader boundary replay on the same corpus
  Parser and normalization boundary replay on the same corpus
  PageIndex boundary replay on the same corpus
  Chunking boundary replay on the same corpus
  Assets, tables, links, identity, and evidence replay on the same corpus
  End to end preprocessing replay with persisted intermediate artifacts and comparison reports
  Local validation scripts, fixture tests, property tests, and diagnostic gates

Out of M025:
  Final graph import
  Production LadybugDB writes
  Positive graph readiness acceptance
  20 document or one week corpus validation
  DSPy optimizer or RLM activation
  LLM first extraction as a milestone dependency
  Network dependent acquisition as a completion requirement
```
