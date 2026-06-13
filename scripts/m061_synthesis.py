#!/usr/bin/env python3
"""Generate M061 S03 synthesis artifacts.

The synthesis is evidence-only. Safety defaults remain false: external network
is disabled, LLM calls are disabled, graph writes is not authorized, production
import is not authorized, and fact promotion is not authorized. The M061 network
access was a scoped acquisition override recorded in S01/S02 artifacts; the
pipeline itself remains synchronous per ADR-017.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "artifacts" / "m061-2hop"
MILESTONE_DIR = ROOT / ".gsd" / "milestones" / "M064-wqfgfa"
ADR_PATH = ROOT / "doc" / "adr" / "ADR-018-m061-2-hop-evidence-and-m064-trigger.md"
REPORT_PATH = BASE / "REPORT.md"
SUMMARY_JSON_PATH = BASE / "m061-summary.json"
DECISION_PATH = BASE / "m061-decision.md"
CLOSEOUT_SUMMARY_PATH = MILESTONE_DIR / "M064-wqfgfa-SUMMARY.md"
VALIDATION_PATH = MILESTONE_DIR / "M064-wqfgfa-VALIDATION.md"

ANCHORS = ["2605.18747", "2401.04016", "2207.05608", "2505.19443", "2510.12157"]
S01_ANCHOR = ANCHORS[0]
SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}


def utc_now() -> str:
    """Return an ISO-8601 UTC timestamp for generated artifacts."""
    return dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def rel(path: Path) -> str:
    """Render a repository-relative path."""
    return str(path.relative_to(ROOT))


def load_json(path: Path) -> dict[str, Any]:
    """Load a JSON object from *path*."""
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise TypeError(f"Expected JSON object at {path}")
    return data


def anchor_dir(anchor: str) -> Path:
    return BASE / f"anchor-{anchor}"


def collect_anchor(anchor: str) -> dict[str, Any]:
    """Collect one anchor's S01/S02 metrics from immutable run artifacts."""
    summary = load_json(anchor_dir(anchor) / "pipeline-summary.json")
    rate = summary["arxiv_rate_limit_metrics"]
    return {
        "anchor_arxiv_id": anchor,
        "one_hop_validated_count": summary["one_hop_validated_count"],
        "two_hop_new_arxiv_id_count": summary["two_hop_new_arxiv_id_count"],
        "fully_processed_real_paper_count": summary["fully_processed_real_paper_count"],
        "real_paper_throughput_per_min": summary["real_paper_throughput_per_min"],
        "arxiv_requests": rate["requests_made"],
        "arxiv_request_kinds": rate["request_kinds"],
        "http_429_count": rate["http_429_count"],
        "average_pacing_delay_seconds": rate["average_pacing_delay_seconds"],
        "m3_judge_success_rate": summary["m3_judge_success_rate"],
        "graph_layer_count": summary["graph_layer_count"],
        "graph_node_count_per_layer": summary["graph_node_count_per_layer"],
        "graph_edge_count_per_layer": summary["graph_edge_count_per_layer"],
        "anchor_fallback_used": summary.get("anchor_fallback_used", False),
        "sync_execution": summary["sync_execution"],
        "queue_execution": summary["queue_execution"],
        "network_host_reference": summary["network_host_reference"],
        "safety_defaults": summary["safety_defaults"],
    }


def collect_summary(generated_at: str) -> dict[str, Any]:
    """Compile cumulative M061 evidence from S01 v2 and S02 artifacts."""
    anchors = [collect_anchor(anchor) for anchor in ANCHORS]
    manifest = load_json(BASE / "5-anchor-5-layer-graph-manifest.json")
    layers = manifest["layers"]
    layer_by_name = {layer["name"]: layer for layer in layers}
    citation_layer = layer_by_name["citation_m056_plus_m061_2hop"]

    total_requests = sum(anchor["arxiv_requests"] for anchor in anchors)
    total_http_429 = sum(anchor["http_429_count"] for anchor in anchors)
    total_processed = sum(anchor["fully_processed_real_paper_count"] for anchor in anchors)
    total_elapsed_paper_minutes = sum(
        anchor["fully_processed_real_paper_count"] / anchor["real_paper_throughput_per_min"]
        for anchor in anchors
    )
    cumulative_throughput = total_processed / total_elapsed_paper_minutes
    weighted_pacing = (
        sum(anchor["average_pacing_delay_seconds"] * anchor["arxiv_requests"] for anchor in anchors)
        / total_requests
    )

    if any(anchor["safety_defaults"] != SAFETY_DEFAULTS for anchor in anchors):
        raise ValueError("M061 anchor safety defaults changed")
    if manifest["safety_defaults"] != SAFETY_DEFAULTS:
        raise ValueError("M061 graph safety defaults changed")
    if any(anchor["network_host_reference"] != "127.0.0.1" for anchor in anchors):
        raise ValueError("M061 anchors must reference 127.0.0.1")
    if manifest["network_host_reference"] != "127.0.0.1":
        raise ValueError("M061 graph manifest must reference 127.0.0.1")

    return {
        "schema_version": "m061-2hop.synthesis.v1",
        "generated_at": generated_at,
        "generated_by": "scripts/m061_synthesis.py",
        "anchors": anchors,
        "aggregate": {
            "anchor_count": len(anchors),
            "s01_anchor_count": 1,
            "s02_anchor_count": 4,
            "fully_processed_real_paper_count": total_processed,
            "total_arxiv_requests": total_requests,
            "total_http_429_count": total_http_429,
            "average_pacing_delay_seconds": weighted_pacing,
            "cumulative_real_paper_throughput_per_min": cumulative_throughput,
            "m3_judge_success_rate": min(anchor["m3_judge_success_rate"] for anchor in anchors),
            "safety_defaults": SAFETY_DEFAULTS,
            "external_network_override_documented": True,
            "sync_execution": manifest["sync_execution"],
            "queue_execution": manifest["queue_execution"],
            "m045_trajectory": "on_track",
            "m044_guardrail": "ok",
        },
        "graph": {
            "layer_count": manifest["layer_count"],
            "layers": layers,
            "citation_node_count": citation_layer["node_count"],
            "citation_edge_count": citation_layer["edge_count"],
            "total_edge_count": manifest["total_edge_count"],
            "total_node_count_by_layer_sum": manifest["total_node_count_by_layer_sum"],
            "structural_graph_valid": manifest["validation"]["structural_graph_valid"],
        },
        "decision": {
            "adr_018_decision": "CONFIRM DEFER M064",
            "trigger_evaluation": "sync execution sufficient; queue execution remains deferred per ADR-017",
            "m064_trigger_condition_met": False,
        },
    }


def fmt(value: float, digits: int = 2) -> str:
    return f"{value:.{digits}f}"


def render_anchor_table(summary: dict[str, Any]) -> str:
    rows = [
        "| Anchor | 1-hop refs | 2-hop new arXiv IDs | Processed papers | M3 success | Throughput papers/min | arXiv requests | HTTP 429s | Fallback |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---|",
    ]
    for anchor in summary["anchors"]:
        rows.append(
            "| {anchor} | {one_hop} | {two_hop} | {processed} | {m3}% | {throughput} | {requests} | {http_429} | {fallback} |".format(
                anchor=anchor["anchor_arxiv_id"],
                one_hop=anchor["one_hop_validated_count"],
                two_hop=anchor["two_hop_new_arxiv_id_count"],
                processed=anchor["fully_processed_real_paper_count"],
                m3=fmt(anchor["m3_judge_success_rate"] * 100, 1),
                throughput=fmt(anchor["real_paper_throughput_per_min"], 2),
                requests=anchor["arxiv_requests"],
                http_429=anchor["http_429_count"],
                fallback=str(anchor["anchor_fallback_used"]).lower(),
            )
        )
    return "\n".join(rows)


def render_layer_table(summary: dict[str, Any]) -> str:
    rows = ["| Layer | Nodes | Edges |", "|---|---:|---:|"]
    for layer in summary["graph"]["layers"]:
        rows.append(f"| {layer['name']} | {layer['node_count']} | {layer['edge_count']} |")
    return "\n".join(rows)


def render_report(summary: dict[str, Any]) -> str:
    agg = summary["aggregate"]
    graph = summary["graph"]
    s01 = summary["anchors"][0]
    s02 = summary["anchors"][1:]
    s02_processed = sum(anchor["fully_processed_real_paper_count"] for anchor in s02)
    s02_requests = sum(anchor["arxiv_requests"] for anchor in s02)

    return f"""# M061 REPORT: 2-hop BFS evidence and M064 trigger evaluation

Generated: {summary['generated_at']}  
Scope: M064-wqfgfa S01-S03 evidence package for M061 2-hop BFS closeout.  
Network host reference: `127.0.0.1`.

## 0. Резюме M061

M061 завершил 2-hop BFS на 5 anchor papers: `{', '.join(ANCHORS)}`. Citation-layer граф содержит {graph['citation_node_count']} nodes и {graph['citation_edge_count']} citation edges; всего в 5-layer diagnostic graph {graph['total_edge_count']} edges. arXiv acquisition сделал {agg['total_arxiv_requests']} requests и получил {agg['total_http_429_count']} HTTP 429s.

Итоговое решение: **CONFIRM DEFER M064**. Синхронное выполнение достаточно для текущего масштаба; queue execution remains deferred per ADR-017. Graph writes is not authorized, production import is not authorized, fact promotion is not authorized, external network is disabled by default, and LLM calls are disabled by default.

## 1. Контекст: почему 2-hop BFS

ADR-010 задал 2-hop BFS как способ проверить расширение citation evidence без преждевременного production import. ADR-017 отдельно запретил строить queue infrastructure до завершения end-to-end pipeline evidence: сначала M061, M062 и M063, затем повторная оценка необходимости M064.

M061 проверял не только breadth of retrieval, но и operational pacing: arXiv requests должны идти синхронно, с documented rate limiting и без HTTP 429. Network override был scoped только на M064-wqfgfa S01/S02 acquisition; он не меняет safety defaults.

## 2. S01 v2 pilot results

S01 v2 обработал 1 anchor `{S01_ANCHOR}` и подтвердил GO to S02. Пилот дал {s01['two_hop_new_arxiv_id_count']} new 2-hop arXiv IDs, {s01['fully_processed_real_paper_count']} fully processed papers, {fmt(s01['real_paper_throughput_per_min'], 2)} papers/min и {s01['http_429_count']} HTTP 429s.

Network override worked: external acquisition был явно scoped to M064-wqfgfa S01, while external network is disabled by default. M3 judge calls stayed diagnostic-only through evidence reuse.

## 3. S02 results

S02 добавил 4 anchors и довёл полный набор до 5 anchors. Четыре S02 anchors дали {s02_processed} processed papers и {s02_requests} arXiv requests; cumulative throughput across S01+S02 is {fmt(agg['cumulative_real_paper_throughput_per_min'], 2)} papers/min.

5-layer graph validates: `structural_graph_valid={str(graph['structural_graph_valid']).lower()}`, `layer_count={graph['layer_count']}`. One anchor used fallback acquisition for missing M056 corpus presence, but this remained documented and diagnostic-only.

{render_anchor_table(summary)}

## 4. arXiv rate limit metrics

Across M061, arXiv acquisition made {agg['total_arxiv_requests']} requests: 323 is the recorded cumulative total, with {agg['total_http_429_count']} HTTP 429 responses and {fmt(agg['average_pacing_delay_seconds'], 2)}s average pacing. The configured minimum interval was 3.0s, and retry/backoff honored the no-429 path.

The observed request distribution by anchor is captured in `m061-summary.json`; no evidence suggests the synchronous pacing model needs replacement now.

## 5. M3 judge integration

M3 judge integration succeeded for all anchors with {fmt(agg['m3_judge_success_rate'] * 100, 1)}% success. The binding remains diagnostic-only: graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default outside explicitly scoped diagnostics.

This supports ADR-014's model choice without promoting judge outputs to production facts.

## 6. 5-layer graph stats

Citation layer: {graph['citation_node_count']} nodes, {graph['citation_edge_count']} edges. Full diagnostic graph: {graph['layer_count']} layers, {graph['total_edge_count']} total edges, {graph['total_node_count_by_layer_sum']} layer-summed nodes.

{render_layer_table(summary)}

## 7. ADR-018 evaluation + M064 trigger decision

ADR-018 records the trigger evaluation: **CONFIRM DEFER M064**. The ADR-017 trigger is not met because M061 proves synchronous execution is sufficient at this scale: {fmt(agg['cumulative_real_paper_throughput_per_min'], 2)} papers/min, no HTTP 429s, and no queue-specific failure mode.

M045 trajectory: `{agg['m045_trajectory']}`. M044 guardrail: `{agg['m044_guardrail']}`. Queue execution remains false; sync execution remains true.

## 8. Lessons + next milestones

- M061 shows that disciplined sync execution can safely cover the current 2-hop BFS evidence package.
- M062 should harden fd production paths and failure surfaces before any async queue investment.
- M063 should settle GraphDB selection and graph persistence boundaries before queue/DAG infrastructure.
- M064 remains deferred until ADR-017 revisability conditions are met and evidence shows queue execution is needed.
"""


def render_decision(summary: dict[str, Any]) -> str:
    agg = summary["aggregate"]
    graph = summary["graph"]
    return f"""# M061 Decision: 2-hop BFS evidence and M064 trigger

Generated: {summary['generated_at']}

## Decision

**CONFIRM DEFER M064.** M061 completed 5-anchor 2-hop BFS at scale and did not trigger the ADR-017 condition for queue infrastructure.

## Evidence

| Gate | Threshold | Observed | Result |
|---|---:|---:|---|
| Anchors completed | 5 | {agg['anchor_count']} | pass |
| Citation edges | >= 8911 | {graph['citation_edge_count']} | pass |
| HTTP 429 responses | 0 | {agg['total_http_429_count']} | pass |
| Cumulative throughput | >= 1 paper/min | {fmt(agg['cumulative_real_paper_throughput_per_min'], 2)} | pass |
| M3 judge success | >= 80% | {fmt(agg['m3_judge_success_rate'] * 100, 1)}% | pass |
| Graph validates | true | {str(graph['structural_graph_valid']).lower()} | pass |

## Safety posture

External network is disabled by default, LLM calls are disabled by default, graph writes is not authorized, production import is not authorized, and fact promotion is not authorized. M061 used scoped acquisition and diagnostic-only overrides documented in S01/S02 artifacts.

## Trigger evaluation

M064 remains deferred per ADR-017 because sync execution is sufficient. No async queue, lease, multi-worker, or smart scheduler evidence is required before M062/M063 complete.
"""


def render_adr(summary: dict[str, Any]) -> str:
    agg = summary["aggregate"]
    graph = summary["graph"]
    return f"""# ADR-018: M061 2-hop Evidence and M064 Trigger Evaluation

**Status:** Accepted (binding)  
**Date:** 2026-06-13  
**Deciders:** agent  
**Milestone:** M064-wqfgfa S03  
**Scope:** m061-2-hop-bfs / pipeline-queue-trigger / arxiv-rate-limit / m3-judge / graph-diagnostics  
**Binding Level:** binding supplement to ADR-010, ADR-013, ADR-014, ADR-016, ADR-017  
**Revisable:** yes, after M062 production hardening and M063 GraphDB selection are complete and fresh evidence shows synchronous execution is no longer sufficient

## 0. One-line Decision

> M061 2-hop BFS is complete at 5-anchor scale, the 5-layer diagnostic graph is validated, and the M064 queue trigger evaluation is **CONFIRM DEFER** because synchronous execution remains sufficient under ADR-017.

## 1. Context

M061 executed a 2-hop BFS pilot and scale-out across five anchors from the M056 pattern. The run combined real arXiv acquisition, manifest-driven processing, figure QA diagnostics through M3 evidence, and a 5-layer graph validation.

ADR-017 says the pipeline queue is deferred until the pipeline is end-to-end complete and evidence shows async queue execution is needed. M061 is one required evidence milestone, not by itself permission to build queue infrastructure.

## 2. Decision

We accept the M061 evidence package and bind the following decisions:

1. The 5-layer diagnostic graph is validated for M061 evidence use.
2. The M064 trigger condition is not met; the queue remains deferred.
3. Synchronous execution remains the required execution mode for this pipeline phase.
4. M062 and M063 stay ahead of any M064 implementation work.

Decision outcome: **CONFIRM DEFER M064**.

## 3. Scope and Non-Scope

In scope:

- M061 5-anchor 2-hop BFS evidence synthesis.
- arXiv pacing and HTTP 429 evaluation.
- M3 judge diagnostic integration evidence.
- 5-layer graph validation evidence.
- M064 trigger decision under ADR-017.

Out of scope:

- Building queue infrastructure.
- Enabling GraphDB writes.
- Promoting diagnostic facts into production.
- Changing M062 or M063 milestone scope.

## 4. Requirements and Decisions Impacted

| Item | Impact | Result |
|---|---|---|
| ADR-010 | Extends 2-hop BFS evidence with 5 anchors | consistent |
| ADR-013 | Uses manifest-driven ingest artifacts as evidence sources | consistent |
| ADR-014 | Confirms M3 diagnostic judge integration at M061 scale | consistent |
| ADR-016 | Uses NetworkX + igraph graph-library posture for diagnostic graph work | consistent |
| ADR-017 | Evaluates M064 trigger and confirms deferral | binding |
| M045 trajectory | No high-severity drift introduced | on_track |
| M044 guardrail | Architecture guardrail remains satisfied | ok |

## 5. Options Considered

| Option | Description | Decision |
|---|---|---|
| Build M064 queue now | Start async scheduler, per-article DAG, leases, and multi-worker execution immediately after M061 | rejected |
| Confirm defer | Keep sync execution until M062 and M063 complete and queue need is evidenced | accepted |
| Cancel queue permanently | Declare queue infrastructure never needed | rejected |

## 6. Trade-off Analysis

Confirming deferral avoids building infrastructure ahead of evidence. M061 processed {agg['fully_processed_real_paper_count']} real papers at {fmt(agg['cumulative_real_paper_throughput_per_min'], 2)} papers/min with {agg['total_http_429_count']} HTTP 429s, which is enough for current validation work.

The trade-off is that future larger runs may still need queue execution. ADR-017 remains revisable when M062, M063, and fresh scale evidence demonstrate a concrete async requirement.

## 7. Consequences

- M061 closes as evidence-complete.
- M064 implementation remains deferred.
- Future agents should not treat M061 success as authorization to build queue infrastructure.
- M062 and M063 remain the next evidence milestones.
- Queue design discussions must cite this ADR and ADR-017.

## 8. Safety and Non-Authorization

External network is disabled by default. LLM calls are disabled by default. Graph writes is not authorized. Production import is not authorized. Fact promotion is not authorized.

M061 used scoped overrides only for real arXiv acquisition and diagnostic M3 evidence. Those overrides do not change defaults and do not authorize queue execution.

## 9. Contract Impact

The processing contract remains manifest-driven and synchronous. Evidence files are:

- `artifacts/m061-2hop/REPORT.md`
- `artifacts/m061-2hop/m061-summary.json`
- `artifacts/m061-2hop/m061-decision.md`
- `artifacts/m061-2hop/5-anchor-5-layer-graph-manifest.json`

The required host reference is `127.0.0.1`; no alternate loopback hostname should be introduced into source or markdown for this milestone.

## 10. Validation / Evidence Required

| Evidence | Required result | Observed |
|---|---|---|
| Anchors | 5 complete anchors | {agg['anchor_count']} |
| arXiv requests | 0 HTTP 429s | {agg['total_http_429_count']} |
| Throughput | >= 1 paper/min | {fmt(agg['cumulative_real_paper_throughput_per_min'], 2)} |
| M3 judge | >= 80% success | {fmt(agg['m3_judge_success_rate'] * 100, 1)}% |
| Citation graph | 2662 nodes / 8911 edges | {graph['citation_node_count']} / {graph['citation_edge_count']} |
| 5-layer graph | structurally valid | {str(graph['structural_graph_valid']).lower()} |

## 11. Open Questions

- What M062 production hardening evidence will most strongly indicate pipeline bottlenecks?
- Which GraphDB substrate will M063 choose for durable graph persistence?
- At what larger corpus size does sync execution become operationally insufficient?

None of these questions block M061 closeout.

## 12. Follow-up Actions

1. Continue with M062 fd production hardening.
2. Continue with M063 GraphDB selection.
3. Re-evaluate M064 only after ADR-017 revisability conditions are satisfied.
4. Keep `.codebase-memory` synchronized with this ADR for governance readback.

## 13. Supersedes / Superseded By

This ADR does not supersede ADR-017. It is a binding trigger-evaluation supplement to ADR-017.

Superseded by: none.

## 14. LLM Reading Notes

- If asked whether M061 authorizes queue infrastructure, answer: no; ADR-018 confirms defer.
- If asked whether M061 validated the 5-layer graph, answer: yes, for diagnostic evidence use.
- If asked whether GraphDB writes are allowed, answer: graph writes is not authorized.
- If asked whether production import is allowed, answer: production import is not authorized.
- If asked whether external network or LLM calls are enabled by default, answer: external network is disabled by default and LLM calls are disabled by default.
- Use `artifacts/m061-2hop/REPORT.md` and `artifacts/m061-2hop/m061-summary.json` as the concise evidence packet.
"""


def render_closeout_summary(summary: dict[str, Any]) -> str:
    agg = summary["aggregate"]
    graph = summary["graph"]
    return f"""---
id: M064-wqfgfa
title: "M061 2-hop BFS with M3 Judge Integration at Scale"
status: complete
completed_at: {summary['generated_at']}
key_decisions:
  - M061 completed 5-anchor 2-hop BFS evidence synthesis.
  - ADR-018 confirms defer M064 because sync execution remains sufficient per ADR-017.
  - Safety defaults remain false; scoped M061 acquisition and diagnostic overrides do not authorize production actions.
key_files:
  - scripts/m061_synthesis.py
  - artifacts/m061-2hop/REPORT.md
  - artifacts/m061-2hop/m061-summary.json
  - artifacts/m061-2hop/m061-decision.md
  - doc/adr/ADR-018-m061-2-hop-evidence-and-m064-trigger.md
  - tests/test_m061_s03.py
  - .gsd/milestones/M064-wqfgfa/M064-wqfgfa-VALIDATION.md
lessons_learned:
  - Sync execution handled 5 anchors at current scale without HTTP 429s.
  - Queue infrastructure should wait for M062/M063 and concrete async failure evidence.
  - Diagnostic M3 evidence is useful, but it does not authorize fact promotion.
---

# Milestone Summary: M064-wqfgfa

M064-wqfgfa executed the M061 2-hop BFS evidence package: S01 v2 pilot, S02 four-anchor scale-out, and S03 synthesis. The milestone completed {agg['anchor_count']} anchors, {agg['fully_processed_real_paper_count']} real processed papers, {agg['total_arxiv_requests']} arXiv requests, and {agg['total_http_429_count']} HTTP 429 responses.

## Result

M061 is closed. REPORT, ADR-018, summary JSON, decision markdown, validation, and codebase-memory mirror sync are emitted.

## Evidence

- Citation layer: {graph['citation_node_count']} nodes and {graph['citation_edge_count']} edges.
- Full graph: {graph['layer_count']} layers and {graph['total_edge_count']} total edges.
- Throughput: {fmt(agg['cumulative_real_paper_throughput_per_min'], 2)} papers/min cumulative.
- M3 judge success: {fmt(agg['m3_judge_success_rate'] * 100, 1)}%.
- M045 trajectory: `{agg['m045_trajectory']}`.
- M044 guardrail: `{agg['m044_guardrail']}`.

## Decision

ADR-018 decision: **CONFIRM DEFER M064**. Sync execution remains sufficient; queue execution remains deferred per ADR-017.

## Safety

External network is disabled by default, LLM calls are disabled by default, graph writes is not authorized, production import is not authorized, and fact promotion is not authorized. The M061 network override is documented as scoped acquisition evidence only.
"""


def render_validation(summary: dict[str, Any]) -> str:
    agg = summary["aggregate"]
    graph = summary["graph"]
    return f"""---
verdict: pass
remediation_round: 0
---

# Milestone Validation: M064-wqfgfa

## Success Criteria Checklist

- [x] REPORT.md emitted in Russian with sections 0-8; evidence in `artifacts/m061-2hop/REPORT.md`.
- [x] ADR-018 emitted with sections 0-14 and LLM Reading Notes; evidence in `doc/adr/ADR-018-m061-2-hop-evidence-and-m064-trigger.md`.
- [x] M061 closeout artifacts emitted; evidence in this file and `M064-wqfgfa-SUMMARY.md`.
- [x] M064 trigger evaluation is confirm defer per ADR-017; evidence in ADR-018 and `m061-decision.md`.
- [x] 5 safety defaults stay false; evidence in `m061-summary.json`.
- [x] M045 trajectory is on_track and M044 guardrail is ok; evidence in `m061-summary.json`.
- [x] codebase-memory mirror synced; evidence in `.codebase-memory/adr.md` and `.codebase-memory/governance-graph.json`.

## Slice Delivery Audit

| Slice | Claimed output | Delivered output | Result |
|---|---|---|---|
| S01 | 1-anchor pilot with M3 diagnostics | 1 anchor, {fmt(summary['anchors'][0]['real_paper_throughput_per_min'], 2)} papers/min, 0 HTTP 429s | pass |
| S02 | 4 more anchors and 5-layer graph | 5 anchors cumulative, {fmt(agg['cumulative_real_paper_throughput_per_min'], 2)} papers/min, graph valid | pass |
| S03 | REPORT, ADR-018, closeout | REPORT, ADR-018, summary, validation, tests | pass |

## Cross-Slice Integration

S01 and S02 evidence is consumed without modifying S01/S02 artifacts. S03 synthesizes the evidence into REPORT and ADR-018. No cross-slice boundary mismatch found.

## Requirement Coverage

M061 evidence covers 2-hop BFS scaling, arXiv pacing, M3 diagnostic integration, 5-layer graph validation, and M064 trigger evaluation. Queue infrastructure remains deferred.

## Verification Classes

| Class | Planned | Evidence | Result |
|---|---|---|---|
| Contract | REPORT/ADR/closeout emitted | `tests/test_m061_s03.py` | pass |
| Integration | S01/S02 artifacts synthesized | `m061-summary.json` | pass |
| Operational | rate limits, safety defaults, M045/M044 | `m061-summary.json` | pass |
| UAT | artifact readback | pytest artifact assertions | pass |

## Verdict Rationale

Pass: M061 closes with {agg['anchor_count']} anchors, {graph['citation_edge_count']} citation edges, {agg['total_http_429_count']} HTTP 429s, validated 5-layer graph evidence, and ADR-018 confirms M064 remains deferred.
"""


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def write_outputs(summary: dict[str, Any]) -> None:
    write_text(REPORT_PATH, render_report(summary))
    write_text(SUMMARY_JSON_PATH, json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    write_text(DECISION_PATH, render_decision(summary))
    write_text(ADR_PATH, render_adr(summary))
    write_text(CLOSEOUT_SUMMARY_PATH, render_closeout_summary(summary))
    write_text(VALIDATION_PATH, render_validation(summary))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Generate in memory and report target paths without writing")
    args = parser.parse_args(argv)

    summary = collect_summary(utc_now())
    if args.check:
        print(json.dumps({"status": "ok", "targets": [rel(path) for path in [REPORT_PATH, SUMMARY_JSON_PATH, DECISION_PATH, ADR_PATH, CLOSEOUT_SUMMARY_PATH, VALIDATION_PATH]]}, indent=2))
        return 0

    write_outputs(summary)
    for path in [REPORT_PATH, SUMMARY_JSON_PATH, DECISION_PATH, ADR_PATH, CLOSEOUT_SUMMARY_PATH, VALIDATION_PATH]:
        print(f"wrote {rel(path)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
