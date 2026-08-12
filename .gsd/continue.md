# Handoff — 2026-08-12 (M385 closed; resume M383)

## Closed milestone

**M385-n601t6 Awesome Agentic Reasoning Corpus Expansion**

Commits:
- `9f061c12` — pinned source manifest;
- `9ba8c30d` — downloader metadata fix, 550 new article metadata records, final reports/index/docs.

## Source extraction

Supplied repository: `https://github.com/weitianxin/Awesome-Agentic-Reasoning`

Pinned raw README:
- SHA256 `20c5aa9e3cc8ba518d3739bf973735362f967d725c869a5321c9317489ae0318`;
- 118,320 bytes / 1,415 lines;
- 740 arXiv abs/pdf links;
- 580 unique versionless IDs;
- 160 duplicates removed;
- baseline overlap 29; missing 551.

Manifest: `artifacts/source_recovery/awesome_agentic_reasoning_manifest.json`.

## Acquisition result

Primary 551-item wave:
- 550 ok;
- 1 fail: `2507.23261`.

Failure-only retry confirmed all PDF endpoints return 404. Search/abstract evidence says the submitter withdrew it. Do not retry indefinitely or create a fake PDF.

Existing-gap wave:
- 29/29 ok;
- filled HTML and normalized bare-root metadata visibility for previously present IDs.

Final source-specific assurance:
- source IDs: 580;
- available articles: 579;
- valid PDF IDs: 579;
- valid substantive HTML IDs: 579;
- valid metadata IDs: 579;
- missing PDF/HTML/metadata: only `2507.23261`;
- invalid assets: 0;
- `.part` files: 0;
- D127 violations: 0;
- import_eligible=false.

Assurance: `artifacts/source_recovery/awesome_agentic_reasoning_assurance.json`.

Local canonical catalog now occupies about 3.3 GB. New PDFs/HTML remain local and gitignored; 568 compact `article.json` records and canonical indexes are committed.

## Product gap fixed

`scripts/download_arxiv_to_catalog.py` previously wrote PDF/HTML but no `article.json`, so catalog reconcile/index could not see new acquisitions.

Now it:
- publishes `article.v00.02` metadata only from checksum-verified representations;
- never overwrites existing metadata;
- atomically writes temporary metadata then renames;
- sets parser/chunk/graph/import readiness false;
- supports offline `--backfill-metadata-from-report`;
- replay is skip-only;
- has 21 passing tests including checksum rejection and idempotency.

Catalog reconcile/index was rebuilt. Global catalog findings still include historical duplicate/conflicting roots and missing variants; no medium/high plan action was auto-applied. Treat those as a separate cleanup milestone, not M385 failure.

## GSD evidence

UAT:
- S01 `4b486242-a647-4fc7-ba40-7df6feb226a6`;
- S02 `d6d82abd-e52d-42bd-9d46-ad256267ddd9`;
- S03 `2f414e47-6041-4899-8571-1c10b3bea428`.

Validation PASS; gates empty; consistency PASS; milestone complete.

## Active milestone

**M383-uber46 Revision Owned Corpus Acceptance** remains active.

### Exact next action

Execute M383/S01 against the exact 579 available Awesome Agentic Reasoning IDs plus existing accepted corpus:
1. inventory explicit revision/source evidence from article.json and paths;
2. classify versioned versus versionless inputs without assuming v1;
3. put versionless policy in ontology DATA;
4. emit bounded per-paper decision/digest;
5. fail closed on ambiguous revision.

Then use the M384 18.59s observable materializer for revision-owned rebuild, grounded summaries and five-class ontology acceptance.

## Locks

D127 false. No graph import from acquisition. `2507.23261` explicitly unavailable. PDFs/HTML are candidate source evidence, not graph truth. D227 temporal strict identity; D229 exact lineage; D230 Citation namespace; D232 cached immutable ontology DATA; D235 no pseudo-atomic bulk; D236 no inactive RocksDB tuning. Samyama sole store; no implicit revision default; no legacy Paper summaries; DSPy disabled.
