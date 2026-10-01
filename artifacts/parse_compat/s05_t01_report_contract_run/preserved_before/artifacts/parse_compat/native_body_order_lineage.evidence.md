# S05/T01 order-lineage diagnostic — run evidence

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`
Bounded empirical diagnostic (read-only, additive). It tests **one** read-only
hypothesis; it does not repair the accepted P1 API and does not close T01.

Producing run: `bash artifacts/parse_compat/s05_t01_order_lineage_run/capture.sh`
(detached, sequential, one cargo at a time, no execution timeout).
Preservation step: `bash artifacts/parse_compat/s05_t01_order_lineage_run/preserve_before.sh`
Durable run directory: `artifacts/parse_compat/s05_t01_order_lineage_run/`
(new report is a separate artifact; nothing frozen is overwritten)

Report artifact: `artifacts/parse_compat/native_body_order_lineage.json`
= `3b08509c6b3b2be84db1b7ab99e41c9c4bd1bf88a68b754c5c3624b22f302395`
(schema `native-body-order-lineage.v1`; native surface
`da-parse.native-body-order-lineage/v1`).

## Hypothesis under test

A read-only diagnosis proposed: the accepted character comparison builds its
native token stream in delivered extraction order `(page_ordinal,
source_ordinal)` rather than in the admitted token reading order
(`token_ordinal`), because the reading-order reorder reassigns `token_ordinal`
while preserving `source_ordinal`; and *that* explains the low exact count
(814 of 70 323).

Stated as a testable claim: **re-ordering the same native tokens into the
admitted/reading order would raise the exact count substantially.**

## Verdict: REFUTED

The same tokens, the same oracle and the same alignment were measured under three
explicit native order axes. The aggregate exact count moves **814 → 814 → 791**:

| axis | disposition | aggregate exact | Δ vs delivered |
|---|---|---|---|
| delivered extraction order | `primary_lane_delivery_order` | **814** | baseline |
| positioned admitted order (via word projection) | `primary_lane_admitted_order` | **814** | 0 |
| XY-cut admitted order | `segmenter_rejected_lattice` | **791** | **−23** |

Re-ordering does not move the count toward the oracle denominator; the reading
order reorder makes it marginally *worse*. The order hypothesis is therefore not
the explanation, and the diagnostic reports that rather than inventing a fix.

The premise is also vacuous on the primary lane: the audit proves the delivered
order and the positioned lane's admitted order are the **same order** (0 positions
differ) for every document, because the positioned donor assigns
`token_ordinal == source_ordinal`.

## Lineage and order map (explicit document token ids, no lexeme mismatch)

Before any comparison the three order maps were audited. Reported per document
(`order_audit.pairs`), positions differ:

| document | tokens | delivered vs admitted | delivered vs XY-cut | identity digest equal | lexeme digests agree |
|---|---|---|---|---|---|
| hub-2507.19457 | 14 118 | **0** (identical) | 4 037 (28.6 %) | yes | yes |
| rlm-2512.24601 | 8 330 | **0** (identical) | 5 392 (64.7 %) | yes | yes |
| reca-2605.26525v1 | 5 430 | **0** (identical) | 3 265 (60.1 %) | yes | yes |

The audit refuses a duplicate identity, a duplicate `token_ordinal` per page, a
differing identity set, or a differing digest for any shared identity. It
succeeded, so all three axes are permutations of exactly the same document token
identities with the same text digest for each identity — no drop, no duplication,
no lexeme mismatch. Every axis fragment is resolved through `ordered_fragments`,
which re-derives `sha256(text)` and refuses a disagreement.

**The oracle is never an order source.** The axis vocabulary is closed over native
order sources (`raw_source_order`, `positioned_admitted_order`,
`xy_cut_admitted_order`); deserializing an oracle-derived lane is a typed error,
and no public entry point accepts oracle input.

## Measured result — P1 (plane vs the same independent oracle)

Oracle units are identical across axes (the oracle is one fixed token stream):

| document | axis | plane units | oracle units | exact | boundary | uncertain (in-token / spanning / material) | absent | verdict |
|---|---|---|---|---|---|---|---|---|
| hub-2507.19457 | delivered | 43 587 | 33 094 | **117** | 0 | 32 977 (32 907 / 63 / 7) | 0 | `not_assessable` |
| hub-2507.19457 | admitted | 43 587 | 33 094 | **117** | 0 | 32 977 (32 907 / 63 / 7) | 0 | `not_assessable` |
| hub-2507.19457 | XY-cut | 43 587 | 33 094 | **119** | 0 | 32 975 (32 905 / 63 / 7) | 0 | `not_assessable` |
| rlm-2512.24601 | delivered | 23 437 | 15 713 | **383** | 0 | 15 330 (15 322 / 6 / 2) | 0 | `not_assessable` |
| rlm-2512.24601 | admitted | 23 437 | 15 713 | **383** | 0 | 15 330 (15 322 / 6 / 2) | 0 | `not_assessable` |
| rlm-2512.24601 | XY-cut | 23 437 | 15 713 | **383** | 0 | 15 330 (15 322 / 6 / 2) | 0 | `not_assessable` |
| reca-2605.26525v1 | delivered | 30 130 | 21 516 | **314** | 0 | 21 202 (21 171 / 23 / 8) | 0 | `not_assessable` |
| reca-2605.26525v1 | admitted | 30 130 | 21 516 | **314** | 0 | 21 202 (21 171 / 23 / 8) | 0 | `not_assessable` |
| reca-2605.26525v1 | XY-cut | 30 130 | 21 516 | **289** | 0 | 21 227 (21 196 / 23 / 8) | 0 | `not_assessable` |
| corpus | delivered | — | 70 323 | **814** | 0 | 69 509 (69 400 / 92 / 17) | 0 | `not_assessable` |
| corpus | admitted | — | 70 323 | **814** | 0 | 69 509 (69 400 / 92 / 17) | 0 | `not_assessable` |
| corpus | XY-cut | — | 70 323 | **791** | 0 | 69 532 (69 423 / 92 / 17) | 0 | `not_assessable` |

`d498_reopen_required = false` on every axis and every document: `absent = 0`
everywhere, so no proven material deficit exists. **That is a measurement result,
not a clearance.** Every band is `NotAssessable`; no axis is `GranularityOnly`.

Cross-check that this additive probe is faithful: on the delivered axis it
reproduces the accepted artifact's P1 numbers exactly — 814 / 69 509
(69 400 / 92 / 17) and the per-document breakdowns 32 907 / 63 / 7,
15 322 / 6 / 2, 21 171 / 23 / 8.

## Measured result — P2 (unchanged primary lane)

Both P2 axes are computed on the unchanged primary lane; the reorder is **not**
spliced into them.

| band | declared | contributing | population complete | typed absences |
|---|---|---|---|---|
| S03 parent span | 3 | 1 | **false** | hub-2507.19457 `NoBodyTokens`, reca-2605.26525v1 `NoBodyTokens` |
| word/heading | 3 | 3 | true | — |

The S03 population gap for hub/reca is reproduced, not cleared. Neither band is a
clean reading.

## What this run does NOT establish

1. **The cause of the low exact count is not established.** Order is refuted as
   the explanation; that is all this run proves. The unit totals differ on the two
   sides (plane 43 587 / 23 437 / 30 130 against oracle 33 094 / 15 713 / 21 516),
   and 69 400 of 70 323 corpus oracle occurrences fall in
   `uncertain_present_in_token`, a bucket that merges occurrences *equal to* one
   side token with occurrences that are only a *proper substring* of one. Those
   two sub-classes have different owners (anchoring vs granularity) and this run
   does not separate them, so neither is attributed.
2. **No quality claim.** Exact/uncertain counts are a comparison, not a quality
   proof; a zero uncertain count would still not be a clearance. No threshold is
   accepted.
3. **Reading order is T02's question.** This diagnostic only shows that the
   primary lane's admitted token order *is* its delivered order, and that the
   reorder does not raise exact matches. It does not evaluate section boundaries.

## Verified while reading (not repeated from a scouting claim)

`crates/da-parse/src/native_segmentation.rs:877` `full_marker` matches on
`text.trim().to_ascii_lowercase()` — ASCII-only folding, not Unicode
`to_lowercase`. This module is outside the diagnostic's inputs (no region
classification is performed here); the fact is recorded only so the claim is not
inherited unverified.

## Preservation (byte facts, re-verified after the producing run)

`05_preserved_artifacts_check` and `06_preserved_run_dirs_check` re-hashed every
preserved prior artifact and every file of the three earlier run directories after
the probe; both returned rc 0 (11 files, 149 files). `hash_compare.txt` reads
`SAME evidence`, `SAME input`, `SAME source`, `SAME artifact`.

| prior artifact | sha256 (run time) | state |
|---|---|---|
| `native_body_character_shadow.json` | `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361` | unchanged |
| `native_body_character_scope_bound.prior_repair.json` | `ce87e3af6fc9f332520df77463001746aecd76fc4a82d949b1974b28d2ae9d3f` | unchanged |
| `native_body_character_scope_bound.repair2_artifact.json` | `1ad2b56f653f0ea22784f9a3c975556ee7771b0f0b365ec169f7914624661638` | unchanged |
| `native_body_character_scope_bound.json` | `027d8c934e9009001b968ecb95d13eef586d3740a83a2884d21924a49438f27a` | unchanged |

Accepted T01 sources were not touched — `preserved_accepted_sources.txt` compares
them to the accepted P1 run's own record and reports `SAME` for
`crates/da-parse/src/native_body_text.rs`,
`crates/da-parse/tests/native_body_character_oracle.rs` and
`crates/da-cli/examples/native_body_character_probe.rs`. The accepted P1 API
(`localize_body_plane_checked`, `BodyPlaneLocalization`, `classify_body_plane`,
`plane_verdict_for_population`) is **called, never edited**.

## Inputs and pins (unchanged before == after; the probe re-verifies from bytes and exits non-zero on drift)

| path | sha256 |
|---|---|
| `data/parse/layout_bakeoff.yaml` | `530ce05297e881ac91b7404c608dec5aadd9ffd22b7bbbbdd68e064b534290b5` |
| `crates/da-parse/tests/fixtures/preserved/provenance.yaml` | `028a5d62cc8412097ea03ee5e529bae67bf8f9d4fe7819116e6365ad2aca0ebf` |
| hub TEI | `985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4` |
| rlm TEI | `e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3` |
| reca TEI | `dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040` |
| hub PDF | `ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec` |
| rlm PDF | `8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca` |
| reca PDF | `a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8` |

## Run (original exit codes preserved; no execution timeout)

Capture script: `artifacts/parse_compat/s05_t01_order_lineage_run/capture.sh`,
sha256 `e88d537fbceb20ebedf6a47561381875d6c7089e5e5d94fcdb90eba5d30c643a`
(identical before the run, at run time in `RUN.identity.txt`, and after).
Preserve script sha256
`7c2c464f637fbf10f16e10cf185b540370c7f073bbb7f53966af443f22f5862e`.
Window (UTC): `2026-09-30T22:34:58.444361668Z` → `2026-09-30T22:38:10.920782982Z`,
elapsed `192 476 ms`. `git_head = a6facbef77a7a53574883dd451270fa87d2891e0`
(`feat/da-parse`), `cargo 1.98.1`, `rustc 1.98.1`.

| step | command | elapsed ms | rc | stdout B | stderr B |
|---|---|---|---|---|---|
| 01_order_lineage_contract | `cargo test -p da-parse --test native_body_order_lineage_contract` | 4 871 | 0 | 1 419 | 269 |
| 02_architecture_contract | `cargo test -p da-parse --test architecture_contract` | 919 | 0 | 7 600 | 176 |
| 03a_clippy_da_parse | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 1 116 | 0 | 0 | 139 |
| 03b_clippy_da_cli | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 919 | 0 | 0 | 15 091 |
| 03c_clippy_probe | `cargo clippy -p da-cli --features pdf-layout --example native_body_order_lineage_probe --no-deps -- -D warnings` | 751 | 0 | 0 | 15 091 |
| 04_probe | `cargo run -p da-cli --features pdf-layout --example native_body_order_lineage_probe` | 175 635 | 0 | 2 203 | 15 058 |
| 05_preserved_artifacts_check | `sha256sum -c …/preserved_before.sha256` | 13 | 0 | 760 | 0 |
| 06_preserved_run_dirs_check | `sha256sum -c …/preserved_run_dirs.sha256` | 27 | 0 | 12 199 | 0 |
| 07_fmt_check | `cargo fmt -p kg-… -p da-… -p da-mcp -- --check` | 7 089 | 0 | 0 | 0 |

Per-step argv, start/end UTC and epoch-ns, elapsed ms, exit code and log sizes are
in `steps.tsv` / `NN_*.meta`; log byte counts and sha256 are in
`logs.manifest.tsv`. The 15 KB stderr on the clippy/probe steps is vendored
`samyama` warnings: every warning in those captures points into
`/root/vendor-source/samyama-graph*`, and no warning names
`da-parse`/`da-cli`/`da-adapters`. The report is byte-reproducible: `hash_compare.txt`
reads `SAME artifact`, i.e. the producing run reproduced the artifact bytes that
the first (test-faulty) run already wrote.

### First capture run, preserved honestly

The first capture run of the same script failed one step: `01_order_lineage_contract`
returned rc 101 because the test `a_reordered_pair_is_reported_as_differing_never_as_identity`
asserted 3 differing positions for a **three**-token reversal, which fixes the
middle position — the correct value is 2. That was a test-expectation fault, not
an implementation fault; the assertion was corrected (and now states the fixed
point explicitly). The failing run's step logs, `steps.tsv`, `RUN.identity.txt`
and `hash_compare.txt` are preserved byte-for-byte under
`artifacts/parse_compat/s05_t01_order_lineage_run/failed_run_1/`. That run also
reported `CHANGED source`, correctly: the test file was corrected while it ran.

## Verification performed (all from the producing run)

- `cargo test -p da-parse --test native_body_order_lineage_contract` → **22 passed**, rc 0
  (closed lane vocabulary and oracle-lane refusal; explicit-identity order map;
  typed absences; duplicate identity/ordinal; non-lowercase digest; cross-order
  document/count/identity/digest refusals; too-few orders; repeated lane;
  reorder-is-not-identity; identity digest order-independence; oracle-permutation
  invariance of the audit; missing-lexeme and digest-mismatch refusals in
  `ordered_fragments`; order preservation; determinism; redaction; entry bound).
- `cargo test -p da-parse --test architecture_contract` → **101 passed**, rc 0
  (100 prior + the new additive purity gate for the new module).
- clippy (da-parse lib+tests; da-cli lib+tests; the new example with
  `pdf-layout`), all `--no-deps -- -D warnings` → rc 0.
- probe → rc 0, report written, artifact byte-reproducible.
- preserved prior artifacts and run directories re-hashed → rc 0.
- `cargo fmt … -- --check` → rc 0, no drift; the last Rust action.

## Locks and scope (unchanged)

`claim_scope = diagnostic_only`, `product_coverage_effect = none`,
`promotion_state = candidate`, `auto_promote = forbidden`,
`graph_mutation = forbidden`, `import_eligible = false`,
`primary_lane = unchanged`, `thresholds = not_accepted`. No new engine,
dependency, download, graph write, import, promotion or region-segmentation
rewrite; the XY-cut lattice keeps its typed `segmenter_rejected_lattice`
disposition and is not spliced into any lane. No API repair was repeated, and no
blocker, waiver, lifecycle, recovery or engine decision was changed.

## T01 status

**Not complete.** Every axis and band reports `NotAssessable`; the body-gap
question is exactly as open as the accepted P1 run left it, and this run neither
raises D498 nor clears anything.

## Next minimal canonical step (T02 owns reading order)

1. **Do not re-derive the plane in "admitted order".** Byte-measured: on the
   primary lane the admitted order *is* the delivered order, and the reorder lowers
   the exact count. A T02 plan that budgets for this would spend on a no-op.
2. **Split the `uncertain_present_in_token` bucket before attributing anything.**
   Add two additive counters — occurrence *equal to* one side token, and occurrence
   a *proper substring* of one — to the P1 shape, then re-measure. 69 400 of
   70 323 occurrences sit there, and only that split separates an anchoring
   (reading-order) question from a tokenization (granularity) question. The
   accepted P1 run already named this separation as future work.
3. **Keep the T02 oracle separate.** Section/boundary/reading-order acceptance must
   use the reviewed segmentation oracle (boundary/count based); the character
   oracle used here must not be reused as an order oracle, exactly as the S05 plan
   requires.
