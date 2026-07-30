# P0 Schema Card — Research Process Plane Kernel

**Status:** Design (ADR-043)  
**Date:** 2026-07-29  
**Related:** ADR-043, DOMAIN-REFERENCE-ARXIV.md, GRAPH-SCHEMA.md, ONTOLOGY-DESIGN.md  
**Scope:** Core node types + edges for Research Process Plane (P0).  
**Not:** implementation. Not domain packs. Not assessment nodes (P1).

---

## 0. Design constraints (from ADR-043)

- Process kernel is domain-agnostic; domain packs specialize vocab only.
- Every process node carries `retrieval_eligible` (D134) and temporal fields.
- `import_eligible=false` until promotion gate (D127); default on creation.
- Literature and live execution share one schema with explicit `origin`/`completeness`.
- No new Layer 8 — these nodes distribute across L1–L6.

### Shared property block (all process nodes)

```text
vid                      string  ✅
retrieval_eligible       bool    ✅ (default false; true only after promotion)
import_eligible          bool    ✅ (default false; D127)
created_at               int     ✅ transaction time
valid_from               int     ✅ valid time (domain)
valid_to                 int?    ✅ 0 = open
scientific_domains       string[] ✅ codes from DOMAIN-REFERENCE-ARXIV
origin                   string  ✅ literature_reported | live_executed |
                                 reanalysis | simulation | observational
source_span_id?          string  optional document grounding
artifact_ref_id?         string  optional artifact grounding
evidence_bundle_id?      string  link to EvidenceBundle (if grounded)
schema_version           int     ✅ default 1
```

---

## 1. ResearchProblem

**Layer:** L1 (Process Metadata)  
**VID pattern:** `vid:problem:<slug>`  
**Semantics:** What must be improved or explained. Domain-agnostic.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `text` | string | ✅ | problem statement |
| `problem_type` | string | ✅ | `improvement` / `explanation` / `prediction` / `control` / `measurement` / `characterization` |
| `parent_problem_id` | string? | | hierarchy (subproblems) |
| `domain_pack_id?` | string | | which pack scoped it |

**Edges:**

```text
ResearchProblem ──SEEKS_SOLUTION_IN──► ResearchEnvironment
ResearchProblem ──HAS_SUBPROBLEM──► ResearchProblem
ResearchProblem ──DESCRIBED_IN──► EvidenceBundle
ResearchProblem ──OPENS──────────► ResearchIdea
```

---

## 2. ResearchEnvironment (two-tier)

**Layer:** L1/L2 (Process context, n-ary anchor)  
**VID pattern:** `vid:env:<hash>` for `full`; `vid:env:<fingerprint>` for `env_lite`  
**Semantics:** Fully or partially specified verification context. The only legitimate anchor for experimental-effectiveness claims.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `completeness` | string | ✅ | `full` / `env_lite` / `unknown` |
| `environment_hash` | string | ✅ (full) / partial (env_lite) | content-addressed fingerprint |
| `research_problem_id` | string | ✅ | parent problem |
| `baseline_ref` | string | ✅ | BaselineSnapshot VID |
| `subject_system` | string | ✅ | model / organism / population / formal-system / apparatus |
| `subject_system_kind` | string | ✅ | domain-pack kind |
| `input_data_refs` | string[] | ✅ (full) / names (env_lite) | dataset/cohort/corpus versions |
| `eval_data_refs` | string[] | ✅ (full) / names (env_lite) | eval split / outcome definition |
| `protocol_ref` | string | ✅ | benchmark / clinical protocol / proof protocol |
| `metric_definition_ids` | string[] | ✅ | associated MetricDefinitions |
| `objective_function` | string? | | may differ from scientific metric |
| `compute_budget` | float? | | gpu-hours / cpu-hours |
| `wall_clock_budget` | int? | | seconds |
| `sample_size_budget` | int? | | clinical/observational |
| `hardware_or_lab_profile` | string? | | hw id / lab id / machine |
| `allowed_change_scope` | string[] | | what may be modified |
| `protected_eval_artifacts` | string[] | | anti reward-hacking (eval code locked) |
| `environment_template_id` | string | ✅ | domain pack template used |
| `evidence_origin` | string | ✅ | `live_executed` / `literature_reported` / `mixed` |

**Tier rules:**

- `full`: `environment_hash` complete over baseline+data+protocol+metrics+budget+scope+protected artifacts. Required for strong live execution-grounded claims.
- `env_lite`: partial fingerprint (named model/system, dataset/cohort, metric, protocol text). Required fields relax to names. Must be visible as weaker auditability.
- `unknown`: env cannot be reconstructed — claims cannot be promoted as process evidence.

**Edges:**

```text
ResearchEnvironment ──DEFINES_BASELINE──► BaselineSnapshot
ResearchEnvironment ──USES_DATASET──────► Entity (Dataset/Cohort kind)   (or ArtifactRef)
ResearchEnvironment ──USES_METRIC───────► MetricDefinition
ResearchEnvironment ──FOLLOWS_PROTOCOL──► ProtocolRef / ArtifactRef
ResearchEnvironment ──RUNS──────────────► ExperimentRun
ResearchEnvironment ──VALIDATES─────────► Claim          (claim MUST link back)
ResearchProblem ──SEEKS_SOLUTION_IN──► ResearchEnvironment
```

**Invariant:** A `Claim` asserting experimental effectiveness **must** link `VALID_UNDER → ResearchEnvironment`. No environment → not process-evidence.

---

## 3. BaselineSnapshot

**Layer:** L2 (Process artifact)  
**VID pattern:** `vid:baseline:<hash>`  
**Semantics:** Concrete baseline artifact+config, not an abstract name.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `artifact_ref_id` | string | ✅ | code/config commit/container hash |
| `description` | string | ✅ | what this baseline is |
| `baseline_type` | string | ✅ | `reference_impl` / `prior_best` / `random_init` / `standard_protocol` / `control_arm` |
| `performance_ref` | string? | | baseline MetricObservation VID |

**Edges:**

```text
ResearchEnvironment ──DEFINES_BASELINE──► BaselineSnapshot
BaselineSnapshot ──IMPLEMENTS──► Entity (Method/Model kind)
BaselineSnapshot ──FROM_ARTIFACT──► ArtifactRef
```

---

## 4. ResearchIdea

**Layer:** L6 (Process/Evidence)  
**VID pattern:** `vid:idea:<hash>` (hash of text+context)  
**Semantics:** Natural-language proposal. Not necessarily testable yet.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `text` | string | ✅ | idea statement |
| `idea_type` | string | ✅ | `method` / `architectural` / `protocol` / `exposure` / `curriculum` / `hparam` / `analysis` / `protocol_change` |
| `research_problem_id` | string | ✅ | parent problem |
| `proposed_at` | int | ✅ | search/creation time |
| `proposed_by` | string | ✅ | `human` / `agent` / `literature` |
| `search_campaign_id` | string? | | if generated in a campaign |
| `status` | string | ✅ | `proposed` / `formalized` / `tested` / `rejected` / `rediscovered` |

**Lineage edges (Idea ↔ Idea):**

```text
ResearchIdea ──VARIANT_OF──► ResearchIdea
ResearchIdea ──REFINES──────► ResearchIdea
ResearchIdea ──COMBINES─────► ResearchIdea
ResearchIdea ──GENERALIZES──► ResearchIdea
ResearchIdea ──REJECTS──────► ResearchIdea
ResearchIdea ──REDISCOVERS──► Entity (Method kind)   # prior art
ResearchIdea ──INSPIRED_BY──► FailureEvent | ResearchIdea | Entity
ResearchIdea ──DECOMPOSES_INTO──► Intervention      # component mapping
ResearchIdea ──FORMALIZES──► Hypothesis
ResearchIdea ──HAS_INTERVENTION──► Intervention | InterventionBundle
```

**Invariant:** `Idea ≠ Hypothesis`. An idea can be vague; a hypothesis must be testable under an environment.

---

## 5. Hypothesis (first-class)

**Layer:** L6 (Process/Evidence)  
**VID pattern:** `vid:hyp:<hash>`  
**Semantics:** Formal pre-test expectation under a specific environment.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `text` | string | ✅ | expected effect statement |
| `environment_id` | string | ✅ | scope (env_lite ok for literature) |
| `metric_definition_id` | string | ✅ | what metric tests it |
| `direction` | string | ✅ | `increase` / `decrease` / `no_change` / `change_any` |
| `expected_effect_size` | float? | | optional magnitude |
| `confidence_prior` | float? | | pre-test belief [0,1] |
| `research_idea_id` | string | ✅ | source idea |

**Edges:**

```text
ResearchIdea ──FORMALIZES──► Hypothesis
Hypothesis ──TESTED_BY──────► ExperimentRun
ResultComparison ──SUPPORTS──► Hypothesis
ResultComparison ──REFUTES───► Hypothesis
ResultComparison ──QUALIFIES─► Hypothesis
```

**Invariant:** Only completed valid runs → ResultComparison → SUPPORTS/REFUTES/QUALIFY. `FailureEvent` **must not** edge `REFUTES → Hypothesis`.

---

## 6. Intervention / InterventionBundle

**Layer:** L3 (Process content)  
**VID pattern:** `vid:interv:<hash>`; bundle `vid:bundle_interv:<hash>`  
**Semantics:** Normalized change(s) to method/arch/protocol/exposure/params.

### Intervention

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `target_component` | string | ✅ | what is changed |
| `change_type` | string | ✅ | `add` / `remove` / `replace` / `tune` / `reorder` / `exposure` / `constraint` |
| `parameter_before` | string? | | serialized value |
| `parameter_after` | string? | | serialized value |
| `change_scope` | string | ✅ | domain-pack scope kind |
| `implementation_artifact_id` | string? | | ArtifactRef |

### InterventionBundle

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `recipe_kind` | string | ✅ | `composite` / `multi_stage` / `ensemble` |

**Edges:**

```text
InterventionBundle ──HAS_COMPONENT──► Intervention
Intervention ──TARGETS_COMPONENT──► Entity | ArtifactRef
ResearchIdea ──HAS_INTERVENTION──► Intervention | InterventionBundle
ExperimentRun ──APPLIES──► InterventionBundle | Intervention

# Future (P1) ablation:
AblationRun ──REMOVES_COMPONENT──► Intervention
AblationResult ──ESTIMATES_CONTRIBUTION_OF──► Intervention
```

**Invariant:** Do **not** store a multi-change recipe as a single opaque `Method` entity. Always decompose to InterventionBundle so ablation evidence remains possible.

---

## 7. ImplementationAttempt

**Layer:** L6 (Process/Evidence)  
**VID pattern:** `vid:attempt:<id>`  
**Semantics:** Attempt to turn an idea into an executable artifact.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `research_idea_id` | string | ✅ | what is being attempted |
| `attempt_number` | int | ✅ | per-idea retry index |
| `status` | string | ✅ | `success` / `failed` / `partial` / `pending` |
| `artifact_version_id` | string? | | produced artifact on success |
| `failure_event_id` | string? | | on failure |
| `patch_diff_ref` | string? | | ArtifactRef to diff |
| `repair_attempts` | int | | repair count |

**Edges:**

```text
ImplementationAttempt ──ATTEMPTS──► ResearchIdea
ImplementationAttempt ──PRODUCES──► ArtifactVersion
ImplementationAttempt ──FAILED_WITH──► FailureEvent
```

**Invariant:** `ImplementationAttempt ≠ ExperimentRun`. Successful patch is not an experiment.

---

## 8. ArtifactVersion

**Layer:** L2 (Process artifact)  
**VID pattern:** `vid:artifact:<content_hash>`  
**Semantics:** Immutable code/config/model/container/notebook snapshot.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `content_hash` | string | ✅ | SHA256 of artifact |
| `artifact_kind` | string | ✅ | `code_diff` / `git_commit` / `config` / `container_image` / `model_checkpoint` / `notebook` / `dataset_snapshot` / `cohort_definition` / `protocol_doc` |
| `uri` | string | ✅ | resolvable reference |
| `path` | string? | | local path |
| `line_start` / `line_end` | int? | | for diffs/logs |
| `parent_artifact_id` | string? | | lineage (prior version) |
| `immutable` | bool | ✅ | true once sealed |

**Edges:**

```text
ArtifactVersion ──IMPLEMENTS──► Intervention
ArtifactVersion ──PARENT_OF──► ArtifactVersion   # version chain
ImplementationAttempt ──PRODUCES──► ArtifactVersion
ExperimentRun ──EXECUTES──► ArtifactVersion
BaselineSnapshot ──FROM_ARTIFACT──► ArtifactVersion
```

---

## 9. ExperimentRun

**Layer:** L6 (Process/Evidence)  
**VID pattern:** `vid:run:<hash>`  
**Semantics:** Execution of an artifact in an environment.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `environment_id` | string | ✅ | RUNS_IN |
| `artifact_version_id` | string | ✅ | EXECUTES |
| `intervention_bundle_id` | string? | | APPLIES (if applies bundle) |
| `run_type` | string | ✅ | `training` / `evaluation` / `ablation` / `replication` / `study_arm` / `cohort_analysis` / `reanalysis` / `simulation` |
| `status` | string | ✅ | `running` / `completed` / `failed` / `invalid` / `preregistered` |
| `started_at` | int | | execution time |
| `finished_at` | int | | execution time |
| `wall_clock_sec` | int? | | |
| `seed` | string? | | reproducibility |
| `preregistered` | bool | | plan existed before run |
| `hypothesis_id` | string? | | TESTED_BY (optional) |

**Edges:**

```text
ExperimentRun ──RUNS_IN──► ResearchEnvironment
ExperimentRun ──EXECUTES──► ArtifactVersion
ExperimentRun ──APPLIES──► InterventionBundle | Intervention
ExperimentRun ──PRODUCES──► MetricObservation
ExperimentRun ──FAILED_WITH──► FailureEvent
ExperimentRun ──TESTS──► Hypothesis
ReplicationRun ──REPLICATES──► ExperimentRun   # subtype relation
```

**Invariant:** Only `status=completed` (and not `invalid`) runs may produce observations that feed ResultComparison.

---

## 10. MetricDefinition

**Layer:** L3 (Process content)  
**VID pattern:** `vid:metric:<slug>`  
**Semantics:** Metric name + computation protocol, reusable across runs.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `name` | string | ✅ | canonical name |
| `direction` | string | ✅ | `higher_better` / `lower_better` / `target` / `none` |
| `split` | string | ✅ | `train` / `val` / `test` / `holdout` / `cohort` |
| `computation_protocol` | string | ✅ | how computed |
| `unit` | string? | | |
| `metric_code_artifact_id` | string? | | ArtifactRef to eval code (protected) |

**Edges:**

```text
ResearchEnvironment ──USES_METRIC──► MetricDefinition
MetricDefinition ──MEASURED_BY──► ExperimentRun
MetricDefinition ──OBSERVED_AS──► MetricObservation
```

**Invariant:** `MetricDefinition ≠ MetricObservation`. Definition is reusable; observation is per-run.

---

## 11. MetricObservation

**Layer:** L6 (Process/Evidence)  
**VID pattern:** `vid:obs:<run_id>:<metric_id>`  
**Semantics:** Raw measured value. Not a comparison. Not a claim.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `run_id` | string | ✅ | source run |
| `metric_definition_id` | string | ✅ | which metric |
| `value` | float | ✅ | measured value |
| `std_dev` | float? | | uncertainty |
| `n_seeds` | int? | | replicates |
| `logged_artifact_id` | string? | | ArtifactRef to metric log |

**Edges:**

```text
ExperimentRun ──PRODUCES──► MetricObservation
MetricObservation ──FROM_DEFINITION──► MetricDefinition
ResultComparison ──COMPARES_CANDIDATE──► MetricObservation
ResultComparison ──COMPARES_BASELINE──► MetricObservation
```

**Invariant:** `MetricObservation ≠ ResultComparison ≠ Claim ≠ RewardSignal`.

---

## 12. ResultComparison

**Layer:** L6 (Process/Evidence)  
**VID pattern:** `vid:cmp:<hash>`  
**Semantics:** Derived comparison of candidate vs baseline/peer.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `candidate_observation_id` | string | ✅ | |
| `baseline_observation_id` | string | ✅ | |
| `absolute_delta` | float? | | candidate − baseline |
| `relative_delta` | float? | | |
| `significance_test` | string? | | `t_test` / `bootstrap` / `none` |
| `p_value` | float? | | |
| `confidence_interval_low` / `high` | float? | | |
| `environment_id` | string | ✅ | under which env |
| `valid` | bool | ✅ | both obs from completed valid runs |

**Edges:**

```text
ResultComparison ──COMPARES_CANDIDATE──► MetricObservation
ResultComparison ──COMPARES_BASELINE──► MetricObservation
ResultComparison ──SUPPORTS──► Hypothesis
ResultComparison ──REFUTES───► Hypothesis
ResultComparison ──QUALIFIES─► Hypothesis
ResultComparison ──GROUNDS──► Claim
```

**Invariant:** `valid=false` (e.g., baseline from failed run) → cannot SUPPORTS/REFUTES. Must mark `INVALID_COMPARISON`.

---

## 13. FailureEvent

**Layer:** L6 (Process/Evidence)  
**VID pattern:** `vid:fail:<hash>`  
**Semantics:** Structured non-execution or invalidity cause.

| Property | Type | Req | Description |
|----------|------|:---:|-------------|
| `stage` | string | ✅ | `ideation` / `implementation` / `scheduling` / `execution` / `evaluation` / `replication` |
| `class` | string | ✅ | see taxonomy below |
| `recoverable` | bool | ✅ | |
| `error_signature` | string | ✅ | hashed/normalized |
| `log_ref_id` | string? | | ArtifactRef to log |
| `artifact_version_id` | string? | | |
| `environment_id` | string? | | |
| `repair_attempt_count` | int | | |
| `resolved_by` | string? | | how recovered |

### Class taxonomy (closed vocabulary, extensible per domain)

```text
specification | patch | build | dependency | runtime | oom | timeout |
data | metric | protocol_violation | guard_violation | nondeterminism |
underpowered | confounding | missing_followup | reproduction_failure |
completed_without_improvement
```

**Edges:**

```text
FailureEvent ──OCCURRED_DURING──► ImplementationAttempt | ExperimentRun
FailureEvent ──LIMITS_EXECUTABILITY_OF──► ResearchIdea
```

**Forbidden edge:**

```text
FailureEvent ──REFUTES──► Hypothesis   # INVALID
```

**Invariant:** Failure blocks execution; it never scientifically refutes a hypothesis. Only completed valid evaluation can refute.

---

## 14. Claim (existing, repositioned)

**Layer:** L6 (Process/Evidence) — already in `evidence_bundle.rs`  
**VID pattern:** `vid:claim:<id>`  
**Semantics:** Post-evidence scoped proposition.

**Required new edges:**

```text
Claim ──VALID_UNDER──► ResearchEnvironment     # NEW MANDATORY for experimental claims
Claim ──SUPPORTED_BY──► ResultComparison
Claim ──GROUND_IN──► EvidenceBundle
EvidenceBundle ──SUPPORTS──► Claim               # existing (re-targeted)
EvidenceBundle ──CONTRADICTS──► Claim             # existing
EvidenceBundle ──QUALIFIES──► Claim               # existing
```

**Invariant:** A `Claim` asserting experimental effectiveness **without** `VALID_UNDER → ResearchEnvironment` is a Publication-plane statement, not Process-plane evidence. Both are allowed, but must be distinguishable in retrieval.

---

## 15. Edge type catalog (process plane additions)

```text
# Environment / problem
SEEKS_SOLUTION_IN     ResearchProblem → ResearchEnvironment
HAS_SUBPROBLEM       ResearchProblem → ResearchProblem
DEFINES_BASELINE     ResearchEnvironment → BaselineSnapshot
USES_DATASET         ResearchEnvironment → Entity | ArtifactRef
USES_METRIC          ResearchEnvironment → MetricDefinition
FOLLOWS_PROTOCOL     ResearchEnvironment → ArtifactRef
RUNS                 ResearchEnvironment → ExperimentRun
VALIDATES            ResearchEnvironment → Claim
VALID_UNDER          Claim → ResearchEnvironment

# Idea / hypothesis / lineage
FORMALIZES           ResearchIdea → Hypothesis
OPENS                ResearchProblem → ResearchIdea
VARIANT_OF           ResearchIdea → ResearchIdea
REFINES              ResearchIdea → ResearchIdea
COMBINES             ResearchIdea → ResearchIdea
GENERALIZES          ResearchIdea → ResearchIdea
REJECTS              ResearchIdea → ResearchIdea
REDISCOVERS          ResearchIdea → Entity
INSPIRED_BY          ResearchIdea → FailureEvent | ResearchIdea | Entity
DECOMPOSES_INTO      ResearchIdea → Intervention
HAS_INTERVENTION     ResearchIdea → Intervention | InterventionBundle
TESTED_BY            Hypothesis → ExperimentRun
TESTS                ExperimentRun → Hypothesis
SUPPORTS             ResultComparison → Hypothesis
REFUTES              ResultComparison → Hypothesis
QUALIFIES            ResultComparison → Hypothesis

# Intervention / bundle
HAS_COMPONENT        InterventionBundle → Intervention
TARGETS_COMPONENT    Intervention → Entity | ArtifactRef
APPLIES              ExperimentRun → InterventionBundle | Intervention
REMOVES_COMPONENT    AblationRun → Intervention           # P1
ESTIMATES_CONTRIBUTION_OF  AblationResult → Intervention  # P1

# Attempt / artifact / run
ATTEMPTS             ImplementationAttempt → ResearchIdea
PRODUCES             ImplementationAttempt → ArtifactVersion
FAILED_WITH          ImplementationAttempt | ExperimentRun → FailureEvent
IMPLEMENTS           ArtifactVersion | BaselineSnapshot → Entity
PARENT_OF            ArtifactVersion → ArtifactVersion
EXECUTES             ExperimentRun → ArtifactVersion
FROM_ARTIFACT        BaselineSnapshot → ArtifactRef

# Observation / comparison
PRODUCES             ExperimentRun → MetricObservation
FROM_DEFINITION      MetricObservation → MetricDefinition
MEASURED_BY          MetricDefinition → ExperimentRun
OBSERVED_AS          MetricDefinition → MetricObservation
COMPARES_CANDIDATE   ResultComparison → MetricObservation
COMPARES_BASELINE    ResultComparison → MetricObservation
GROUNDS              ResultComparison → Claim
SUPPORTED_BY         Claim → ResultComparison

# Failure
OCCURRED_DURING      FailureEvent → ImplementationAttempt | ExperimentRun
LIMITS_EXECUTABILITY_OF  FailureEvent → ResearchIdea

# Replication
REPLICATES           ReplicationRun → ExperimentRun

# Literature bridge
DESCRIBED_IN         ResearchProblem → EvidenceBundle
APPROXIMATES         ExperimentSetup → ResearchEnvironment   # EvidenceBundle subtype bridge
```

---

## 16. Invariants checklist (normative)

1. `ResearchIdea ≠ Hypothesis ≠ Claim` — separate node types.
2. `ImplementationAttempt ≠ ExperimentRun` — separate node types.
3. `FailureEvent` has no `REFUTES` edge.
4. `MetricObservation` has no baseline/comparison fields.
5. `ResultComparison` has no truth interpretation fields.
6. `Claim` with experimental-effectiveness assertion requires `VALID_UNDER → ResearchEnvironment`.
7. `environment.completeness` explicit (`full` | `env_lite` | `unknown`).
8. Compound recipe uses `InterventionBundle` (not single `Method`).
9. All process nodes carry `retrieval_eligible`, `import_eligible`, `origin`, temporal fields.
10. `RewardSignal` (Experience plane only) has no edge to `Claim`.
11. ConceptCluster is never a process node / evidence target.
12. D127: `import_eligible=false` default on all process nodes until promotion.

---

## 17. Layer placement summary

| Node | Layer | Notes |
|------|-------|-------|
| ResearchProblem | L1 | process metadata |
| ResearchEnvironment | L1/L2 | n-ary anchor |
| BaselineSnapshot | L2 | artifact-grounded |
| ResearchIdea | L6 | evidence/process |
| Hypothesis | L6 | first-class |
| Intervention / Bundle | L3 | process content |
| ImplementationAttempt | L6 | process evidence |
| ArtifactVersion | L2 | process artifact |
| ExperimentRun | L6 | process evidence |
| MetricDefinition | L3 | process content |
| MetricObservation | L6 | raw evidence |
| ResultComparison | L6 | derived evidence |
| FailureEvent | L6 | process evidence |
| Claim (existing) | L6 | repositioned target |

---

## 18. Non-goals (this card)

- No domain pack contents (see DOMAIN-REFERENCE-ARXIV.md).
- No assessment nodes (Novelty/Generalization/Replication) — P1 card.
- No SearchCampaign/Epoch detailed fields — P1 card.
- No Cypher DDL; design-level only.
- No code; no schema.rs additions yet.

---

## 19. Next steps after this card

1. Review / amend invariants with user.
2. Expand GRAPH-SCHEMA.md with this kernel (section per node).
3. Author seed domain packs (`cs.LG`, `da.medicine`, `da.microbiome`).
4. Plan Wave 1 literature-bridge slice: ExperimentSetup → env_lite → Claim VALID_UNDER.
5. Plan Wave 2 live-kernel slice (full env, run, observation, comparison, failure).
