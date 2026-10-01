# S05/T01 native body-character repair — run evidence (second bounded repair + final cleanup)

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`
Findings repaired: `S05-T01-COUNTS-DO-NOT-PROVE-PRESERVATION` (first repair),
`S05-T01-BOUNDARY-LOCATOR-NOT-GROUNDED` (second repair; independent review:
arbitrary prefix/suffix matches, a boundary bool excusing unlimited residual
occurrences, and no order proof), and the final cleanup findings
`S05-T01-RUNG-NOT-OBEYED`, `S05-T01-P1-PLANE-VERDICT-MISSING`,
`S05-T01-LOCATOR-CLAIM-OVERSTATED` and `S05-T01-CAPTURE-SCRIPT-NOT-SAVED`.
Producing run: `bash artifacts/parse_compat/s05_t01_repair2_cleanup_run/capture.sh`
(detached, sequential, no execution timeout).
Report: `artifacts/parse_compat/native_body_character_scope_bound.json` (schema `native-body-character-scope-bound.v3`)
Durable run directory: `artifacts/parse_compat/s05_t01_repair2_cleanup_run/`
(capture script, per-step `*.argv` / `*.meta` / `*.rc` / `*.stdout` / `*.stderr`,
`steps.tsv`, `logs.manifest.tsv`, `hashes.*.txt`, `hash_compare.txt`, `RUN.identity.txt`)

Every number in this file is a byte-level fact of the capture run below, not a
restatement of an earlier document. Where the first repair's evidence recorded a
stale SUT hash, stale `ContentLoss` counts and log sizes that did not match the
produced logs, this file records what the bytes now are.

> **Superseded in part (2026-09-30).** A later bounded P1 repair
> (`artifacts/parse_compat/s05_t01_p1_rung_checked_run/EVIDENCE.md`) added the
> checked P1 entry point `localize_body_plane_checked`, which refuses a
> plane/oracle pair tokenized at different rungs, moved the detached probe onto
> that checked API, and corrected the unsupported "whole-token order mismatch"
> cause previously asserted in the remaining-blocker section below. The measured
> P1 numbers and the body-gap blocker are unchanged.

## What the final cleanup changed

Focus: make the code obey what it declares, make the plane verdict explicit, and
correct claims that were stronger than the algorithm. No new engine, dependency,
graph, import, download, promotion, splice or Java activation; routing and locks
are unchanged.

1. **Tokenization now obeys the declared rung; case policy is explicit.**
   `native_body_text.rs` previously hardcoded zero-width/ligature
   normalization plus lowercasing inside `tokenize_fragment`, while each
   `BodyScope` declared a *normalization rung* that nothing applied to
   tokenization. Now:
   - `BodyCasePolicy { CaseSensitive, CaseFolded }` is a new declared type, and
     `BodyUnitRung { normalization, case }` bundles the ladder rung with the case
     policy;
   - `BodyUnitInventory` stores the rung it was built at (`from_text` /
     `from_fragments` / `empty` all take it; `add_fragment` uses it) and exposes
     `rung()`;
   - `BodyScope` carries `case` beside `normalization` and exposes `rung()`;
   - `classify_body_coverage` refuses an inventory whose rung is not the rung its
     scope declares (`NativeBodyTextError::InventoryRungMismatch`), and treats a
     native/oracle rung difference as `NormalizationMismatch`.
   The new tests assert that the same bytes tokenize differently at `Raw` and at
   `ZeroWidthAndLigatureNormalized` (a `BodyCoverageVerdict` difference, not just
   an assessability refusal), and that the case policy — not an implicit
   lowercase — decides an `alpha`/`Alpha` comparison.
2. **P1 plane evidence now carries a typed verdict, reason and D498 trigger.**
   The plane localization was a bare count struct. `BodyPlaneVerdict` plus
   `classify_body_plane` now derive `{verdict, reason, d498_reopen_required}` for
   both the per-document row and the aggregate. A zero `absent_from_plane` count
   is **never** reported as preservation: a plane with no absent material but
   unlocalized occurrences is `NotAssessable` with
   `P1LocalizationUncertain`, a fully located plane is the only
   `GranularityOnly`, and a proven material deficit stays `ContentLoss` with
   `d498_reopen_required = true`. `plane_verdict_for_population` additionally
   withholds a clean plane reading when the declared population is incomplete
   (`PopulationIncomplete`).
3. **The locator claim was corrected from "optimum" to a two-phase greedy
   decomposition.** The exact-anchor phase is optimal *for the exact phase alone*;
   the gap tiler is optimal *inside a gap given the anchors already fixed*. The
   combination is **not** a joint exact-plus-tile optimum, and the module, the
   probe and this file now say so. The counterexample is named and tested: oracle
   `x y` against side `x xy` anchors `x` exactly and then cannot resolve `y`,
   although tiling the oracle run `x y` onto the side token `xy` would resolve
   both; the conservative outcome is `NotAssessable` (uncertainty), never a
   resolution obtained by undoing an anchor. Substring *containment* (`the`
   contains `he`) is likewise documented as a weaker fact behind the uncertain
   classes, never as resolution; only a whole-token-boundary tile resolves.
4. **The acting capture script and per-step metadata are now durable.** The prior
   run directory (`s05_t01_repair2_run/`) holds logs but no script and no timing,
   and the `/tmp/s05_t01_repair2_runbook.sh` found on disk writes to
   `artifacts/parse_compat/logs`, which does not exist — so it was not the script
   that produced those logs. `capture.sh` was written into this run directory
   *before* the run, hashed at run time, and records per-step argv, start/end UTC
   and epoch-ns, elapsed ms, original exit code, and final stdout/stderr sizes;
   sources and inputs are hashed before and after. The `/tmp` script itself is
   preserved byte-identical at `prior_runbook_repair2.tmp.sh` and is **not**
   re-run or reconstructed.

## What the second repair changed (claims corrected where they were too strong)

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

The second repair replaced the locator with an **ordered, occurrence-conserving,
token-boundary-grounded** alignment:

- **Exact occurrences** are the optimal earliest-match monotone subsequence of
  the side token stream (binary search over per-token position lists). Every side
  occurrence is consumed at most once, and the mapping is order-preserving by
  construction. This optimality is **within the exact phase**; it is not a claim
  about the final result (see cleanup item 3).
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
  - `uncertain_present_in_token`: the characters sit as a substring inside one
    token (`he` in `the`);
  - `uncertain_spanning_tokens`: the characters occur in the side's concatenated
    token run;
  - `uncertain_character_material`: the side's character *material* still carries
    every ASCII character the occurrence needs (`BODY_ABSENCE_POLICY`).
- **`ContentLoss` requires a deficit in ASCII character material**, i.e. the
  side's character bag is short of an ASCII character the occurrence needs. This
  is the change that stops encoding differences from being reported as text loss
  at the declared rung.

Two further defects found and fixed while measuring:

- the P1 plane evidence was produced by passing an **empty body** into
  `align_body_content`, which fabricated P2 fields (`in_native_body = 0`,
  `in_plane_not_in_body = total`). P1 now uses the plane-only type
  `BodyPlaneLocalization`, which has no body/P2 field at all;
- the word band joined all word tokens into **one** string and called
  `from_joined_text`, inventing a single whole-document fragment. It now passes
  **one fragment per word token** to `NativeBodyText::from_fragments`, so a tile
  cannot be invented across a boundary the caller declared.

## Preservation of prior and historical artifacts (all immutable)

| artifact | sha256 | state |
|---|---|---|
| `artifacts/parse_compat/native_body_character_shadow.json` | `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361` | historical diagnostic, before == after |
| `artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md` | `a00a761d2716a21c0a83b71ed92146eb79d1278cdfba925c503aedba02831982` | first-repair evidence, before == after |
| `artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json` | `ce87e3af6fc9f332520df77463001746aecd76fc4a82d949b1974b28d2ae9d3f` | first-repair artifact, before == after |
| `artifacts/parse_compat/native_body_character_scope_bound.repair2_artifact.json` | `1ad2b56f653f0ea22784f9a3c975556ee7771b0f0b365ec169f7914624661638` | second-repair artifact, preserved before any edit; these are the pre-cleanup live bytes (the final run's `hashes.artifact.before.txt` is already the v3 report) |
| `artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md` | `c79b518c1fdca05e09f345e50569435fbef7a6852c4ce379ed4abeb406883af9` | second-repair evidence, preserved before this rewrite |
| `artifacts/parse_compat/s05_t01_repair2_cleanup_run/prior_runbook_repair2.tmp.sh` | `74eb48d9c108795414ceedf3835dfda8d6d81b34a49a3804428a1b941a2e4e0a` | the `/tmp` script found on disk, byte-identical (`/tmp` source hash equal) |
| `artifacts/parse_compat/s05_t01_repair2_cleanup_run/source_snapshot_before_cleanup.sha256` | `04354f28280e844b4df9373cdabf2b3ee9dec2c5cb6492399ad9521caac71701` | pre-edit source snapshot index (the repair2 source hashes) |

`native_body_character_scope_bound.json` is the run's own output. This cleanup
rewrites it from `1ad2b56f…` (repair2, schema v2) to `027d8c93…` (schema v3); the
prior bytes survive at the `repair2_artifact.json` path above, and the captured
run shows the v3 report reproducing byte-identically across runs.

## Run (exact metadata; original exit codes preserved; no execution timeout)

Capture script: `artifacts/parse_compat/s05_t01_repair2_cleanup_run/capture.sh`,
sha256 `2d9dca02f28d8ca23c90e0c0acb10036e0c1482e12a4ced04fdf128d072aaef4`
(identical before the run, at run time in `RUN.identity.txt`, and after the run).
Run window (UTC): `2026-09-30T21:09:52.974918139Z` → `2026-09-30T21:13:45.970415001Z`,
elapsed `232995 ms` (host local `+07`: `2026-10-01 04:09:52` → `04:13:45`).
`git_head = a6facbef77a7a53574883dd451270fa87d2891e0` (`feat/da-parse`), `cargo 1.98.1`, `rustc 1.98.1`.

| step | command | start UTC | end UTC | elapsed ms | rc | stdout bytes | stderr bytes |
|---|---|---|---|---|---|---|---|
| 01_oracle | `cargo test -p da-parse --test native_body_character_oracle` | 21:09:53.174384944 | 21:10:09.370231858 | 16195 | 0 | 2490 | 258 |
| 02_architecture_contract | `cargo test -p da-parse --test architecture_contract` | 21:10:09.404169583 | 21:10:12.576736272 | 3173 | 0 | 7524 | 243 |
| 03a_clippy_da_parse | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 21:10:12.608884470 | 21:10:29.259124089 | 16650 | 0 | 0 | 140 |
| 03b_clippy_da_cli | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 21:10:29.286884880 | 21:10:53.233488755 | 23946 | 0 | 0 | 15295 |
| 03c_clippy_probe | `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` | 21:10:53.262087569 | 21:11:14.581807430 | 21317 | 0 | 0 | 15295 |
| 04_probe | `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` | 21:11:14.640278088 | 21:13:39.189314133 | 144548 | 0 | 1337 | 15258 |
| 05_fmt_check | `cargo fmt -p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline -p da-domain -p da-ports -p da-application -p da-adapters -p da-parse -p da-cli -p da-mcp -- --check` | 21:13:39.287133922 | 21:13:45.634792363 | 6349 | 0 | 0 | 0 |

Per-step argv, timestamps and sizes are in `steps.tsv` and `NN_*.meta`; the exact
byte count and sha256 of every captured log (`*.stdout`, `*.stderr`, `*.rc`) are
in `logs.manifest.tsv`, recomputed from the bytes after the final step. The 15 KB
stderr on the clippy/probe steps is vendored `samyama-graph` warnings outside this
repair: every warning in those captures points into
`/root/vendor-source/samyama-graph`, and a search of each step's stderr for a
warning naming `da-parse`, `da-cli` or `da-adapters` returns **0** matches. The two
clippy captures are both 15 295 bytes (their sha256 differ only in cargo progress
lines); the probe capture is 15 258 bytes.

## Hash bindings (sources and inputs SAME before/after, from the run bytes)

`hash_compare.txt`: `SAME source`, `SAME input`, `SAME artifact`. Sources and
inputs are frozen, and the report is byte-reproducible: the artifact was
`027d8c93…` before this run and the run reproduced the same bytes (an earlier
independent execution over the same sources produced `027d8c93…` as well). The
second-repair bytes (`1ad2b56f…`) predate this cleanup and survive at
`native_body_character_scope_bound.repair2_artifact.json`.

Source under repair (after == before in `hashes.source.*.txt`):

| path | sha256 (this run) | sha256 (pre-cleanup snapshot) |
|---|---|---|
| `crates/da-parse/src/native_body_text.rs` | `37c746c1b2dc71a60fca14cc0217d8232ecd570ac5c8602892a140711b13a19f` | `7ecf1f49c8c180f7150be851885830911f4b63e4be4ab79610d8fad6a9a6b260` |
| `crates/da-parse/src/lib.rs` | `227fb866a395d848be5ab54d053989c00fd0808273b5a03dd244f6fe854f3a11` | `2348eb42c96f76ea4272360d54e3b58ecc5fb135f77cbaadc6cea1fe1008662d` |
| `crates/da-parse/tests/native_body_character_oracle.rs` | `d8fc4be72c83fff866beeddf839d222f12b2e9f8812aa62dcef67b4b19c989b0` | `1d470ce51d3fa9222020e264560ed8a35f99c1e07fbc2ec27fe6c76fb04e2875` |
| `crates/da-parse/tests/architecture_contract.rs` | `a0bd3864d5bf0aa7ad5da5aa10d52a62cf60c6fa8746861ba5acd94f22e0910c` | `a0bd3864d5bf0aa7ad5da5aa10d52a62cf60c6fa8746861ba5acd94f22e0910c` |
| `crates/da-cli/examples/native_body_character_probe.rs` | `967f0300906b4c45510a91a0476023167f3605ac512949b99515ff218b9ca937` | `096ff5c7a514c0ac25d40534e3475f9da57d0225081754b75fd278f7be14413f` |
| `crates/da-cli/Cargo.toml` | `6e9e213e7e1e6b13d65f7f3a9a69d48b346c20be0677e5bdb37895e42bca5eb8` | `6e9e213e7e1e6b13d65f7f3a9a69d48b346c20be0677e5bdb37895e42bca5eb8` |

The pre-cleanup snapshot is byte-identical to the repair2 source table (the first
repair's evidence recorded a different, stale hash); the snapshot files and their
index are in `source_snapshot_before_cleanup/`.

Inputs (before == after):

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
| `native_body_character_shadow.json` | `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361` |
| `…scope_bound.prior_repair.evidence.md` | `a00a761d2716a21c0a83b71ed92146eb79d1278cdfba925c503aedba02831982` |
| `…scope_bound.prior_repair.json` | `ce87e3af6fc9f332520df77463001746aecd76fc4a82d949b1974b28d2ae9d3f` |
| `…scope_bound.repair2_artifact.json` | `1ad2b56f653f0ea22784f9a3c975556ee7771b0f0b365ec169f7914624661638` |
| `…cleanup_run/source_snapshot_before_cleanup.sha256` | `04354f28280e844b4df9373cdabf2b3ee9dec2c5cb6492399ad9521caac71701` |

The probe re-verifies the TEI and PDF bytes against their pins from the bytes and
would exit non-zero on any drift. The report is deterministic: the capture run
reproduced the same artifact bytes (`027d8c93…`) that an earlier independent
execution over the same sources produced.

Final artifact: `artifacts/parse_compat/native_body_character_scope_bound.json`
= `027d8c934e9009001b968ecb95d13eef586d3740a83a2884d21924a49438f27a`
(schema `native-body-character-scope-bound.v3`; native surface
`da-parse.native-body-text/v5`). Both scopes now serialize an explicit
`"case": "case_folded"`, and the P1 rows/aggregate serialize a typed verdict.

## Measured result (P1 plane, now with typed verdict/reason/D498)

| document | oracle units | exact | boundary tile | uncertain (in-token / spanning / material) | absent | verdict | d498 |
|---|---|---|---|---|---|---|---|
| hub-2507.19457 | 33 094 | 117 | 0 | 32 977 (32 907 / 63 / 7) | **0** | `not_assessable` | false |
| rlm-2512.24601 | 15 713 | 383 | 0 | 15 330 (15 322 / 6 / 2) | **0** | `not_assessable` | false |
| reca-2605.26525v1 | 21 516 | 314 | 0 | 21 202 (21 171 / 23 / 8) | **0** | `not_assessable` | false |
| corpus (P1, population 3/3) | 70 323 | 814 | 0 | **69 509** (69 400 / 92 / 17) | **0** | `not_assessable` | false |

The 69 509 uncertain occurrences are exactly the case the brief named: 0 absent,
so a count-based rule could call the plane preserved. The typed verdict refuses
that reading — `NotAssessable` with `P1LocalizationUncertain { present_in_token_units: 69400, spanning_token_units: 92, character_material_units: 17 }`.
The 17 material-uncertain occurrences are the encoding/math class:
`balunović` (diacritic), `ĉ`, `ỳ` (non-ASCII), `grpofoot`, `tutorialfoot`. Under
the corrected policy they are **not** loss.

Bands (from the same report):

| band | documents | population | verdict | d498 |
|---|---|---|---|---|
| S03 parent span | 1 contributing (rlm), 2 typed `NoBodyTokens` (hub, reca, named absent) | incomplete (3 declared) | `not_assessable` | false |
| word/heading | 3 contributing (body exact 117 / 349 / 284) | complete | `not_assessable` | false |
| P1 plane | 3 measured | complete | `not_assessable` | false |

`absent_from_plane = 0` for every document and for the aggregate, and
`d498_reopen_required = false` everywhere; `thresholds = not_accepted`.

## Remaining blocker (honest, unchanged in substance)

**T01's body-gap question is NOT resolved.** Every band is `NotAssessable`; the
P1 plane has zero proven absence and 69 509 occurrences that are present as
characters but not localized at a token boundary. Nothing below is a clearance.

1. **No matched-scope P1 character loss is demonstrated.** The first repair
   reported 124 "unresolved" units as `ContentLoss`; with
   occurrence-conserving, material-aware localization the plane count is **0
   absent**, with 17 material and 92 spanning / 69 400 in-token occurrences that
   are present as characters but not localized. `GranularityOnly` is correctly
   withheld.
2. **Two of three documents still yield no S03 body band.** hub and reca are
   typed `NoBodyTokens` in the parent-span band (named in `aggregate.absent`),
   so the S03 population stays `population_complete = false`. **P2
   `NoBodyTokens` for HUB/RECA is retained, not cleared.**
3. **The in-token bucket is a combined uncertainty, not a proven cause.** The
   69 400 `uncertain_present_in_token` occurrences are those whose characters sit
   inside one side token; the bucket merges occurrences equal to a side token with
   occurrences that are only a proper substring of one, and the counters do not
   separate the two. No order-based cause is asserted: the measurement proves
   presence-of-characters and absence-of-material, not token-boundary coverage,
   and either sub-class would need its own counter to be attributed.
4. **Regional assignment was deliberately not touched.** This cleanup changes no
   region-assignment logic; the P2 population gap is out of its scope.

Because every band is `NotAssessable` and `absent_from_plane = 0`,
`d498_reopen_required` is `false` throughout — and that is a measurement result,
not a clearance. **No canonical blocker, waiver, lifecycle, recovery or engine
decision was changed.** D498 stays untouched and reopens only if a future run
demonstrates real P1 loss. Locks unchanged: `import_eligible = false`,
`auto_promote = forbidden`, `graph_mutation = forbidden`,
`product_coverage_effect = none`, `claim_scope = diagnostic_only`,
`promotion_state = candidate`, `thresholds = not_accepted`. No engine was
adopted, no threshold was introduced, and no sidecar output was spliced into the
parse route.

## Verification performed (all from this capture run)

- `cargo test -p da-parse --test native_body_character_oracle` → **37 passed**, rc 0
  (27 from the second repair + 10 cleanup tests: rung obedience, case policy,
  case/rung mismatch refusals, plane verdicts, plane population, greedy-anchor
  counterexample)
- `cargo test -p da-parse --test architecture_contract` → **100 passed**, rc 0
- `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` → rc 0
- `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` → rc 0, report written
- `cargo fmt` (repository pre-commit hook scope) `-- --check` → rc 0, no drift; this is the last Rust action of the run

---
