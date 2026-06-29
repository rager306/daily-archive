# M195 S01 Scope Map

## Verdict

**PASS: downstream implementation slices have scoped active boundaries and required GitNexus impact targets.**

## Active boundaries by slice

| Slice | Primary boundary | Active files likely touched or verified | Required impact targets before edits |
|---|---|---|---|
| S02 Candidate Packet Contract | Domain universal KB contracts and graph candidate packet shape | `src/research_graph/domain/universal_kb/contracts.py`, related tests under `tests/test_*universal*`, `tests/test_m033_opendataloader_adaptix_adapter.py` | `CandidatePacket`, `SafetyFlags`, touched methods such as `assert_no_write` |
| S03 Queue Job Lifecycle Contract | Existing queue and dispatch primitives | `src/research_graph/workflows/universal_kb/queue.py`, `src/research_graph/application/orchestrator.py`, `tests/test_universal_kb_queue.py`, `scripts/soak_universal_kb_queue.py` | `UniversalKBQueue`, `DispatchProtocol`, `QueueDispatch`, any edited queue methods |
| S04 Failure Taxonomy and Guards | Typed failure diagnostics surfaced through queue and pipeline payloads | queue/contracts files from S02/S03, low-quality source tests, source conversion scripts where needed | exact edited diagnostic classes/functions; no broad source edits without impact |
| S05 Resume and Artifact Safety | Artifact writes, stale hashes, retry state | `src/research_graph/workflows/universal_kb/queue.py`, smoke/rehearsal files, manifest or artifact IO helpers if touched | `UniversalKBQueue`, artifact writer functions, stale hash validators |
| S06 Continuity Audit Runner | Existing continuity audit surfaces | `src/research_graph/workflows/universal_kb/smoke_audit.py`, `scripts/verify_m030_process_continuity_audit.py`, `scripts/verify_m031_process_continuity_audit.py`, related tests | `audit_smoke`, `validate_continuity`, touched verifier functions |
| S07 Graph Projection Port | Domain/application graph projection contracts | `src/research_graph/domain/ports.py`, new or existing graph projection contract module, tests for substitutability | `GraphDBPort`, new Port symbols if created, package skeleton import boundaries |
| S08 NetworkX Projection Rehearsal | Lightweight graph-shape rehearsal | existing NetworkX probe adapter files and `tests/test_networkx_graph_probe_adapter.py` | exact adapter class/function touched, NetworkX probe functions |
| S09 Backend Adapter Seam Shells | LadybugDB and FalkorDB disabled/dry-run adapter seams | `src/research_graph/infrastructure/graph/ladybug_adapter.py`, future Falkor adapter file only if needed, `tests/test_ladybug_adapter_port.py` | `LadybugAdapter`, `GraphDBPort`, new Falkor adapter class if added |
| S10 Pipeline Projection Handoff | Queue to candidate to projection integration | `src/research_graph/workflows/universal_kb/rehearsal.py`, `smoke_runner.py`, candidate contracts, projection contracts | `run_universal_kb_no_write_rehearsal`, `run_article`, candidate/projection symbols |
| S11 Schema Version and Migration Plan | Graph candidate schema governance | candidate/projection contract files and tests | schema validator symbols before edits |
| S12 End to End No Write Rehearsal | Integrated command or verifier | rehearsal/smoke runner/audit/projection files | all touched integration entrypoints |
| S13 Governance Ratchets | Guardrails and package skeleton tests | `tests/test_research_graph_package_skeleton.py`, architecture guardrail tests, command docs if active | touched guardrail/test helpers |
| S14 Final Validation and Closeout | Artifacts only unless remediation needed | `data/architecture-assessment/m195-*`, GSD validation artifacts | impact only if remediation edits code |

## No-touch boundaries unless a later slice explicitly replans

- `archive/**`
- `.gsd/**` except GSD tool-managed artifacts
- `mutants/**`
- historical `artifacts/**`
- production graph backend data stores
- direct extractor-to-graph write paths
- DSPy or RLM optimizer runtime behavior
- retired `arxiv_archive` runtime shims

## Required sequence guardrails

1. S02 must define candidate packet shape before queue and projection integration consume it.
2. S03 must reuse `UniversalKBQueue` and `DispatchProtocol`; no parallel scheduler.
3. S04 and S05 must make failures and partial writes safe before integrated rehearsal.
4. S07 must define minimal graph projection contracts before NetworkX or backend seams expand.
5. S08 may use NetworkX only for graph-shape rehearsal, not production KG claims.
6. S09 may prepare LadybugDB and FalkorDB seams only with writes disabled or unavailable.
7. S12 is the first slice allowed to claim assembled no-write pipeline preparedness.
8. S13 must keep import eligibility, production graph writes, LadybugDB production writes, and optimizer readiness false.

## GitNexus practice for later slices

Before editing any function, class, or method:

```text
gitnexus_impact(repo='daily-archive', target='<exact symbol or UID>', direction='upstream')
```

If impact is HIGH or CRITICAL, stop and warn before edits. Before closeout of every code-bearing slice, run scoped `gitnexus_detect_changes(repo='daily-archive', scope='all')` or compare mode when appropriate.

## M195 architecture stance

M195 prepares the graph projection boundary and pipeline preparedness layer. It does **not** transition to production graph import. All graph outputs remain candidate, dry-run, rehearsal, or diagnostic outputs unless a future milestone explicitly proves otherwise.
