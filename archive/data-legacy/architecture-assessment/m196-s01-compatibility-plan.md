# M196 S01 Compatibility Plan and Risk Register

## Verdict

**PASS: M196 has a bounded compatibility plan.** Downstream slices may add tests and metadata artifacts first; production queue/source edits require fresh exact GitNexus impact and expanded compatibility verification.

## Source edit gate

Before editing any production source symbol, run exact GitNexus impact on that symbol or file. Special cases:

- `UniversalKBQueue._dependencies_satisfied#1`: always treat as load-bearing; include queue and no-write rehearsal tests.
- `run_universal_kb_no_write_rehearsal`: include no-write runtime smoke and artifact leakage checks.
- `smoke_runner.run_article`: include Universal KB smoke/runner compatibility where touched.

## Compatibility suites

### Contract and no-write floor

```bash
uv run pytest \
  tests/test_universal_kb_contracts.py \
  tests/test_universal_kb_queue.py \
  tests/test_universal_kb_rehearsal.py \
  tests/test_universal_kb_substrate_rehearsal.py \
  tests/test_graph_projection_schema_gate.py \
  tests/test_graph_projection_port.py \
  tests/test_networkx_graph_probe_adapter.py \
  tests/test_projection_backend_seams.py \
  tests/test_m195_governance_ratchets.py -q
```

### Graph readiness command floor

```bash
uv run pytest tests/test_graph_readiness_review.py tests/test_m195_governance_ratchets.py -q
```

### M196 final floor

The final M196 suite should include all new `tests/test_m196_*.py` files plus the M195 no-write/governance floor above.

## Risk register

| Risk | Mitigation |
|---|---|
| Hardening turns into graph import enablement | Governance ratchets and final blocked-readiness statement keep write/import paths disabled |
| Queue semantic drift | Exact impact before queue edits and queue/no-write compatibility tests |
| Observability leaks payloads or secrets | Metadata-only tests scan for raw prompt/text/vector/credential terms |
| Staged validation becomes too broad | S02 contract uses bounded commands and explicit expected artifacts |
| New files not indexed immediately | Use focused tests plus GitNexus detect_changes until next index refresh |

## Required artifacts by slice

- S02: staged validation contract and no-leak audit
- S03: queue resilience evidence
- S04: run artifact observability audit
- S05: governance ratchet audit
- S06: final validation evidence and requirement outcomes

## Blocked readiness statement

M196 production hardening does not enable production graph import, LadybugDB/FalkorDB writes, schema migration execution, or `import_eligible=true`.
