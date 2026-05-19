# Final recommendation: thirty-paper deviation scan

## Recommendation

Plan a follow-on M007 milestone for a deterministic CLI-first validation loop:

```text
+10 papers -> source preflight -> bounded acquisition/repair -> structure-aware scan -> route/refusal delta analysis -> outlier review -> automation improvement -> repeat toward 100 papers
```

This recommendation is based on M006 proving operational scan readiness at 30 papers, not semantic KG readiness. The positive KG import remains blocked.

## What M006 proved

M006 established that the project can move beyond the M005 10-paper baseline to a 30-paper Markdown-scan-ready corpus and produce redacted, repeatable deviation evidence.

Key evidence from S03:

| Metric | Value |
|---|---:|
| Papers scanned | 30 |
| Markdown-ready for scan | 30 |
| Markdown bytes scanned | 1,761,102 |
| Structure-aware chunks | 4,289 |
| Elements | 4,319 |
| Annotations | 17,573 |
| Per-paper diagnostics | 30 |
| Outlier papers | 11 |
| Import-eligible chunks | 0 |
| Refused chunks | 4,289 |
| Cached PDFs | 8/30 |

The useful result is an operational one: the pipeline can summarize route distributions, refusal distributions, outliers, and source caveats without serializing raw paper text, chunk text, embeddings, vectors, optimizer traces, secrets, binary payloads, or production-write activity.

## What M006 did not prove

M006 did not prove:

- semantic correctness of claim/method/table candidates;
- entity/relation extraction quality;
- semantic/vector retrieval quality;
- positive trusted KG import readiness;
- PDF/multimodal completeness;
- broad corpus readiness beyond this staged 30-paper sample.

The positive KG import remains blocked because all 4,289 chunks are refused and `import_eligible_chunk_count=0`.

## Correct framing after independent review

Independent review returned `FLAG`, not `PASS`, because the S03 evidence is useful for automation planning but must not be over-claimed.

Use these corrected terms going forward:

- Say **Markdown-scan readiness**, not full source readiness.
- Treat missing Markdown/PDF risk tags as historical/source-state signals that future automation must reconcile, not as proof that S03 failed.
- Treat route shifts as **routing and review-prioritization evidence**, not semantic validation.
- Keep M005/S03 and M005/S06 baselines separate.

## Baseline interpretation

Use two separate baselines:

### M005/S03 structure-aware baseline

This is the apples-to-apples baseline for route/type/state/refusal shares.

| Metric | M005/S03 | M006/S03 |
|---|---:|---:|
| Papers | 10 | 30 |
| Chunks | 1,831 | 4,289 |
| Chunks per paper | 183.10 | 142.97 |
| Import-eligible chunks | 0 | 0 |

Observed route-share shifts from M005/S03 to M006/S03:

- `retrieval_only`: 76.41% -> 70.09%
- `method_extraction`: 7.43% -> 10.35%
- figure route: 4.70% -> 6.60%
- `citation_graph`: 0.60% -> 1.87%
- `claim_extraction`: 13.38% -> 14.53%
- `table_extraction`: 2.08% -> 3.12%
- equation route: 7.97% -> 6.02%

These shifts may indicate that the larger sample exposes more review-heavy routes. They should not be read as semantic correctness claims.

### M005/S06 mixed benchmark

M005/S06 had 2,471 mixed benchmark candidates and zero import-eligible chunks. It is useful as import-boundary context only, not as a direct route-share baseline for M006/S03.

## Outlier method

S03 flagged 11 papers using deterministic heuristics:

- `high_chunk_count`: chunk count at least `max(2 × median chunk count, median + 25)`.
- `claim_candidate_heavy`: at least 25 claim-route chunks.
- `table_heavy`: at least 10 table-route chunks.
- `unexpected_import_eligible_chunks`: any import-eligible chunks; none occurred in S03.

Future automation should add normalized density thresholds, especially chunks per 10k bytes, so long papers and over-fragmented papers can be distinguished.

## M007 CLI milestone requirements

M007 should implement a deterministic, resumable validation CLI with the following capabilities.

### 1. Batch state model

Persist a per-batch state object with:

- batch id;
- selected paper ids;
- selection roles;
- input manifests;
- acquisition status;
- conversion methods;
- scan artifact paths;
- review verdict;
- next recommended action.

### 2. Deterministic +10 selection

Select the next +10 papers reproducibly. Persist role labels such as:

- baseline overlap;
- deterministic expansion;
- retry;
- repaired;
- excluded;
- manual review target.

### 3. Source preflight and contradiction checks

Track these states separately:

- Markdown present;
- Markdown quality accepted;
- PDF present;
- PDF missing;
- conversion repaired;
- conversion failed;
- unavailable source.

Add contradiction checks such as:

```text
ready_for_markdown_scan=true while risk_tags still contain missing_markdown
```

These should produce warnings or review gates, not silent success.

### 4. Bounded acquisition and repair

Default sequence:

1. fast existing Markdown check;
2. fast arxiv2md acquisition;
3. targeted Docling fallback for specific failures;
4. no bulk slow repair without an explicit bounded plan;
5. optional MiniMax adapter only after a separate spike proves it can return redacted, bounded repair/review outputs.

MiniMax should not be the orchestrator or source of truth.

### 5. Scan and evidence generation

Each batch should emit redacted machine artifacts:

- aggregate summary JSON;
- per-paper diagnostics JSONL;
- route/share delta report;
- outlier report;
- review summary.

Safety flags must remain explicit:

```text
raw_text_included=false
chunk_text_included=false
embeddings_included=false
vectors_included=false
secrets_included=false
optimizer_traces_included=false
production_import_attempted=false
ladybugdb_written=false
```

### 6. Delta reporting

Compare each +10 batch against:

- previous batch;
- cumulative corpus;
- M005/S03 structure-aware baseline;
- M005/S06 only as separate import-boundary context.

### 7. Review gates

Block or flag progression when:

- source acquisition leaves unresolved Markdown gaps;
- conversion risk tags contradict readiness state;
- outlier count or route-share shift exceeds threshold;
- import eligibility becomes non-zero outside a reviewed promotion path;
- raw/chunk text appears in machine artifacts;
- production LadybugDB writes are attempted.

### 8. Resume and rerun behavior

The CLI should be resumable. It should skip completed verified stages, rerun failed or stale stages, and preserve historical run artifacts by batch id.

## Go/no-go decision

### Go for M007 automation planning

M006 provides enough evidence to plan M007 because it identified real operational needs:

- source-readiness preflight is mandatory;
- fast acquisition plus targeted Docling repair works better than bulk fallback;
- route-share deltas reveal changing review categories;
- outlier detection is useful for review prioritization;
- import gates stayed safe at 30 papers.

### No-go for positive KG import

Positive KG import remains blocked. No S03/S04 evidence validates semantic correctness or trusted graph facts.

## Final M006 recommendation

Close M006 after S04 review and validation, then create M007 for the deterministic +10-to-100 validation CLI. M007 should automate operational evidence collection and review gating, not trusted KG promotion. Trusted import should remain a separate future milestone after reviewed, route-specific promotion criteria exist.
