# M058-cmjp1u: M059 Pilot Cycle plotextractor v2 + Marker Iterative Expansion

**Vision:** Run plotextractor v2 pilot (figure caption + path from TeX source, 5 PDF) + Marker iterative expansion (5 → 10 → 30 PDF) with explicit eval gates. After M058 produces evidence, decide whether to scale Marker to full 166-PDF corpus and what graph-readiness gate v2 looks like.

## Slices

- [x] **S01: plotextractor v2 pilot: figure caption from TeX source (5 PDF)** `risk:medium` `depends:[]`
  > After this: plotextractor installed, 5 PDFs TeX downloaded + extracted, figure captions v2 emitted, comparison with M057 regex extraction

- [x] **S02: Marker pilot stage 1: 5 PDF + quality eval** `risk:low` `depends:[S01]`
  > After this: 5 PDFs Marker-extracted, OpenDataLoader comparison, decision: continue to 15 PDF or stop

- [ ] **S03: Marker pilot stage 2: 10 more PDF (cumulative 15) + eval** `risk:low` `depends:[S02]`
  > After this: 15 PDFs cumulative Marker-extracted, comparison report, decision: continue to 45 PDF or stop

- [ ] **S04: Marker pilot stage 3: 30 more PDF (cumulative 45) + final eval** `risk:medium` `depends:[S03]`
  > After this: 45 PDFs cumulative Marker-extracted, full comparison report, decision: scale to 166 or stop

- [ ] **S05: Synthesis + ADR-012 + decision deferred** `risk:low` `depends:[S01,S02,S03,S04]`
  > After this: Combined pilot evidence, ADR-012 emitted, M060 scope proposal

## Boundary Map

Not provided.
