# M061 REPORT: 2-hop BFS evidence and M064 trigger evaluation

Generated: 2026-06-13T10:51:29Z  
Scope: M064-wqfgfa S01-S03 evidence package for M061 2-hop BFS closeout.  
Network host reference: `127.0.0.1`.

## 0. Резюме M061

M061 завершил 2-hop BFS на 5 anchor papers: `2605.18747, 2401.04016, 2207.05608, 2505.19443, 2510.12157`. Citation-layer граф содержит 2662 nodes и 8911 citation edges; всего в 5-layer diagnostic graph 14025 edges. arXiv acquisition сделал 323 requests и получил 0 HTTP 429s.

Итоговое решение: **CONFIRM DEFER M064**. Синхронное выполнение достаточно для текущего масштаба; queue execution remains deferred per ADR-017. Graph writes is not authorized, production import is not authorized, fact promotion is not authorized, external network is disabled by default, and LLM calls are disabled by default.

## 1. Контекст: почему 2-hop BFS

ADR-010 задал 2-hop BFS как способ проверить расширение citation evidence без преждевременного production import. ADR-017 отдельно запретил строить queue infrastructure до завершения end-to-end pipeline evidence: сначала M061, M062 и M063, затем повторная оценка необходимости M064.

M061 проверял не только breadth of retrieval, но и operational pacing: arXiv requests должны идти синхронно, с documented rate limiting и без HTTP 429. Network override был scoped только на M064-wqfgfa S01/S02 acquisition; он не меняет safety defaults.

## 2. S01 v2 pilot results

S01 v2 обработал 1 anchor `2605.18747` и подтвердил GO to S02. Пилот дал 2491 new 2-hop arXiv IDs, 30 fully processed papers, 7.26 papers/min и 0 HTTP 429s.

Network override worked: external acquisition был явно scoped to M064-wqfgfa S01, while external network is disabled by default. M3 judge calls stayed diagnostic-only through evidence reuse.

## 3. S02 results

S02 добавил 4 anchors и довёл полный набор до 5 anchors. Четыре S02 anchors дали 120 processed papers и 259 arXiv requests; cumulative throughput across S01+S02 is 7.11 papers/min.

5-layer graph validates: `structural_graph_valid=true`, `layer_count=5`. One anchor used fallback acquisition for missing M056 corpus presence, but this remained documented and diagnostic-only.

| Anchor | 1-hop refs | 2-hop new arXiv IDs | Processed papers | M3 success | Throughput papers/min | arXiv requests | HTTP 429s | Fallback |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| 2605.18747 | 165 | 2491 | 30 | 100.0% | 7.26 | 64 | 0 | false |
| 2401.04016 | 166 | 2496 | 30 | 100.0% | 6.78 | 67 | 0 | true |
| 2207.05608 | 165 | 2480 | 30 | 100.0% | 7.22 | 64 | 0 | false |
| 2505.19443 | 165 | 2416 | 30 | 100.0% | 7.21 | 64 | 0 | false |
| 2510.12157 | 165 | 2479 | 30 | 100.0% | 7.12 | 64 | 0 | false |

## 4. arXiv rate limit metrics

Across M061, arXiv acquisition made 323 requests: 323 is the recorded cumulative total, with 0 HTTP 429 responses and 2.88s average pacing. The configured minimum interval was 3.0s, and retry/backoff honored the no-429 path.

The observed request distribution by anchor is captured in `m061-summary.json`; no evidence suggests the synchronous pacing model needs replacement now.

## 5. M3 judge integration

M3 judge integration succeeded for all anchors with 100.0% success. The binding remains diagnostic-only: graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default outside explicitly scoped diagnostics.

This supports ADR-014's model choice without promoting judge outputs to production facts.

## 6. 5-layer graph stats

Citation layer: 2662 nodes, 8911 edges. Full diagnostic graph: 5 layers, 14025 total edges, 2912 layer-summed nodes.

| Layer | Nodes | Edges |
|---|---:|---:|
| citation_m056_plus_m061_2hop | 2662 | 8911 |
| table_similarity_m057 | 83 | 4934 |
| figure_similarity_m057_v1 | 1 | 15 |
| figure_similarity_m058_v2 | 16 | 15 |
| judge_scores_m3_m060g_diagnostic | 150 | 150 |

## 7. ADR-018 evaluation + M064 trigger decision

ADR-018 records the trigger evaluation: **CONFIRM DEFER M064**. The ADR-017 trigger is not met because M061 proves synchronous execution is sufficient at this scale: 7.11 papers/min, no HTTP 429s, and no queue-specific failure mode.

M045 trajectory: `on_track`. M044 guardrail: `ok`. Queue execution remains false; sync execution remains true.

## 8. Lessons + next milestones

- M061 shows that disciplined sync execution can safely cover the current 2-hop BFS evidence package.
- M062 should harden fd production paths and failure surfaces before any async queue investment.
- M063 should settle GraphDB selection and graph persistence boundaries before queue/DAG infrastructure.
- M064 remains deferred until ADR-017 revisability conditions are met and evidence shows queue execution is needed.
