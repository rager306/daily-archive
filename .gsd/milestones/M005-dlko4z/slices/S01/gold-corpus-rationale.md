# M005/S01 — Gold Corpus Rationale

## Purpose

This corpus is the benchmark target for M005 import-ready chunking work. It is not a new crawl, not a broad corpus run, and not permission to import KG facts. It reuses the deterministic ten-paper M004 validation corpus so chunking improvements can be measured against known real-data failures and trusted-subset evidence.

## Selection Rule

The outer gate is the full M004 ten-document corpus. Keeping all ten papers prevents cherry-picking and keeps M005 anchored to the real-data validation path that exposed the chunking problem.

The inner review set prioritizes papers that exercise distinct failure modes:

- repaired conversion and previous zero-chunk failures;
- existing S10 chunk-review blocker evidence;
- S07 trusted candidate-claim papers;
- math/theory papers with equation and definition risk;
- multimodal/security/CV papers with table/figure risk;
- methods/results prose where claims can be over-bundled;
- administrative/metadata contamination risk.

## Inner Review Minimum

The following papers must be reviewed manually or by independent subagent before S05 claims benchmark success:

| Paper | Why it is mandatory |
|---|---|
| `2605.14259v1` | Previous low-quality/zero-chunk case, repaired conversion, S07 trusted candidate. |
| `2605.14517v1` | Previous low-quality/zero-chunk case with S10 chunk sample showing non-zero chunks were not graph-ready. |
| `2605.14995v1` | Trusted S07 claim candidate and prose/methods baseline. |
| `2605.14743v1` | Math/network theory case with definition/equation risk. |
| `2605.14799v1` | Multimodal/CV case likely to stress figures, tables, numeric results, and model-evaluation chunks. |
| `2605.14291v1` | Multimodal/security case with cross-modal claim and figure-dependency risk. |

Other corpus papers remain in the automated outer gate and should be added to review if S02 diagnostics find blockers.

## Why These Papers Are Sufficient for S02 Baseline

S02 is a baseline measurement slice, not final chunker selection. The corpus is sufficient because it contains:

- all 9 papers with S07 accepted trusted candidate records;
- the one ten-doc paper not persisted in S06/S07 but known to have chunk-quality review evidence;
- two repaired conversion cases;
- theory/math, social/network, LLM behavior, multimodal/security, and CV/model-evaluation topics;
- known route risks: claim extraction, method extraction, relation extraction, table/figure handling, references, metadata, and retrieval-only content.

If S02 discovers that no paper exercises a required table/equation/figure route in available artifacts, it should report that as a benchmark coverage gap rather than silently adding a broad corpus. A diagnostic-only extra paper can be proposed later, but it must be explicitly planned.

## Artifact Policy

The manifest records paths and source-artifact identifiers for measurement. It does not assert that all paths currently exist. Missing artifacts are valid S02 findings and should become blockers or repair-required diagnostics.

Machine outputs derived from this corpus must keep:

- `raw_text_included=false`;
- `embeddings_included=false`;
- `production_import_attempted=false`;
- `ladybugdb_written=false`.

Bounded human review snippets may be generated later for semantic review, but they must be separate from JSON/JSONL machine logs.

## Stop/Go Boundary

S02 may proceed to measure current chunking on this corpus.

S02 may not claim import readiness. It may only report baseline readiness, blockers, route exclusions, and measurement gaps.

M005 should not proceed to production KG import even if S02 counts look good. Import readiness requires improved chunk model implementation, annotation sidecar review, independent benchmark review, and isolated dry-run import rehearsal.
