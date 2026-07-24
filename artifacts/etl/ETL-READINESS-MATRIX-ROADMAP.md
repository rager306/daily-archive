# ETL Readiness Matrix + Dependency Roadmap

**As of:** 2026-07-24 (post M257–M263)
**Execution status:** M257–M263 complete. Import locked (D127). GEPA closed (D126).
**Live hybrid_found:** 64/230 (0.2783). Ship path: header_priority. Structure: partial.  
**Horizon policy:** Wave A residual → glue → coverage scale → Wave B quality → structure → **import only Wave D**  
**Hard locks:** `import_eligible=false`, no DSPy optimizer until LLM beats header, no Falkor write pilot until D.

Live baseline used for this plan:

| Metric | Value |
|--------|-------|
| catalog articles | 230 |
| hybrid_found | **49** (0.213) |
| hybrid_missing | 181 |
| PDF ready among missing | **160** (frac ~0.884) |
| PDF absent | **21** (+ 3 gold IDs not in catalog PDF) |
| multi_root | 20 ids, identical-only, divergent=0 |
| preprocess | 50 bodies, 0 errors, soft_signal=15 |
| Wave A closeout | `wave_a_closed` (min_hybrid_found=40) |
| Wave B stamp | D124 open |
| header F1 n=20 | entity **0.675** / relation **0.35** |
| floor/oracle | 1.0 / 1.0 |
| LLM compact+prefer | 0.625 / 0.30 (loses) |
| gold missing IDs | `2507.19457`, `2511.20639`, `2605.18211` |

---

## 0. Wave map (long horizon)

```
A data readiness ──► B extraction quality ──► C structure/graph-ready ──► D import pilot ──► E agents
         │                    │                        │                      │
         │                    │                        │                      └─ blocked until D evidence
         │                    │                        └─ PageIndex/TEI/citations as structure truth
         │                    └─ header floor → constrained LLM only if delta>0 → GEPA last
         └─ catalog/PDF/hybrid/preprocess/expand/closeout (import false)
```

**Optimization principle:** fix **glue + measurement** before **scale**; fix **scale + acquisition** before **quality optimization**; fix **quality gates** before **structure promotion**; never open **import** to “make metrics look complete”.

---

## 1. Master matrix

Legend status: `done` | `partial` | `gap` | `debt` | `locked` | `n/a`

### 1.1 Milestone → function → module → links → defects

| ID | Milestone / band | Function (what it does) | Primary modules / operators | Links (depends on / feeds) | Algorithms / ideas | Status | Defects / not glued |
|----|------------------|-------------------------|-----------------------------|----------------------------|--------------------|--------|---------------------|
| **M029–M031** | Unified catalog + refusal boundary | Canonical article inventory; graph-readiness refusal | `catalog_ingest`, `import_boundary`, catalog index | → all ETL; → import hold | SHA idempotent ingest; fail-closed import | done | catalog↔body id drift (unique 50 vs found 49) |
| **M209** | Pipeline continuity (7-layer) | Existence checklist of pipeline seams | `pipeline_continuity.py` | parallel to ETL pack; **not same cockpit** | layer health inventory | partial | **dual continuity models** (M209 vs etl_continuity_*) |
| **M213–M216** | Hybrid body gate + coverage handoff | Select + live hybrid batch; catalog coverage; readiness handoff | `hybrid_batch_gate`, selection-*, `hybrid_catalog_coverage` | → body roots; → preprocess; → Wave B join | hybrid_claimed_success needs body evidence | done | selection naming `selection-40-*` stale vs hybrid 49 |
| **M217–M221** | GROBID TEI + citations review | Header/cites ETL; citation inventory; review policy | `grobid_tei_parse`, citation_* | → structure C; not import | TEI parse; review-only cites | partial | not on continuity dashboard; not Wave B relation input |
| **M222–M223** | Multi-source strengthen | non-arxiv register/proof | non_arxiv_*, universal_source | → catalog diversity | HTML native ≠ hybrid TEI | partial | hybrid body almost arxiv-only |
| **M224–M235** | Non-LLM preprocess stack | clean/lang/outline/fingerprint/spans/YAKE/rollup | `article_preprocess*`, `preprocess_rollup` | ← hybrid bodies; → readiness | YAKE optional; non-gating rollup | partial | yake=false in fleet; soft_signal 30%; not on pack dashboard |
| **M236–M240** | Import-hold ratchet | Scan trees; pre-commit block enablement | `composition_import_hold_inventory`, verify + hook | safety for all waves | fail-closed enablement hits | done | must stay on every expand/quality commit |
| **M241–M242** | Body coverage + hybrid metrics | Join catalog↔hybrid bodies; multi-root taxonomy | `etl_body_coverage_audit`, verify_etl_body_coverage | → pack, closeout, PDF readiness | multi_root identical vs divergent SHA | done | multi_root **storage debt** (20 identical copies) |
| **M243** | Preprocess fleet audit | Fleet quality over hybrid bodies | `etl_preprocess_fleet_audit` | → continuity readiness | quality ok/soft/error | done | soft_signal debt; not auto after expand |
| **M244** | Continuity readiness | Compose coverage+preprocess → signal | `etl_continuity_readiness` | → closeout; → pack inputs | blocked\|repair\|ready_for_review | done | signal ≠ full cockpit |
| **M245** | Selection expand plan | Pure PDF inventory → proposal | `hybrid_selection_expand` | → preflight/batch | round-robin category; size cap | done | does not run batch |
| **M246–M249** | Preflight + controlled expand | ready_to_batch; live limit batches | `hybrid_expand_preflight`, `verify_hybrid_expand_batch`, body roots include expand | → hybrid_found growth | live hybrid only with flag+sidecars | done | **no post-expand auto pack/preprocess** |
| **post-M249 ops** | Expand batch gate | Gate: ready + GROBID/ODL + limit + live flag | `hybrid_expand_batch_gate` | wired into expand operator | allow_limited_batch fail-closed | done | gate stdout/artifact ok; not in pack dashboard |
| **post-M249 ops** | Continuity pack dashboard | One operator report | `etl_continuity_pack`, `verify_etl_continuity_pack` | composes coverage+pdf+closeout; op also hold+preprocess | alerts on divergent/low expand | partial | dashboard missing preprocess/hold/expand_gate fields |
| **post-M249 ops** | Stamp guard | Refuse silent human_go rewrite | `write_human_go_stamp(force_rewrite=False)` | Wave B gate stamp | durable D124 auth | done | no pre-commit stamp immutability yet |
| **M250** | Wave A closeout | wave_a_closed when thresholds met | `wave_a_closeout`, verify_wave_a_closeout | ← readiness/hold/preprocess; → Wave B policy | min_hybrid_found=40 | partial | **threshold stale** vs live 49; no residual fraction target |
| **M251–M254** | Wave B gate + stamp + live extract | human_go open; inventory | `wave_b_gate`, stamp, live hybrid extraction | stamp ≠ import | D123/D124 stamp gate | partial | **standalone gate closeout_pass=None** (not loaded) |
| **M255–M256** | Statistical extract + gold metrics | hybrid statistical path; gold join metrics | wave_b_hybrid_statistical_*, gold_hybrid_* | → constrained select | lexical floor/oracle | done | three metric worlds (baseline/floor/header) confuse ship gate |
| **post-M256** | Constrained select + prefer-header | candidate_id select; structural score | `wave_b_constrained_select` | ← gold join+body; → LLM only if delta>0 | header_priority; score_selection_structural; top-k=2 | partial | relation F1 0.35; no dedicated relation module |
| **post-M256** | LLM compact pilot | constrained LLM + prefer-header | make_llm_constrained_select_fn, pilots | 9router agnes-free | progressive cache; trim top-k | gap | loses to header (−0.05); GEPA unjustified |
| **post-M256** | Gold body grounding | gold must be body+candidate grounded | `wave_b_gold_body_grounding_audit` | gate for gold expand | substring/cand grounding | done (n=20) | cannot expand gold until 3 PDFs exist |
| **C (future)** | Structure graph-ready | TEI/PageIndex/citation structure truth | page_index, structure_aware_chunking, cites | ← hybrid+preprocess; → import package | structure-aware chunking | partial | not wired as Wave C gate |
| **D (future)** | Explicit import pilot | production graph write under auth | pilot_write_*, promotion_boundary, Falkor | only after B+C evidence | pilot_eligible ≠ import_eligible | **locked** | correctly closed |
| **E (future)** | Agents / SymFSM loop | agent operators over ready graph | symfsm_* | after D | read-only first | locked | n/a now |

### 1.2 Defect register (actionable)

| Def ID | Class | Defect | Why it hurts | Owning wave | Fix type | Blocks |
|--------|-------|--------|--------------|-------------|----------|--------|
| D-G1 | glue | Expand success does not auto-run continuity pack / preprocess | Stale ops view after batch | A residual | wire | unattended expand loops |
| D-G2 | glue | Wave B gate standalone ignores live Wave A closeout | Stamp-only open without A context | A→B seam | wire | honest B open criteria |
| D-G3 | glue | Continuity pack dashboard omits preprocess + import-hold + expand_gate | Incomplete cockpit | A residual | schema+op | single-pane ops |
| D-G4 | glue | Dual continuity (M209 pipeline vs ETL pack) | Two “green” languages | A residual | unify or bridge report | false readiness claims |
| D-G5 | glue | Three Wave B metric worlds (baseline 0.92, floor 1.0, header 0.675) | Wrong proceed decision | B | metrics matrix artifact | quality ship |
| D-P1 | policy | min_hybrid_found=40 while live=49; no hybrid_fraction target | Closeout under-specified residual | A residual | policy | “A done” ambiguity |
| D-P2 | policy | PDF readiness never starts batch (correct) but no acquisition SLA for 21+3 | Silent permanent gaps | A residual | acquisition plan | gold + coverage ceiling |
| D-S1 | storage | multi_root identical copies (20) | Inflated files; root confusion | A residual | consolidate or hard-link policy | clean coverage |
| D-S2 | storage | hybrid_unique 50 vs found 49 | catalog join leak | A residual | id normalize audit | trust metrics |
| D-S3 | naming | selection-40-* naming drift | Operator confusion | A residual | rename/alias | ops UX |
| D-Q1 | quality | relation F1 0.35 header | Weak graph edges | B | relation path | C/D |
| D-Q2 | quality | LLM < header; GEPA unjustified | Optimizer temptation | B | hold until delta>0 | wasted opt |
| D-Q3 | quality | preprocess soft_signal 15/50 | Body quality debt | A/B | profile repair | extract noise |
| D-Q4 | quality | 3 gold IDs no PDF/body | Gold set incomplete | A acquis. → B | download+ingest+hybrid | gold expand |
| D-Q5 | quality | YAKE off in fleet | Weaker keyword spans | B optional | enable non-gating | optional lift |
| D-X1 | scale | hybrid_fraction 0.213; 160 PDF ready idle | Corpus still thin | A scale | gated expand loops | B sample bias |
| D-L1 | lock | import/Falkor/DSPy | Must stay closed | D later | keep ratchet | safety |
| D-C1 | CI | no pre-commit stamp immutability | Accidental re-auth risk | A residual | hook/test | D124 integrity |

### 1.3 Function × module adjacency (glue map)

```
catalog_index ──► body_coverage_audit ──┬──► continuity_readiness ──► wave_a_closeout ──┐
       │                │               │            ▲                                  │
       │                │ multi_root    │            │ preprocess_fleet                 │
       │                ▼               │            │                                  ▼
       ├──► pdf_readiness ──────────────┴──► continuity_pack ◄── import_hold ──► [dashboard]
       │                                         ▲
       │                                         │ (MISSING auto wire)
selection_expand ─► preflight ─► expand_batch_gate ─► hybrid_batch_gate ─► body roots
                                                                  │
                                                                  ▼
                                                         hybrid bodies ──► Wave B join/select
stamp human_go ──► wave_b_gate ──► constrained_select / pilots
         ▲              │
         │              └── closeout input often MISSING (D-G2)
         └── force_rewrite guard (done)
```

**Critical missing edges (optimize glue first):**
1. `hybrid_batch_gate` success → `continuity_pack` + optional preprocess refresh  
2. `wave_b_gate` ← always load `evaluate_wave_a_closeout` or pack artifact  
3. pack dashboard ← preprocess + hold + last expand_gate  
4. gold missing → `ingest_to_canonical_catalog` → pdf_readiness → expand (not free invent)

---

## 2. Dependency-ordered roadmap

### Planning rules

1. **Glue before scale** — unattended measurement first.  
2. **Scale before quality claims** — n=20 header metrics biased if corpus=49.  
3. **Acquisition before gold expand** — no PDF ⇒ no hybrid ⇒ no gold.  
4. **Header/relation before LLM/GEPA** — optimizer only if constrained LLM beats header with ASI.  
5. **Structure before import** — Wave C evidence package before D.  
6. **Never open import to unblock A/B metrics.**  
7. Thin slices: each milestone demoable; fail-closed; TDD; onion clean.

### Wave A residual — **M257 ETL Glue Cockpit** (FIRST)

**Goal:** eliminate D-G1, D-G2, D-G3, D-P1, D-C1 without growing hybrid_found.

| Slice | Demo | Depends | Touches |
|-------|------|---------|---------|
| S01 Post-expand continuity hook | expand limit≥1 (or dry) writes/refreshes continuity-pack fields; operator chain documented/auto flag | M246+pack | `verify_hybrid_expand_batch`, optional small app helper |
| S02 Wave B gate loads closeout | standalone gate shows live closeout_pass/signal | S01 optional | `wave_b_gate`, `verify_wave_b_gate` |
| S03 Pack dashboard v2 | dashboard includes preprocess ok/soft/err, hold hits, last expand_gate | S01 | `etl_continuity_pack`, operator |
| S04 Closeout policy refresh | min_hybrid_found + hybrid_fraction residual target documented+coded; PROJECT sync | S03 | `wave_a_closeout`, PROJECT |
| S05 Stamp immutability CI | pre-commit or verify refuses human_go authorized_at drift without force marker | stamp guard | hook or verify script |

**Success:** one command path: expand → pack green; B gate sees A; cockpit fields complete; stamp CI.  
**Out of scope:** live expand volume, LLM, import.

### Wave A residual — **M258 Coverage Scale + Acquisition**

**Goal:** D-X1, D-P2, D-Q4, D-S2; grow hybrid under gate.

| Slice | Demo | Depends | Notes |
|-------|------|---------|-------|
| S01 Expand loop 2–3× limit 10–20 | hybrid_found → ~70–90 if sidecars ok | M257 S01 | always post-pack |
| S02 PDF-absent queue operator | list 21 + 3 gold; download+`ingest_to_canonical_catalog` with network audit | catalog rules | gold IDs first |
| S03 Hybrid for new gold PDFs | gold join count >20 if bodies ok | S02 | grounding audit re-run |
| S04 Catalog join leak audit | explain unique50 vs found49; fix or tag | coverage | |
| S05 Selection artifact rename | selection naming matches state | cosmetic after S01 | |

**Success:** hybrid_fraction target from M257 policy; gold PDF present or explicitly blocked with reason; expand never without gate.

### Wave A residual — **M259 Storage Hygiene (multi_root)**

**Goal:** D-S1 without data loss.

| Slice | Demo | Depends |
|-------|------|---------|
| S01 Consolidation plan (identical-only) | choose: keep primary root + refs, or hardlink inventory | M241 taxonomy |
| S02 Apply + re-audit | multi_root_identical reduced or documented keep; divergent still 0 | S01 |
| S03 Pack alert thresholds | only alert on divergent; identical is debt metric | M257 pack |

**Depends on:** taxonomy done (yes). **Not before** glue (need clean metrics after).

### Wave B — **M260 Quality Metric Matrix + Relation Path**

**Goal:** D-G5, D-Q1; no GEPA yet.

| Slice | Demo | Depends |
|-------|------|---------|
| S01 Ship-gate matrix artifact | single JSON: floor / header / baseline / LLM deltas; which gate blocks ship | M256+constrained |
| S02 Relation candidate model | relation candidates from header/structure/co-occurrence; constrained select parallel to entities | S01, hybrid bodies |
| S03 Relation metrics on gold n | relation F1 vs header baseline; no invent | S02, gold join |
| S04 Prefer-header for relations | fail-closed under header if LLM weak | S03 |

**Success:** relation path measurable; ship criteria explicit.  
**Blocked if:** gold still n=20 only and biased — prefer after M258 partial scale.

### Wave B — **M261 Constrained LLM only if beats header**

**Goal:** D-Q2 properly; optional D-Q5.

| Slice | Demo | Depends |
|-------|------|---------|
| S01 Re-run compact LLM on current gold | delta_vs_header recorded | M260 matrix |
| S02 Prompt/cand ranking iterate **only if** path to delta>0 | ASI compare | S01 |
| S03 YAKE non-gating fleet trial | yake on/off ablate preprocess→select | optional parallel |
| S04 GEPA/DSPy decision gate | open **only if** constrained LLM > header entity+relation | S02 | else skip forever |

**Hard stop:** if delta≤0 after S02 budget → freeze LLM path; keep header.

### Wave C — **M262 Structure Graph-Ready Package**

**Goal:** citations + PageIndex + chunk structure as import package inputs (still no write).

| Slice | Depends |
|-------|---------|
| S01 Structure continuity bridge (M209 layers ↔ ETL pack) | M257 D-G4 |
| S02 Citation review package on expanded hybrid | M217–M221, M258 |
| S03 Chunk/PageIndex readiness on hybrid bodies | structure modules |
| S04 Graph-data readiness validate-only package | M209 lineage |

**Success:** import-shaped package with import_eligible still false.

### Wave D — **M263 Explicit Import Pilot** (later, explicit user go)

**Only when:** A residual targets met; B ship matrix green (or accepted header floor); C package complete; separate human auth (not D124 alone).

| Slice | Notes |
|-------|-------|
| S01 Promotion rules pilot_eligible→import_eligible | D204 lineage |
| S02 Falkor/write dry-run → tiny pilot | pilot_write_* |
| S03 Rollback + observability | required |

**Until then:** keep M236–M240 ratchets.

### Wave E — agents: **after D only**

---

## 3. Critical path (optimized order)

```
M257 Glue Cockpit          ← START (highest ROI, unblocks truth)
   │
   ├─► M258 Scale+Acquisition (expand 10–20 loops + gold PDFs)
   │      │
   │      ├─► M259 multi_root hygiene (can parallel late M258)
   │      │
   │      └─► M260 Relation + metric matrix
   │               │
   │               └─► M261 LLM only if delta>0 (else skip)
   │                        │
   └─► M262 Structure package (needs scale+quality inputs)
            │
            └─► M263 Import pilot (user go) → E agents
```

**Parallelism allowed:**
- M259 after first M258 expand (needs stable body roots).  
- M260 S01 matrix can start during M258 if n=20 accepted as pilot.  
- Stamp CI (M257 S05) anytime.

**Forbidden parallelism:**
- GEPA with M257/M258.  
- Import with any open A/B defect.  
- Gold invent without PDF/body.

---

## 4. Optimization priorities (cost vs value)

| Rank | Work | Cost | Value | Why |
|------|------|------|-------|-----|
| 1 | M257 glue cockpit | S | H | Stops lying metrics; enables unattended expand |
| 2 | M258 gated expand loops | M | H | Coverage 21% → useful corpus |
| 3 | Gold PDF ingest (3 IDs) | S–M | H | Unblocks honest gold growth |
| 4 | M260 relation path | M | H | Relation 0.35 is main quality hole |
| 5 | M259 multi_root hygiene | S | M | Clarity, not capability |
| 6 | M261 LLM iterate | M | L–M | Only if beats header |
| 7 | M262 structure | M | H for D | Import-shaped evidence |
| 8 | GEPA/DSPy | H | L now | Unjustified |
| 9 | Import pilot | H | H later | Locked |

---

## 5. Definition of “ETL ready enough” by wave

### Wave A residual complete when
- [ ] Expand → auto pack (D-G1)  
- [ ] B gate sees A closeout (D-G2)  
- [ ] Pack dashboard v2 (D-G3)  
- [ ] Policy targets documented (D-P1)  
- [ ] Stamp CI (D-C1)  
- [ ] hybrid_fraction ≥ policy target (propose **≥0.35** interim, stretch **≥0.50**)  
- [ ] 3 gold PDFs ingested or explicitly blocked with ticket  
- [ ] multi_root divergent remains 0; identical debt tagged or reduced  
- [ ] import-hold still 0 hits  

### Wave B pilot complete when
- [ ] Ship-gate matrix artifact single source of truth  
- [ ] Relation F1 improved vs 0.35 **or** accepted with documented ceiling  
- [ ] Grounding 1.0 on all gold  
- [ ] LLM path either beats header or permanently header-default  
- [ ] No GEPA without delta>0  

### Import still **not** ready until Wave C+D checklist green.

---

## 6. Immediate next execution queue (thin)

1. **M257-S01** post-expand continuity hook  
2. **M257-S02** Wave B gate ← closeout  
3. **M257-S03** pack dashboard v2  
4. **M257-S04** closeout policy (min_hybrid_found / fraction)  
5. **M257-S05** stamp CI  
6. **M258-S01** expand ×2–3 (limit 10–20) under gate  
7. **M258-S02** gold PDF download+ingest (3 IDs)  
8. Then M260 relation matrix — **not** LLM-first  

---

## 7. Operator commands (current truth surface)

```bash
# A cockpit
uv run python scripts/verify_etl_continuity_pack.py
uv run python scripts/verify_etl_body_coverage.py
uv run python scripts/verify_etl_hybrid_missing_pdf_readiness.py
uv run python scripts/verify_etl_preprocess_fleet.py
uv run python scripts/verify_wave_a_closeout.py
uv run python scripts/verify_import_hold_inventory.py

# Expand (fail-closed without flags)
uv run python scripts/verify_hybrid_expand_batch.py --limit 0
# Live only when intentional:
# uv run python scripts/verify_hybrid_expand_batch.py --limit 10 --enable-live-hybrid

# B quality
uv run python scripts/verify_wave_b_gate.py
uv run python scripts/verify_wave_b_constrained_select.py --mode header
uv run python scripts/verify_wave_b_gold_body_grounding_audit.py
uv run python scripts/verify_wave_b_gold_hybrid_metrics.py
```

---

## 8. Non-goals (explicit)

- Opening `import_eligible` to “finish ETL”  
- Free-form LLM invent as quality path  
- GEPA before constrained LLM > header  
- Treating multi_root identical as content corruption  
- Treating PDF readiness as auto-batch authorization  
- Rewriting `human_go.json` timestamps without force re-auth  

---

*Artifact path: `artifacts/etl/ETL-READINESS-MATRIX-ROADMAP.md`*  
*Live numbers re-check before each milestone start; re-run continuity pack after every expand.*
