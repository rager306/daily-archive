# ADR-080: Two-Arm Paired Observations for Policy Learning (F4)

- Status: Accepted
- Date: 2026-10-12
- Deciders: repository owner + agent (owner: proceed on the branch, no merge until M516 closes)
- Decisions: D508 (coverage comes from same-document two-arm runs, never from production exploration), D509 (a pair is one document measured on both parse arms under one frozen metric; the reward is the arm's measured outcome), D510 (pairs are candidate-only evidence; the table learns from the arm that won, and the losing arm is kept as the negative observation)
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

## 6. Relations

- ADR-078: §5 coverage constraint now has its data source; locks unchanged.
- ADR-079: Faculty D pre-parse analysis gains real paired history; Faculty A
  health can break down by arm.
- ADR-076: unchanged — the pair harness is offline, candidate-only, and does
  not touch waves, promotion, or the production route.
