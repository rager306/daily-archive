# ETL Readiness Matrix + Dependency Roadmap (post val-aware GEPA / M266 / M267)

**As of:** 2026-07-24 (live recheck)  
**Import:** **locked** (D127) — no user go.  
**Deploy extract path:** `header_priority_constrained_select` (D128).  
**GEPA:** staged offline only; promote requires dual F1 > header **and** val_gap ≤ 0.35.

---

## 0. Verdict (plain)

| Layer | Status | One line |
|-------|--------|----------|
| **A data readiness** | **mostly green** | hybrid 81/230 = **0.3522 ≥ 0.35**; closeout closed; expand gated; pack/fleet ok |
| **B extraction quality** | **yellow** | header deploy; relation weak; LLM/GEPA do not beat header under promote rules |
| **C structure** | **yellow** | seams present; **continuous chunk quality not gated** |
| **D import** | **red by policy** | hold hits 0, but **intentionally locked** |
| **Glue** | **green with debt** | fleet + pack + matrix + hygiene exist; **stale n=20 vs n=23 metrics** still mix |

**Overall ETL readiness for import:** **not ready** (correct).  
**Overall ETL readiness for continued hybrid scale + quality work:** **ready**.

---

## 1. Live baseline (evidence)

| Metric | Live value | Source |
|--------|------------|--------|
| hybrid_found / unique | **81 / 81** | continuity pack |
| hybrid_fraction | **0.3522** (≥ residual 0.35) | pack dashboard |
| hybrid_missing | 149 | pack |
| missing_with_local_pdf | **135** | pack / pdf queue |
| missing_without_pdf | **14** | pack |
| expand_ready_frac | 0.906 | pack |
| expand_gate (default) | blocked (`live_hybrid_not_enabled`, limit 0) | pack expand_gate |
| multi_root ids | 20 | pack / hygiene |
| multi_root identical / divergent | **20 / 0** | pack |
| multi_root hardlinks applied | **40** (same_inode 20) | multi-root-hygiene-applied |
| preprocess bodies / errors | 81 / 0 | pack |
| preprocess quality | ok **55**, soft_signal **26** (~32%) | pack |
| closeout | `wave_a_closed` | pack |
| import-hold hits | **0** | import-hold inventory |
| import_eligible | **false** | all operators |
| gold PDF / gold hybrid | **3/3 / 3/3** | gold-pdf + hybrid-gold3 |
| joined gold-hybrid n | **23** (live compare) | gepa-vs-header |
| header n=23 | entity **0.50** / rel **0.26** | gepa-vs-header-n23* |
| header n=20 (matrix disk) | entity **0.675** / rel **0.35** | ship-gate-matrix skip-live |
| floor/oracle | **1.0 / 1.0** | matrix (not deploy) |
| LLM compact+prefer n=20 | **0.625 / 0.30** (δ −0.05/−0.05 vs n=20 header) | constrained-select-header-vs-llm |
| GEPA overfit n=23 | **0.753 / 0.409** (val_gap **0.85**) | gepa-vs-header-n23 |
| GEPA val-aware n=23 | **0.414 / 0.074** (val_gap **0.23**, loses header) | gepa-vs-header-n23-valaware |
| ship_path | **header_priority_constrained_select** | matrix / fleet |
| gepa_justified | **false** | matrix |
| structure_signal | **partial** | structure-readiness |
| fleet status | **ok**, alerts 0 | fleet-report |
| GROBID / ODL | available (when probed) | expand_gate / gold batch |

---

## 2. Wave map

```
A data readiness ──► B extraction quality ──► C structure ──► D import ──► E agents
  pack/fleet/expand      header deploy           partial          LOCKED
  residual ≥0.35 OK      GEPA staged offline     chunk gate gap   (user go)
  multi_root hardlink    relation ceiling weak
  gold hybrid 3/3        n=20 vs n=23 metric debt
```

---

## 3. Milestone delivery audit

| Roadmap ID | GSD ID | Function | Status | Residual after |
|------------|--------|----------|--------|----------------|
| M257 | 257-ypm1nw | Glue cockpit (pack, expand refresh flag, closeout, stamp) | **done** | refresh was opt-in → fixed in M266 |
| M258 | 258-pz9x0t | Scale expand + gold PDF + join leak | **done** | fraction was &lt;0.35 → M264 |
| M259 | 259-rw8vqv | Multi-root inventory/map | **done** | identical×20 storage → M267 hardlink |
| M260 | 260-p0q1ao | Ship matrix + relation ceiling | **done** | relation still ceiling |
| M261 | 261-y4ckwi | GEPA skip snapshot (D126) | **done** | revised by **D128** |
| M262 | 262-cupppn | Structure readiness package | **done** | signal partial |
| M263 | 263-h5yqww | Import hold | **done** | locked correct |
| M264 | 264-o7zle4 | Residual hybrid ≥0.35 | **done** | scale optional beyond 0.35 |
| M265 | 265-1uoo3t | Gold hybrid 3 PDF | **done** | joined n 20→23 quality drop header |
| M266 | **269-rqyrh1** | Fleet defaults + pack refresh default ON | **done** | fleet does not auto-expand |
| M267 | **270-xfaaq1** | Multi-root hardlink hygiene | **done** | path multi_root count still 20 |
| M268 | 268-oqi2vv | Relation + staged GEPA compare | **done** | no promote; relation weak |
| G1 val-aware | (code in spike) | Val-aware offline GEPA | **done** | gap fixed; F1 &lt; header |
| M269 structure continuous | **not planned as next GSD** | Continuous chunk quality gate | **open** | structure partial |
| M270 import | user-go only | Falkor/import pilot | **blocked** | D127 |

> Note: roadmap labels M266/M267 ≠ GSD numeric IDs (GSD used M269/M270). Treat roadmap labels as functional IDs.

---

## 4. Defect → function → module matrix

### 4.1 Closed / mitigated

| Def | Class | Defect | Fix | Modules / operators | Algorithms / ideas | Glue |
|-----|-------|--------|-----|---------------------|--------------------|------|
| D-G0 | glue | no single dashboard | M257 pack | `etl_continuity_pack`, `verify_etl_continuity_pack` | compose coverage+hold+closeout | pack is cockpit |
| D-G1 | glue | expand without pack refresh | M257 flag → **M266 default ON** | `verify_hybrid_expand_batch` BooleanOptionalAction | post-batch pack write | expand→pack |
| D-G7 | glue | many operators no fleet entry | **M266** | `etl_fleet`, `verify_etl_fleet` | pack+matrix+hold compose | fleet |
| D-X1 | scale | hybrid 0.28 | M258+M264 | `hybrid_expand_preflight`, expand batch, DEFAULT_BODY_ROOTS | gated live hybrid + GROBID/ODL | expand→coverage |
| D-Q4b | gold | gold PDF no body | M265 | gold selection, hybrid batch | hybrid_route body evidence | gold→join |
| D-S1a | storage | multi_root false divergent | M259 | `inventory_multi_root_hybrid_copies` | SHA identical vs divergent | coverage |
| D-S1b | storage | identical×20 disk waste | **M267** | `multi_root_hygiene` hardlink | primary root order + hardlink | hygiene |
| D-Q0 | quality | free invent | constrained select | `wave_b_constrained_select`, pilot | candidate_id only | prefer-header |
| D-Q6 | quality | GEPA hard-forbid | D128 | spike + ship matrix | staged offline→promote | matrix |
| D-Q7 | quality | train-only GEPA overfit | val-aware spike | `offline_reflective_spike` | min_support, max_new_hints, val composite | spike→compare |
| D-L1 | lock | import risk | M263/D127 | import-hold inventory, pre-commit | fail-closed True assignment scan | hold |
| D-J1 | join | original.hybrid.body leak | M258 join fix | multi_root map parent paper_id | path alias | join |

### 4.2 Open / yellow / red

| Def | Class | Defect (live) | Severity | Function needed | Module(s) | Links / algorithms | Depends | Wave |
|-----|-------|---------------|----------|-----------------|-----------|--------------------|---------|------|
| **D-M1** | metric glue | **n=20 vs n=23 header/LLM/GEPA/matrix mix**; skip-live matrix shows n=20 header 0.675 while live n=23 is 0.50 | **high** | single joined-n contract on all quality artifacts | `wave_b_ship_gate_matrix`, grounding audit, LLM compare, fleet | same-n guard already partial; need **default live rescore** + refuse mixed n | M260, M268 | **M271 glue** |
| **D-M2** | metric glue | grounding audit frozen at **n=20** after gold3 | med | re-run grounding on full join | `wave_b_gold_body_grounding_audit` | body/cand coverage | M265 | M271 |
| **D-Q1** | quality | relation F1 **0.26–0.35** header ceiling; floor 1.0 not deploy | **high** | relation candidate builder (typed edges among selected entities, not invent) | constrained select + statistical extraction | co-occurrence → typed link candidates; GEPA relation hints only if val-aware | M268 | **M272 quality** |
| **D-Q2** | quality | LLM &lt; header (n=20); llm_kept 1/20 | med | better compact LLM + prefer-header; same-n rescore | constrained LLM pilot | MiniMax/9router free path | M260 | M272 |
| **D-Q3** | quality | soft_signal **26/81 (~32%)** | med | profile repair / body_quality triage (non-gating) | body_quality, preprocess rollup | soft_signal taxonomy | pack | M273 hygiene |
| **D-Q5** | quality | YAKE off in fleet path | low | optional non-gating YAKE inject | yake inject, preprocess | keywords as spans only | preprocess | optional |
| **D-Q8** | quality | val-aware GEPA gap OK but **F1 &lt; header** | med | type priors / cross-paper hints, not paper-id flood; or stop GEPA until candidate+relation better | `wave_b_gepa_constrained_spike` | reflective ASI with min_support≥2 + type priors | G1, M268 | M272 |
| **D-Q9** | quality | header drop n=20→n=23 after gold join | med | diagnose 3 gold cases vs header heuristic; optional gold-aware select tests | header_priority_select, gold join | do **not** gold-overfit | M265 | M272 |
| **D-C2** | structure | continuous chunk quality **not gated** | **high** for Wave C | structure continuous gate on real corpus | structure_readiness + chunk quality | M209 seams exist | M262 | **M273 structure** |
| **D-C3** | structure | structure package can blank hybrid if --skip-etl | low | default full compose (already default; document) | verify_structure_readiness_package | pack bridge | M262/M266 | docs |
| **D-S2** | storage | multi_root **path count still 20** after hardlink | low | optional path prune / primary-only index (explicit flag) | multi_root_hygiene | hardlink done; delete non-primary only with flag | M267 | optional |
| **D-X2** | scale | 135 PDF ready idle (beyond residual) | low | gated expand batches (refresh default on) | expand batch gate | limit + enable-live-hybrid | M264/M266 | optional scale |
| **D-G8** | glue | fleet does not refresh matrix with live n=23 by default | med | fleet flag `--rescore-quality` | verify_etl_fleet | call ship matrix live | M266 | M271 |
| **D-G9** | docs | roadmap artifact lag (was hybrid 64) | med | refresh roadmap at each residual close | this file | pipeline continuity audit | convention | now |
| **D-L1** | lock | import closed | policy | user go only | pilot_write, import boundary | D127 | — | **M274 import** only on yes |

---

## 5. Function × module adjacency (current)

```
catalog ─► coverage ─► readiness ─► closeout ─► continuity_pack ─► etl_fleet
              │              │                      │                  │
              │              └─ expand_gate ─► hybrid_expand_batch ────┤
              │                         │  (refresh pack default ON)   │
              └─ multi_root inventory ─► multi_root_hygiene (hardlink) ┤
                                                                       │
gold fixtures ─► gold hybrid join ─► constrained select ─► evaluate_records
                      │                    │ header deploy
                      │                    ├─ LLM compare (n-debt)
                      │                    └─ GEPA offline (val-aware / overfit)
                      └─ ship_gate_matrix ◄── fleet
structure seams (M209) ─► structure_readiness (partial) ─/─ continuous chunk gate
import-hold inventory ─► pre-commit ─► import LOCKED
```

**Onion:** application pure for pack/select/GEPA/hygiene; workflows compose sidecars/DEFAULT_BODY_ROOTS; no import_eligible=True.

---

## 6. Not done / not glued / missed

### Not done (by design or open)
1. Import / Falkor write  
2. GEPA promote to deploy  
3. LLM beat header under same-n dual F1  
4. Relation quality above header co-occurrence ceiling  
5. Continuous structure/chunk quality gate  
6. YAKE default on  
7. Hybrid fraction stretch 0.50  
8. Drop multi_root secondary paths (only hardlink done)

### Not glued (process debt)
1. **Quality n-contract:** matrix/LLM/grounding often **n=20**; gold join **n=23** — different truths  
2. Fleet composes **disk** matrix; does not force live rescore  
3. Structure readiness vs pack: optional skip blanks hybrid (default ok, CI not forced)  
4. Expand gate default blocked — correct, but unattended scale still needs explicit enable  
5. Multiple GEPA artifacts (overfit vs val-aware) — operator must pin artifact path  

### Missed / under-specified earlier
1. Residual 0.35 met **does not** mean quality ready  
2. Gold hybrid join **lowered** header F1 — not tracked as first-class defect until recheck  
3. Hardlink hygiene does not change multi_root **count** metric (need same_inode metric in pack)  
4. Roadmap doc not auto-refreshed after M264/M266/M267  

---

## 7. Optimization opportunities (safe)

| Area | Opportunity | Risk if wrong |
|------|-------------|----------------|
| Metrics | Canonical `joined_n` + live rescore in fleet | Stale vanity metrics / false promote |
| GEPA | Stop paper-id TYPE_HINT flood; priors + relation candidates | Overfit return |
| Relation | Candidate edges from entity pairs + section windows | Free invent if unconstrained |
| Storage | Pack dashboard field `multi_root_same_inode_count` | Misleading “fixed” multi_root |
| Scale | Batch expand 135 PDF with pack refresh default | Sidecar load; still no import |
| Preprocess | soft_signal triage top buckets | Noise chasing |
| Structure | Gate chunk quality on hybrid bodies sample | False red if threshold wrong |

---

## 8. Roadmap (dependency-ordered waves)

### Wave G — Glue truth (do first)
**M271 — Quality n-contract + fleet rescore**  
- Goal: one joined_n for header/LLM/GEPA/grounding/matrix; fleet `--rescore-quality` default or documented.  
- Depends: M260, M266, M268.  
- Demo: matrix joined_count == gepa-vs-header joined_count; no n=20/n=23 silent mix.  
- Risk: med. Closes D-M1, D-M2, D-G8, D-G9.

### Wave Q — Extraction quality
**M272 — Relation candidates + honest same-n LLM/GEPA**  
- Goal: relation path above co-occurrence without invent; optional LLM/GEPA only if dual F1 > header and val_gap ok.  
- Depends: M271 (honest n), M268, val-aware spike.  
- Sub-slices:  
  - S01 relation candidate builder + metrics  
  - S02 same-n LLM rescore  
  - S03 GEPA only if relation+entity improve under val_aware  
- Closes D-Q1, D-Q2, D-Q8, D-Q9.

### Wave H — Hygiene / preprocess (parallel with Q after G)
**M273a — soft_signal triage** (optional thin)  
- Depends: pack. Non-gating. Closes D-Q3.

**M273b — Structure continuous chunk gate**  
- Depends: M262, hybrid bodies. Closes D-C2.  
- Proof: structure_signal improves or explicit residual list.

### Wave S — Optional scale
**M274-scale — expand toward 0.50** only if ops want  
- Depends: M266 refresh default, sidecars. Not quality-blocking.

### Wave I — Import (hard gate)
**M275 — Import pilot**  
- Depends: user **yes** + Wave G green + quality floor agreed + structure not red.  
- D127 non-bypassable.

---

## 9. Recommended execution order (thin slices)

1. **M271 S01** — pin joined_n; live rescore matrix default path; refresh grounding n=23  
2. **M271 S02** — fleet wires rescore; pack field `multi_root_same_inode_count`  
3. **M272 S01** — relation candidates (no invent)  
4. **M272 S02** — same-n LLM compare  
5. **M273b** — structure continuous gate  
6. Optional: soft_signal, scale, YAKE  
7. Import **only** with explicit go  

**Do not** open import to “finish” ETL.  
**Do not** promote GEPA while dual F1 ≤ header or val_gap bad.  
**Do not** treat residual 0.35 as quality done.

---

## 10. Definition of “ETL ready for import” (checklist)

- [x] hybrid residual ≥ 0.35 (or waived)  
- [x] import-hold hits 0  
- [x] pack/fleet/closeout green  
- [ ] **same-n quality contract** (M271)  
- [ ] deploy path dual F1 agreed (header or promoted winner)  
- [ ] relation not only co-occurrence toy ceiling **or** accepted ceiling documented  
- [ ] structure continuous gate not red  
- [ ] grounding n matches join  
- [ ] explicit **user yes** for import  

**Current:** 4/9 process boxes green; quality/structure/user-go open → **not import-ready**.

---

## 11b. Next strategic wave (evidence + verification)

See **`artifacts/etl/EVIDENCE-TRACE-AND-VERIFICATION-ROADMAP.md`**.

Binding gap: reversible evidence (ODL JSON + raw TEI + CanonicalDocument + page/bbox),
then ARS-shaped verification (intent, constraints, risk audit) before any import.
Do not rewrite ETL; do not open import without user go.

## 11. Operator quick check

```bash
uv run python scripts/verify_etl_fleet.py
uv run python scripts/verify_etl_continuity_pack.py
uv run python scripts/verify_wave_b_ship_gate_matrix.py   # prefer live, not only --skip-live-score
uv run python scripts/verify_wave_b_gepa_vs_header.py \
  --gepa-artifact artifacts/wave-b/gepa-constrained-spike-n23-valaware-incr.json
uv run python scripts/verify_multi_root_hygiene.py
uv run python scripts/verify_structure_readiness_package.py
uv run python scripts/verify_import_hold_inventory.py
```
