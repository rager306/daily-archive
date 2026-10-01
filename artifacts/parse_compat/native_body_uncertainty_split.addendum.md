# Addendum (narrow correction) — S05/T01 uncertainty-split REPORT CONTRACT

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`

This file **adds** a bounded correction beside
`artifacts/parse_compat/native_body_uncertainty_split.evidence.md`
(sha256 `5a062aad37dc284d18fb709ea97f7ba4f3ba9cdf2333e38b2649ac6c1d5886c4`) and the
report it documents, `artifacts/parse_compat/native_body_uncertainty_split.json`
(prior bytes sha256 `e4dc96e4d4b0250213c40ec8d6eb4e798e0546c66a4d7bf6fe668b4488a39039`).
It does **not** edit, replace or erase either document: both are preserved
byte-for-byte under
`artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/` and are
re-verified from the copies by that run's `07_preserved_copies_check` (rc 0).

The report JSON was necessarily rewritten by this repair — the correction *is* a
report-contract change. The prior bytes survive at the preserved path and are
pinned below; the new bytes are `478054d5…`.

## What was wrong (three report-contract defects, measured)

| # | defect | why it mattered |
|---|---|---|
| 1 | the aggregate `accepted_bucket_reproduced` folded **only P1**: it was `p1_documents_reproduced_all && p1_split_bucket_total == 69 400` | a drift in the P2 word-body bucket (`62 302`) still read as a clean reproduction, because the number the accepted P2 pin disagrees with was never consulted |
| 2 | every P2 non-measurement was reported as the literal `"NoBodyTokens"` (`_ => (None, None, None, Some("NoBodyTokens"))`) | a typed band absence and a terminal band failure were relabelled as "this document has no body tokens", which is a different fact. The word row's own `absence` was computed right beside it and then discarded |
| 3 | `split_body_uncertainty(...).ok()` discarded a typed refusal | a refused split (the public entry point returns `Err(NativeBodyTextError::InventoryRungMismatch { side: "native body" })` on a rung mismatch) silently became "no split", with no refusal named anywhere in the report |

Defect 2 and 3 also shared a failure mode: both turned a *named* typed outcome
into an unnamed absence, so the reader could not tell "not applicable" from
"measurement refused".

## What the correction changes in the report

The measurement is untouched. The report contract gains:

* aggregate `accepted_bucket_reproduced` is now a conjunction over **both**
  bands, per document and in aggregate, over the complete declared population
  (`accepted_buckets_reproduced`); a P2 drift, a P2 band absence or a refused P2
  split makes it `false`;
* aggregate `p2_population_complete` and `p2_refusals`, so a missing or refused
  P2 measurement is visible as a count rather than inferred;
* per document, `p2_absence` now carries the word band's **own** typed reason and
  `p2_split_refusal` carries the typed refusal of the split call;
* `conservation_invariant` now states that the identity holds **by construction**,
  not through an independent test.

New report-level negatives (in the probe, `cargo test -p da-cli --features
pdf-layout --example native_body_uncertainty_split_probe`, 7 tests): a P2 bucket
drift fails the claim on the aggregate-total axis, on the per-document axis and
on the population axis; a band absence keeps its own reason and never becomes a
fallback; an unnamed absence is a typed refusal; a refused split fails closed and
is never an absence; an inconsistent measurement is a typed refusal; a measured
band reproduces its pin; the report states the by-construction basis.

## Conservation is by construction, not an independent gate

`SideLocalization::uncertain_present_in_token()` in
`crates/da-parse/src/native_body_text.rs` **is** `uncertain_equal_in_token +
uncertain_proper_substring_in_token` — the accepted bucket is not stored
separately, and `split_side_uncertainty_checked` returns those same two stored
fields. So `equal + proper == bucket` cannot fail independently of the shared
computation that produces both sides. `p1_conservation_ok` / `p2_conservation_ok`
are therefore a self-consistency reflex, and they are **not** evidence that the
body is preserved, not a quality gate, and not a substitute for the P1/P2
verdicts. The report now says so in `conservation_invariant`, one report-level
negative pins the wording, and the da-parse contract test documents it beside the
assertion it qualifies.

## Own pin — every relevant source hash, before → after

Captured before any edit (`preserved_before.sha256`, copies re-verified by step
`07_preserved_copies_check`, rc 0) and after the frozen run
(`hashes.source.after.txt`, identical to `hashes.source.before.txt` — `SAME source`).

| path | before edit | after run | state |
|---|---|---|---|
| `crates/da-cli/examples/native_body_uncertainty_split_probe.rs` | `d45dce4745f9022be3a10e440cb2cb7a457051f6f55ebccec71846d849c1ecea` | `3ce6fe891d4036222db4cf7a3e9102b6fb7b271f2b82d25ca312d8c118906195` | **edited** (report contract + tests) |
| `crates/da-parse/tests/native_body_uncertainty_split_contract.rs` | `374505942b8ba7aa40287bb088cbee92538f9c1c3a72a72af6e6ed8a3036940e` | `34eb794d25fa3ee544cfa9b3149d54292bfb177926f9079039b88cc31f7cf7cf` | **edited** (doc comment only) |
| `crates/da-parse/tests/architecture_contract.rs` | `a4fe3ba038fbeabeeae74ebeae59f17ade3b9fc0f32c6db02b1a7362025888c4` | `a4fe3ba038fbeabeeae74ebeae59f17ade3b9fc0f32c6db02b1a7362025888c4` | pinned, **unchanged** |
| `crates/da-parse/src/native_body_text.rs` | `1067584e8f8589b7463d587beacb01616e50aaa78dc7186db45f3a4094fd5bbd` | `1067584e8f8589b7463d587beacb01616e50aaa78dc7186db45f3a4094fd5bbd` | pinned, **unchanged** |
| `crates/da-parse/src/lib.rs` | `166d9fa7b0826ce86bf0a08a6e8c2d0286d3d66f7dd6704f79bd7a0818db0d73` | `166d9fa7b0826ce86bf0a08a6e8c2d0286d3d66f7dd6704f79bd7a0818db0d73` | pinned, **unchanged** |
| `crates/da-cli/Cargo.toml` | `5885062267e534d015e68df6cf32bdc91630faa55d48dffaa0bcc4c296d94a42` | `5885062267e534d015e68df6cf32bdc91630faa55d48dffaa0bcc4c296d94a42` | pinned, **unchanged** |

No alignment, engine, segmentation, regional or primary-lane source was touched.
`native_body_text.rs` — the module that owns the split — is byte-identical.

Report artifact:

| artifact | before | after |
|---|---|---|
| `artifacts/parse_compat/native_body_uncertainty_split.json` | `e4dc96e4d4b0250213c40ec8d6eb4e798e0546c66a4d7bf6fe668b4488a39039` | `478054d5bd36d6d59057d53f4ba6ddbc21ccfb5578241fd081b595c6c877d69b` |

Accepted sibling artifacts are untouched and were re-verified from their copies:
`native_body_character_scope_bound.json`, `native_body_order_lineage.json`,
`native_body_character_shadow.json` (plus their evidence documents and
`native_body_order_lineage.addendum.md`).

Script identities, recorded **before** the run
(`scripts.identity.before_run.txt`, timestamp `2026-09-30T23:32:03.456168990Z`):

| script | sha256 |
|---|---|
| `preserve_before.sh` | `625fca22d4e6143431a54cfeb275edf4c6a59c5b36dcb2e11dcad5d5dea62c29` |
| `capture.sh` (actual capture) | `16140535f240e032ad867ac5c96e2edfbb052a04e48bb83ce463a4c36952c29c` |
| `verify_after_fmt.sh` (post-format script) | `1b88428de4087d804a1f73b2ea89fdc292e714d144f33c44544a4ae31b411203` |

## What this correction does NOT change

* **The counts are the same.** hub `32 872 + 35 = 32 907` (P1) / `32 872 + 35 =
  32 907` (P2); rlm `15 320 + 2 = 15 322` / `12 895 + 494 = 13 389`; reca
  `21 144 + 27 = 21 171` / `14 777 + 1 229 = 16 006`. Aggregate P1 `69 400`
  (`69 336 + 64`), aggregate P2 `62 302` (`60 544 + 1 758`). These match the
  independently reviewed facts; the correction only changes how the report
  *folds and names* them.
* **Every verdict is still `not_assessable`** on P1 and on the word/heading body
  band; `d498_reopen_required` is `false` on every document; `thresholds =
  not_accepted`; all six locks are unchanged. The correction adds no clearance.
* **No cause is attributed.** The split remains a fact count, not a threshold,
  ratio, quality proof or promotion.

## T01 status

**Not complete.** The body-gap question is unchanged in substance: the corrected
report says the same thing about the measurement and only stops overstating the
reproduction claim (P2 is now folded) and mislabelling typed outcomes. No
canonical blocker, waiver, lifecycle, recovery or engine decision was changed,
and no recovery authorization is requested or implied. D498 stays untouched.
