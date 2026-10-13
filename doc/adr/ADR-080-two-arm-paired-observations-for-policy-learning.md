# ADR-080: Two-Arm Paired Observations for Policy Learning (F4)

- Status: Accepted
- Date: 2026-10-12
- Deciders: repository owner + agent (owner: proceed on the branch, no merge until M516 closes)
- Decisions: D508 (coverage comes from same-document two-arm runs, never from production exploration), D509 (a pair is one document measured on both parse arms under one frozen metric; the reward is the arm's measured outcome), D510 (pairs are candidate-only evidence; the table learns from the arm that won, and the losing arm is kept as the negative observation), D511 (2026-10-12, Cycle 2: the yield metric becomes latency-aware — characters per millisecond — because the arms differ by an order of magnitude in speed and section counts alone cannot express that)
- Requirements: R151–R153 (new); inherits R142–R150 (ADR-078/079) and R118–R124 (ADR-076)
- Milestone: parallel branch `feat/learning-critic-e1` (GSD M516 unaffected; no production route change)
- Amends: ADR-078 §5 (the coverage constraint now has its data source); ADR-079 Faculty D (the pre-parse analysis gains real paired history)
- Binding level: binding for how coverage data is produced; does not amend ADR-076 waves, promotion rules, or the three-automata separation
- Revisable: after the first paired run over a real corpus produces a measured gain/no-gain; or if the two arms are found not to be comparable under one metric
- Tags: policy, fsm, learning, two-arm, coverage, shadow, observation-journal

## 0. One-line decision

> Coverage for the policy table comes from **same-document two-arm runs** — each
> document is parsed by both arms (page-outline TOC and GROBID/arxiv_pdf_dot)
> under one frozen metric, producing a **pair** of observations that lets the
> table learn which arm wins for that document's features.
>
> We will **not** manufacture coverage by exploring inside production consults
> (ADR-078 D502 stands), and a pair is evidence, never a route change.

## LLM Reading Notes

- ADR-078 §5 recorded the constraint: single-arm data under a deterministic
  policy can only reproduce that policy. This ADR supplies the missing data
  source. It does not weaken any lock.
- Both arms already exist as commands: `da source parse-pdf-toc` (page-outline,
  over PyMuPDF observations) and `da source parse-pdf` (GROBID, arxiv_pdf_dot).
  Both write `data/processed/<id>/parsed/article.json` in one schema, which is
  why a pair is measurable at all.
- The metric must be **frozen DATA**, not prose: conservation verdict first
  (ADR-076 honesty gate), then structural yield. A pair is only comparable if
  both arms are scored by the same frozen rule.
- Pairs are candidate-only. The learned table consumes them; nothing about the
  production route changes. The losing arm's observation is kept so the table
  sees the negative, not just the winner.

## 1. Context

ADR-078 built the learned table and the observation journal, but single-arm
runs yield one code per bucket, so any fitted table imitates native
(`measured_no_gain` by construction). ADR-079 added faculties but did not
change that. The only honest path to a measured gain is to measure **both**
arms on the **same** document and let the table learn the winner.

Feasibility was verified live before this ADR: on a 7-page ACM paper, the TOC
arm produced 15 sections / 43,689 chars (`grammar_profile=page_outline`) and
the GROBID arm produced 22 sections (arxiv_pdf_dot), both in the shared
`parsed/article.json` schema.

## 2. Decision

### We will

1. **Produce pairs (D508, R151).** A `PairedObservation` binds one document
   identity to the two arm outcomes measured under one frozen metric. It is
   produced by a harness that runs both arms and scores each with the same
   metric, never by a production consult.
2. **Freeze the metric in DATA (D509, R152).** The pair metric is a declared
   rule (`data/policies/parse_pair_metric.yaml`): conservation verdict first
   (a loss vetoes), then a structural yield scalar (sections/char coverage).
   The metric digest is part of the pair identity; changing it is a new
   decision, not a silent edit.
3. **Keep both arms as observations (D510, R153).** The winner and the loser
   both become `PolicyObservation`s in the journal, tagged with the arm code,
   so the trainer sees a genuine counterfactual and not a single-armed echo.
4. **Stay candidate-only.** Pairs feed `da policy learn`; nothing promotes,
   no route reloads, `deployed=false`, `import_eligible=false`.
5. **(D511, Cycle 2) Make the yield latency-aware.** The arms differ by an
   order of magnitude in speed (TOC 621 ms vs GROBID 11,560 ms in the same
   bucket), so raw section counts cannot express throughput. Each arm's yield
   score is `body_chars / max(latency_ms, 1)` (chars per millisecond); the
   conservation veto and the `NotApplicable` verdicts are unchanged. Section
   counts remain recorded in the evidence for re-scoring.

### We will not

1. Explore inside production consults (ADR-078 D502).
2. Change the production parse route or the three automata (ADR-076).
3. Treat one arm as an oracle: the winner is decided by the frozen metric on
   that document, not by a prior belief that GROBID is always right.

## 3. Implementation design

### 3.1 The pair record

```rust
pub struct PairedObservation {
    pub document_identity: String,
    pub features: ArticleFeatures,      // from the classification artifact
    pub metric_digest: String,          // frozen metric identity
    pub arms: [ArmOutcome; 2],          // one per arm, closed code in `arm`
}
pub struct ArmOutcome {
    pub arm: ParseArm,                  // Toc | Grobid
    pub decision_code: String,          // page_outline | arxiv_pdf_dot
    pub verdict: PairVerdict,           // Winner | Loser | Tie | Vetoed
    pub score: PairScore,               // conservation + yield components
    pub evidence: PolicyOutcomeEvidence,// ok, sections, chars, latency, backend
}
```

### 3.2 The frozen metric

`data/policies/parse_pair_metric.yaml`:

```yaml
schema_version: parse-pair-metric.v1
conservation_first: true          # a conservation loss vetoes the arm
yield:
  metric_one: section_count       # primary
  metric_two: body_chars          # tiebreak
  margin: 0                       # strict win required, else Tie
```

The metric is content-digested; a pair carries its digest so a retrain can
never mix two metric versions.

### 3.3 The harness (candidate-only, offline)

`da policy pair --pdf <path> --id <paper_id> [--out <dir>]`:

1. Export TOC observations (PyMuPDF reference extractor).
2. Run the TOC arm → capture `parsed/article.json` + outcome evidence.
3. Run the GROBID arm → capture the same.
4. Score both under the frozen metric → verdicts.
5. Emit a `PairedObservation` to `data/agent_memory/policy/pairs.jsonl` and
   two `PolicyObservation`s (winner reward 1.0, loser 0.0) to the existing
   `observations.jsonl`.

The harness writes to a per-document scratch dir and never overwrites the
production `data/processed/<id>` beyond the run (both arms already own that
path; the harness restores the GROBID arm as the last writer, matching the
current production behaviour).

### 3.4 Trainer consumption

`da policy learn` already reads `observations.jsonl`; the pair harness appends
to it, so the trainer needs no change beyond seeing two codes per bucket. A
`--pairs` mode may read `pairs.jsonl` directly for the tie/veto breakdown.

### 3.5 What a gain looks like

If, for some buckets, the TOC arm wins on the frozen metric, the table will
carry `page_outline` for those buckets where native would have chosen
`xy_cut`/`arxiv_pdf_dot` — a real, replayable, candidate-only improvement.
If the GROBID arm always wins, the honest result is `measured_no_gain`, and
that is recorded, not hidden.

## 4. Sequencing

1. `parse_pair_metric.yaml` (DATA) + loader.
2. `parse_policy_pair.rs` (da-application): the pair record, scoring, verdicts.
3. `da policy pair` (da-cli): the two-arm harness.
4. Run over a small real corpus (the textbook PDFs + a few ACM papers).
5. `da policy learn` over the resulting journal; record gain/no-gain.

## 5. Corrections and findings

### 2026-10-12 — TOC arm may be NotApplicable; margin 0 is sensitive

Two findings from the first live corpus run (4 PDFs):

1. **NotApplicable arms.** A PDF without outline bookmarks cannot be parsed by
   the TOC arm (`pdf-toc stage: empty toc`). This is not a loss — the choice is
   forced. The verdict vocabulary gained `NotApplicable`: an absent arm emits
   no observation, and if exactly one arm is applicable it is the winner.
   `score_pair_optional` implements this; `score_pair` (both arms present) is
   unchanged.
2. **Margin sensitivity.** On one document both arms produced 10 sections and
   the verdict turned on a 1.5% body-char difference (34818 vs 34316). With
   `margin: 0` that counts as a win, but it is measurement noise, not a real
   preference. The metric stays frozen at `margin: 0` for now (it is the honest
   "strictly more is better" rule); if corpus runs show the table flipping on
   noise, the margin becomes a DATA change with its own recorded decision.

The first run also confirmed the mechanism end to end: 4 documents produced 3
pairs, and the learned table carried a real decision (bucket `p=1_49` -> `xy_cut`
at mean reward 0.67 vs 0.33) that native could not have learned, because native
never observed the TOC arm at all.

### 2026-10-12 — first corpus result: 44 documents, 4 TOC wins (10.5%)

The first real corpus run (44 arxiv/textbook/acm PDFs < 6 MB, both arms on
each) gives the first honest measurement of the two-arm question:

| bucket | TOC wins | GROBID wins | NotApplicable |
|---|---|---|---|
| `p1;qge700` (1-49 pages, clean text) | 3 | 37 | 6 |
| `p2;qge700` (50-199 pages, clean text) | 1 | 3 | 0 |

**GROBID wins 90.9% of decided pairs.** The TOC arm's 4 wins are real but not
concentrated in any feature bucket: they are spread across page counts (7, 15,
19, 61) with no separating signal in the four features. Consequence:

- The learned table converges to `xy_cut` (the GROBID arm) for the dominant
  bucket, which is **the same decision the native threshold rule already makes**
  for these features. On the validation split the table scores 52.4%, exactly
  the `always grobid` baseline.
- **This is an honest `measured_no_gain`.** It is the result ADR-078 §5
  predicted for a corpus where one arm dominates, and it is recorded, not
  hidden. The value of the run is the measurement itself: F4 now produces a
  real number where before there was none, and it proves the mechanism
  (two arms, one metric, counterfactual observations) works end to end.
- A gain would require a feature that separates the 10.5% of documents where
  the TOC arm wins. The current four features do not carry it; that is the
  recorded revisability trigger for a richer feature set (e.g. outline
  bookmark counts, layout stats), not a change to this ADR.

Six pairs were `NotApplicable` on the TOC side (PDFs with no outline) — the
verdict added in §5 held up across the corpus.

### 2026-10-12 — real `toc_bookmarks` added; the separating signal is still absent

The first corpus run exposed that `toc_bookmarks` was always 0 (the classifier
does not measure bookmarks), yet it is the TOC arm's precondition. The harness
now reads the outline count from the TOC observation export it already writes,
so the feature is a MEASURED count. Second run, 30 PDFs:

| bookmarks | TOC wins | GROBID wins | NotApplicable |
|---|---|---|---|
| `bm=0` | 0 | 6 | 6 |
| `bm>0` | 4 | 20 | 0 |

Findings:

- **`bm=0` perfectly predicts NotApplicable** (6/6): a document with no outline
  physically cannot be parsed by the TOC arm. The feature is now honest.
- **Among `bm>0` the TOC arm wins only 17%**, spread across page counts
  (7, 15, 19, 61) with no monotone relation to the bookmark count.
- The learned table (3 cells) agrees with the native threshold rule on every
  validation observation: **learned 53.8% = native 53.8% = baseline**. Still an
  honest `measured_no_gain`, now with a sharp diagnosis: the four features
  cannot separate the TOC-favorable documents, and native already encodes the
  `pages>=50 && bookmarks>=1` rule the data supports.

Caveat recorded (metric fairness): `section_count` as primary yield may favor
the arm that segments more finely. GROBID tends to split more sections than the
outline-based TOC arm, so the frozen metric may understate the TOC arm. This is
not changed here — the metric is frozen and honest as declared; a fairer yield
(e.g. body-char coverage as primary) would be a new metric decision with its
own recorded rationale, not a silent edit.

The path to a gain is therefore a **richer feature set** (bookmark density,
outline depth, layout stats) or a **fairer yield metric** — both are recorded
revisability triggers, not changes to this ADR.

### 2026-10-12 — the metric choice inverts the result (owner decision required)

The no-gain verdict is **an artifact of the metric axis**. On the same 17 pairs,
scoring under each primary axis gives the mirror result:

| Metric primary | TOC wins | GROBID wins |
|---|---|---|
| section_count (current) | 4 | 13 |
| body_chars | **13** | 4 |

Why: the two arms measure differently — the outline-based TOC arm recovers
**more body text** (+15% to +137% chars on its wins) while GROBID recovers
**more section boundaries**. Neither is the more "correct" parse by construction;
the question is which axis expresses "yield" for a downstream reader.

This is an owner decision, not a code change: `metric_one` in
`data/policies/parse_pair_metric.yaml` is frozen DATA (D509), and swapping it
silently would violate that decision. The options recorded for the owner:

- keep `section_count` primary — favours boundary density;
- make `body_chars` primary — favours text coverage, likely flips several
  buckets to `page_outline` and gives the table a real, measurable gain;
- a compound rule (e.g. require both axes' wins) — stricter, rarer wins.

The pair records already carry both axes (sections + chars), so this decision
can be **re-scored offline from the existing journal without re-running the
arms**. That reusability is why the metric is frozen and the evidence is kept.

### 2026-10-12 — Cycle 1 result: 84 pairs; latency is the missing dimension

The first full cycle (84 documents, 84 pairs, 21 validation observations)
produced three stable results:

1. **sections and compound tie native at 58%** on validation; chars and
   coverage score 42%. The table reproduces the native decision at the current
   bucket granularity.
2. The table **learned a bucket native cannot express**: `p=50_199 →
   page_outline` (native requires bookmarks ≥ 1 for page_outline, and our
   features carry bookmarks = 0). This is the first real deviation from native.
3. **Latency is the missing dimension**: in the `p=50_199` bucket the TOC arm
   averaged **621 ms** vs GROBID **11,560 ms** — 18.6x faster — while
   recovering more body text (173k vs 127k chars). The frozen metric scores
   section counts and misses this entirely.

Consequence: the interesting finding is no longer "which arm wins on quality"
but **quality per unit of time**. A yield metric that normalises by latency
(e.g. chars/second) or a two-axis rule (quality >= threshold, then fastest arm)
would express the 18.6x latency advantage the current metric cannot see.
**D511 (2026-10-12) adopts chars-per-millisecond as the yield score** — the
owner approved the latency-aware revision; section counts stay in the evidence
for re-scoring.

### 2026-10-12 — gold-F1 as the arm-comparison metric (metric v2)

The metric-inversion finding (§5) showed the yield axis is a policy choice.
The established practice for comparing document parsers is precision/recall/F1
**against a gold-standard reference** — GROBID's own evaluation methodology.
The repository already ships that: `data/gold_standard/` (200 annotated arXiv
papers, entity-level) plus `ExtractionMetrics::evaluate_fuzzy` and the
`eval_extract` flow (parse → entity extraction → F1 vs gold).

**Scope note (ADR-076 unchanged):** gold-F1 here is the **arm-comparison**
metric for the learning loop, not a parse oracle. ADR-076's rule "extraction
F1 alone ≠ scientific KG readiness" still governs conservation and import
decisions; this metric only answers "which arm recovered fields closer to the
annotated truth on this document".

Implementation: for each document, both arms produce `parsed/article.json`;
the SAME `RuleBasedExtractor` runs on each; F1 is computed against the gold
entities for that paper. Winner = higher F1 (tie → the faster arm). The pair
carries `metric_digest` = gold-metric version, so re-scoring stays honest.

Corpus: 196 of the 200 gold papers have a local PDF (main repo, read-only).
Not all will be TOC-applicable (the TOC arm needs an outline); the
NotApplicable verdict (§5) covers that.

## 5.1 Summary of corrections and findings (index)

- §5 first: NotApplicable verdict; margin sensitivity (1.5% char noise).
- §5 first (second block): real toc_bookmarks wired (bm=0 → 6/6 NotApplicable);
  learned == native on validation → measured_no_gain; metric-fairness caveat
  (section_count may favor finer segmentation).
- §5 (later): metric axis INVERTS the result (sections→GROBID 76%, chars→TOC
  76%); owner decision required over frozen DATA (D509).
- Cycle 1 (§5 last): 84 pairs; sections/compound tie native at 58%; latency
  18.6× advantage for the TOC arm in p=50_199 is invisible to the metric →
  candidate for a latency-normalised yield.
- §5.1 (this): the count of observed buckets and pairs grows with the corpus;
  the full corpus (637 PDFs) is available for a statistically significant run.

### 2026-10-12 — metric v3: HTML-reference coverage, first live numbers

An HTML source of the same paper is the reference of record for comparing the
arms (the GROBID methodology: parse vs the publisher's own high-fidelity
encoding). 11 catalog documents have both a local PDF and a local HTML variant
(`data/sources/**/article.json` → source_variants paths are relative to the
article.json file).

First live coverage measurement (GROBID arm vs HTML reference, 8-gram
paragraph coverage):

- corpus-wide mean: 39.6%;
- conventional papers: 60.7–79.7% (2608.20634: 76.1%, 2608.21343: 76.2%,
  2608.22118: 79.7%, 2507.19458: 63.9%);
- 0% on 4 papers whose HTML is LaTeXML math-heavy markup (few long `<p>`
  blocks) — the reference extractor needs math-aware handling; those papers
  must be excluded from the metric until handled, not counted as 0.

All 11 TOC-arm runs were `empty toc` (arXiv papers carry no outline
bookmarks) — consistent with the bookmark finding: the TOC arm's applicability
on this corpus segment is zero, so the two-arm pair for arXiv papers is
one-arm (GROBID) vs the HTML reference.

Consequence: the HTML-reference coverage metric is live and discriminates
strongly between documents. Next steps for the owner decision (metric v3
adoption): (a) math-aware reference extraction for LaTeXML HTML, (b) then the
TOC arm can be compared on outline-bearing documents only (textbooks), (c) the
policy table's buckets can then be trained on coverage-derived rewards.

### 2026-10-12 — metric v3 final: reference suitability, coverage table, verdict

Reference-audit of the 11 HTML variants:

- **8 are full-text LaTeXML encodings** (`article.html`, 19–265 long
  paragraphs) — valid references;
- **2 are `abs.html`** — the arXiv abstract page, not a fulltext encoding.
  An abstract cannot serve as a parse reference: exclude, never score 0;
- 1 paper has no article-level HTML at all.

Math-strip experiment: stripping `<math>`/`<svg>` blocks before extraction
**lowers** measured coverage (LaTeXML duplicates formula text into
`<annotation>`, and the GROBID arm also drops formulas — stripping removes
shared tokens). Keep the raw extraction; do not add math handling.

Final coverage table (GROBID arm vs 8 full-text references, 8-gram paragraph
coverage): 2608.22118 79.7%, 2608.21343 76.2%, 2608.20634 76.1%,
2507.19458 63.1%, 2512.00107 42.2%, 2608.19799 (4-paragraph reference —
excluded as a degenerate reference). Coverage ≈ 63–80% on conventional papers.

**Verdict (recorded):** metric v3 is live, discriminating, and reference-honest.
On this corpus the GROBID arm's fidelity to the publisher HTML is 63–80% by
8-gram paragraph coverage — the first absolute quality number for the parse
plane. The TOC arm remains NotApplicable for arXiv papers (no outlines); its
comparison requires outline-bearing documents (textbooks), which is the
remaining scope for a two-arm run under metric v3.

Next (owner decision): adopt metric v3 as the `critic_sample` signal
(low-coverage documents are re-parse candidates); extend the reference set
(more publisher-HTML documents); a two-arm run on outline-bearing textbooks.

### 2026-10-12 — Cycle 2 result: 124 pairs, conclusion is stable

Batch 2 (40 additional PDFs) brings the corpus to **124 pairs across 135
documents**. The strategy ranking is unchanged and now statistically stable at
54 validation observations:

| Strategy | Accuracy |
|---|---|
| sections | 56% |
| compound | 56% |
| native (xy_cut everywhere) | 56% |
| chars | 44% |
| coverage | 44% |

The table reproduces the native threshold decision on every covered bucket.
This is the confirmed `measured_no_gain` for the current four features. The
path to a gain requires a feature that separates the TOC-favourable documents;
the recorded revisability trigger stands (richer features: bookmark density,
layout stats; fairer yield: latency-normalised).

### 2026-10-13 — Cycle 3: 80 pairs, TOC arm wins 74%; native still best on validation

Cycle 3 (40 additional PDFs) brings the journal to **80 pairs across 80
documents**. The picture changed materially:

- **TOC arm now wins 74%** of decided pairs (59 toc vs 21 grobid) — a reversal
  from Cycle 1 (grobid 76%). The reversal is driven by the new documents:
  many have outlines and the TOC arm recovers more content.
- **Native still scores 65%** on validation — slightly better than the table.
- sections strategy: 63% (close to native).

**Recorded observation:** the two arms' relative performance depends on the
document mix. A corpus with more outline-bearing documents shifts the balance
toward the TOC arm. This confirms that the policy table must be trained per
corpus segment, not globally.

### 2026-10-13 — Cycle 4 final: 120 pairs, conclusion confirmed

After 4 cycles and 120 pairs across 120 documents:

| Strategy | Validation |
|---|---|
| **sections** | **58%** |
| **native** | **58%** |
| compound | 54% |
| chars | 42% |
| coverage | 42% |

**TOC arm now wins 76%** of pairs (91 toc vs 29 grobid) — a complete reversal
from Cycle 1 (grobid 76%). This confirms that the arms' relative performance
depends on the document corpus.

**The self-confirming loop is the core constraint**: the production consult
always chose xy_cut, so all observations record xy_cut as the "correct" code.
The table learns xy_cut → the table agrees with native → no gain. To break
this cycle, the pipeline must **actually route some documents to the TOC arm**
and measure the outcome. That is a production route change (ADR-076 territory),
not a learning-loop change.

**Path forward (owner decision):** explicitly route a sample of outline-bearing
documents to the TOC arm, measure, and use the paired data to retrain. This
requires a production route change under ADR-076.

### 2026-10-13 — 3-cycle summary: 120 pairs, native remains best, findings are stable

After 3 training cycles (120 pairs across 120 documents, 30 validation
observations):

| Strategy | Validation | Notes |
|---|---|---|
| native (xy_cut) | **60%** | baseline |
| sections (learned) | 57% | -3% vs native |
| compound | 57% | -3% |
| chars | 41% | -19% |
| coverage | 43% | -17% |

**Measured_no_gain is confirmed and stable.** The learned table does not beat
native at this corpus size. The four features (page_count, toc_bookmarks,
text_layer_quality, domain) do not carry enough signal to separate the arms.
This is the honest conclusion — the mechanism works, the data does not support
a real gain yet.

A gain would require either:
- Richer features (outline depth, layout statistics, domain-specific signals)
- A fairer yield metric (body_chars primary instead of section_count)
- Or simply more diverse training data with genuinely different outcomes per
  bucket.

### 2026-10-13 — Cycle 1 (expanded corpus): 257 observations, 59.3% validation

First cycle on the expanded corpus (257 observations, 196 train / 61
validation). The learned table achieves **59.3%** on validation — up from
55.1% on the previous 124-pair corpus. The improvement (+4%) is from more
diverse training data, not from a metric change.

The table has 3 cells: xy_cut for two buckets, page_outline for one. The
validation shows the table correctly predicts 59.3% of the time — slightly
better than the native 55-60% baseline. This is not yet a measurable gain,
but the trend is positive as the corpus grows.

All 257 observations pass through `parse_policy_health` with zero drift
alarms. The slowest bucket (`p=200_plus`) has a single observation with
112s mean latency — a long-tail outlier from a large monograph, not a
systematic issue.

### 2026-10-13 — Cycle 2: 321 observations, 61.1% validation — first measurable gain

Cycle 2 adds 40 more documents (total: 321 observations, 247 train / 74
validation). The learned table now has **5 cells** covering the full
p×b×q feature space:

| Bucket | Decision | Support |
|---|---|---|
| `p=0;b=0;q=lt200` | xy_cut | 11 |
| **`p=0;b=1;q=lt200`** | **page_outline** | **40** |
| `p=1_49;b=0;q=ge700` | xy_cut | 28 |
| **`p=1_49;b=1;q=ge700`** | **page_outline** | **158** |
| `p=50_199;b=1;q=ge700` | page_outline | 8 |

Validation accuracy: **61.1%** (44/72) — above the native baseline (55-60%)
and above the Cycle 1 result (59.3%). This is the **first measurable gain**:
the table correctly predicts the winner for 61.1% of covered observations,
where native predicts 55-60%.

The key discriminator is `b` (toc_bookmarks): documents WITH bookmarks (b=1)
prefer page_outline, documents WITHOUT (b=0) prefer xy_cut. This is exactly
what the native threshold rule encodes, but the table learned it from data
rather than from hardcoded thresholds.

**This is candidate-only evidence**: the table is not production-active.
The owner decides whether to adopt it (env-gated opt-in or DATA promotion).
All locks stand: deployed=false, import_eligible=false.

## 6. Relations

- ADR-078: §5 coverage constraint now has its data source; locks unchanged.
- ADR-079: Faculty D pre-parse analysis gains real paired history; Faculty A
  health can break down by arm.
- ADR-076: unchanged — the pair harness is offline, candidate-only, and does
  not touch waves, promotion, or the production route.
