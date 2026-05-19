# Thirty-paper deviation report

## Summary

M006/S03 ran a Markdown-based structure-aware deviation scan over all 30 selected papers. This is the first corpus larger than the M005 10-paper baseline where source readiness is complete: all 30 papers are Markdown-ready after S02 acquisition/repair.

The scan produced 4,289 structure-aware chunks over 1,761,102 Markdown bytes. All chunks remain refused for trusted KG import: `import_eligible_chunk_count=0`, `import_ready_count=0`, and `refused_chunk_count=4289`. This preserves the M005 safety conclusion while giving a broader picture of recurring routes and outliers.

## Headline metrics

| Metric | Value |
|---|---:|
| Papers scanned | 30 |
| M005 overlap | 10 |
| Expansion papers | 20 |
| Markdown bytes | 1,761,102 |
| Elements | 4,319 |
| Chunks | 4,289 |
| Mean chunks per paper | 142.97 |
| Min chunks per paper | 55 |
| Max chunks per paper | 411 |
| Annotations | 17,573 |
| Import-eligible chunks | 0 |
| Refused chunks | 4,289 |
| Outlier papers | 11 |
| Cached PDFs | 8/30 |

## M005 baseline comparison

The closest apples-to-apples baseline is M005/S03 structure-aware evidence, not M005/S06's mixed benchmark. Compared with M005/S03:

| Metric | M005/S03 10-paper | M006/S03 30-paper | Delta |
|---|---:|---:|---:|
| Papers | 10 | 30 | +20 |
| Chunks | 1,831 | 4,289 | +2,458 |
| Chunks per paper | 183.10 | 142.97 | -40.13 |
| Import-eligible chunks | 0 | 0 | 0 |

Compared with M005/S06 mixed benchmark, the 30-paper scan has 4,289 current structure-aware chunks versus 2,471 mixed benchmark candidates, a +1,818 delta. That S06 number is not a pure structure-aware denominator, so it should be used only as a broad import-boundary comparison.

## Route distribution shifts

| Route | M005/S03 share | M006/S03 share | Delta |
|---|---:|---:|---:|
| `retrieval_only` | 0.7641 | 0.7009 | -0.0632 |
| `method_extraction` | 0.0743 | 0.1035 | +0.0292 |
| `figure_caption_context` / figure route | 0.0470 | 0.0660 | +0.0190 |
| `citation_graph` | 0.0060 | 0.0187 | +0.0127 |
| `claim_extraction` | 0.1338 | 0.1453 | +0.0115 |
| `table_extraction` | 0.0208 | 0.0312 | +0.0104 |
| `equation_context` | 0.0797 | 0.0602 | -0.0195 |

## New or stronger patterns

### 1. Retrieval-only still dominates, but less than in M005

Retrieval-only chunks remain the majority:

```text
M006 retrieval_only: 3006 / 4289 = 70.09%
```

But the share is lower than M005/S03's 76.41%. The larger corpus has relatively more method, figure, citation, claim, and table candidates. That suggests the 10-paper baseline underrepresented some review-heavy routes.

### 2. Method and review routes grow materially

`method_extraction` grows from 7.43% to 10.35%. This is one of the clearest route shifts. Future automation should add method-specific review/repair diagnostics rather than treating method candidates as generic claim candidates.

### 3. Figure/table/citation routes become more visible

Figure route share rises from 4.70% to 6.60%, table route from 2.08% to 3.12%, and citation route from 0.60% to 1.87%. These are still not trusted KG facts, but they show that multimodal/citation-adjacent structure becomes more common as the sample broadens.

### 4. Equation share decreases

Equation-context share drops from 7.97% to 6.02%. The M005 10-paper set likely overrepresented math/equation-heavy cases compared with this 30-paper availability-biased expansion.

### 5. Positive import remains correctly blocked

Across 4,289 chunks, import eligibility remains zero. This confirms that broader source readiness and more chunks do not imply KG readiness. A positive import subset still requires separate route-specific review and promotion.

## Outliers

The scanner flagged 11 outlier papers:

| Paper | Flags | Chunk count |
|---|---|---:|
| `2605.14743v1` | high_chunk_count | 411 |
| `2605.14291v1` | high_chunk_count | 317 |
| `2605.15033v1` | high_chunk_count, claim_candidate_heavy | 242 |
| `2001.00119v2` | claim_candidate_heavy | 227 |
| `2001.00139v1` | claim_candidate_heavy | 201 |
| `2605.15156v1` | table_heavy | 165 |
| `2605.15034v1` | claim_candidate_heavy | 132 |
| `2001.00137v2` | table_heavy, claim_candidate_heavy | 107 |
| `2605.14918v1` | claim_candidate_heavy | 106 |
| `2001.00208v2` | claim_candidate_heavy | 90 |
| `2605.14995v1` | claim_candidate_heavy | 88 |

These outliers are the best candidates for manual review in S04 and for future automation improvements.

## Source and conversion caveats

S02 made the corpus 30/30 Markdown-ready, but PDF/source completeness is still partial:

```text
cached PDFs: 8/30
```

This S03 analysis is therefore valid as a Markdown-based chunking/import-model deviation scan. It is not a full multimodal/PDF completeness scan. The targeted Docling repair for `2001.00186v1` should be tracked as a conversion-method caveat.

## Automation implications for the proposed +10 loop to 100 papers

The 30-paper scan supports the user's proposed iterative loop, but with specific automation needs:

1. Each +10 batch needs source-readiness preflight before chunking metrics.
2. Fast acquisition should run first; slow Docling/PDF repair should be targeted, not bulk.
3. Each batch should compute route-share deltas, not just total chunk counts.
4. Outlier selection should prioritize high chunk count, claim-heavy, table-heavy, figure-heavy, citation-heavy, and unexpected import eligibility.
5. Positive import must remain blocked unless a separate reviewed non-zero import-eligible subset exists.

## Recommendation

Proceed to S04 independent review. The review should focus on whether these patterns are semantically useful enough to drive the next automation milestone:

- method-route growth;
- table/figure/citation route visibility;
- high-chunk-count outliers;
- claim-heavy expansion papers;
- source-readiness and Docling repair caveats;
- continued zero import eligibility.

After S04, plan a dedicated CLI automation milestone for iterative +10 batches toward 100 papers.
