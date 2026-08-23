# M195 GitNexus Boundary Context

## Verdict

**PASS: M195 planning is grounded in active current-layout symbols and excludes historical archive copies unless explicitly noted.**

## Active current-layout anchors

| Concern | Active symbol or file | GitNexus observation | Downstream slices |
|---|---|---|---|
| Graph DB seam | `Class:src/research_graph/domain/ports.py:GraphDBPort` | Domain Port exists with `init_schema` and `upsert_scientific_kg`; no process flows attached in GitNexus. | S07, S09, S13 |
| Application dispatch seam | `Class:src/research_graph/application/orchestrator.py:DispatchProtocol` | Typed property on `PipelineOrchestrator.dispatch`; methods `admit` and `dispatch`. | S03, S10, S12 |
| Queue dispatch seam | `Class:src/research_graph/application/orchestrator.py:QueueDispatch` | Application-level queue dispatch exists with `queue`, `worker_id`, `contract_version`, and `llm_lane_check`. | S03, S04, S12 |
| Universal KB queue | `Class:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue` | Active queue class has enqueue, diagnostics update, unblock, claim, heartbeat, complete, retryable failure, terminal failure, block, reclaim leases, mark stale, inspect, events, and close. | S03, S05, S10, S12 |
| No-write rehearsal | `Function:src/research_graph/workflows/universal_kb/rehearsal.py:run_universal_kb_no_write_rehearsal` | Calls `CandidatePacket.assert_no_write`, queue methods, review assistance builders, sidecar conversion, and readiness handoff. Participates in no-write processes and assert-no-write flows. | S06, S10, S12 |
| Smoke runner | `Function:src/research_graph/workflows/universal_kb/smoke_runner.py:run_article` | Calls candidate packet serialization, queue enqueue/unblock/inspect/close, review assistance, readiness handoff, false-flag assertions, and candidate builders. | S03, S06, S10 |
| Candidate safety | `Method:src/research_graph/domain/universal_kb/contracts.py:CandidatePacket.assert_no_write` and `SafetyFlags.assert_no_write` | GitNexus process `Run_universal_kb_no_write_rehearsal -> Assert_no_write` confirms active fail-closed safety flow. | S02, S10, S12, S13 |
| NetworkX rehearsal | `tests/test_networkx_graph_probe_adapter.py` and `test_networkx_adapter_extracts_fixture_metrics` | Existing tests cover fixture metrics and missing dependency behavior. | S08, S12 |
| Ladybug adapter seam | `Class:src/research_graph/infrastructure/graph/ladybug_adapter.py:LadybugAdapter`; `tests/test_ladybug_adapter_port.py` | Existing adapter and Port satisfaction tests are active surfaces for backend seam work. | S09, S13 |
| Continuity audit | `src/research_graph/workflows/universal_kb/smoke_audit.py:audit_smoke`, `validate_continuity`, `write_markdown_report`; `scripts/verify_m030_process_continuity_audit.py`; `scripts/verify_m031_process_continuity_audit.py` | Active audit and historical verifier surfaces cover continuity metadata, no-write flags, and stale evidence checks. | S06, S13 |

## Archive ambiguity to avoid

GitNexus name lookup for `UniversalKBQueue` and `run_universal_kb_no_write_rehearsal` can return `archive/package-rename-waves/wave-17/src/arxiv_archive/*`. M195 must use exact current-layout UIDs or file paths under `src/research_graph/**` when planning impact or edits.

## Required impact targets before code edits

Before editing any function, class, or method in later slices, run GitNexus impact on the exact target, likely including:

- `GraphDBPort`
- `DispatchProtocol`
- `QueueDispatch`
- `UniversalKBQueue`
- `run_universal_kb_no_write_rehearsal`
- `run_article`
- `CandidatePacket.assert_no_write`
- `SafetyFlags.assert_no_write`
- `LadybugAdapter`
- any NetworkX probe adapter symbol touched
- any continuity audit validator function touched

## Planning implications

- Reuse existing `UniversalKBQueue` and `DispatchProtocol`; do not introduce a parallel scheduler.
- Treat `run_universal_kb_no_write_rehearsal` and `run_article` as current integration anchors.
- Keep graph projection no-write and candidate-only until S12 final rehearsal evidence exists.
- Use NetworkX for lightweight graph-shape rehearsal before backend production readiness.
- Keep LadybugDB and FalkorDB behind adapter seam or disabled dry-run behavior until a later backend comparison milestone.

## Disallowed in M195

- No direct extractor-to-graph write.
- No production LadybugDB write.
- No production FalkorDB write.
- No import eligibility promotion.
- No DSPy or RLM optimizer invocation.
- No restoration of retired `arxiv_archive` runtime shims.
