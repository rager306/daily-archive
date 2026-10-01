# Addendum (bounded candidate) — S05/T01 additive third-route Body candidacy

Milestone `M514-1lss2s` · Slice `S05` · Task `T01`.
Finding addressed: the P2 localisation of `NoBodyTokens` on HUB/RECA. The
read-only scout found exactly one existing third candidate,
`NativeSegmenter::segment` (`segment_document`, v1) in
`crates/da-parse/src/native_segmentation.rs`, whose `Body` entry predicate is the
textual `heading_depth` (a numeric prefix such as `1`/`1.2`, or an ALL-CAPS
line), **not** the v2 `segment_document_full` `full_marker` route and **not** the
heading/geometry prominence route.

This file **adds** a bounded candidate evaluation beside the preserved evidence
documents. It does **not** edit, replace or erase
`artifacts/parse_compat/native_body_additive_assignment.evidence.md`,
`artifacts/parse_compat/native_body_boundary_grounding.evidence.md` or
`artifacts/parse_compat/native_body_identity_intersection.addendum.md`; all are
preserved byte-for-byte.

No engine, dependency, download, graph write, import promotion or Java
activation was introduced; no threshold was accepted; routing and locks are
unchanged. No lifecycle tool was called, no resume was performed, no stage,
commit, reset or stash was run, and the canonical abort/recovery record
`1c74d263-b855-4b7c-b79d-1fe54b40daee` was not touched, resumed, settled or made
terminal. `T01` remains `NotAssessable` on the real corpus and is **not**
completed or waived by this addendum.

## 1. What was added (all of it additive)

| file | change |
|---|---|
| `crates/da-parse/src/native_body_text.rs` | `NATIVE_BODY_THIRD_ROUTE_SCHEMA_VERSION`, `BodyThirdRouteProvenance`, `BodyThirdRouteVerdict`, `NativeBodyThirdRouteCandidate`, `evaluate_body_third_route_candidate`, two typed error variants (`ThirdRouteInputMismatch`, `ThirdRouteUncorroborated`) |
| `crates/da-parse/src/lib.rs` | exports the five new names |
| `crates/da-parse/tests/native_body_character_oracle.rs` | one synthetic helper and **10** additive tests |
| `crates/da-parse/tests/architecture_contract.rs` | the S05/T01 purity surface must also name `pub fn evaluate_body_third_route_candidate` and `BodyThirdRouteVerdict` |
| `crates/da-cli/examples/native_body_character_probe.rs` | a separate `third_route_candidate` band and aggregate; report schema `native-body-character-scope-bound.v5` -> `.v6` |

`evaluate_body_third_route_candidate` reuses the **same engine** as
`build_native_body_assignment`: the heading run is the same
`project_native_words` -> `assign_heading_regions` ->
`build_native_word_region_carrier` chain. The third route is the caller-supplied
v1 `SegmentationOutcome`; its accepted input is `LayoutDocument` + `LexemeRow`
(the v1 `segment` signature does not even take the lattice) and it never reads
TEI. That is recorded in `provenance.accepted_input = "layout_document+lexeme_rows"`
and `provenance.reads_tei = false`.

The candidate adopts a bounded body only when **both** routes carry word-bearing
`Body` parent identities and share at least one. The adopted stream is the
identity intersection in the heading route's admitted reading order. A third
route that enters `Body` alone, only outside the word-bearing universe, or
disjointly from the heading route is a named refusal. A receipt that is not the
v1 wire format or names another document is refused before any comparison.

No new instrument and no new boundary rule were introduced; the v2 marker band
(`additive_assignment`) and every pre-existing row are untouched in shape and
value.

## 2. Preservation

Pre-edit copy:
`artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/`
(22 files, `MANIFEST.sha256` sha256
`2d7915aee7de6f434850cf25b3ca27c8885ea36a3af55804ec7ace75c57b690e`).

| file | before this change |
|---|---|
| `crates/da-parse/src/native_body_text.rs` | `29e25e1d17030a3baf064c396866eeddaa16ab133a9d244aed26f0390b39adec` |
| `crates/da-parse/src/lib.rs` | `c62cd8f29865eed679f436349e3098fa3eed1ddcc7b0112e3cb3948716c83285` |
| `crates/da-parse/tests/native_body_character_oracle.rs` | `1440787f5d73cbab4e70bb132f872c6b3b29b46e24f7e9f5eab83a7c70f855fc` |
| `crates/da-parse/tests/architecture_contract.rs` | `6e951a200926520198117eaffdab82d925f54b41116fb6106df8650120b09821` |
| `crates/da-cli/examples/native_body_character_probe.rs` | `fe61838528aab9fdd8a13fb0c3a3e48b29d0353606a7e0aa45d9ae374aa78300` |
| `artifacts/parse_compat/native_body_character_scope_bound.json` | `68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9` (v5) |

Frozen inputs re-hashed before and after the producing run (all SAME):
`data/parse/layout_bakeoff.yaml`
`530ce05297e881ac91b7404c608dec5aadd9ffd22b7bbbbdd68e064b534290b5`,
`crates/da-parse/tests/fixtures/preserved/provenance.yaml`
`028a5d62cc8412097ea03ee5e529bae67bf8f9d4fe7819116e6365ad2aca0ebf`, the three
preserved TEI (`98546123…` / `e6aa41f5…` / `dac18882…`), the three pinned PDFs
(`ab3a5139…` / `8567362c…` / `a5672497…`), and the read-only routeC source
`crates/da-parse/src/native_segmentation.rs`
`5a1f926b9ba655f060d95db368ff16ff2d06c37f5f98475d9e0e18e6b9a81b5a`. The
accepted v3 report `027d8c93…` is SAME before and after.

## 3. Producing run (exact metadata; original rc; no execution timeout)

New, separate, non-reused directory
`artifacts/parse_compat/s05_t01_route_c_run/`; capture script sha256
`37dcab99bc843561cbad64eee75ff10d9112f3ff370102b8a864f212047fbb9f` (identical at
start and after the run). Run window (UTC) `2026-10-01T02:22:33.040789172Z` ->
`2026-10-01T02:25:53.563324208Z` (~201 s). `git_head =
a6facbef77a7a53574883dd451270fa87d2891e0`, branch `feat/da-parse`,
`cargo 1.98.1`, `rustc 1.98.1`. Sequential cargo steps, detached, no execution
timeout; `cargo fmt --all -- --check` is the last action.

| step | command | elapsed ms | rc |
|---|---|---|---|
| 01 | `cargo test -p da-parse --test native_body_character_oracle` | 2255 | 0 |
| 02 | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 2574 | 0 |
| 03 | `cargo test -p da-parse --test architecture_contract` | 3413 | 0 |
| 04 | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 20992 | 0 |
| 05 | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 30851 | 0 |
| 06 | `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` | 23051 | 0 |
| 07 | `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` | 89091 | 0 |
| 08 | `cargo fmt --all -- --check` (LAST Rust action) | 27902 | 0 |

Test totals from the captured stdout: **62 passed** (oracle, 52 pre-existing +
10 additive), **11 passed** (uncertainty split), **101 passed** (architecture).
The 15 295 B stderr on the clippy and probe steps is vendored `samyama-graph`
warning noise outside this change; `-D warnings` returned rc 0. Every per-step
`*.argv` / `*.meta` / `*.rc` / `*.stdout` / `*.stderr` and `steps.tsv` is durable.

`hash_compare.txt`: `SAME source`, `SAME input`, **`CHANGED artifact`**
`68835c94…` -> `d88d7eed…` (the new v6 band). No failed sub-run needed a
separate directory.

A separate determinism replay
(`cargo run … --example native_body_character_probe -- --report /tmp/route_c_replay.json`)
produced byte-identical bytes: replay sha256 = capture sha256 =
`d88d7eed29e1bb33b21247061ac3aea44b2798a8c76c531a4627d1d8057e8605`.

Post-edit source hashes (SAME before/after the run):
`native_body_text.rs` `9dfbd44f…`, `lib.rs` `6103fc34…`,
`native_body_character_oracle.rs` `0750d562…`, `architecture_contract.rs`
`732e41e0…`, `native_body_character_probe.rs` `bfe3d033…`,
`native_body_uncertainty_split_contract.rs` `34eb794d…` (unchanged),
`da-cli/Cargo.toml` `58850622…` (unchanged).

## 4. Measured result (real three pinned PDFs)

Probe step 07, captured in `07_probe.stdout`, report v6 `d88d7eed…`:

| document | v1 third-route `Body` parents | heading `Body` parents | intersection | verdict | adopted bounds (parent ids) | adopted body word tokens |
|---|---|---|---|---|---|---|
| hub-2507.19457 | 10 688 (of 14 118 word-bearing) | 14 117 | 10 688 | `ThirdRouteSubsetOfHeading` | `(0,1)`–`(70,91)` | 31 313 |
| rlm-2512.24601 | 1 216 (of 8 330) | 1 219 | 1 216 | `ThirdRouteSubsetOfHeading` | `(0,4)`–`(9,4)` | 5 927 |
| reca-2605.26525v1 | 1 941 (of 5 430) | 1 971 | 1 941 | `ThirdRouteSubsetOfHeading` | `(0,31)`–`(9,50)` | 6 319 |

Third-route aggregate: 3 declared / 3 contributing / 0 absent. Every adopted
body's gap verdict is `NotAssessable` with reason
`p1_localization_uncertain`; the P1 plane is `NotAssessable` for all three
(`absent_from_plane = 0`), `d498_reopen_required = false`, and no D498 decision
is recorded.

**What this is.** HUB and RECA, which the primary additive band still refuses as
`SingleRouteUncorroborated`, now have a bounded candidate body (31 313 and 6 319
word tokens) grounded by a second route. RLM's candidate (5 927 tokens) is
consistent with its existing additive intersection. That is the bounded P2
repair candidate.

## 5. Shared rules/data — the independence is one-sided, and the corpus shows it

The scout's caution is confirmed by the source and by the result:

* the v1 entry predicate is `heading_depth` (`native_segmentation.rs:379`,
  `marker` at `:349`); the heading route enters through `line_is_prominent`
  (`native_heading_regions.rs:754`) or `heading_marker`
  (`native_heading_regions.rs:866`), which strips a leading enumeration. The
  **entry** predicates are distinct;
* the **exit** vocabulary is **shared**: v1 `marker` recognises
  references/annex at `native_segmentation.rs:350-357` from the same literal
  lists as `full_marker` (`native_segmentation.rs:877`), and the heading route
  resolves every marker through `full_marker`
  (`native_heading_regions.rs:869`). The two routes therefore cannot disagree
  about where the body *ends*; only where it *starts* is independent.

On all three documents the third-route word-bearing set is a **strict subset**
of the heading route's, so the verdict is `ThirdRouteSubsetOfHeading`, not
two-sided agreement. The corroboration is one-sided: the heading route confirms
that every adopted token is `Body` to it, but it does **not** independently
confirm the third route's entry boundary — any subset of the heading body would
pass. In particular, on HUB the third route's entry identity `(0,1)` is
identical to the heading route's entry identity `(0,1)`, so the entry point is
**not** independently confirmed there; only on RLM `(0,4)` vs `(0,1)` and RECA
`(0,31)` vs `(0,1)` does the third route enter later and thereby narrow the
adopted extent. The third route is therefore **not** treated as ground truth: it
is recorded with distinct provenance (`function = segment_document`,
`schema_version = da-parse.native-segmentation/v1`,
`accepted_input = layout_document+lexeme_rows`), its own band, and a typed
one-sided verdict.

## 6. Negatives covered (synthetic planes, all green in step 01)

* an ALL-CAPS banner (`PREPRINT`) makes v1 enter `Body` before the real heading:
  `HeadingSubsetOfThirdRoute`, and the banner is **not** in the adopted body;
* a bare numeric line (`2024`) is the same shape: the numeric false positive is
  not adopted;
* a repeated page-margin line is `PageFurniture` to v1 and is **never** adopted
  as body even though the heading route carries it
  (`ThirdRouteSubsetOfHeading`);
* equal extent endpoints over a different interior is **not** whole-run
  agreement (`HeadingSubsetOfThirdRoute`);
* a proper two-sided disagreement adopts the conservative intersection
  (`CorroboratedHeadingIntersection`), never either whole run;
* a permuted `source_ordinal` keeps identity-set corroboration and the heading
  route's reading order;
* the third route alone (`HeadingRouteAbsent`) and the heading route alone
  (`ThirdRouteAbsent`) are named refusals with no adopted body — single-route
  never passes;
* a foreign third-route receipt is refused with
  `ThirdRouteInputMismatch("document_id")` before any comparison.

The pre-existing 52 oracle tests, 11 uncertainty-split tests and 101
architecture tests stay green; the v2 `additive_assignment` band is unchanged.

## 7. What stays unresolved (no clearance)

1. **P1 is unresolved.** `absent_from_plane = 0` on all three documents, so there
   is no proven content loss and no D498 trigger; the body-gap question is not
   closed. The candidate's own gap is `NotAssessable` for the same reason.
2. **The corpus corroboration is one-sided.** All three verdicts are
   `ThirdRouteSubsetOfHeading`; the public corpus does not exercise a two-sided
   boundary agreement. HUB's entry boundary is not independently confirmed.
3. **The exit boundary is not independent** (shared `full_marker` vocabulary),
   so this band cannot be presented as two independent routes agreeing on the
   whole extent.
4. Locks unchanged: `claim_scope = diagnostic_only`,
   `product_coverage_effect = none`, `promotion_state = candidate`,
   `auto_promote = forbidden`, `graph_mutation = forbidden`,
   `import_eligible = false`; `thresholds = not_accepted`. This is a diagnostic
   candidate, not a general body claim and not a product coverage increase.
5. `T01` remains open/`NotAssessable`; no completion, waiver, recovery or
   lifecycle change is claimed here.

## 8. Residuals noted, not fixed

* `native_body_order_lineage_probe.rs` names
  `native_body_character_scope_bound.json` as a `superseded_artifact` and records
  its bytes; that separate previous-worker report was not regenerated, so its
  recorded hash for the character report is now the v5 value. This is a
  pre-existing untracked diagnostic outside this change.
* The added probe band reads the same gitignored PDFs as the existing bands; the
  report remains a self-declaration bounded by
  `claim_scope=diagnostic_only` / `thresholds=not_accepted`.
* **Handoff risk (pre-existing, not caused or repaired here).** The seven files
  named in section 1 were already present in the git **index** before this work
  began (residue of the aborted attempt). The staged blobs are older than the
  pre-change working tree: for example the index blob for
  `crates/da-parse/src/native_body_text.rs` is
  `49c286b0e3ea46c6e4735d55962a4b5ba1366308427608fb29c24c5e9785d992` (the
  559-line partial snapshot that does not contain
  `build_native_body_assignment`), while the working tree is `9dfbd44f…`. No
  `git add`, commit, reset or stash was performed here (staging is forbidden for
  this task), so the index still shadows the working tree. A commit that does
  not re-add these paths from the working tree would commit the stale partial
  snapshot and lose the verified T01 work; the engine must `git add` them.

## 9. Correction (documentation-only, dated 2026-10-01; sections 1-8 preserved byte-for-byte)

This section is appended. Nothing above is edited or erased. The pre-append bytes
are preserved in
`artifacts/parse_compat/s05_t01_third_route_addendum_preservation.20261001T025608Z/`
(`MANIFEST.sha256`; sha256
`37fb0fb3920007e631b1e640f1651d3b529d3833ba275bab76c1607c28bb28fb`, 14 732 B).
No source, report, capture, cargo, test, lifecycle, git or DB/WAL action was
taken by this correction. The cited source/report/capture hashes as they stood
before the append are recorded in that preservation directory's
`CITED-EVIDENCE.before.sha256`.

### 9.1 §8 order lineage — the recorded hash is the accepted v3 `027d8c93…`

§8 states that `native_body_order_lineage_probe.rs` "records its bytes" for
`native_body_character_scope_bound.json` and that its recorded hash is "now the
v5 value". That is incorrect. The value recorded in
`artifacts/parse_compat/native_body_order_lineage.json`
(sha256 `3b08509c6b3b2be84db1b7ab99e41c9c4bd1bf88a68b754c5c3624b22f302395`)
under `prior_artifacts` for that path is
`027d8c934e9009001b968ecb95d13eef586d3740a83a2884d21924a49438f27a` — the
**accepted v3** bytes, byte-identical to
`native_body_character_scope_bound.accepted.v3.json`. It is **not** the v5 value
`68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9`, and not the
frozen v6 value
`d88d7eed29e1bb33b21247061ac3aea44b2798a8c76c531a4627d1d8057e8605`.
`superseded_artifact` at `native_body_order_lineage_probe.rs:978` names the path.

### 9.2 Interim route-C probe (02:14:36Z → 02:16:03Z): `CorroboratedHeadingIntersection` on all three

A retained **interim** capture exists at `/tmp/route_c_feas/probe.stdout`
(sha256 `1d8c778a025caad08b0e5c47f32dafa3c31e176d00d7800a7d9f9a0ec3527fd7`,
2 572 B, mtime 2026-10-01T02:16:03Z) with `/tmp/route_c_feas/probe.stderr`
(sha256 `011845537748c4701e4fff4ad69166e523d8956de6f6a21280d12f2647d5fb6d`,
15 258 B, mtime 2026-10-01T02:14:36Z). The interim run reports
`verdict Some("CorroboratedHeadingIntersection")` for **all three** documents
(hub 10 688/14 117; rlm 1 216/1 219; reca 1 941/1 971), and its final line says
it wrote the character report to
`artifacts/parse_compat/native_body_character_scope_bound.json`.

The interim report **is not retained** and cannot be reconstructed: the retained
interim stdout carries verdicts only, not report bytes. Before the frozen run
began, that path held the v5 bytes: the frozen run's own pre-run capture
`artifacts/parse_compat/s05_t01_route_c_run/hashes.artifact.before.txt`
(sha256 `b4c6328030d31281ab09e03ab8d6588451473d99d5f6a5d3aa00247d9777e0ac`,
written at frozen-run start `2026-10-01T02:22:33.040789172Z` per
`RUN.identity.txt`) records `68835c94…` for it. The v5 preservation
`artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/MANIFEST.sha256`
line 5 records the same `68835c94…`. So the report was restored/replaced to the
v5 bytes between the interim capture and the frozen run. The **exact command,
time and actor of that restore/replacement are not recorded** in any artifact
inspected here, and none is asserted. Asserted only: an interim
all-`CorroboratedHeadingIntersection` run occurred; it wrote the report path; and
the report path did not hold the interim bytes at frozen-run start.

Byte-identical copies of both interim captures are preserved, without overwriting
the originals, at
`artifacts/parse_compat/s05_t01_route_c_interim_capture.20261001T025608Z/`
(`MANIFEST.sha256` records the original paths and hashes).

### 9.3 §5 exit-boundary claim is too strong: the shared part is references/annex only; Acknowledgement differs

§5 concludes "The two routes therefore cannot disagree about where the body
*ends*". The sharing is real but narrower than "the exit vocabulary". v1 `marker`
(`crates/da-parse/src/native_segmentation.rs:349-357`) shares exactly the
references and annex literal lists with `full_marker`
(`crates/da-parse/src/native_segmentation.rs:877-893`):
`["references","bibliography","参考文献"]` and
`["appendix","appendices","annex","supplementary material"]`. It shares nothing
else. `full_marker` also recognises `acknowledgement`/`acknowledgments` (and
`toc`, `funding`, `availability`, `page_furniture`), while v1 `marker` has no
such arm and falls through to `heading_depth`/`None`. The heading route resolves
every marker through `full_marker` via `heading_marker`
(`crates/da-parse/src/native_heading_regions.rs:866-871`). The two routes can
therefore disagree about a terminal other than references/annex — a plain
`Acknowledgement` heading ends the heading route's `Body` but leaves v1 in
`Body`. Correct statement: *references and annex terminals are shared; not all
body-ending boundaries are, so the exit boundary is not fully independent either.*
Source hashes at correction time: `native_segmentation.rs`
`5a1f926b9ba655f060d95db368ff16ff2d06c37f5f98475d9e0e18e6b9a81b5a`,
`native_heading_regions.rs`
`11015e3b70f13884f71871b6590649297213ec6f6143797c4bd51ca8af307a3a`,
`native_body_text.rs`
`9dfbd44f913d31c83fce4ff6a4c94081370d0312dbece2dab7e4ace846e4f57a`.

### 9.4 The v5 → v6 report change is additive only

Diffing the pre-edit v5 report (`68835c94…`) against the frozen v6 report
(`d88d7eed…`, 50 107 B): **10 of the 13 top-level keys are byte-identical**
(`corpus_id`, `page_denominator`, `lane`, `supersedes`, `historical_note`,
`oracle`, `scopes`, `input_bindings`, `locks`, `thresholds`). Only
`schema_version` (v5→v6), `documents` and `aggregate` differ, and both differ
only by the added `third_route_candidate` member: every pre-existing document
field is identical, and every pre-existing aggregate band
(`s03_parent_span`, `word_heading`, `additive_assignment`,
`p1_plane_preservation`) is identical. §4's numbers come from this frozen v6
report (`artifacts/parse_compat/s05_t01_route_c_run/07_probe.stdout`
`7a1ba09aed5ba51af9da3a266a7d1ab837758d966bbef6e42a8eccaae7b76c46`,
`07_probe.rc` = `0`).

### 9.5 Candidate-only, restated

- **No P2 repair.** §4 records a bounded repair *candidate*; no repair is applied
  to any route, no threshold accepted, no route switched.
- **No independent bound proof.** All three frozen verdicts are
  `ThirdRouteSubsetOfHeading` (one-sided); the exit boundary is not independent
  (§9.3) and, on HUB, both routes' entry identity is `(0,1)`, so HUB's entry is
  not independently confirmed either.
- **No P1 clearance.** The P1 plane stays `NotAssessable` on all three
  (`absent_from_plane = 0`), each candidate gap is `NotAssessable`, and no D498
  decision is recorded.
- **T01 remains `NotAssessable`** (open, not completed or waived). Locks
  unchanged: `claim_scope=diagnostic_only`, `product_coverage_effect=none`,
  `promotion_state=candidate`, `auto_promote=forbidden`,
  `graph_mutation=forbidden`, `import_eligible=false`,
  `thresholds=not_accepted`.
- **Recovery: closed on this candidate.** No recovery action is opened, resumed,
  settled or made terminal by this correction; no recovery outcome is claimed,
  and the abort/recovery record `1c74d263-b855-4b7c-b79d-1fe54b40daee` named in
  §1 stands exactly as §1 recorded it.

### 9.6 Readback and integrity

This correction modifies exactly one file,
`artifacts/parse_compat/native_body_third_route.addendum.md`, by appending
section 9. The original 14 732 B prefix is byte-identical to the preservation
copy (sha256 `37fb0fb3…`); that prefix identity and the SAME status of every
cited source, report and capture (before vs after) are verified in
`artifacts/parse_compat/s05_t01_third_route_addendum_preservation.20261001T025608Z/VERIFICATION.txt`.
