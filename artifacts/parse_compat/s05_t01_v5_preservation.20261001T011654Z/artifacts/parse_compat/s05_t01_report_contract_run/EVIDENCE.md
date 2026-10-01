# S05/T01 uncertainty-split REPORT-CONTRACT repair — run evidence

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`

One bounded, read-only, **additive** repair of the uncertainty-split diagnostic's
**report contract**. It changes how the report folds and names facts; it does not
change a single measurement, verdict, threshold or lock, and it **does not close
T01**. The measurement-level correction is documented beside the prior evidence
in `artifacts/parse_compat/native_body_uncertainty_split.addendum.md`.

Producing run (detached, sequential, one cargo process at a time, no execution
timeout): `bash artifacts/parse_compat/s05_t01_report_contract_run/capture.sh`
(capture script sha256 `16140535…`, recorded before the run).
Preservation step, run before any edit:
`bash artifacts/parse_compat/s05_t01_report_contract_run/preserve_before.sh`
→ `preserved_before.sha256` (28 files, copies under `preserved_before/`) +
`preserved_run_dirs.sha256` (409 files: every pre-existing durable run directory).
Post-format verification replay:
`bash artifacts/parse_compat/s05_t01_report_contract_run/verify_after_fmt.sh`
(capture-side script sha256 `1b88428d…`, recorded before the run) →
`steps.after_fmt.tsv`.
Pre-run validation of the corrected report, written before the frozen capture and
**not** touching the canonical artifact:
`pre_run_validation.*` → rc 0, 52 624 ms, `--report …/pre_run_validation.json`.

Report artifact: `artifacts/parse_compat/native_body_uncertainty_split.json`
= `478054d5bd36d6d59057d53f4ba6ddbc21ccfb5578241fd081b595c6c877d69b`
(schema `native-body-uncertainty-split.v1`), replacing the preserved prior bytes
`e4dc96e4…`. The probe ran twice inside the capture (`06_probe`,
`06b_probe_replay`), both produced that same sha256 (`SAME artifact-replay`), and
the after-format replay produced it a third time
(`SAME artifact-after-fmt`), so the artifact is byte-deterministic.

## The three defects fixed (the reason for this run)

Finding: **the split report's reproduction claim was P1-only, and its P2
non-measurements were relabelled.**

| # | defect in the report | correction |
|---|---|---|
| 1 | aggregate `accepted_bucket_reproduced` was `p1_documents_reproduced_all && p1_split_bucket_total == 69 400`, so a drift in the accepted P2 word-body bucket (`62 302`) still read as a clean reproduction | `accepted_buckets_reproduced` now conjoins **both** bands: P1 total `69 400`, P2 total `62 302`, every declared document on both bands, and a complete P2 population |
| 2 | every P2 non-measurement was reported as the literal `"NoBodyTokens"`, discarding the word row's own typed `absence` | `p2_absence` is taken from `word_row.absence`; an unnamed absence is a typed refusal (`P2BandAbsenceUnnamed`), never a fallback |
| 3 | `split_body_uncertainty(...).ok()` swallowed `NativeBodyTextError::InventoryRungMismatch` | the refusal is recorded as `p2_split_refusal` (fail-closed) and fails the aggregate claim; a measured band with no split, or a split with no band, is `P2MeasurementInconsistent` |

Report-level negatives added to the probe (7 tests, `04_report_contract_example`
/ `v04_report_contract_example`, rc 0): P2 drift on the aggregate axis, on the
per-document axis and on the population axis; a band absence keeping its own
reason; an unnamed absence as a typed refusal; a refused split failing closed and
never becoming an absence; an inconsistent measurement as a typed refusal; a
measured band reproducing its pin; and the by-construction statement.

## Conservation is by construction, not an independent gate

`SideLocalization::uncertain_present_in_token()` (`crates/da-parse/src/native_body_text.rs`)
**is** the sum of the two fields the split returns, so `equal + proper == bucket`
cannot fail independently of the shared computation that produces both sides.
`p1_conservation_ok` / `p2_conservation_ok` are a self-consistency reflex, not
evidence of preservation. The report's `conservation_invariant` now says this
("BY CONSTRUCTION", "NOT evidence"), a report-level negative pins the wording, and
the da-parse contract test documents it beside the assertion it qualifies.

## Capture run (original exit codes; no execution timeout)

| step | command | rc | elapsed ms | stdout B | stderr B |
|---|---|---|---|---|---|
| `01_uncertainty_split_contract` | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 0 | 523 | 811 | 210 |
| `02_native_body_character_oracle` | `cargo test -p da-parse --test native_body_character_oracle` | 0 | 2 202 | 2 641 | 190 |
| `03_architecture_contract` | `cargo test -p da-parse --test architecture_contract` | 0 | 1 101 | 7 600 | 176 |
| `04_report_contract_example` | `cargo test -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe` | 0 | 839 | 763 | 15 138 |
| `05a_clippy_da_parse` | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 0 | 20 308 | 0 | 140 |
| `05b_clippy_da_cli` | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 0 | 26 154 | 0 | 15 295 |
| `05c_clippy_probe` | `cargo clippy -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe --no-deps -- -D warnings` | 0 | 797 | 0 | 15 091 |
| `05d_clippy_probe_tests` | `cargo clippy -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe --tests --no-deps -- -D warnings` | 0 | 942 | 0 | 15 091 |
| `06_probe` | `cargo run -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe` | 0 | 49 289 | 1 857 | 15 062 |
| `06b_probe_replay` | same | 0 | 48 820 | 1 857 | 15 062 |
| `07_preserved_copies_check` | `sha256sum -c preserved_before.sha256` (from the copies) | 0 | 19 | 1 924 | 0 |
| **`08_fmt_check`** | `cargo fmt -p … -- --check` (LAST Rust action) | **0** | 7 420 | 0 | 0 |

Capture window (UTC): `2026-09-30T23:35:25.592728073Z` →
`2026-09-30T23:38:05.269638530Z`, elapsed 159 677 ms. `git_head =
a6facbef77a7a53574883dd451270fa87d2891e0` (`feat/da-parse`), `cargo 1.98.1`,
`rustc 1.98.1`.

The 15 KB stderr on the clippy and probe steps is vendored `samyama`,
`samyama-sdk`, `samyama-graph-algorithms` warnings outside this repair. Counted
rather than eyeballed: in each of `05c`/`05d`/`06` stderr there are **zero**
lines containing `error`, **zero** lines naming an owned crate path
(`/root/daily-archive/crates/…`), and the only packages reporting generated
warnings are the three vendored ones above. No
pipeline is used to run any step, so every recorded exit code is the command's
own (`run_step` calls the command directly and reads `$?`); no `| tee`,
`|| true` or trailing-pipe status can discard it.

## Post-format verification replay (the previous run's gap)

The prior run recorded clippy **before** its formatting fix and never re-ran it
afterwards, so its clippy result was not a statement about the frozen final
sources. This run closes that: the capture's clippy steps (`05a`–`05d`) already
ran after `cargo fmt` was applied to the final sources, and a separate replay
re-ran the whole set — including clippy — on those frozen sources.

| step | command | rc | elapsed ms | stdout B | stderr B |
|---|---|---|---|---|---|
| `v01_uncertainty_split_contract` | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 0 | 525 | 811 | 210 |
| `v02_native_body_character_oracle` | `cargo test -p da-parse --test native_body_character_oracle` | 0 | 2 007 | 2 641 | 190 |
| `v03_architecture_contract` | `cargo test -p da-parse --test architecture_contract` | 0 | 846 | 7 600 | 176 |
| `v04_report_contract_example` | `cargo test -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe` | 0 | 827 | 763 | 15 138 |
| `v05a_clippy_da_parse` | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 0 | 772 | 0 | 72 |
| `v05b_clippy_da_cli` | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 0 | 889 | 0 | 15 091 |
| `v05c_clippy_probe` | `cargo clippy -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe --no-deps -- -D warnings` | 0 | 727 | 0 | 15 091 |
| `v05d_clippy_probe_tests` | same, `--tests` | 0 | 892 | 0 | 15 091 |
| `v06_probe` | `cargo run -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe` | 0 | 50 777 | 1 857 | 15 062 |
| **`v07_fmt_check`** | `cargo fmt -p … -- --check` (LAST Rust action) | **0** | 7 565 | 0 | 0 |

`RUN.identity.after_fmt.txt` records the premise of that replay before running it:
the capture's own `08_fmt_check` was rc 0, i.e. there was **no** formatting drift
to fix, so clippy genuinely ran on the formatted final bytes. If the premise had
failed, the script exits 92 without running. The replay's artifact is byte-equal
to the capture's (`SAME artifact-after-fmt`, `478054d5…`).

## Hash bindings

`hash_compare.txt` reads `SAME source`, `SAME evidence`, `SAME input`,
`CHANGED artifact`: the frozen run changed no source, no evidence document and no
input, and changed only its own diagnostic artifact (that is the correction). The
full before → after source table, including `architecture_contract.rs`, is in the
addendum's "Own pin" section; the accepted
`native_body_character_scope_bound.json` / `native_body_order_lineage.json` /
`native_body_character_shadow.json` are byte-identical and were re-verified from
their preserved copies.

## Measured facts (unchanged — corrected report bytes)

| document | P1 equal | P1 proper | P1 bucket | P2 equal | P2 proper | P2 bucket | P2 absence | P2 refusal |
|---|---|---|---|---|---|---|---|---|
| hub-2507.19457 | 32 872 | 35 | 32 907 | 32 872 | 35 | 32 907 | none | none |
| rlm-2512.24601 | 15 320 | 2 | 15 322 | 12 895 | 494 | 13 389 | none | none |
| reca-2605.26525v1 | 21 144 | 27 | 21 171 | 14 777 | 1 229 | 16 006 | none | none |
| **corpus (3/3)** | **69 336** | **64** | **69 400** | **60 544** | **1 758** | **62 302** | — | `p2_refusals = 0` |

`accepted_bucket_reproduced = true` **including P2**: P1 `69 400`,
P2 `62 302`, `p2_population_complete = true`, `p2_refusals = 0`, every document
reproducing both accepted pins. These are the independently reviewed facts, so
the correction folds the same measurement more strictly rather than moving it.

## Preservation of the prior run and the original rustfmt rc 1

The previous run's records are preserved by both jobs of
`preserve_before.sh`: copied by name (`RUN.identity.txt`,
`RUN.identity.after_fix.txt`, `steps.tsv`, `steps.after_fix.tsv`,
`07_fmt_check.{argv,meta,rc,stdout,stderr}`, `capture.sh`, `preserve_before.sh`,
`verify_after_fix.sh`, `hash_compare.txt`, `live_sources_vs_accepted.txt`, the
prior evidence document and the prior artifact) and covered by the
`preserved_run_dirs.sha256` manifest of all five pre-existing run directories
(409 files). The original `07_fmt_check.rc` value of **1** and every failed-run
document are therefore frozen as they were; this repair did not overwrite, delete
or re-run them, and writes its own records under a separate run directory.

## Locks and scope (unchanged)

`claim_scope = diagnostic_only`, `product_coverage_effect = none`,
`promotion_state = candidate`, `auto_promote = forbidden`,
`graph_mutation = forbidden`, `import_eligible = false`, `thresholds =
not_accepted`. No graph write, import, download, promotion, engine, alignment,
segmentation, regional or primary-lane change. Java remains authority and
fallback. D498 is untouched.

## T01 status

**Not complete.** Every P1 verdict and every word/heading verdict is still
`not_assessable`; `d498_reopen_required` is `false` on every document. This repair
fixed a report contract — it did not resolve the body-gap question, and it does
not claim to. No canonical blocker, waiver, lifecycle, recovery or engine
decision was changed, and no recovery authorization is requested or implied.
