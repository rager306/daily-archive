# ADR-043: Research Process Plane — Execution-Grounded Scientific Memory

**Status:** Proposed  
**Date:** 2026-07-29  
**Deciders:** collaborative  
**Related:** ADR-038 (schema modules, ExperimentSetup), ADR-040 (Samyama/RuVector/RVF), ADR-041 (embedded Cypher), ADR-042 (EvidenceBundle/Claim), ONTOLOGY-DESIGN.md, GRAPH-SCHEMA.md

## Context

daily-archive aims to support **scientific research, analysis, and synthesis** —
not only document storage and entity extraction. Current ontology (L0–L7) is
strong on **published knowledge**:

```text
Source → Paper → Section → Entity
EvidenceBundle → Claim
ConceptCluster (community only)
```

That is insufficient for questions like:

- which idea, under which environment, produced which observation;
- whether a failure refutes a hypothesis or only blocks execution;
- how ideas evolve, recombine, and rediscover prior art;
- whether a result generalizes across domains, datasets, or populations;
- what regions of intervention space remain unexplored.

External work on execution-grounded automated research shows value in closed
`ResearchEnvironment` loops and idea→implementation→run trajectories, but also
shows that **scalar reward collapses science** (execution failure mixed with
refutation; mode collapse; no novelty/generalization objects).

Additionally, the corpus is multi-source and must be **multi-domain from day
one** (physics, mathematics, biology, medicine/biohacking/microbiome/metabolism,
genetics, social sciences, CS/ML, …). Current GNN/arxiv fixtures are a seed
corpus, not an ontology boundary.

## Decision

### 1. Add a cross-cutting Research Process Plane

Do **not** invent Layer 8. Keep L0–L7. Add a second axis:

| Plane | Question answered | Store |
|-------|-------------------|-------|
| **Publication** | What was published / described? | Samyama |
| **Research Process** | How was knowledge obtained, tested, qualified? | Samyama |
| **Experience** | How did agent/human search and operate? | RVF (Tier 3); promote only via gate |

Process objects are distributed across L1–L6 (and L7 only for search/experience
summaries). Experience traces never become canonical facts without promotion.

### 2. Multi-domain is structural, not a later add-on

Split profiles that were previously conflated:

```text
source_profile      document genre / parse path
                    paper | textbook | lecture | code_repo | protocol | ...

scientific_domain   knowledge domain (multi-valued)
                    cs.ml | physics | mathematics | biology | medicine |
                    microbiome | metabolism | genetics | social_science | ...
```

Domain-specific vocabulary lives in **domain packs** (config):

```text
data/domain_packs/<domain>/
  entity_types.yaml
  relation_types.yaml
  environment_template.yaml
  metric_conventions.yaml
  extraction_patterns.json
```

Process kernel types are domain-agnostic. Packs specialize entity kinds,
environment templates, and metric conventions only.

### 3. Process kernel (canonical Samyama types)

#### Core nodes

| Node | Semantics |
|------|-----------|
| `ResearchProblem` | What must be improved or explained |
| `ResearchEnvironment` | Fully or partially specified verification context |
| `BaselineSnapshot` | Concrete baseline artifact/config version |
| `ResearchIdea` | Natural-language proposal (not necessarily testable) |
| `Hypothesis` | First-class pre-test expectation (≠ Claim) |
| `Intervention` | Normalized change (method/arch/protocol/exposure/param) |
| `InterventionBundle` | Compound recipe of interventions |
| `ExperimentPlan` | Intended procedure before execution |
| `ImplementationAttempt` | Attempt to turn idea into executable artifact |
| `ArtifactVersion` | Immutable code/config/model/container/notebook hash |
| `ExperimentRun` | Execution of artifact in an environment |
| `MetricDefinition` | Metric name, direction, split/protocol |
| `MetricObservation` | Raw measured value |
| `ResultComparison` | Candidate vs baseline/peer comparison |
| `FailureEvent` | Structured non-execution or invalidity cause |
| `Claim` | Post-evidence scoped proposition (existing; process target) |
| `EvidenceBundle` | Source-grounded n-ary evidence unit (existing) |
| `ResearchInsight` | Synthesis across multiple runs/claims |
| `NoveltyAssessment` | Prior-art comparison for an idea/claim |
| `GeneralizationAssessment` | Transfer/robustness evaluation |
| `ReplicationRun` | Repeat of a prior experiment |
| `SearchCampaign` / `SearchEpoch` | Search program and rounds (canon summary) |

#### Literature vs live (same shape)

```text
EvidenceOrigin:
  literature_reported | live_executed | reanalysis | simulation | observational
```

Paper-reported experiments use the same chain with weaker artifact completeness.
Live runs require full hashable environments when claims are promoted as
execution-grounded.

### 4. ResearchEnvironment is two-tier

| Tier | When | Requirements |
|------|------|--------------|
| `full` | live execution / strong promotion | complete `environment_hash` over baseline, data, protocol, metrics, budget, hardware/lab, allowed change scope |
| `env_lite` | literature reconstruction | partial fingerprint (named model/system, dataset/cohort, metric, protocol text); must set `completeness=env_lite` |

Rules:

- Every process `Claim` that asserts experimental effectiveness **must** link
  `VALID_UNDER → ResearchEnvironment`.
- `ResultComparison` without environment is invalid in Process plane.
- `env_lite` is allowed for literature coverage; auditability is lower and must
  be visible to retrieval/synthesis.
- `full` is required before treating a live run as strong execution-grounded
  evidence.

Environment is n-ary and domain-templated (ML benchmark, clinical cohort,
proof context, lab apparatus, etc.), not a flat property bag on Claim.

### 5. Hypothesis is first-class and distinct from Claim

```text
ResearchIdea  ──formalizes──►  Hypothesis   (pre-test expectation)
ResultComparison ──SUPPORTS|REFUTES|QUALIFIES──► Hypothesis
EvidenceBundle / ResultComparison ──grounds──► Claim  (post-evidence proposition)
Claim ──VALID_UNDER──► ResearchEnvironment
```

- `Hypothesis` is **not** a Claim subtype.
- `Hypothesis` is **not** merely `EntityType::Hypothesis` long-term (abstract
  entity label may remain for extraction surfaces, but process reasoning uses
  the first-class node).
- Bare `Entity` is never a SUPPORTS/CONTRADICTS target.

### 6. Four result objects must never collapse

```text
MetricObservation   raw measurement
ResultComparison    derived delta / test vs baseline
Claim               scoped interpreted proposition
RewardSignal        search/policy utility only (Experience / RVF)
```

`RewardSignal` must not create `SUPPORTS → Claim`.

### 7. Failure is not negative science

`FailureEvent` stages/classes cover ideation, implementation, scheduling,
execution, evaluation, replication (patch/build/OOM/timeout/data/metric/
protocol/confounding/underpowered/…).

Allowed:

```text
FailureEvent ── OCCURRED_DURING ──► ImplementationAttempt | ExperimentRun
FailureEvent ── LIMITS_EXECUTABILITY_OF ──► ResearchIdea
```

Forbidden:

```text
FailureEvent ── REFUTES ──► Hypothesis   # invalid
```

Only completed, evaluation-valid runs may yield comparisons that
SUPPORT/REFUTE/QUALIFY a hypothesis.

### 8. Idea lineage and compound recipes

Store evolutionary structure explicitly:

```text
VARIANT_OF | REFINES | COMBINES | GENERALIZES |
REJECTS | REDISCOVERS | INSPIRED_BY | DECOMPOSES_INTO
```

Compound solutions use `InterventionBundle` + component roles
(`required|optional|enabling`), enabling ablation evidence later.

Do **not** store a multi-change recipe as a single opaque `Method` entity.

### 9. Multi-objective fitness (not scalar idea.score)

Canonical assessments may include vector fitness for search/synthesis:

```text
effectiveness, executability, novelty, diversity,
robustness, generalization, cost,
safety_or_ethics?, evidence_strength
```

Selection guidance: Pareto frontier / domain-weighted priorities — not a single
reward field as scientific truth. Effectiveness is always parameterized by
environment and time window.

### 10. Temporal axes

Process plane requires more than ingest timestamps:

| Axis | Meaning |
|------|---------|
| Document time | publication / source version |
| Valid time | when proposition is in force |
| Transaction time | when recorded in our system |
| Execution time | when run occurred |
| Search time | when idea entered a campaign |

Domain packs may add windows (follow-up, exposure, assay time, survey wave).

### 11. Provenance beyond PDF

Keep `SourceSpan` + `EvidenceAssertion` for documents. Add parallel
`ArtifactRef` for code diffs, commits, configs, containers, dataset/cohort
snapshots, checkpoints, logs, notebooks, protocols, plots — all content-hashed
and immutable where possible.

One claim may ground simultaneously in paper span + code + config + metric log.

### 12. Samyama vs RVF (binding with ADR-040)

**Samyama (canonical knowledge):** process kernel nodes, validated runs,
comparisons, failure summaries, assessments, claims, environments.

**RVF (experience):** prompts, tool calls, raw logs, patch repairs, scheduler
traces, query-local evidence DAGs, reflections, policy state.

**Promotion gate:** RVF → schema/hash/env/evidence checks → Samyama.
`import_eligible=false` remains default (D127). Past trajectories are not facts.

### 13. Compatibility with ADR-042 and existing schema

| Existing | Role under ADR-043 |
|----------|--------------------|
| `EvidenceBundle` | publication or process evidence unit |
| `Claim` | post-evidence proposition; process target |
| `ExperimentSetup` | EvidenceBundle subtype for paper-reported n-ary setups; may approximate `ResearchEnvironment` (`env_lite`) |
| `ConceptCluster` | community only; never evidence hyperedge |
| `PARTICIPATES_IN` | entity participation in bundles (and later env/run participants) |
| `SUPPORTS/CONTRADICTS/QUALIFIES` | toward Claim/Hypothesis only |
| `MEMBER_OF_CLUSTER` | community membership only |
| L0–L7 | unchanged numbering; process is cross-cutting |

### 14. Relation to automated research paper

Adopt as ontology inspiration:

1. closed environment context for results;
2. idea → implementation → execution → observation chain;
3. separation of execution failure from hypothesis refutation;
4. evolutionary lineage over flat idea lists.

Reject as knowledge model:

1. scalar reward as scientific evidence;
2. treating non-execution as reward 0 ≡ refuted idea;
3. requiring live RL/GPU search for ontology validity;
4. mode-collapsed template search as default synthesis policy.

## Consequences

### Positive

- Graph can answer research/synthesis questions, not only “what paper mentions X”.
- Multi-domain growth does not require redesigning process kernel.
- Literature and live execution share one chain with explicit completeness.
- Prevents semantic bugs: failure≠refutation, observation≠claim, reward≠evidence.
- Aligns with Samyama/RVF split and fail-closed import.

### Negative / costs

- Larger schema surface (process kernel + assessments).
- Literature extraction must eventually populate env_lite + comparisons, not only entities.
- Domain packs require governance (who adds medicine/physics vocabularies).
- Risk of over-modeling if implementation tries to land all nodes in one slice.

### Migration / implementation posture

Design-first, implement in thin waves:

1. **Docs:** this ADR + ONTOLOGY-DESIGN plane section + GRAPH-SCHEMA process draft.
2. **Literature process:** ExperimentSetup/env_lite → Claim VALID_UNDER; lineage edges.
3. **Live kernel:** Environment full, Run, Observation, Comparison, Failure, ArtifactRef.
4. **Evolution memory:** Campaign/Epoch, InterventionBundle, fitness vectors.
5. **Graph-guided ops:** diversity, salvage, recombination, coverage gaps.

No PPR/evidence activation over ConceptCluster. No automatic promotion of RVF
traces. No `idea.score` as canonical truth.

## Invariants (normative)

1. `ResearchIdea ≠ Hypothesis ≠ Claim`.
2. `ImplementationAttempt ≠ ExperimentRun`.
3. `ExecutionFailure ≠ NegativeResult`.
4. `MetricObservation ≠ ResultComparison ≠ Claim ≠ RewardSignal`.
5. Experimental effectiveness claims require `VALID_UNDER ResearchEnvironment`.
6. Environment completeness is explicit (`full` | `env_lite` | `unknown`).
7. `ConceptCluster` is never an evidence unit or reasoning step.
8. Query-local evidence chains are ephemeral (Experience); not canon without promotion.
9. Effectiveness is contextual (env × time × metric), not a static idea property.
10. Novelty and generalization are assessment nodes, not boolean fields alone.
11. Multi-domain via packs; process kernel stays domain-agnostic.
12. D127: `import_eligible=false` until explicit promotion policy/human gate.

## Open follow-ups (non-blocking for this ADR)

- Exact Cypher/Samyama property lists per process node (GRAPH-SCHEMA expansion).
- First domain packs to author after schema draft (`cs.ml`, `medicine`, `biology` skeletons).
- Whether paper-extracted `EntityType::Hypothesis` auto-promotes to process `Hypothesis` nodes or stays extraction surface only.
- AblationRun as distinct node vs ExperimentRun subtype.
