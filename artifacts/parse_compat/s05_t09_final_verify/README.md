# S05/T09 final slice gate — integration contracts

Milestone `M514-1lss2s`, slice `S05`, task `T09`. This directory holds the
recorded exit codes and outputs of the four declared verification commands.
T09 is **verify-first**: reconnaissance found both integration contracts already
satisfied on the live tree, so no source, DATA or carrier edit was made.

## Result

| # | Command | Exit | Wall | Observed | Evidence |
|---|---------|------|------|----------|----------|
| 01 | `cargo test -p da-cli --lib logging` | 0 | 45.9 s | 4 passed / 0 failed | `gsd_exec ec0920c2-4d41-44e8-b247-3819a5ae51de`; `01_logging.{stdout,stderr,exit,duration_ms,exec_id}` |
| 02 | `cargo test -p da-parse --test data_contracts` | 0 | 19.7 s | 104 passed / 0 failed | `gsd_exec f00a5c21-9c7b-4af2-8fd6-feeb1c6fdc69`; `02_data_contracts.*` |
| 03 | `cargo test -p da-cli --features pdf-layout --tests` | 0 | ~12 m 22 s | 49 test binaries, 800 passed / 0 failed | detached runner (`bg_shell 90d6900f`), see `03_dacli_pdf_layout_tests.*` |
| 04 | `cargo fmt --all -- --check` | 0 | 32.4 s | clean, no diff | `gsd_exec da90babe-382c-4875-86ad-154eecf14371`; `04_fmt.*` |

**Step 03 runs detached on purpose.** The feature-gated da-cli suite exceeds the
600 s `gsd_exec` cap (the slowest single binary is ~170 s), and per AGENTS.md a
`timed_out` capture must be treated as a failure, not a PASS. The suite was
therefore executed in a persistent background runner that writes its own exit
code to `03_dacli_pdf_layout_tests.exit`; that file — `0` — is the verified
exit code, and the two logs are the complete run. An earlier `gsd_exec` attempt
at the same command (`0667aa89-a618-455f-84f4-3d2ab406d6b6`) was killed by the
cap mid-suite and is **superseded** by this run.

## Contract 1 — structured logging of the new examples

Contract: `crates/da-cli/src/logging.rs::tests::active_pipeline_examples_initialize_structured_file_logging`
walks `crates/da-cli/examples/*.rs` and asserts every example contains
`init_pipeline_logging`.

Measured: all **95** example files contain the call, including the three S05/T01
probes (`native_body_character_probe.rs:421`,
`native_body_uncertainty_split_probe.rs:597`,
`native_body_order_lineage_probe.rs:418`), each holding the `WorkerGuard` as the
first statement of `main()`. The contract passes; **no edit was required**, and
re-adding the guard would have created a second `WorkerGuard` for the same
database.

## Contract 2 — GAR-006 shadow-report enumeration

Contract: `crates/da-parse/tests/data_contracts.rs::population_contract_enumerates_every_shadow_report`
— every `artifacts/parse_compat/*_shadow.json` must be declared in
`data/parse/acceptance_manifest.yaml#population_contract.report_paths`, and
`validate_shadow_population_reports` must find no contract violation.

Measured: the live shadow set is 11 files, all declared. The superseded T01
diagnostic `native_body_character_shadow.json` is archived at
`artifacts/parse_compat/historical/native_body_character_shadow.json` and is
therefore **out of the non-recursive `read_dir` glob**; its bytes are unchanged
by the move (`sha256 55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361`,
the digest recorded in the T01 evidence documents). The report keeps its
`native-body-character-shadow.v1` shape, which is not reshaped into a
population split that no producer derives.

Consequently `data/parse/acceptance_manifest.yaml` was **not** edited: there is
no measured enumeration defect to repair, and adding the archived path would
introduce declaration drift.

## Boundaries preserved

- No byte-pinned S03/S04 carrier touched (`layout_bakeoff.yaml`,
  `native_visual_heads.yaml`, `document_segmentation.yaml`,
  `native_content_receipt.json`, `document_segmentation*`,
  `native_visual_oracle_source.json`).
- Measured values unchanged; `claim_scope=diagnostic_only`,
  `thresholds=not_accepted`, locks `import_eligible=false`, `deployed=false`,
  `auto_promote=forbidden`, `graph_write=forbidden` unchanged.
- Only additive artifacts: this verification directory.
