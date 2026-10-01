# S05/T01 uncertainty-split diagnostic — run evidence

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`

One bounded, read-only, **additive** diagnostic. It splits the accepted
`uncertain_present_in_token` bucket into the two facts it merges, conserves the
accepted bucket exactly, and **does not close T01**. It is a fact count, not a
verdict, a quality proof, a threshold, or an API repair.

Producing run: `bash artifacts/parse_compat/s05_t01_uncertainty_split_run/capture.sh`
(detached, sequential, one cargo process at a time, no execution timeout).
Preservation step (run before any edit):
`bash artifacts/parse_compat/s05_t01_uncertainty_split_run/preserve_before.sh`
→ `preserved_before.sha256` (22 files) + byte copies under `preserved_before/`.
Pre-frozen validation (argv/timing captured before the frozen run):
`pre_frozen_validation.*` → rc 0, 125 380 ms.
Post-format-fix verification (see the honest failure below):
`bash artifacts/parse_compat/s05_t01_uncertainty_split_run/verify_after_fix.sh`
→ `steps.after_fix.tsv`.

Report artifact: `artifacts/parse_compat/native_body_uncertainty_split.json`
= `e4dc96e4d4b0250213c40ec8d6eb4e798e0546c66a4d7bf6fe668b4488a39039`
(schema `native-body-uncertainty-split.v1`). The probe was run twice inside the
capture (`05_probe`, `05b_probe_replay`) and both runs produced that same
sha256 (`SAME artifact-replay`), so the artifact is byte-deterministic. It was
re-run once more after the format fix and was byte-identical again
(`SAME artifact-after-fmt-fix`).

## What was asked of this diagnostic

Split the accepted `uncertain_present_in_token` facts into

* **equal** — the occurrence equals one side token exactly but the ordered
  alignment did not bind it (present, unbound: an anchoring fact); and
* **proper substring** — the occurrence is only a proper substring of one side
  token (present, no boundary: a tokenization fact);

with **equality tested first**, and with the **conservation invariant**
`equal + proper == the previous bucket`. No claim that “present” means
“preserved”, and no arbitrary matching threshold.

## Measured fact counts (frozen artifact bytes)

P1 = the whole native plane; P2 = the repaired word/heading body band. Both are
compared against the same independent preserved-TEI character oracle
(`preserved-java-tei-lexical-scanner`, `java-grobid-crf`, sha256-verified from
bytes) at the declared rung.

| document | P1 equal | P1 proper | P1 bucket | P2 equal | P2 proper | P2 bucket | conservation |
|---|---|---|---|---|---|---|---|
| hub-2507.19457 | 32 872 | 35 | 32 907 | 32 872 | 35 | 32 907 | both true |
| rlm-2512.24601 | 15 320 | 2 | 15 322 | 12 895 | 494 | 13 389 | both true |
| reca-2605.26525v1 | 21 144 | 27 | 21 171 | 14 777 | 1 229 | 16 006 | both true |
| **corpus (3/3)** | **69 336** | **64** | **69 400** | **60 544** | **1 758** | **62 302** | **both true** |

`accepted_bucket_reproduced = true`: the P1 bucket total 69 400 and the P2 word
bucket total 62 302 match the frozen accepted artifacts exactly, so the split
refines the accepted measurement rather than replacing it. Every document
reproduces its accepted per-document bucket.

**The answer to the task's question:** at the plane, the un-split bucket was
99.91 % equal-token (69 336 of 69 400) and 0.09 % proper-substring (64). At the
word body the proper-substring share is larger (1 758 of 62 302, 2.82 %) because
the body band holds fewer, more separated tokens. These are the two facts the
bucket merged; neither is attributed to a cause here.

## What this run does NOT establish

1. **No cause, no quality proof.** The counts say how many unbound occurrences
   equal a side token versus sit inside one. They do not decide whether the
   native lane preserves the body, and a nonzero equal count is not a clearance.
   Every P1/P2 verdict stays `NotAssessable`; `d498_reopen_required` is `false`
   on every document (no proven material deficit), which is a measurement result,
   not a clearance.
2. **No threshold and no ratio.** The counters are integers, serialized with no
   verdict/ratio/threshold field; the contract test asserts those keys cannot be
   read out. `thresholds = not_accepted`.
3. **T01 is not complete.** The body-gap question is exactly as open as the
   accepted P1 run left it. This run adds one finer fact; it does not resolve it.
4. **The order question is untouched.** No oracle-derived order source, no
   XY-cut promotion, no primary-lane switch, no engine or segmentation change.
   The XY-cut lattice keeps its typed `segmenter_rejected_lattice` standing.

## Narrow correction to the order evidence (addendum, not an edit)

`artifacts/parse_compat/native_body_order_lineage.addendum.md` adds a bounded
correction beside the historical order-lineage evidence and JSON. The historical
documents are **not** edited or erased; both are preserved byte-for-byte under
`preserved_before/` (sha256 `891422ed…` and `3b08509c…`) and re-verified by
`06_preserved_copies_check` (rc 0). The narrow claim: **the tested reorderings
did not improve the exact count (814 → 814 → 791); all reading order is not
refuted.**

## API change (additive, not a repair)

* `crates/da-parse/src/native_body_text.rs`: the private `SideLocalization`
  stores `uncertain_equal_in_token` and `uncertain_proper_substring_in_token`;
  `uncertain_present_in_token` is now the **derived sum**, so conservation holds
  by construction. `classify_unresolved` tests equality first. New public
  additive API: `BodyUncertaintySplit`, `split_plane_uncertainty`,
  `split_body_uncertainty` (rung-checked, same refusal as the localization paths).
* Every existing public struct and field is **unchanged**; the accepted
  `native_body_character_scope_bound.json` schema is not edited. The split is
  carried by a **new dedicated example**
  `crates/da-cli/examples/native_body_uncertainty_split_probe.rs` into a
  **separate** P1/P2 diagnostic report.
* BLOCKER check before editing: GitNexus returned `risk: UNKNOWN` for
  `align_body_content` / `ordered_side_localization` (the new file is not in the
  index), so the blast radius was confirmed by bounded text search instead of
  trusting the empty result: callers are `classify_body_coverage` (same module),
  the `lib.rs` re-export, the two da-parse test files and the new example. No
  production route consumes the changed symbols.

## Contract tests (`native_body_uncertainty_split_contract.rs`, 11 passed)

Equality case; proper-substring case; **ambiguity + precedence** (`ab` equal to
`ab` and a proper substring of `abc` → equal); repeated occurrences counted once
each; occurrence in no token → neither sub-class; **conservation** against the
accepted bucket on a mixed plane; **false-claim** refusal (nonzero equal ⇒ still
`NotAssessable`, no verdict/ratio/threshold key); uncertainty is global and not a
proof; rung-mismatch refusal on both entry points; empty oracle → zero split, not
a pass; body entry point conserves the P2 bucket.

## Verification performed (all recorded in the run directory)

| step | command | rc | elapsed ms |
|---|---|---|---|
| 01_uncertainty_split_contract | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 0 | 608 |
| 02_native_body_character_oracle | `cargo test -p da-parse --test native_body_character_oracle` | 0 | 2 280 |
| 03_architecture_contract | `cargo test -p da-parse --test architecture_contract` | 0 | 1 000 |
| 04a_clippy_da_parse | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 0 | 27 407 |
| 04b_clippy_da_cli | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 0 | 52 747 |
| 04c_clippy_probe | `cargo clippy -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe --no-deps -- -D warnings` | 0 | 803 |
| 05_probe | `cargo run -p da-cli --features pdf-layout --example native_body_uncertainty_split_probe` | 0 | 50 142 |
| 05b_probe_replay | same | 0 | 48 198 |
| 06_preserved_copies_check | `sha256sum -c preserved_before.sha256` | 0 | 20 |
| **07_fmt_check** | `cargo fmt -p … -- --check` | **1** | 6 747 |

Frozen capture window (UTC): `2026-09-30T23:09:12.815741420Z` →
`2026-09-30T23:12:24.198966226Z`, elapsed 191 381 ms. `git_head =
a6facbef77a7a53574883dd451270fa87d2891e0` (`feat/da-parse`), `cargo 1.98.1`,
`rustc 1.98.1`.

### The one failure, preserved and then fixed (not erased)

`07_fmt_check` returned **rc 1**: rustfmt drift in three files this task
added/edited (`native_body_uncertainty_split_probe.rs`,
`da-parse/src/lib.rs`, `native_body_uncertainty_split_contract.rs`). That
failure record is preserved byte-for-byte (`07_fmt_check.{argv,meta,rc,stdout,stderr}`)
and was **not** overwritten. The fix and its verification are recorded as
separate, differently-named steps:

| step | command | rc | elapsed ms |
|---|---|---|---|
| 08_fmt_check_after_fix | `cargo fmt -p … -- --check` (after `cargo fmt -p da-parse -p da-cli`) | 0 | 6 843 |
| 09_uncertainty_split_contract_after_fix | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 0 | 12 843 |
| 10_native_body_character_oracle_after_fix | `cargo test -p da-parse --test native_body_character_oracle` | 0 | 3 387 |
| 11_architecture_contract_after_fix | `cargo test -p da-parse --test architecture_contract` | 0 | 2 661 |
| 12_probe_after_fmt_fix | `cargo run … --example native_body_uncertainty_split_probe` | 0 | 71 674 |
| 13_fmt_check_final | `cargo fmt -p … -- --check` (LAST Rust action) | 0 | 6 519 |

The format fix touched only those three task files (verified by pre/post sha256
over every `.rs` file in `da-parse`/`da-cli`), and the artifact bytes after the
fix are identical to the frozen replay (`SAME artifact-after-fmt-fix`), so the
fix is semantically neutral for the measurement. `13_fmt_check_final` is the last
Rust action of the after-fix phase.

## Preservation (byte facts)

`preserved_before.sha256` records 22 files (prior artifacts, prior evidence,
accepted-run manifests, and the frozen/to-be-edited sources) and
`06_preserved_copies_check` re-verified every copy (rc 0). `hash_compare.txt`
reads `SAME source`, `SAME evidence`, `SAME input`, `CREATED artifact`,
`SAME artifact-replay` — the frozen run changed no source, evidence or input, and
created only the new separate artifact. `live_sources_vs_accepted.txt` names the
source drift honestly: `native_body_text.rs` and `Cargo.toml` changed versus both
the accepted manifest and the preserved copies (this task's additive edits);
`lib.rs` likewise; `architecture_contract.rs` differs from the accepted manifest
(an earlier order-lineage change) but matches its preserved copy exactly.

## Locks and scope (unchanged)

`claim_scope = diagnostic_only`, `product_coverage_effect = none`,
`promotion_state = candidate`, `auto_promote = forbidden`,
`graph_mutation = forbidden`, `import_eligible = false`, `thresholds =
not_accepted`. No graph write, import, download, promotion, engine,
segmentation or primary-lane change. Java remains authority and fallback. The
S03/S04 byte-pinned artifacts and the accepted P1/P2 artifacts are untouched.

## Minimal next canonical step

The split shows the plane bucket is 99.91 % equal-token and 0.09 %
proper-substring, and the word-body bucket is 97.18 % / 2.82 %. That is the
attribution input the accepted P1 evidence named as future work, and it is now
measured — but **`NotAssessable` persists**, so the single minimal next step is:

1. **Attribute the equal-token sub-class on the plane, not the ratio.** The
   dominant plane sub-class is *equal but unbound* (69 336). The smallest next
   diagnostic that can separate a legitimate anchoring limit from a real content
   question is a bounded, read-only check of whether those equal-but-unbound
   occurrences are repeats already proven elsewhere in the same document (a
   multiplicity fact), so the residual — not the total — is what faces scrutiny.
   Do this without changing the primary lane, the oracle, or the accepted P1/P2
   schema; write a separate artifact, as this run did.
2. **Stop here until that is planned.** Do not re-derive the plane in “admitted
   order” (already refuted by bytes), do not re-open T02's reading-order question
   from this character oracle, and do not treat any split count as a verdict.
   T01 remains open.

## T01 status

**Not complete.** Every P1/P2 verdict stays `NotAssessable`; the body-gap
question is unchanged in substance. No canonical blocker, waiver, lifecycle,
recovery or engine decision was changed, and no recovery authorization is
requested or implied. D498 stays untouched.
