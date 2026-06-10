# 02 — SkillGenome Patterns

> **Source:** jscheiber78/skillgenome on GitHub; modules: `fragment.py`, `dag_recombine.py`, `coding_chain_llm.py`, `eval_harness.py`, `behavioral_runner.py`, `canalization.py`, `plausibility.py`, `goal_driven.py`
> **Scope:** pattern extraction; genome model itself not adopted
> **Verdict:** cascaded gates, semantic prefilter, race/successive halving, fingerprint dedupe applicable to bounded eval (M051, M053)

## 0. Reading Order

This file extracts **patterns** from SkillGenome that are applicable to daily-archive's bounded eval pipeline. The genome model itself (skills as genes, fragments, type-DAG recombination) is **not adopted** — daily-archive doesn't have skill fragments to recombine.

Sections:

1. SkillGenome core model (what it is)
2. Bottlenecks
3. Patterns we adopt
4. Patterns we don't adopt (overkill)
5. Application to daily-archive milestones

## 1. SkillGenome Core Model

SkillGenome treats **skills as genes**:

- `SkillGene` — a unit of capability, composed of `Fragment`s
- `Fragment` — a primitive operation (LOAD, VALIDATE, RULE, MAP, FILTER, MERGE, RENDER, FETCH, STORE, DRAFT, SUMMARIZE, CLASSIFY, REVIEW, REWRITE, SEND, DELETE, TRANSACT)
- `Type` — typed input/output of fragments
- `Type-DAG` — directed acyclic graph of type compatibility
- `Capability` — what a fragment/gene binds to
- `Origin/Trust` — provenance and trust scoring
- `Recombination` — generate new genes by combining fragments
- `Eval Harness` — schema → type → safety → plausibility → behavioral
- `Canalization` — repeated runs to dampen variance
- `SkillGenome UDFs (not to confuse with FalkorDB UDFs)** — local module functions for risk scoring, plausibility, type coverage, path fingerprint

## 2. Bottlenecks

| Rank | Bottleneck | Why |
|---:|---|---|
| 1 | LLM / external tool calls | Latency, cost, rate limits |
| 2 | Linear permutation recombination | Naive `2..max_len` permutation search grows factorially |
| 3 | Behavioral eval + canalization | `candidates × testcases × n_runs` cost |
| 4 | Candidate explosion | Type-valid chains can be functionally absurd (RENDER·RENDER, LOAD after RENDER) |
| 5 | Eval cache misses | Without `gene_fp + input_hash` cache key, redundant evals |
| 6 | Plausibility checking without dedicated gate | Type-valid does not mean semantically meaningful |

## 3. Patterns We Adopt

### 3.1 Pattern SG-A: Cascaded gates (already partially ours)

```text
Schema → Type → Safety → Plausibility → (Behavioral eval tiers) → Finalist
```

**Why we adopt:** our M035 contracts + M044 guardrail implement Schema/Type/Safety. Adding Plausibility as a cheap gate is a 1-day addition. Behavioral eval tiers (mock first, real second) is in our eval pattern.

**Adoption in daily-archive:**

- **M051 (eval fixtures):** Tier 1 = fixture validity, Tier 2 = M052 RLM produces Classification, Tier 3 = score against expected
- **M053 (RLM benchmark):** Tier 1 = all 3 baselines × all fixtures, Tier 2 = top 30% full test suite, Tier 3 = top 10% canalization, Tier 4 = top 1-2 finalists full trajectory
- **M058 (graph-readiness gate v1):** Tier 1 (cheap, sync) = safety flags + audit; Tier 2 (moderate) = R024/R027/R029 evidence; Tier 3 (expensive) = M057 hybrid + M053 RLM; Tier 4 (finalist) = structural diff for top-3-5

### 3.2 Pattern SG-B: Plausibility gate

```text
After Type, before Behavioral:
  - RENDER after RENDER → reject (already output, cannot render again)
  - LOAD after RENDER → reject (too late, no source to load)
  - terminal primitives (RENDER, SEND, STORE, DELETE, TRANSACT) in middle → reject
  - LOAD without prior VALIDATE → reject (unvalidated input)
```

**Why we adopt:** M051 fixtures need a plausibility check before M052 RLM evaluation. Cheap CPU-only gate.

**Adoption in daily-archive:** M051 includes a small Python plausibility function for fixture chains; rejected fixtures are explicitly marked and don't proceed to M052.

### 3.3 Pattern SG-C: Semantic prefilter + DAG backward build

```text
Goal → semantic top-K (vector or lexical) → producers by type → backward DAG build
       → beam search / top-N candidates → hard gates → behavioral eval
```

**Why we adopt partially:** the **principle** (broad input → narrow via cheap prefilter → expensive eval) is applicable. But the full **DAG backward build** is overkill at our scale (5-20 articles per batch).

**Adoption in daily-archive:**

- **M051 (eval fixtures):** semantic prefilter (vector top-K) before scoring; reject fixtures with semantic distance > threshold
- **M053 (RLM benchmark):** linear benchmark at our scale; no DAG search needed
- **M057 (hybrid pilot):** if FalkorDB chosen, semantic prefilter via vector index

### 3.4 Pattern SG-D: Race / successive halving

```text
Stage 1: 1 testcase × 1 run для всех plausible candidates
Stage 2: 3 testcases × 1 run для top 30%
Stage 3: full test suite для top 10%
Stage 4: canalization n_runs=5 только для finalists
Stage 5: ActiveGraph fork/diff только для top 3-5
```

**Why we adopt as methodology:** cheap screen first, expensive only for survivors.

**Adoption in daily-archive:** methodology applied to M053 (RLM benchmark) and M058 (graph-readiness gate). Not implemented as a runtime pattern — explicit Tier 1 → Tier 2 → Tier 3 cascade in scripts.

### 3.5 Pattern SG-E: Fingerprint dedupe

```text
fingerprint = sha256(primitive_chain + input_hash + binding + model + prompt + tool_version + policy_version)
```

**Why we adopt:** M049 (models.yaml) + M050 (LLM helper v2) work together: each `(model_id, prompt_hash, input_hash, binding_id, tool_version, policy_version)` tuple has a deterministic fingerprint. Re-computing is no-op; cache hit returns result.

**Adoption in daily-archive:**

- M049: `models.yaml` schema includes `tool_version, policy_version`
- M050: helper computes `fingerprint = sha256(...)` before calling MiniMax; checks `artifacts/m050-work-requests/<fingerprint>.json` cache; if hit, returns immediately; if miss, calls MiniMax, persists result

### 3.6 Pattern SG-F: Risk scoring (primitive-level)

```text
base risk per primitive:
  LOAD=1, VALIDATE=1, RULE=1, MAP=1, FILTER=1, MERGE=2
  FETCH=4, STORE=5, DRAFT=4, SUMMARIZE=4, CLASSIFY=4
  REVIEW=5, REWRITE=5, SEND=9, DELETE=10, TRANSACT=10

adjustments:
  external: +1
  irreversible: +2
  generative: +1
  cap at 10
```

**Why we adopt:** M050 (LLM helper v2) needs risk classification for outputs. We can borrow this scale.

**Adoption in daily-archive:** M050 output Classification includes a `risk_level` field computed via this scale (or a smaller variant for our bounded primitives). Diagnostic only — no graph writes, no approval.

## 4. Patterns We Don't Adopt (overkill at our scale)

| SkillGenome pattern | Why not adopted | What we use instead |
|---|---|---|
| Genome model (genes, fragments, types) | We don't have skills to recombine; we have scientific articles | M035 contracts (no genome needed) |
| Linear permutation recombination | Factorially expensive | Linear search at our scale |
| DAG backward build | Bounded candidate count | M051/M053 linear eval |
| Canalization (n_runs=5) | Bounded fixtures | M053 single-run + post-hoc variance analysis |
| Eval harness as full gate stack | We have M035 contracts + M044 guardrail | Extend with plausibility gate (M051) |
| LLM encoding of SKILL.md | We don't encode skills | M050 calls MiniMax directly for specific task |
| `Body-Hash → chain` cache | We use fingerprint per call | Same pattern, smaller scope |
| `MAX_BODY_CHARS = 8000` | SkillGenome specific | Our M050 has its own input limits |

## 5. Application to Daily-Archive Milestones

| Milestone | Patterns applied | Implementation |
|---|---|---|
| **M049** (models.yaml) | SG-E (fingerprint dedupe) | `models.yaml` schema includes fingerprint inputs |
| **M050** (LLM helper v2) | SG-A, SG-B, SG-D (methodology), SG-E, SG-F | Worker pool, Plausibility gate, Fingerprint cache, Risk scoring |
| **M051** (eval fixtures) | SG-A, SG-B, SG-C (prefilter), SG-D (methodology) | Cascaded gates, Plausibility, Semantic prefilter, Tier methodology |
| **M052** (RLM S09) | SG-A, SG-E | Cascaded gates, Trajectory capture with work_id |
| **M053** (RLM S10) | SG-A, SG-D (methodology) | Tier cascade for benchmark; final report with per-tier breakdown |
| **M058** (graph-readiness gate v1) | SG-A, SG-D (methodology) | Tier 1 (cheap) → Tier 4 (finalist structural diff) |

## 6. LLM Reading Notes

- **Pattern-only, not model adoption.** SkillGenome's genome model is interesting but not our problem.
- **Cascaded gates are the highest-impact pattern** for daily-archive. Currently we have Schema/Type/Safety; adding Plausibility is a 1-day win.
- **Semantic prefilter is a methodology**, not a runtime — applicable to M051/M053/M057.
- **Race/successive halving is overkill at our scale** as runtime; useful as **methodology**.

## 7. Cross-References

- ActiveGraph patterns: `01-activegraph-patterns.md`
- FalkorDB evaluation: `03-falkordb-evaluation.md`
- Applicability matrix: `04-applicability-matrix.md`
