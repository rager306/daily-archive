# continue — M495-gnxzxu next; M494-a1i14t complete

## Active state (updated 2026-09-06 after M494 closeout)

**M494-a1i14t is complete** (`completedAt=2026-09-06T08:08:10.761Z`).
Header atoms same-n + shadow sequence stand. Java GROBID remains production.
Cascade P0–P4+P6–P9 live; P5 stays `accountant_later`. Six DecisionPoint wires
frozen. `visual_bundle` stays later. `this_wave_promotes=none`. Native not
splice. Graph frozen. D127 false. Hub 20 vs 21 stays DATA.

**Next milestone: M495-gnxzxu** (active, 5 slices planned, 0 complete).
Bibliography atoms same-n then shadow reference sequence. Not a family
promote. Not graph. Not M446.

| Slice | Goal |
|-------|------|
| S01 | Citation names 2198 stay a named DATA flag; no pooling with biblStruct (R128) |
| S02 | Native citation fields same-n on hub/rlm/reca (80+78+49=207) |
| S03 | Native reference segments same-n on reconstructed raw lists |
| S04 | Native citation anchors same-n on bibr refs |
| S05 | Reference sequence stays a shadow runner; P4 stays Java biblStruct (R129) |

HEAD at plan time: `8b093b1e2` Native affiliation same-n. Branch `feat/da-parse`.

Product **M446-rw9a2p** is another train — do not steal auto onto it.

## Locks (do not violate)

- Native not splice into parse-pdf / ParserPort.
- Do not pool `citation_names` 2198 with biblStruct.
- Do not close hub 20 vs 21 by compose.
- Do not add `visual_bundle` / `head_fallback` / `critic_sample` to DecisionPoint.
- P5 stays `accountant_later`. `this_wave_promotes=none`.
- Graph / Samyama / import_eligible frozen (R118, D127, D321).
- Expected builders must not use TeiLifter or `compose_references`.
- `independently_selectable` stays `[header, references]`.
- SHA-bind YAML in the same change. Per-crate cargo only (rocksdb trap).
- `gsd_slice_complete` required or SUMMARY missing → `completed-no-advance`.
- Lock **`M495-gnxzxu`**, not `/gsd auto M495`. `unique_milestone_ids: true`.
- Do not `/gsd auto M494-a1i14t` again unless reopen.
- Do not `/gsd auto M446`.
- Stale env `GSD_MILESTONE_LOCK=M493` (bare) empties headless registry — never reuse.

## GSD auto

Plan is ready. Start with **`/gsd auto M495-gnxzxu`** (suffixed). Do not use
`--resume-wedge` unless a live unacknowledged wedge exists.

Any native family promote still needs a **separate explicit user yes**.
Do not unfreeze graph.
