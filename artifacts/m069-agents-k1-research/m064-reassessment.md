# M069 S03 T02 M064 Queue Reassessment

## Verdict

**Verdict: adjust before execution.**

The future queue foundation remains directionally correct, but it should not proceed from the old ADR-017/M064 sketch unchanged. M069 Appendix D and metrics research adds required payload metadata, stable-ID constraints, and evaluation gates that must be built into the queue contract before implementation.

This is not a recommendation to execute M064 now. It is a recommendation to plan a future queue milestone with updated assumptions and a fresh GSD milestone ID.

---

## Why not `confirmed unchanged`

The old queue concept focused on async execution, per-article atomic DAGs, smart scheduling, multi-worker execution, and lease-based claiming. M069 research shows that future graph/extraction jobs also need:

- schema version tracking,
- stable ID policy tracking,
- metrics bundle tracking,
- extractor/prompt program versioning,
- evidence path references,
- cost and latency diagnostics,
- explicit write/promotion eligibility flags,
- support for richer schema modules beyond current five-layer graph edges.

If the queue is built without these fields, it will become an infrastructure bottleneck for Agents-K1-inspired work.

---

## Why not `split prerequisite milestone`

The prerequisite research requested by the user is already covered by M069 S01/S02:

- Appendix D schema evidence and daily-archive schema diff exist.
- Metrics source notes and benchmark contract exist.

A separate prerequisite milestone is not required before drafting the future queue milestone. However, a future queue milestone must incorporate M069 outputs in its plan.

---

## Required queue contract adjustments

Future queue payloads should support at least:

```json
{
  "article_id": "arxiv:<id>",
  "source_artifact_refs": ["..."],
  "schema_version": "...",
  "stable_id_version": "...",
  "metric_bundle_id": "...",
  "extractor_version": "...",
  "prompt_program_hash": "...",
  "evidence_path_refs": ["..."],
  "cost_estimate": null,
  "latency_ms": null,
  "retry_count": 0,
  "write_eligibility": false,
  "promotion_eligibility": false
}
```

The exact JSON is illustrative; implementation should define a typed Python dataclass or Pydantic model during the future queue milestone.

---

## Stable IDs impact

M069 S01 showed that stable IDs are now a first-class queue concern, not a graph-only concern. The queue must not schedule anonymous extraction outputs that later need lossy matching.

Future queue tasks should carry or derive stable IDs for:

- Paper,
- Source PDF,
- Section,
- Figure,
- Table,
- Equation,
- Citation,
- Task/Method/Dataset/Metric candidates,
- Claim or Evidence nodes.

If stable ID policy is not ready, queue tasks should still run but remain in research/non-write eligibility mode.

---

## Hyperedge and evidence path impact

M069 S01 also showed that binary edges are insufficient for complex scientific claims. The queue should be able to carry evidence and claim payloads that may later become reified `Claim` or `Evidence` nodes.

Required planning implication:

- queue should not assume output is only `source`, `relation`, `target` edges;
- queue should allow n-ary payloads and evidence anchors;
- queue should log when a job output loses endpoints due to binary projection.

---

## Metrics impact

M069 S02 benchmark contract means the queue should be observability-aware from day one.

Future queue jobs should record:

- JSON parse validity,
- schema validity,
- evidence-path validity,
- entity F1 when gold labels exist,
- relation F1 when gold labels exist,
- cost,
- latency,
- retry count,
- empty/low-quality output rate.

This is needed before DSPy + MiniMax can be optimized responsibly.

---

## What remains unchanged

The following ADR-017 ideas remain valid:

- Do not build infrastructure before pipeline evidence exists.
- Queue is for per-article atomic work.
- Multi-worker and lease-based claiming remain reasonable future capabilities.
- Production graph writes remain disabled unless separately authorized.
- Queue work should be driven by actual pipeline bottlenecks and not built speculatively.

---

## Recommended next GSD direction

When the user is ready for queue work, create a new milestone rather than reusing M064:

**Suggested title:** `Queue foundation with schema and metrics payloads`

Suggested slices:

1. **Payload contract**: typed queue payload model with schema/metric/evidence fields.
2. **Research-only queue runner**: no production writes, handles per-article jobs and writes artifacts.
3. **Observability and leases**: retry, lease, latency, cost, failure state.
4. **M069 compatibility check**: verify stable ID and benchmark metadata are preserved.

---

## Final recommendation

Proceed with M064-style queue foundation only after updating its plan. The required change is **not** to implement Agents-K1 schema immediately, but to ensure the queue can carry versioned schema, stable IDs, evidence paths, and metrics so future Agents-K1-inspired work does not require another queue rewrite.
