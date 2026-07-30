# Domain Reference - arXiv Taxonomy + Extension Packs

**Status:** Design (ADR-043) — synced with official arXiv taxonomy (154 categories, verified 2026-07-29)
**Date:** 2026-07-29
**Source:** https://arxiv.org/category_taxonomy
**Purpose:** Canonical *scientific_domain* codes for multi-domain ontology.
**Not:** document genre (`source_profile`). Not extraction whitelist.

---

## 1. Two independent axes

| Axis | Field | Meaning | Example |
|------|-------|---------|---------|
| Document genre | `source_profile` | how the artifact is shaped/parsed | `paper`, `textbook`, `protocol` |
| Knowledge domain | `scientific_domain[]` | what field the knowledge belongs to | `cs.LG`, `q-bio.QM`, `da.medicine` |

Current seed corpus is mostly `cs.LG` / GNN-adjacent papers. That is **coverage**, not an ontology limit.

`Source.domain` in today's schema means *genre* (`scientific_paper` / `textbook`).
Do **not** overload it with arXiv categories. Prefer:

```text
Work.source_profile = "paper"
Work.scientific_domains = ["cs.LG", "cs.AI"]
Work.primary_scientific_domain = "cs.LG"   # optional denorm of arXiv primary
```

---

## 2. Coding rules

1. **Prefer official arXiv category codes** when the work is arXiv-native or mappable.
2. Use **exact arXiv spelling**: `cs.LG` (Learning), not `cs.ml` / `cs.ML`.
3. Alias map may accept user/input variants → canonical code.
4. Non-arXiv domains use extension namespace: `da.<domain>` (daily-archive pack id).
5. A Work may have **multiple** `scientific_domains` (ordered by relevance).
6. Domain packs live under `data/domain_packs/<code>/` (config; future).
7. Process kernel types do **not** fork per domain - only vocab/templates do.

### Alias examples

| Input / informal | Canonical |
|------------------|-----------|
| `cs.ml`, `machine-learning` | `cs.LG` |
| `nlp`, `cs.nlp` | `cs.CL` |
| `cv`, `computer-vision` | `cs.CV` |
| `gnn`, `graph-ml` | `cs.LG` (+ optional topic tag `gnn`) |
| `rl`, `reinforcement-learning` | `cs.LG` or `cs.AI` (context) |
| `biohacking` | `da.biohacking` |
| `microbiome` | `da.microbiome` |

Topics/entities still carry fine-grained labels (`GCN`, `GraphSAGE`).
Domain codes are **coarse routing + pack selection**, not entity identity.

---

## 3. arXiv category registry (canonical examples)

Full official list evolves; this table is the **working registry seed**.
Groups below cover research synthesis needs; packs can load a fuller dump later.

### 3.1 Computer Science (`cs.*`)

| Code | Name | Pack relevance |
|------|------|----------------|
| `cs.AI` | Artificial Intelligence | agents, reasoning, planning |
| `cs.LG` | Machine Learning | default ML/GNN/RL methods |
| `cs.CL` | Computation and Language | NLP, LLM, RAG |
| `cs.CV` | Computer Vision and Pattern Recognition | vision models/datasets |
| `cs.NE` | Neural and Evolutionary Computing | neuroevolution, NAS |
| `cs.RO` | Robotics | embodied agents |
| `cs.IR` | Information Retrieval | retrieval, ranking |
| `cs.MA` | Multiagent Systems | swarm/multi-agent |
| `cs.SI` | Social and Information Networks | graphs/social |
| `cs.DS` | Data Structures and Algorithms | theory/algorithms |
| `cs.CR` | Cryptography and Security | security/privacy |
| `cs.DB` | Databases | data systems |
| `cs.DC` | Distributed, Parallel, and Cluster Computing | systems scale |
| `cs.SE` | Software Engineering | tooling, code agents |
| `cs.HC` | Human-Computer Interaction | HMI, UX studies |
| `cs.CY` | Computers and Society | socio-technical |
| `cs.CE` | Computational Engineering, Finance, and Science | sci-computing bridge |
| `cs.CG` | Computational Geometry | geometry/graphics algos |
| `cs.GT` | Computer Science and Game Theory | mechanisms/games |
| `cs.IT` | Information Theory | coding/info theory |
| `cs.LO` | Logic in Computer Science | formal methods bridge |
| `cs.PL` | Programming Languages | compilers/PL |
| `cs.PF` | Performance | systems perf |
| `cs.OH` | Other Computer Science | residual |

**Seed corpus note:** current gold set is predominantly `cs.LG` / `cs.AI` / `cs.CL` with GNN textbook overlay.

### 3.2 Statistics (`stat.*`)

| Code | Name |
|------|------|
| `stat.ML` | Machine Learning |
| `stat.ME` | Methodology |
| `stat.TH` | Theory |
| `stat.AP` | Applications |
| `stat.CO` | Computation |
| `stat.OT` | Other Statistics |

### 3.3 Mathematics (`math.*`) - selected

| Code | Name |
|------|------|
| `math.AG` | Algebraic Geometry |
| `math.AT` | Algebraic Topology |
| `math.CO` | Combinatorics |
| `math.CT` | Category Theory |
| `math.DG` | Differential Geometry |
| `math.DS` | Dynamical Systems |
| `math.FA` | Functional Analysis |
| `math.GR` | Group Theory |
| `math.GT` | Geometric Topology |
| `math.LO` | Logic |
| `math.MG` | Metric Geometry |
| `math.MP` | Mathematical Physics |
| `math.NA` | Numerical Analysis |
| `math.NT` | Number Theory |
| `math.OA` | Operator Algebras |
| `math.OC` | Optimization and Control |
| `math.PR` | Probability |
| `math.QA` | Quantum Algebra |
| `math.RT` | Representation Theory |
| `math.ST` | Statistics Theory |
| `math.SG` | Symplectic Geometry |

### 3.4 Physics (selected roots + common leaves)

| Code | Name |
|------|------|
| `physics.comp-ph` | Computational Physics |
| `physics.data-an` | Data Analysis, Statistics and Probability |
| `physics.bio-ph` | Biological Physics |
| `physics.chem-ph` | Chemical Physics |
| `physics.soc-ph` | Physics and Society |
| `physics.med-ph` | Medical Physics |
| `quant-ph` | Quantum Physics |
| `cond-mat.dis-nn` | Disordered Systems and Neural Networks |
| `cond-mat.stat-mech` | Statistical Mechanics |
| `cond-mat.soft` | Soft Condensed Matter |
| `hep-th` | High Energy Physics - Theory |
| `hep-ph` | High Energy Physics - Phenomenology |
| `hep-ex` | High Energy Physics - Experiment |
| `nucl-th` | Nuclear Theory |
| `nucl-ex` | Nuclear Experiment |
| `gr-qc` | General Relativity and Quantum Cosmology |
| `astro-ph.CO` | Cosmology and Nongalactic Astrophysics |
| `astro-ph.GA` | Astrophysics of Galaxies |
| `astro-ph.HE` | High Energy Astrophysical Phenomena |
| `astro-ph.IM` | Instrumentation and Methods for Astrophysics |
| `astro-ph.SR` | Solar and Stellar Astrophysics |
| `nlin.AO` | Adaptation and Self-Organizing Systems |
| `nlin.CD` | Chaotic Dynamics |
| `nlin.PS` | Pattern Formation and Solitons |

### 3.5 Quantitative Biology (`q-bio.*`)

| Code | Name | Bridge |
|------|------|--------|
| `q-bio.BM` | Biomolecules | structural bio |
| `q-bio.CB` | Cell Behavior | cell systems |
| `q-bio.GN` | Genomics | genetics bridge |
| `q-bio.MN` | Molecular Networks | pathways |
| `q-bio.NC` | Neurons and Cognition | neuro |
| `q-bio.OT` | Other Quantitative Biology | residual |
| `q-bio.PE` | Populations and Evolution | evo/ecology |
| `q-bio.QM` | Quantitative Methods | methods |
| `q-bio.SC` | Subcellular Processes | subcellular |
| `q-bio.TO` | Tissues and Organs | physiology |

### 3.5 Quantitative Finance (`fin.*`) — replaced `q-fin.*`

arXiv renamed `q-fin.*` to `fin.*`. Legacy codes migrate via `canonicalize()`.

| Code | Name |
|------|------|
| `fin.CP` | Computational Finance |
| `fin.EC` | Economics |
| `fin.GN` | General Finance |
| `fin.MF` | Mathematical Finance |
| `fin.PM` | Portfolio Management |
| `fin.PR` | Pricing of Securities |
| `fin.RM` | Risk Management |
| `fin.ST` | Statistical Finance |
| `fin.TR` | Trading and Market Microstructure |

### 3.5b Condensed Matter (`cond-mat.*`) — 9 codes
| `eess.AS` | Audio and Speech Processing |
| `eess.IV` | Image and Video Processing |
| `eess.SP` | Signal Processing |
| `eess.SY` | Systems and Control |
| `econ.EM` | Econometrics |
| `econ.GN` | General Economics |
| `econ.TH` | Theoretical Economics |

---

## 4. Extension domains (`da.*`) - non-arXiv first-class packs

arXiv is incomplete for clinical medicine, biohacking practice, microbiome nutrition, etc.
These are **first-class scientific_domain codes**, not second-class tags.

| Code | Name | Typical sources | Notes |
|------|------|-----------------|-------|
| `da.medicine` | Clinical / biomedical research | PubMed, guidelines, preprints | PICO-friendly env templates |
| `da.microbiome` | Host-microbiome science | papers, reviews, datasets | taxon/metabolite entities |
| `da.metabolism` | Metabolism / metabolic health | papers, protocols | pathways, biomarkers |
| `da.genetics` | Genetics / genomics (applied) | papers, ClinVar-like refs | may overlap `q-bio.GN` |
| `da.biohacking` | Human enhancement / self-experiment protocols | protocols, n-of-1, reviews | strong safety/ethics fitness |
| `da.nutrition` | Nutrition science | trials, reviews | exposure interventions |
| `da.longevity` | Aging / longevity research | papers, trials | multi-domain often |
| `da.social_science` | Social / behavioral sciences | papers, surveys | observational runs |
| `da.chemistry` | Chemistry (non-arXiv-heavy applied) | papers, protocols | optional bridge to `physics.chem-ph` |
| `da.general` | Unspecified / routing fallback | mixed | avoid when better code exists |

### Mapping policy when both exist

```text
arXiv q-bio.GN paper on GWAS     → scientific_domains: ["q-bio.GN", "da.genetics"?]
clinical RCT not on arXiv        → ["da.medicine"]
microbiome diet study on medRxiv → ["da.microbiome", "da.nutrition", "da.medicine"]
GNN for drug discovery on cs.LG  → ["cs.LG", "q-bio.BM"] or ["cs.LG", "da.medicine"]
```

Prefer **specific + honest multi-label** over forcing a single code.

---

## 5. Domain pack skeleton (config contract)

```text
data/domain_packs/<canonical_code>/
  PACK.md                     # human summary, scope, non-goals
  entity_types.yaml           # extra/override entity kinds
  relation_types.yaml         # domain relations (optional)
  environment_template.yaml   # ResearchEnvironment fields/defaults
  metric_conventions.yaml     # metric direction, splits, units
  extraction_patterns.json    # optional pattern overlays
  aliases.yaml                # informal → canonical
```

### Minimal `environment_template.yaml` keys

```yaml
scientific_domain: cs.LG
subject_system_kinds: [model_checkpoint, algorithm, architecture]
data_ref_kinds: [dataset_version, benchmark_split]
protocol_kinds: [benchmark_protocol, training_recipe]
default_metrics: [accuracy, loss, f1]
budget_kinds: [gpu_hours, wall_clock, steps]
required_for_full: [baseline_ref, eval_data_ref, metric_definition_ids, environment_hash]
optional_for_env_lite: [model_name, dataset_name, metric_name, protocol_text]
```

Medicine example differs by kinds (`population`, `cohort_definition`, `study_design`, `outcome`), not by process kernel types.

---

## 6. Work / Paper field additions (design)

| Field | Type | Notes |
|-------|------|-------|
| `source_profile` | string | genre; today's `domain_profile` migrates here |
| `scientific_domains` | string[] | canonical codes from this registry |
| `primary_scientific_domain` | string | usually arXiv `primary_category` when present |
| `domain_assignment_method` | string | `arxiv_primary` / `openalex` / `manual` / `classifier` |
| `domain_pack_ids` | string[] | resolved packs used at ingest/extract time |

arXiv ingest path:

```text
primary_category = "cs.LG"
→ primary_scientific_domain = "cs.LG"
→ scientific_domains ⊇ {cs.LG} ∪ mapped cross-lists
```

---

## 7. Non-goals

- Not a replacement for Topic/OpenAlex hierarchy.
- Not entity resolution (GCN still an Entity, not a domain).
- Not requiring every domain pack implemented before ingest.
- Not blocking unknown codes: unknown → record + `da.general` warning, don't drop paper.

---

## 8. Seed priority packs (design order, not exclusivity)

1. `cs.LG` - current corpus / GNN adjacency
2. `cs.AI`, `cs.CL` - nearby seed mass
3. `q-bio.QM`, `q-bio.GN` - biology bridge
4. `da.medicine`, `da.microbiome`, `da.metabolism` - explicit user domains
5. `math.OC`, `stat.ML` - methods bridges
6. others on demand

---

## 9. Relationship to ADR-043

- Domain codes select **packs** (vocab + env templates + metric conventions).
- Process plane kernel remains shared.
- `ResearchEnvironment.scientific_domains` should use this registry.
- Novelty/generalization assessments are comparable within/across domain codes.
