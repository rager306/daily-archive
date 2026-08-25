# vasiliev2026knowledge-graph-memory-graph

Captured 2026-08-25 from Substack. Primary bytes: `source/article.html`
(sha256 in identity.json). D127: not graph truth.

## Mapping to daily-archive (roles, not three databases)

| Role in article | Our surface | Binding |
|---|---|---|
| Knowledge graph (governed meaning) | Samyama + ontology YAML + D127 | ADR-040 T1 |
| Memory graph (retained experience) | CVF trajectories, SONA patterns, critic RepairOutcome, RVF | ADR-040 T2/T3, M452–M454 |
| Context graph (task/time projection) | ContextBundle Layer H, ACAP | ADR-053 H, ADR-055 |

Operating principle (article): *AI proposes. Systems validate. Analysts decide. The graph records the decision.*

Five rules we keep:

1. Governed knowledge ≠ agent-generated claims (candidate → verify → promote).
2. Memory links to canonical ids (`paper_id`, VID); does not remaster identity.
3. Context is assembled per task; no permanent `RELEVANT_TO`.
4. Separate stores only when workload/retention require it (already: Samyama vs CVF/RVF).
5. Graph structure only when hops, time, or provenance change the answer.

## Follow-ups (not this capture)

- Negative semantic knowledge must block retrieval, not only log Reject.
- Context snapshots need versioned policy_hash + MSC + as_of for replay.
- CVF/SONA records should cite canonical paper/section ids when present.

Do not treat this post as a mandate to materialize the scientific corpus graph.
