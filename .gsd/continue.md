# continue — M494-a1i14t next; M493-jzmctv complete

## Active state (updated 2026-09-06 after planning M494-a1i14t)

**M493-jzmctv is complete** (`completedAt=2026-09-05T21:59:07.729Z`). Honesty
closeout stands. Java GROBID remains production. Cascade P0–P4+P6–P9 live;
P5 stays `accountant_later` (no `stamp_p5`). Six DecisionPoint wires frozen.
`visual_bundle` stays later. PARSE_BARE is lifetime/use. Corpus constructors
use `corpus_grobid_parser` (structural). S12 was candidate-only:
`this_wave_promotes=none`. Native not splice. Graph frozen. D127 false.
Hub 20 vs 21 stays DATA.

**Next milestone: M494-a1i14t** (active, 5 slices planned, 0 complete).
W0 ledger hygiene then W1 header atoms. Not a family promote. Not graph.

| Slice | Goal |
|-------|------|
| S01 | Observation lanes stay off independently selectable (R125, D323) |
| S02 | Native date same-n on hub/rlm/reca TEI identities (R126) |
| S03 | Native personal name same-n; Hub 20 vs 21 stays named gap |
| S04 | Native affiliation same-n with document_id (MEM990) |
| S05 | Header sequence stays a shadow runner (R127); parse-pdf still Java P3 |

HEAD at plan time: `6bf3f39b1` Governor fail-closed freeze. Branch `feat/da-parse`.

Product **M446-rw9a2p** is another train — do not steal auto onto it.

## Locks (do not violate)

- Native not splice into parse-pdf / ParserPort.
- Do not close hub 20 vs 21 by compose.
- Do not add `visual_bundle` / `head_fallback` / `critic_sample` to DecisionPoint.
- P5 stays `accountant_later`. `this_wave_promotes=none`.
- Graph / Samyama / import_eligible frozen (R118, D127, D321).
- Expected builders must not use TeiLifter or `compose_header` (MEM996).
- `independently_selectable` stays `[header, references]`. `strata.lanes` may be a superset.
- SHA-bind YAML in the same change. Per-crate cargo only (rocksdb trap).
- `cargo fmt` on `architecture_contract.rs` / `data_contracts.rs` before commit.
- `gsd_slice_complete` required or SUMMARY missing → `completed-no-advance`.
- Lock **`M494-a1i14t`**, not `/gsd auto M494`. `unique_milestone_ids: true`.
- Do not `/gsd auto M493-jzmctv` again unless reopen.
- Do not `/gsd auto M446`.

## GSD auto

Plan is ready. Start with **`/gsd auto M494-a1i14t`** (suffixed). Do not use
`--resume-wedge` unless a live unacknowledged wedge exists.

Any native family promote still needs a **separate explicit user yes** after
header same-n + sequence receipts. Do not unfreeze graph.
