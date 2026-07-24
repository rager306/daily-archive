# ETL Readiness Matrix + Dependency Roadmap (GEPA-aware)

**As of:** 2026-07-24 (post M257–M263 + **D128 staged GEPA**)  
**Execution status:** M257–M263 complete. Import **locked** (D127).  
**GEPA policy (D128 revises D126 hard-stop):** gradual staged enablement under Wave B stamp; **deploy path stays header** until GEPA-improved constrained select beats header on **entity and relation F1**.  
**Hard locks:** `import_eligible=false`; no free invent; no Falkor write without explicit user go.  
**Soft locks:** production *deploy* = `header_priority` while `llm_beats_header=false`; offline/staged GEPA spikes **in-scope**.

## Live baseline

| Metric | Value |
|--------|-------|
| hybrid_found / unique | **64 / 64** (0.2783) of 230 |
| residual target | **0.35** not met (stretch 0.50) |
| PDF ready idle | **152**; without PDF **14** |
| multi_root | identical **20**, divergent **0** |
| preprocess | 64 bodies, 0 err, soft_signal **19**, yake=false |
| closeout | wave_a_closed (min=49) |
| header n=20 | entity **0.675** / relation **0.35** |
| floor/oracle | **1.0 / 1.0** |
| LLM compact+prefer | **0.625 / 0.30** (δ −0.05/−0.05) |
| ship_path | header_priority; ship_ready true; gepa_justified false |
| gold PDF / hybrid | **3/3** / **0/3** |
| structure_signal | partial |
| import-hold | pass, hits=0 |

---

## 0. Wave map + GEPA lane

```
A data readiness ──► B extraction quality ──► C structure ──► D import ──► E agents
         │                    │  ▲                  │              │
         │                    │  │ staged GEPA      │              └─ D127 user go
         │                    │  │ (offline→live→   │
         │                    │  │  promote-if-wins)│
         │                    └── header deploy ────┘
         └─ pack/gate/expand (import false)
```

**GEPA optimizes (existing spike contract):** instruction text / constrained select rules — **not** weights, **not** candidate coverage, **not** import.

---

## 1. Master matrix

### 1.1 Done residual (M257–M263)

| ID | Function | Modules / operators | Links / algorithms | Status | Residual |
|----|----------|---------------------|--------------------|--------|----------|
| M257 | Glue cockpit | continuity_pack, expand `--refresh-continuity-pack`, B gate closeout default, closeout residual policy, stamp immutability | expand→pack opt-in; B←A | done | refresh not default |
| M258 | Scale + gold PDF + join | expand×15, gold_pdf_acquisition, join leak | original.hybrid.body.md→parent | done | fraction&lt;0.35; gold hybrid 0 |
| M259 | Multi-root map | inventory_multi_root | SHA identical/divergent | done | identical×20 debt |
| M260 | Ship matrix | wave_b_ship_gate_matrix, header select | floor/header/baseline/LLM worlds | done | relation 0.35 |
| M261 | GEPA skip snapshot | D126 historical | recorded LLM&lt;header | done | **revised by D128** |
| M262 | Structure package | structure_readiness_package | M209 structure + ETL | done | signal partial |
| M263 | Import hold | D127 + import-hold | pilot≠import | done | locked correct |

### 1.2 Defect → fix wave

| Def | Class | Defect | Fix wave | Fix type |
|-----|-------|--------|----------|----------|
| D-X1 | scale | hybrid 0.278; 152 PDF idle | **M264** | gated expand + pack refresh |
| D-Q4b | gold | 3 PDF, 0 hybrid body | **M265** | gold-only hybrid batch |
| D-G1b | glue | expand refresh opt-in | **M266** | default refresh / fleet |
| D-G6 | glue | structure --skip-etl blanks hybrid | **M266** | default full compose |
| D-G7 | glue | many operators, no fleet entry | **M266** | verify_etl_fleet |
| D-S1b | storage | multi_root identical×20 | **M267** | hardlink/primary root |
| D-S3 | naming | selection-40 + original.body names | **M267** | alias/rename policy |
| D-Q1 | quality | relation F1 0.35 | **M268** | relation candidates + GEPA |
| D-Q2 | quality | LLM &lt; header | **M268** | staged GEPA→LLM | 
| D-Q3 | quality | soft_signal ~30% | M268 optional | profile repair |
| D-Q5 | quality | YAKE off | M268 optional | non-gating |
| D-C2 | structure | structure partial | **M269** | continuous chunk gate |
| D-L1 | lock | import closed | **M270** | user go only |

### 1.3 Function × module adjacency

```
catalog ─► coverage ─► readiness ─► closeout ─► continuity_pack
selection ─► preflight ─► expand_gate ─► hybrid_batch ─► bodies ─► preprocess
gold PDF acquisition ─► source/<id>.pdf ─✗─ hybrid body (3 gold)
stamp ─► wave_b_gate(+closeout) ─► header_priority_select ─► ship_matrix
                              └─► make_llm_constrained_select_fn (+prefer-header)
                              └─► wave_b_gepa_constrained_spike (offline/live gepa)
M209 structure ─► structure_readiness ◄─ etl pack
import_hold ─► all fail-closed
```

---

## 2. GEPA insertion map (where optimization fits)

Policy: **run early and often under stamp**; **promote late** (only if beats header).

| # | Insertion site | Module / surface | What GEPA optimizes | When | Depends | Promote gate | Risk if misused |
|---|----------------|------------------|---------------------|------|---------|--------------|-----------------|
| **G1** | Offline reflective spike | `wave_b_gepa_constrained_spike` / `offline_reflective_spike` | instruction hints, select_max, type/relation hints | **Now (n=20)** | stamp D124, gold join | never auto-deploy | invent if unconstrained — keep candidate_id only |
| **G2** | Live `gepa.optimize` (optional pkg) | `try_gepa_optimize` + `WaveBConstrainedGEPAAdapter` | same instruction program | after G1 stable | gepa package optional | ship matrix δ&gt;0 | cost/timeout; no import |
| **G3** | Constrained LLM prompt | `render_constrained_select_prompt` + `make_llm_constrained_select_fn` | prompt text / ranking prefs from GEPA instruction | after G1/G2 | 9router models | prefer-header until δ&gt;0 | weak LLM without prefer-header |
| **G4** | Prefer-header envelope | `make_header_prefer_select_fn` / `score_selection_structural` | margin / structural arbiter (not GEPA core, but safety shell) | always with G3 | header_priority | always on until promote | over-fallback hides GEPA learning |
| **G5** | Relation-focused instruction | relation hints in GEPA instruction + `ALLOWED_RELATION_TYPES` | which typed links among top-k entities | **M268 S01–S02** | header relation baseline 0.35 | relation F1 &gt; header | free invent types — forbid |
| **G6** | Post-gold-hybrid re-fit | same spike on n=20+3 | re-run G1–G3 on expanded gold | **after M265** | gold hybrid bodies | re-matrix | premature fit on n=20 only |
| **G7** | Ship matrix promotion | `build_wave_b_ship_gate_matrix` | reads deltas; sets gepa_justified / ship_path | continuous | header+GEPA metrics | entity **and** relation F1 &gt; header | promoting on entity-only |
| **G8** | Soft preprocess (optional) | preprocess profile / YAKE non-gating | **not** primary GEPA target; ablate only | M268 optional | fleet metrics | never opens import | GEPA on preprocess is low ROI now |
| **G9** | Expand/selection policy | selection expand round-robin | **out of GEPA scope** (ops/scale) | M264 | PDF readiness | n/a | don't GEPA-ize batch sizing |
| **G10** | Structure/chunk gate | structure_readiness | **out of GEPA scope** (deterministic quality) | M269 | hybrid scale | n/a | optimizers ≠ structure truth |
| **G11** | Import/Falkor | pilot write | **forbidden for GEPA** | M270 | user go | n/a | never |

### GEPA stage ladder (D128)

```
Stage 0  header_priority_select          ← current deploy (ship_path)
Stage 1  offline_reflective_spike        ← G1 now
Stage 2  optional gepa.optimize package  ← G2 when installed
Stage 3  constrained LLM + GEPA instr.   ← G3 + G4 shell
Stage 4  relation-specialized GEPA       ← G5 in M268
Stage 5  re-fit on gold hybrid expand    ← G6 after M265
Stage 6  PROMOTE ship_path if δ>0 both   ← G7 matrix
```

**Do not skip Stage 0 deploy** until Stage 6 proof.

### Where GEPA does **not** belong (optimize differently)

| Area | Why not GEPA | Better tool |
|------|--------------|-------------|
| Hybrid expand throughput | infra/sidecars | M264 batch gate + limit |
| PDF acquisition | network/catalog | M265 ingest/download |
| multi_root identical | storage | M267 hardlink |
| import eligibility | governance | D127 human go |
| body grounding | hard constraint | audit gate stay 1.0 |
| candidate generation coverage | lexical/header inventory | improve candidates before GEPA |

**Rule:** GEPA cannot fix missing hybrid bodies or missing candidates — fix coverage first, then optimize select/instruction.

---

## 3. Dependency-ordered roadmap

### Critical path

```
M264 Residual expand → fraction ≥ 0.35
   │
   ├─► M265 Gold hybrid (3 PDF) ──────────────┐
   │         │                                │
   │         └─ G1/G2 early GEPA spike        │
   │                                          │
   └─► M266 Fleet glue defaults               │
            │                                 │
            ├─► M267 Storage/naming hygiene   │
            │                                 │
            └─► M268 Relation + staged GEPA ◄─┘
                     G1→G5→G3→G7 promote-if-wins
                     │
                     └─► M269 Structure continuous gate
                              │
                              └─► M270 Import  [USER GO]
```

**Parallel:** M264 ∥ M265; **G1 GEPA offline can start immediately** on n=20 (does not wait for M264).  
**Serial for promote:** M265 recommended before Stage 6 promotion (avoid overfit n=20).

### Milestone specs

#### M264 — Residual hybrid scale
- **Goal:** hybrid_fraction ≥ 0.35  
- **Modules:** expand_batch_gate, hybrid_batch, continuity_pack refresh  
- **GEPA:** not required (G9 out of scope)  
- **Success:** hybrid_found ≳ 81; pack residual alert cleared; import false  

#### M265 — Gold hybrid bodies
- **Goal:** hybrid for 3 gold PDFs; re-ground; matrix  
- **Modules:** hybrid_batch on gold selection; grounding audit; ship matrix  
- **GEPA:** unlocks G6 re-fit  
- **Success:** 3/3 bodies; grounding 1.0; import false  

#### M266 — Unattended fleet glue
- **Goal:** expand default pack refresh; one fleet entrypoint  
- **Modules:** verify_hybrid_expand_batch, continuity_pack, optional stamp hook  
- **GEPA:** none  
- **Success:** one command A/B/C read-only green  

#### M267 — Storage/naming hygiene
- **Goal:** identical multi_root debt plan; naming  
- **GEPA:** none  

#### M268 — Relation + staged GEPA (quality spine)
- **Goal:** lift relation/entity under constrained select via gradual GEPA  
- **Modules:**  
  - `wave_b_gepa_constrained_spike` (G1/G2)  
  - `wave_b_constrained_select` LLM+prefer (G3/G4)  
  - relation candidates (G5)  
  - `wave_b_ship_gate_matrix` (G7)  
- **Slices:**  
  - S01 Relation candidates vs header  
  - S02 Offline GEPA reflective spike (n=20, then post-M265)  
  - S03 Live GEPA/LLM vs header; promote or keep header  
- **Success:** either ship_path promote with δ&gt;0 both F1, or documented plateau with header deploy  

#### M269 — Structure continuous gate
- **Goal:** structure_signal not stuck on known_gaps without evidence path  
- **GEPA:** none (G10)  

#### M270 — Import pilot
- **Requires:** user yes + A residual + B promote-or-accept + C structure  
- **GEPA:** never authorizes import (G11)  

---

## 4. Optimization ROI (with GEPA)

| Rank | Work | GEPA? | Cost | Value | Why |
|------|------|-------|------|-------|-----|
| 1 | M265 gold hybrid | unlocks G6 | S–M | **H** | PDF ready; honest gold growth |
| 2 | G1 offline GEPA spike now | **yes** | S | **H** | cheap learning signal on n=20 |
| 3 | M264 expand to 0.35 | no | M | **H** | coverage truth |
| 4 | M266 fleet defaults | no | S | **H** | unattended truth |
| 5 | M268 relation + G2–G5 GEPA | **yes** | M | **H** | main quality hole + user policy |
| 6 | M267 multi_root identical | no | S | M | storage clarity |
| 7 | M269 structure gate | no | M | H for D | import-shaped |
| 8 | Promote GEPA deploy | only if δ&gt;0 | S | H if wins | ship_path switch |
| 9 | Import pilot | no | H | later | D127 |

---

## 5. Glue checklist (eliminate remaining seams)

| Seam | Status | Next |
|------|--------|------|
| expand → pack | opt-in flag | M266 default on |
| expand → preprocess | manual | M266 fleet |
| B gate → closeout | **done** default | keep |
| gold PDF → hybrid | **open** | M265 |
| GEPA spike → ship matrix | partial (manual) | M268 wire δ into matrix |
| structure ↔ ETL pack | works unless --skip-etl | M266 default full |
| multi_root identical | tagged only | M267 |
| import | locked | M270 user go |

---

## 6. Definition of ready enough

### A residual
- [x] cockpit, gate, stamp, join, multi_root map  
- [ ] fraction ≥ 0.35 (M264)  
- [ ] fleet defaults (M266)  

### B quality
- [x] ship matrix + header deploy  
- [x] D128 staged GEPA allowed  
- [ ] G1 spike current n=20 artifact fresh  
- [ ] 3 gold hybrid (M265)  
- [ ] M268 promote **or** accept header ceiling with GEPA plateau evidence  

### C structure
- [x] package exists  
- [ ] continuous gate (M269)  

### D import
- [ ] user go + above  

---

## 7. Immediate queue (execute order)

1. **G1 now:** `verify_wave_b_gepa_constrained_spike` offline on n=20 → artifact  
2. **M265:** hybrid 3 gold IDs → ground → matrix  
3. **M264:** expand loops to fraction ≥ 0.35 with pack refresh  
4. **M266:** fleet defaults  
5. **M268:** relation candidates + G2/G3 staged GEPA → compare header → promote if wins  
6. M267 / M269 as capacity allows  
7. **M270** only with explicit user yes  

### Operators

```bash
# A
uv run python scripts/verify_etl_continuity_pack.py
uv run python scripts/verify_hybrid_expand_batch.py --limit 0
# B + GEPA
uv run python scripts/verify_wave_b_ship_gate_matrix.py
uv run python scripts/verify_wave_b_gepa_constrained_spike.py
uv run python scripts/verify_wave_b_constrained_select.py --mode header
# C / safety
uv run python scripts/verify_structure_readiness_package.py
uv run python scripts/verify_import_hold_inventory.py
```

---

## 8. Binding decisions

| ID | Choice |
|----|--------|
| D124 | Wave B human_go stamp |
| D126 | Historical: GEPA closed when LLM&lt;header (M261 snapshot) |
| **D128** | **Staged GEPA in-scope; deploy=header until δ&gt;0 both F1** |
| D127 | Import held without user go |

---

## 9. Non-goals

- Free invent entities/relations  
- Auto-promote GEPA to ship_path without positive dual F1 delta  
- GEPA on expand sizing / import / Falkor  
- Claiming residual complete while fraction &lt; 0.35  
- Opening import without user go  

---

*Artifact: `artifacts/etl/ETL-READINESS-MATRIX-ROADMAP.md`*  
*Handoff: `.gsd/continue.md`*  
*Memory: D128 staged GEPA; G1 can run before scale complete; promote only via ship matrix.*
