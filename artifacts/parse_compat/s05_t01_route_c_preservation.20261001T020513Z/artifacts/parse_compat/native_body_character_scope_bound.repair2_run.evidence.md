# S05/T01 native body-character repair — run evidence (second bounded repair)

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`
Findings repaired: `S05-T01-COUNTS-DO-NOT-PROVE-PRESERVATION` (first repair) and
`S05-T01-BOUNDARY-LOCATOR-NOT-GROUNDED` (independent review of the first repair:
arbitrary prefix/suffix matches, a boundary bool excusing unlimited residual
occurrences, and no order proof).
Producing run: detached `cargo run -p da-cli --features pdf-layout --example native_body_character_probe`
Report: `artifacts/parse_compat/native_body_character_scope_bound.json` (schema `native-body-character-scope-bound.v2`)
Durable logs: `artifacts/parse_compat/s05_t01_repair2_run/` (per-step `*.stdout`, `*.stderr`, `*.rc`, `hashes.*.txt`)

This file corrects the first-repair evidence, which recorded a stale SUT hash
(`e0c37d13…`), stale `ContentLoss` counts, and log sizes that did not match the
produced logs. Every number below is the actual final fact of the second repair.

## What the second repair changed

The first repair located an oracle unit three ways: exact, "boundary variant"
(an arbitrary `starts_with`/`ends_with` prefix or suffix of one larger token, or
*any* consecutive-token accumulator inside a fragment), or unresolved. That
locator had three structural defects:

1. **Arbitrary boundary matching.** `he` resolved against `the`, because `the`
   ends with `he`. A fragment could be excused by a partial token.
2. **Unbounded residual excuse.** The locator returned a `bool`, so one matching
   token excused *every* residual occurrence of that unit.
3. **No order proof.** There was no monotone, occurrence-conserving mapping
   between the two token streams.

The second repair replaces the locator with an **ordered, occurrence-conserving,
token-boundary-grounded** alignment:

- **Exact occurrences** are the optimal earliest-match monotone subsequence of
  the side token stream (binary search over per-token position lists). Every side
  occurrence is consumed at most once, and the mapping is order-preserving by
  construction.
- **Boundary tiles** are the only way a residual occurrence is resolved, and only
  as a *whole* run: a contiguous run of consecutive tokens **inside one fragment**
  on one side whose concatenation is exactly one token on the other side
  (`average` + `case` ↔ `averagecase`). A partial token can never resolve, and a
  tile consumes the whole run, so one side token can never excuse an unbounded
  number of residuals. Residual inter-anchor gaps are tiled by a bounded DP
  (whole-run tiles only); a gap beyond the bounded window is left pending
  (fail-closed).
- **Unresolved occurrences are localized by what the side still proves**, and
  they are `NotAssessable`, never loss and never granularity:
  - `uncertain_present_in_token`: the characters sit inside one token (`he` in `the`);
  - `uncertain_spanning_tokens`: the characters occur in the side's concatenated token run;
  - `uncertain_character_material`: the side's character *material* still carries
    every ASCII character the occurrence needs — this is where diacritic, script
    and math-symbol differences land, because the declared normalization rung does
    not fold them (`BODY_ABSENCE_POLICY`).
- **`ContentLoss` now requires a deficit in ASCII character material**, i.e. the
  side's character bag is short of an ASCII character the occurrence needs. This
  is the change that stops encoding differences from being reported as text loss.

Two further defects found and fixed while measuring:

- the P1 plane evidence was produced by passing an **empty body** into
  `align_body_content`, which fabricated P2 fields (`in_native_body = 0`,
  `in_plane_not_in_body = total`). P1 now uses the plane-only type
  `BodyPlaneLocalization`, which has no body/P2 field at all;
- the word band joined all word tokens into **one** string and called
  `from_joined_text`, inventing a single whole-document fragment. It now passes
  **one fragment per word token** to `NativeBodyText::from_fragments`, so a tile
  cannot be invented across a boundary the caller declared.

Negative controls added and passing: `he`/`the`; repeated oracle occurrences with a
single candidate; reordered content (including a 64-token reversal); equal-length
different text; a surplus masking a selective loss; Raw vs normalized scope rung;
a 2000-token ordered positive control; genuine boundaries refusing an invented
fragment; an ASCII deficit still proving loss; a non-ASCII/encoding difference
staying uncertainty; plane localization carrying no body field.

## Preservation of the first repair and the historical artifact

| artifact | sha256 | state |
|---|---|---|
| `artifacts/parse_compat/native_body_character_shadow.json` | `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361` | historical diagnostic, **immutable** (before == after) |
| `artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md` | `a00a761d2716a21c0a83b71ed92146eb79d1278cdfba925c503aedba02831982` | first-repair evidence, preserved byte-identical before this repair |
| `artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json` | `ce87e3af6fc9f332520df77463001746aecd76fc4a82d949b1974b28d2ae9d3f` | first-repair scope-bound artifact, preserved byte-identical (matches the hash the first-repair evidence recorded) |

The live `native_body_character_scope_bound.json` was necessarily rewritten by
this repair: `4db5e61e…` was an abandoned intermediate alignment (anchorless
greedy) and `1ad2b56f…` is the final artifact. The first-repair bytes survive at
the `prior_repair` path above.

## Run (original exit code preserved; no execution timeout)

| step | command | rc | stdout bytes | stderr bytes |
|---|---|---|---|---|
| 01_oracle | `cargo test -p da-parse --test native_body_character_oracle` | 0 | 1837 | 258 |
| 02_architecture_contract | `cargo test -p da-parse --test architecture_contract` | 0 | 7524 | 243 |
| 03a_clippy_da_parse | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 0 | 0 | 140 |
| 03b_clippy_da_cli | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 0 | 0 | 15295 |
| 03c_clippy_probe | `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` | 0 | 0 | 15295 |
| 04_probe | `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` | 0 | 1238 | 15258 |
| 05_fmt_check | `cargo fmt -p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline -p da-domain -p da-ports -p da-application -p da-adapters -p da-parse -p da-cli -p da-mcp -- --check` | 0 | 0 | 0 |

The 15 KB stderr on the clippy/probe steps is vendored `samyama` warnings
(`samyama`, `samyama-sdk`, `samyama-graph-algorithms`) outside this repair; a
search of every step's stderr for a warning naming `da-parse`, `da-cli` or
`da-adapters` returns nothing. The `cargo-fmt` step is the repository's own
pre-commit hook scope and is the last Rust action.

Run window (log mtimes, host local +0700): `2026-10-01 02:51:52` → `2026-10-01 02:56:18`
(`2026-09-30T19:51:52Z` → `2026-09-30T19:56:18Z`), 279.5 s wall.

## Hash bindings (before == after; captured before the producing run)

Source under repair:

| path | sha256 |
|---|---|
| `crates/da-parse/src/native_body_text.rs` | `7ecf1f49c8c180f7150be851885830911f4b63e4be4ab79610d8fad6a9a6b260` |
| `crates/da-parse/src/lib.rs` | `2348eb42c96f76ea4272360d54e3b58ecc5fb135f77cbaadc6cea1fe1008662d` |
| `crates/da-parse/tests/native_body_character_oracle.rs` | `1d470ce51d3fa9222020e264560ed8a35f99c1e07fbc2ec27fe6c76fb04e2875` |
| `crates/da-parse/tests/architecture_contract.rs` | `a0bd3864d5bf0aa7ad5da5aa10d52a62cf60c6fa8746861ba5acd94f22e0910c` |
| `crates/da-cli/examples/native_body_character_probe.rs` | `096ff5c7a514c0ac25d40534e3475f9da57d0225081754b75fd278f7be14413f` |
| `crates/da-cli/Cargo.toml` | `6e9e213e7e1e6b13d65f7f3a9a69d48b346c20be0677e5bdb37895e42bca5eb8` |

Inputs:

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

Every source and input row above is `SAME` before/after in
`artifacts/parse_compat/s05_t01_repair2_run/hashes.source.*.txt` and `hashes.input.*.txt`.
The probe re-verifies the TEI and PDF bytes against their pins from bytes and
would exit non-zero on any drift.

Final artifact: `artifacts/parse_compat/native_body_character_scope_bound.json`
= `1ad2b56f653f0ea22784f9a3c975556ee7771b0f0b365ec169f7914624661638`.

## Measured result (P1 plane, order-independent localization)

| document | oracle units | exact | boundary tile | uncertain (in-token / spanning / material) | absent |
|---|---|---|---|---|---|
| hub-2507.19457 | 33 094 | 117 | 0 | 32 977 (32 907 / 63 / 7) | **0** |
| rlm-2512.24601 | 15 713 | 383 | 0 | 15 330 (15 322 / 6 / 2) | **0** |
| reca-2605.26525v1 | 21 516 | 314 | 0 | 21 202 (21 171 / 23 / 8) | **0** |
| corpus (P1, population 3/3) | 70 323 | 814 | 0 | 69 509 (69 400 / 92 / 17) | **0** |

The 17 material-uncertain occurrences are exactly the encoding/math class the
brief named: `balunović` (diacritic), `ĉ`, `ỳ` (non-ASCII), `grpofoot`,
`tutorialfoot`. Under the corrected policy they are **not** loss.

Bands:

| band | documents | population | verdict |
|---|---|---|---|
| S03 parent span | 1 contributing, 2 typed `NoBodyTokens` (hub, reca, named absent) | incomplete (3 declared) | `not_assessable` |
| word/heading | 3 contributing (body exact 117 / 349 / 284) | complete | `not_assessable` |
| P1 plane | 3 measured | complete | `not_assessable`, `absent_from_plane = 0` |

## Remaining blocker (honest)

**No matched-scope P1 character loss is demonstrated.** The first repair reported
124 "unresolved" units as `ContentLoss`; with occurrence-conserving,
material-aware localization the count is **0 absent** at the plane, 17 material
occurrences (`balunović`, `ĉ`, `ỳ`, `grpofoot`, `tutorialfoot`) and 92 spanning /
69 400 in-token occurrences that are present as characters but not localized at a
token boundary.

Two distinct facts remain open and are reported as such, not resolved:

1. **The in-token bucket is a combined uncertainty, not a proven cause.** The
   69 400 `uncertain_present_in_token` occurrences are those whose characters sit
   inside one side token; the bucket merges occurrences equal to a side token with
   occurrences that are only a proper substring of one, and the counters do not
   separate the two. No order-based cause is asserted: the measurement therefore
   proves presence-of-characters and absence-of-material, not token-boundary
   coverage, and `GranularityOnly` is correctly withheld.
2. **Two of three documents still yield no S03 body band** (hub, reca are typed
   `NoBodyTokens` in the parent-span band) and each band reports its absent
   documents by name; the S03 population stays `population_complete = false`.

Because every band is `NotAssessable` and `absent_from_plane = 0`,
`d498_reopen_required` is `false` for every document. This is a measurement
result, not a clearance: **no canonical blocker, waiver, lifecycle, recovery or
engine decision was changed by this repair.** D498 stays untouched and reopens
only if a future run demonstrates real P1 loss. Locks unchanged:
`import_eligible = false`, `auto_promote = forbidden`, `graph_mutation = forbidden`,
`product_coverage_effect = none`, `thresholds = not_accepted`.

No engine was adopted, no threshold was introduced, and no sidecar output was
spliced into the parse route.

## Verification performed (all from this run)

- `cargo test -p da-parse --test native_body_character_oracle` → **27 passed** (rc 0)
- `cargo test -p da-parse --test architecture_contract` → **100 passed** (rc 0)
- `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` → rc 0
- `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` → rc 0, report written
- `cargo fmt` (repository pre-commit hook scope) `-- --check` → rc 0, no drift
