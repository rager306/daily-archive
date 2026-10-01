# Archived: `native_body_character_shadow.json`

This file is the **superseded** T01 body-character diagnostic. It was moved here
(byte-identical) by M514-1lss2s / S05 / T09 so that the GAR-006 population
contract can enumerate the live shadow set without either rewriting this
artifact or hand-authoring a population split no code produces.

* schema: `native-body-character-shadow.v1`
* sha256 (unchanged by the move): `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361`
* superseded by: `artifacts/parse_compat/native_body_character_scope_bound.json`

## Why the move instead of conforming the report

GAR-006 (`data/parse/acceptance_manifest.yaml#population_contract`,
`crates/da-parse/tests/data_contracts.rs::population_contract_enumerates_every_shadow_report`)
requires every live `artifacts/parse_compat/*_shadow.json` to declare the
`reference_population` / `executed_population` / `evaluated_population` /
`outcomes` split, and the governor rule
`scripts/governor/checks/executed_population_binding.yaml#report_contract.glob`
fails closed on a shadow report the contract does not declare.

Two resolutions were available. This one was chosen:

1. **Chosen — archive the artifact out of the shadow namespace.** The artifact
   has no live producer (grep for `native-body-character-shadow.v1` finds only
   preserved copies under the `s05_t01_*` run directories; the current probe
   emits `native-body-character-scope-bound.v6`). Its bytes are recorded as
   immutable in at least five tracked T01 evidence documents. Moving it
   preserves every recorded digest; it removes the report from the live shadow
   set, which is exactly what `scholarly_parse_plane.yaml` already does for
   `parse_batch_budget_live.json`, `native_family_promotion_rehearsal.json` and
   `grobid_retirement_release_decision_package.json` ("the `*_shadow.json`
   enumeration is the shadow-report set ... mixing a measured live run with
   shadow runs is exactly the conflation that contract exists to prevent").
2. **Rejected — reshape the report to carry the population split.** It would
   rewrite a byte-pinned artifact (falsifying the recorded `55647743...`
   digest in the T01 evidence documents) and would require hand-authoring a
   reference/executed/evaluated split that no producer derives and no
   independent oracle can falsify, which is the fabrication shape GAR-006
   exists to catch.

## Why a subdirectory and not a rename

Both discovery sites are non-recursive, so a subdirectory is outside the live
set:

* `crates/da-parse/tests/data_contracts.rs` uses `std::fs::read_dir` on
  `artifacts/parse_compat` and matches names ending `_shadow.json`;
* `scripts/governor/checks/executed_population_binding.yaml#report_contract.glob`
  is `artifacts/parse_compat/*_shadow.json`, resolved by `Path.glob`, whose
  single `*` segment does not cross `/`.

A subdirectory also keeps the exclusion structural and self-describing.

## Reference updates

* `crates/da-cli/examples/native_body_character_probe.rs` — `HISTORICAL_REPORT`
  and the module note now name the archive path.
* `crates/da-cli/examples/native_body_order_lineage_probe.rs` — `PRIOR_ARTIFACTS`
  entry updated (this list is read from disk, so a stale path would silently
  degrade the recorded digest to `null`).
* `artifacts/parse_compat/native_body_order_lineage.json` — the recorded
  `prior_artifacts[0].path` updated; the recorded sha256 is unchanged.

## Known boundary

The four frozen `native_body_character_scope_bound*.json` reports still carry
`"supersedes": "artifacts/parse_compat/native_body_character_shadow.json"`. That
field is a historical record of the predecessor's identity at production time,
not a resolvable pointer: nothing reads it, and rewriting it would change
byte-pinned T01 artifacts. The predecessor's digest is unchanged, only its
location.
