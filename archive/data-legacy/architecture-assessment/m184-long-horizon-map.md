# M184 Long Horizon Execution Map

## Execution order

1. **S01 Baseline and Long Horizon Map** — artifact-only baseline and planning contract.
2. **S02 Script Only Ratchet and Ownership Contract** — prevent silent `script-only` regression before movement waves.
3. **S03 Acquisition Source Exact Wave** — exact movement or no-move for acquisition/source records.
4. **S04 Audit Analysis Exact Wave** — exact movement for audit/analysis records.
5. **S05 Render Report Contract Wave** — exact movement for render/report contracts.
6. **S06 Replay and Conversion Seam Wave** — higher-risk GitNexus-backed replay/conversion decisions.
7. **S07 Graph Connectivity Probe Wave** — graph/probe decisions without violating no direct extractor-to-graph write.
8. **S08 Governance Sync and Misc Triage** — bucket all leftovers and avoid blind spots.
9. **S09 Script Wrapper Extraction Pilot** — one real script-to-src seam after candidate evidence exists.
10. **S10 Test Contract Alignment Wave** — align tests with the pilot seam contract.
11. **S11 Cache Manifest Lifecycle Proof Gate** — reusable checklist and no-move unless proven.
12. **S12 Architecture State and Final Verification** — current-state docs, canonical refresh, quality, GitNexus closeout.

## Family to slice map

| Family | Count | Primary slice | Notes |
|---|---:|---|---|
| acquisition-source | 10 | S03 | Higher care for source/PDF semantics. |
| audit-analysis | 24 | S04 | Likely largest exact classification wave. |
| render-report-contract | 8 | S05 | Report contracts should remain behavior-tested. |
| replay-conversion | 2 | S06 | GitNexus shows active replay flows. |
| graph-connectivity-probe | 13 | S07 | Preserve graph write boundaries. |
| experiment-probe | 9 | S08 | Likely no-move or exact artifact categories. |
| governance-sync | 4 | S08 | Governance artifacts need owner clarity. |
| manifest-cache-index | 3 | S11 | No movement without lifecycle proof. |
| misc | 16 | S08 | Bucket into movement, no-move, or pilot candidates. |

## Proof gates

Every movement slice must produce:

- fresh baseline before edits;
- exact source-path audit;
- GitNexus impact before symbol edits;
- generated delta via `scripts/inventory_write_paths.py --delta-from`;
- canonical refresh after scanner movement;
- strict canonical drift pass;
- focused tests;
- no increase in `unknown`, `shared-state`, `dynamic`, `legacy`, or onion violations.

## Stop conditions

- Do not add broad prefix, target-name, cache, index, manifest, markdown, or converter scanner rules.
- Do not move cache/index/manifest records without owner, invalidation, consumer, and concurrency proof.
- Do not extract a script function into `src` until impact and behavior tests identify a safe seam.
