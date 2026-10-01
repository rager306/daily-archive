# Addendum (narrow correction) — S05/T01 order-lineage diagnostic

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`

This file **adds** a bounded correction beside
`artifacts/parse_compat/native_body_order_lineage.evidence.md`
(sha256 `891422ed8f4e0e844ba880c9ab7be5ca51a2629e2e00c721c51567166a6825e2`) and
`artifacts/parse_compat/native_body_order_lineage.json`
(sha256 `3b08509c6b3b2be84db1b7ab99e41c9c4bd1bf88a68b754c5c3624b22f302395`).
It does **not** edit, replace or erase either document: both are preserved
byte-for-byte under
`artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/` and are
re-verified by that run's `preserved_before.sha256`.

## What the historical document measured (unchanged, restated here)

The same native tokens, the same independent preserved-TEI character oracle and
the same ordered alignment, under three explicit native order axes; the aggregate
exact-match count moves **814 → 814 → 791**:

| native order axis | aggregate exact | Δ vs delivered |
|---|---|---|
| delivered extraction order | **814** | baseline |
| positioned admitted order (via word projection) | **814** | 0 |
| XY-cut admitted order | **791** | **−23** |

Per document the delivered and positioned admitted orders are the **same order**
(0 positions differ), because the positioned donor assigns
`token_ordinal == source_ordinal`; the XY-cut lattice differs in position but
keeps its typed `segmenter_rejected_lattice` disposition and was never spliced
into any lane (the primary lane is unchanged).

## The narrower claim the measurement supports

**The tested XY-cut reordering did not improve the exact count** (it lowered it by
23 on this three-document slice), and the positioned/admitted reorder is a no-op
on the primary lane. That is the whole measured result.

## The overstatement this addendum refuses

The historical heading “Verdict: REFUTED” refutes **one** hypothesis: *that
re-ordering the same tokens into the admitted/reading order would raise the exact
count substantially*. It does **not** refute “reading order” in general:

- it tests only **three** native order axes, on **one** three-document corpus, on
  **one** metric (exact token matches), against **one** oracle;
- it does not measure section boundaries, hierarchy or reading-order quality —
  that is T02's question, to be answered against the reviewed segmentation oracle
  (boundary/count based), not against this character oracle;
- at the time of that run 69 400 of 70 323 oracle occurrences sat in the
  un-split `uncertain_present_in_token` bucket, so the dominant residual was not
  order-attributed at all.

**Correct statement:** *the tested reorderings did not improve the exact count
(814 → 814 → 791); all reading order is not refuted.*

## Additive step taken in the same task (separate artifact)

`artifacts/parse_compat/native_body_uncertainty_split.json`
(schema `native-body-uncertainty-split.v1`) splits that un-split bucket into
**equal-token** and **proper-substring** facts, conserving the accepted bucket
exactly. It is a fact count, not a verdict, and it promotes nothing.
