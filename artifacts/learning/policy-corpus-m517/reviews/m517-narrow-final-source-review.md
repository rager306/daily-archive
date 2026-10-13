# M517 narrow final source review — approval with limits

Scope (bounded, read-only): M517 corrections in
`crates/da-application/src/{parse_policy_health,parse_policy_pair,parse_policy_table}.rs`
vs git HEAD, retired `pair`/`gold-pair` refusal in `crates/da-cli/src/main.rs`,
and isolated capture semantics in `crates/da-cli/examples/policy_corpus_capture.rs`.
No cargo, no index/repo edits, no metric/tuning proposals.

Repo: `/root/daily-archive`  HEAD `718be28b5`  branch `feat/da-parse`.
Reviewed files match the source-map+repair seal
`verification/current-source-verification.json`: recomputed SHA-256 of **all 22
sealed paths equal** the recorded values (0 mismatches). The seal's
`post_measurement_changes` (domain nested-if clippy collapse, ADR-080
constructor inventory, endpoint virtual-time timeout) reconcile exactly with the
diffs observed. Measured binaries remain frozen separately; frozen
320-PDF/60/120/240 cycles are treated as immutable and are not re-derived here.

Graph-first evidence: `verification/gitnexus-detect-all.json` — `changed_count
288`, `affected_count 75`, `risk_level critical`, no `partial`/`truncated`
markers, and all five reviewed source files are present in `changed_symbols`.
`affected_processes` carries only coarse cross-community steps (`Main ->
BoundaryCoordinate` etc.) plus one evaluator `Run -> Ratio`; no process step
reaches a graph/promotion/import write. Impact is consistent with a
candidate-only, offline evaluation surface.

## Verdict: APPROVE WITH LIMITS — no real blocker found

No scoring incorrectness that misleads a claim, no evidence fabrication, no
unsafe graph/import/promotion write in the reviewed scope.

### 1. parse_policy_table.rs — ROUGE-L and mean-reward corrections are correct
- `rouge_l` (L321–356): the HEAD version had a broken LCS recurrence (`tmp`
  unused; `prev_diag` mutated in place, `dp[j-1]` never consulted). It could
  return values > 1.0 and reused repeated tokens. The new recurrence is the
  textbook LCS DP. Verified independently: an oracle matrix implementation and
  3,000 random cases agree to <1e-12; symmetry holds; spot values match the
  added tests (`a`/`a a`=2/3, `b a`/`b b a`=4/5). This is a genuine
  correctness fix.
- Blast radius is nil for the frozen cycles: `rouge_l` has **no caller outside
  its own module** (grep over `crates/`, examples included). It cannot alter the
  recorded 60/120/240 token-F1 or ordered-5gram numbers.
- `table_mean_reward` (L358–373): previously summed `reward` over any
  *covered* bucket regardless of which arm the observation measured, silently
  mixing the counterfactual loser into the mean. Now it counts an observation
  only when the table's selected code equals `obs.decision_code`, and returns
  0.0 when nothing is selected. Semantics match the doc comment and the new
  tests. No production caller (re-exported in `lib.rs`, used only by tests).

### 2. parse_policy_health.rs — success now derives from outcome evidence
- Success is read from `outcome.ok` (L156), not from `obs.reward`; latency is
  read from the outcome (L155). Correct: reward is the pair/learner channel and
  must not drive the health faculty.
- `parse_health` refuses the whole journal when any observation has no outcome
  (`MissingOutcomeEvidence`, L141–147) instead of silently counting it as
  failure. Fail-loud and honest.
- `nearest_rank` L121–123 `div_ceil(100)` is arithmetically identical to the
  prior `(x+99)/100` for all inputs (checked p=0,100 and general).
- Test helper `obs()` sets a real `Some(outcome)` (L380), so the guard does not
  dead-lock the existing suite.

### 3. parse_policy_pair.rs — metric validation added, scoring unchanged
- `from_yaml_str` (L90–104) now rejects axes outside
  `{section_count, body_chars}` and a negative `margin`. The frozen
  `data/policies/parse_pair_metric.yaml` (`section_count`/`body_chars`/margin 0)
  still passes; `evaluation.rs::legacy()` still parses.
- `score_pair` / `score_pair_optional` / `pair_to_observations` verdict logic is
  untouched by this diff; conservation-first veto and NotApplicable handling
  are unchanged.

### 4. main.rs — retired actions refuse before any parser runs
- `"pair" | "gold-pair"` (L1229–1234) now print a single reason and
  `exit(2)` before reading a PDF, opening a journal, or invoking a parser. The
  removed `run_policy_gold_pair` leaves no dangling reference (`grep` clean).
- `health` maps `Err` to stderr + `exit(1)` (L1293–1296); `learn` remains
  candidate-only with explicit `deployed=false, import_eligible=false` prints.
- Covered by `tests/policy_retired_capture_test.rs` (exit 2, no `data/` or
  `artifacts/` created under a temp cwd).

### 5. policy_corpus_capture.rs — isolated, non-overwriting, no promotion write
- Output must be a **new** directory (`create_dir`, L383; `create_new` on every
  artifact write, L50–53), and is refused under `data/` and
  `artifacts/pipeline_cache` (L378–382). Inputs are hashed and re-verified after
  capture (L392–398); PDF-hash mismatches on TOC/GROBID outputs fail before the
  article is written (L136–139, L215–218, L252–255).
- The capture record hard-codes `candidate_only:true, deployed:false,
  import_eligible:false, graph_write:false` (L409–412). No graph, import, or
  promotion write exists anywhere in the file (grep clean). Failures are
  retained in `capture.json` and the process exits non-zero (L416–424).
- Only typed `EmptyToc` becomes `NotApplicable` (L128–131); all other errors stay
  failures. Version metadata is read from a dedicated JSON file, not decoded from
  native PyMuPDF stdout/stderr (L150–155, test L433–450).

## Limits / notes (non-blocking, no action requested)
1. `parse_health` now refuses journals containing any outcome-less observation
   (exit 1). Legacy journal lines parse with `outcome=None` (field is
   `#[serde(default)]`), so `da policy health` will fail loudly on such
   historical journals. This is the intended honesty guard, not a defect, but it
   is a behavioral change operators should expect.
2. Axis/margin validation lives in `PairMetric::from_yaml_str`, not in a
   `Deserialize` invariant; a hand-constructed `PairMetric` (fields are `pub`,
   `Deserialize` derived) could still carry an unknown axis and fall through
   `yield_of`'s `_ => 0` arm. All in-tree callers go through `from_yaml_str`.
3. `parse_policy_gold_f1` remains exported in `lib.rs` but is now unused by the
   CLI after the pair/gold-pair retirement — dead-but-harmless candidate code.

## Standing constraints (unchanged, reaffirmed)
- No overall parse-quality claim: token F1 rose (0.799→0.871) but ordered-5gram
  F1 fell (0.713→0.688) with 31 regressions; the earlier 61.1% gain claim stays
  withdrawn.
- The three Q6 tenfold-capacity flags must stand; validation is an adaptive
  diagnostic, not an untouched final test.
- All artifacts remain candidate-only; production routing and parser defaults are
  unchanged. Frozen 320-PDF/60/120/240 cycles are immutable.

Conclusion: the reviewed corrections are honest, correct, and safely
non-promoting. Approval with the limits above; no real blocker to file.
