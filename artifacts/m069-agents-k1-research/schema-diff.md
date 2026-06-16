# M069 S01 T02 daily-archive Schema Diff

## Scope

Compare the verified Agents-K1 schema evidence with daily-archive's current scientific KG state. This is a research artifact only: no graph writes, migrations, fact promotion, or production import are enabled.

## Summary verdict

Agents-K1's schema is richer in three areas that daily-archive currently under-models:

1. **Implicit scientific abstractions**: motivations, hypotheses, mechanisms, limitations, threats, future work.
2. **Typed content-level relations**: method-to-task, method-to-component, method-to-dataset, method-to-limitation, method-to-property.
3. **Evaluation-oriented evidence structure**: answerable evidence nodes and semantic anchors suitable for multi-hop QA.

daily-archive should not immediately implement the whole Agents-K1 schema. The safe next step is to define stable IDs, a minimal FalkorDB schema, and benchmark metrics first.

---

## Current daily-archive state

| Area | Current state | Evidence basis |
|---|---|---|
| Canonical corpus | 220 PDFs in canonical arXiv catalog | M061/M068 closeout memory |
| Graph layers | citation, table, figure v1, figure v2, judge | M061 five-layer graph evidence |
| Graph DB choice | FalkorDB for self-hosted daily-archive | ADR-022 |
| Embeddings | fd v2 env-driven wrapper | M068 |
| Multimodal judge | MiniMax-M3 diagnostic-only figure QA | ADR-014 / M061 |
| Production graph writes | disabled by safety defaults | project rules |

---

## Schema diff by module

### Module A: Meta or factual entities

**Agents-K1 verified fields:** Paper title, pub year, type, language; Authors name, ordering, corresponding flag.

**daily-archive current fit:** partial. The catalog stores papers and source PDFs, but current arXiv metadata is known to be incomplete for DOI, journal ref, category, comments, license, abs URL, query provenance, pagination state, and typed timeout/429 diagnostics.

**Needed adaptation:**

- Define `Paper` stable ID as canonical arXiv ID plus version normalization policy.
- Add optional metadata fields only when source provenance is present.
- Preserve current canonical PDF source path as a first-class provenance field.

**Do not do yet:** infer missing DOI/license/category fields from weak sources.

### Module B: Textually mentioned entities

**Agents-K1 verified fields:** Tasks with name/type/input/output/constraints/aliases; Methods with name/proposed_or_cited/components/training_objectives/inference_strategies/aliases.

**daily-archive current fit:** weak to partial. Existing pipeline extracts artifacts and graph edges, but does not yet maintain a first-class typed entity inventory for tasks, methods, datasets, metrics, baselines, or implementation details.

**Needed adaptation:**

- Introduce candidate node classes: `Task`, `Method`, `Dataset`, `Metric`, `Baseline`, `ImplementationDetail`.
- Require source spans or section/page references for each candidate.
- Keep these as candidate/extracted nodes until review promotes them.

**Do not do yet:** make extracted tasks/methods eligible for production KG claims without benchmarked extraction quality.

### Module C: Implicit or abstracted entities

**Agents-K1 verified behavior:** extracts abstractions not always present as exact strings, such as `involved_task = Symbolic Reasoning` for CoT.

**daily-archive current fit:** mostly missing. This is the largest schema gap.

**Needed adaptation:**

- Add candidate abstraction types: `Motivation`, `Gap`, `Contribution`, `Hypothesis`, `Assumption`, `Finding`, `Mechanism`, `Limitation`, `Threat`, `DesignRationale`, `FutureWork`, `ErrorAnalysis`.
- Require explicit evidence references and confidence/diagnostic status.
- Treat abstractions as review-only until metrics and human or automated review gates exist.

**Do not do yet:** allow implicit abstractions to become factual KG claims without evidence-path validation.

### Module D: Citation relationships

**Agents-K1 verified behavior:** maps `Paper -> Paper` citation relationships and has a citation-context classification appendix.

**daily-archive current fit:** strong for raw citation edges, weak for argumentative citation semantics.

**Needed adaptation:**

- Preserve existing citation edges.
- Add optional enrichment fields: `citation_context`, `relation_semantics`, `strength`, `directness`, `section`, `paragraph_or_span`.
- Keep `CITES` separate from argumentative relations such as `SUPPORTS`, `CONTRASTS`, `EXTENDS`.

**Do not do yet:** rewrite existing citation ingestion around argumentative classes.

### Module E: Knowledge relations between content entities

**Agents-K1 verified behavior:** captures content-level `Content -> Content` relations; example: CoT `implements` few-shot prompting.

**daily-archive current fit:** partial and mostly indirect. Current graph stores citation/table/figure/judge layers, but relation semantics between methods, tasks, datasets, metrics, and claims are not first-class.

**Needed adaptation:**

- Define minimal relation types for first prototype: `IMPLEMENTS`, `USES_TECHNIQUE`, `APPLIED_TO`, `EVALUATED_ON`, `MEASURED_BY`, `HAS_LIMITATION`, `SUPPORTS`, `CONTRASTS`.
- Represent provenance as edge properties or attached Evidence nodes.
- Defer full 25-relation taxonomy until metrics show extraction quality.

**Do not do yet:** import all 25 relation types into production schema without usage examples and extraction metrics.

---

## Stable ID requirements

| Entity | Proposed ID rule | Status |
|---|---|---|
| Paper | `arxiv:<id>[vN optional]` with canonical version policy | needs decision |
| Source PDF | canonical catalog path plus SHA256 | existing pattern |
| Section | paper ID plus normalized heading path and ordinal | needs prototype |
| Figure/Table/Equation | paper ID plus artifact locator and page/span/hash | partial |
| Citation | citing paper ID plus cited paper ID plus context span hash | needs prototype |
| Task/Method/Dataset/Metric | normalized string plus source paper and evidence span; canonical merge deferred | needs review |
| Claim/Abstraction | paper ID plus abstraction type plus evidence path hash | needs benchmark |

Stable IDs must be designed before FalkorDB writes. This follows Agents-K1 P1/P2/P3 constraints and prevents false merges.

---

## Hyperedge and n-ary relation implications

Agents-K1 emphasizes that binary projections lose information for high-arity scientific reasoning. daily-archive should not force every scientific claim into one binary edge.

Recommended FalkorDB options to research before implementation:

1. **Reified Relation node**: `(Method)-[:PARTICIPATES_IN]->(Relation)-[:HAS_TASK]->(Task)` etc.
2. **Evidence node as anchor**: entities connect to a shared Evidence or Claim node.
3. **Edge properties only**: simplest, but likely insufficient for n-ary constraints.

Preliminary recommendation: use reified `Claim` or `Evidence` nodes for n-ary relations, not edge properties only.

---

## Minimal viable schema for future prototype

Do not implement yet, but if M069 proceeds after metrics:

- Nodes: `Paper`, `SourceArtifact`, `Section`, `Figure`, `Table`, `Equation`, `Evidence`, `Task`, `Method`, `Dataset`, `Metric`, `Claim`, `Limitation`, `FutureWork`.
- Edges: `CITES`, `HAS_SECTION`, `HAS_ARTIFACT`, `MENTIONS`, `EVIDENCES`, `IMPLEMENTS`, `APPLIED_TO`, `EVALUATED_ON`, `MEASURED_BY`, `HAS_LIMITATION`, `SUPPORTS`, `CONTRASTS`.
- Required properties: `source_ref`, `extractor`, `extracted_at`, `confidence_or_status`, `review_status`, `schema_version`.

---

## Verified vs inferred

### Verified from source snippets

- Agents-K1 uses modules A-E.
- Appendix D includes JSON-like structured extraction examples.
- Module A includes Paper and Authors metadata fields.
- Module B includes Task and Method fields.
- Module C extracts implicit abstractions.
- Module D models Paper-to-Paper citation relationships.
- Module E models Content-to-Content semantic relationships.

### Inferred for daily-archive

- FalkorDB can represent the schema as a property graph, but exact performance and Cypher compatibility must be tested later.
- Reified Claim/Evidence nodes are likely safer for hyperedges than edge-only encoding.
- MiniMax + DSPy should optimize extraction prompts only after this schema and metric contract are stable.

---

## Recommendation for M064 impact

M064 queue foundation may proceed only if it treats schema as versioned payloads and does not assume a fixed thin graph shape. The queue should carry:

- schema version,
- stable ID version,
- extractor version,
- metric bundle ID,
- evidence artifact references,
- write eligibility flags.

If M064 currently assumes only current five-layer edges, it should be adjusted before graph-write execution.
