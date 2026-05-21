# M021 implementation impact map

## Scope

M021 will turn the M020 candidate locator protocol into deterministic code. The intended first implementation boundary is additive:

```text
src/arxiv_archive/candidate_locators.py
tests/test_candidate_locators.py
```

No existing runtime symbol should be modified in S02 unless implementation discovers a real need.

## GitNexus context and impact results

### `SemanticChunk`

Command:

```text
gitnexus_context(name="SemanticChunk", repo="daily-archive")
gitnexus_impact(target="SemanticChunk", direction="upstream", repo="daily-archive")
```

Result:

```text
risk=MEDIUM
impactedCount=9
direct=5
processes_affected=0
```

Direct importers:

```text
src/arxiv_archive/scientific_extraction.py
src/arxiv_archive/rlm_workflow.py
src/arxiv_archive/ladybug_client.py
src/arxiv_archive/graph_readiness_export.py
src/arxiv_archive/chunk_baseline_measurement.py
```

Decision:

```text
Do not modify SemanticChunk in M021 S02.
```

The candidate locator module may reference semantic chunk IDs and coordinate concepts, but should not change `SemanticChunk` fields or behavior.

### `ImportCandidate`

Command:

```text
gitnexus_impact(target="ImportCandidate", direction="upstream", repo="daily-archive")
```

Result:

```text
risk=LOW
impactedCount=0
direct=0
processes_affected=0
```

Decision:

```text
Do not modify ImportCandidate in S02.
```

Use import-boundary rehearsal only as a safety-style reference. Candidate locators need their own schema because they are evidence pointers, not import candidates.

### `ValidationBatchState`

Command:

```text
gitnexus_impact(target="ValidationBatchState", direction="upstream", repo="daily-archive")
```

Result:

```text
risk=LOW
impactedCount=7
direct=3
processes_affected=0
```

Direct importers:

```text
src/arxiv_archive/validation_batch_workflow.py
src/arxiv_archive/validation_batch_provenance.py
src/arxiv_archive/cli.py
```

Decision:

```text
Do not modify ValidationBatchState in S02.
```

S03 may integrate with validation batch workflows, but that should have separate impact analysis for CLI/workflow symbols.

## Proposed edit boundary

### S02 allowed edits

```text
ADD src/arxiv_archive/candidate_locators.py
ADD tests/test_candidate_locators.py
```

Optional only if required by tests:

```text
NO existing module edits expected.
```

### S03 possible edits after fresh impact analysis

```text
src/arxiv_archive/cli.py
src/arxiv_archive/__main__.py
src/arxiv_archive/validation_batch_workflow.py
```

Before touching any of those, run GitNexus impact on the exact symbol to edit.

## Risk summary

| Area | Risk | Mitigation |
|---|---|---|
| Existing chunk/evidence dataclasses | Medium if modified | Avoid edits; add new module |
| Import boundary schema | Low but semantically different | Reference style only, do not reuse schema |
| Validation CLI/workflow | Low to medium if integrated | Defer to S03 with fresh impact analysis |
| Raw payload leakage | High project risk | Recursive forbidden-key validation and writer tests |
| Semantic overclaim | High project risk | Keep `import_eligible=false`, `promoted_to_fact=false`, and review-only states |

## Implementation recommendation

Proceed with additive module/tests in S02. Treat any need to edit existing symbols as a blocker requiring fresh GitNexus impact analysis and a short replan note.
