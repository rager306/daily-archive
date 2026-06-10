# M056 Wave 6 Final 1-hop Analysis

Generated: `2026-06-10T14:46:24.940950+00:00`

## Safety

- Graph writes: false
- Production import attempted: false
- Promotion allowed: false
- Facts promoted: false
- External mutation allowed: false
- This evidence is not authorized for graph import or fact promotion.

## Acquisition

- Requested refs: 16
- Success: 16
- Blocked: 0
- Network errors: 0
- Status counts: acquired: 16

## Parser quality

- GROBID packets: 16
- GROBID success: 16
- GROBID quality counts: success: 16
- OpenDataLoader packets: 16
- OpenDataLoader success: 14
- OpenDataLoader quality counts: low_quality_source: 2, success: 14
- Packet safety defaults all false: True

## Connectivity gain

- Target set: 20 existing corpus PDFs + anchor `2605.18747`
- Wave 1 directed edges to target set: 3
- Wave 2 new directed edges to target set: 2
- Wave 3 new directed edges to target set: 1
- Wave 4 new directed edges to target set: 2
- Wave 5 new directed edges to target set: 0
- Wave 6 new directed edges to target set: 0
- Delta vs Wave 5: 0
- Cumulative directed edges: 7
- Final saturation status: final-saturated

### Wave 6 new edges

- none

### Edge saturation by wave

- wave_1: 3, wave_2: 2, wave_3: 1, wave_4: 2, wave_5: 0, wave_6: 0

## Final 1-hop corpus accounting

- Wave-order entries: 166
- Wave-order unique IDs: 166
- Anchor present in wave-order: True
- Acquired wave entries: 166
- Acquired unique wave PDFs: 148
- Total unique PDFs with anchor: 149
- Evidence corpus unique PDFs including prior target corpus: 169
- Cumulative corpus path: `artifacts/m056-bfs-graph/wave-6/cumulative-corpus.json`

## Final recommendation

- Decision: 2-hop needed for graph-readiness; accept 1-hop as benchmark-only evidence
- Rationale: The final two waves added zero new target-set edges after sparse cumulative connectivity, so the 1-hop corpus is operationally complete but not graph-ready.

## Self-citation cluster

- Anchor first author: Xuying Ning
- Matching Wave 6 PDFs: 1 / 16 (6.2%)

## Category distribution

- mixed-source: 16

## Length distribution

- 1-10: 3, 11-25: 8, 26+: 5
