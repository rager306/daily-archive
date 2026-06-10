# M056 1-hop BFS graph-readiness report

Schema version: `m056-bfs-graph-report.v1`
Milestone: `M056-lchpnp`
Anchor: `2605.18747`
Status: final diagnostic synthesis for S07

## 1. Executive summary

M056 executed a 1-hop BFS expansion from anchor `2605.18747` across 166 extracted references.
The run acquired 148 referenced PDFs, and with the anchor produced 149 unique PDFs for analysis.
Acquisition success was 100.0% (148/166); the remaining references were not included in the local PDF corpus.
The target-set connectivity metric found 7-8 cumulative directed edges after six waves, which is a saturation signal rather than a graph-ready structure.
The self-citation cluster remained 0.0%, indicating healthy source diversity around the anchor rather than a narrow author-local cluster.
All five safety defaults stayed false throughout the wave packets, candidate edge packet, report, and ADR recommendation.

The main conclusion is deliberately conservative: 1-hop BFS is useful for parser-scale evidence, but insufficient for M058 graph-readiness.
A 2-hop expansion, or a materially different anchor strategy, is needed before treating the corpus as ready for graph import evaluation.

## 2. Scope and inputs

| Input | Value |
| --- | --- |
| Anchor PDF | `2605.18747` |
| Total extracted references | 166 |
| Acquired referenced PDFs | 148 |
| Unique PDFs including anchor | 149 |
| Waves | 6 |
| Parser evidence | GROBID fulltext TEI + OpenDataLoader markdown packets |
| Candidate edge evidence | `grobid_biblstruct` |
| Graph writes | false |
| Production import | false |

## 3. Mermaid evidence flow

```mermaid
flowchart LR
  A[Anchor 2605.18747] --> B[1-hop BFS over 166 references]
  B --> C[148 acquired referenced PDFs]
  C --> D[149 unique PDFs including anchor]
  D --> E[7-8 target-set internal edges]
  E --> F[Saturation signal]
  F --> G[Recommend 2-hop expansion for M058]
  D --> H[Candidate citation JSON]
  H --> I[Diagnostic only: graph writes false]
```

## 4. Per-wave acquisition and parser summary

| Wave | Requested | Acquired | GROBID success | OpenDataLoader success | Status counts | Safety defaults false |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| 1 | 30 | 30 | 30 | 29 | acquired: 30 | `true` |
| 2 | 30 | 30 | 30 | 28 | acquired: 30 | `true` |
| 3 | 30 | 30 | 30 | 29 | acquired: 30 | `true` |
| 4 | 30 | 30 | 30 | 30 | acquired: 30 | `true` |
| 5 | 30 | 30 | 30 | 29 | acquired: 30 | `true` |
| 6 | 16 | 16 | 16 | 14 | acquired: 16 | `true` |

## 5. Edge saturation chart

The chart below uses the target-set connectivity metric from the wave analyses: edges from wave PDFs to the 20-PDF M055 target set plus the anchor.
This metric is intentionally narrower than the full candidate citation JSON. It answers whether 1-hop expansion densifies the known target set.

| Wave | New target-set edges | Cumulative increment sum | Saturation interpretation |
| --- | ---: | ---: | --- |
| 1 | 3 | 3 | expanded |
| 2 | 2 | 5 | saturated |
| 3 | 1 | 6 | saturated |
| 4 | 2 | 8 | expanded |
| 5 | 0 | 8 | saturated |
| 6 | 0 | 8 | saturated |

```text
Wave 1: +3  cumulative 3  ███
Wave 2: +2  cumulative 5  ██
Wave 3: +1  cumulative 6  █
Wave 4: +2  cumulative 8  ██
Wave 5: +0  cumulative 8  ·
Wave 6: +0  cumulative 8  ·
```

The wave increments sum to 8; de-duplicated target-set cumulative evidence in the wave JSON is 7.
That 7-8 range is too sparse for meaningful graph-readiness at 149 nodes.

## 6. Candidate edge packet summary

| Metric | Value |
| --- | ---: |
| Candidate nodes | 2,448 |
| Candidate citation edges | 3,983 |
| Corpus-internal candidate edges | 427 |
| TEI files read | 167 |
| biblStruct records with arXiv evidence | 4,350 |
| Parse errors | 0 |

The candidate JSON intentionally preserves broad GROBID citation evidence, including references outside the M056 corpus.
The graph-readiness recommendation, however, is based on the target-set saturation metric because M058 needs a useful connected seed graph, not just many outbound citation candidates.

## 7. Self-citation cluster

| Metric | Value |
| --- | ---: |
| Anchor first author | Xuying Ning |
| Matching acquired PDFs | 0 |
| Acquired PDFs checked | 148 |
| Self-citation cluster ratio | 0.0% |

This is a healthy diversity signal. Saturation is therefore not explained by an overly tight self-citation cluster around the anchor author set.

## 8. Category distribution

| Category | Unique PDFs |
| --- | ---: |
| cs-ai | 19 |
| cs-cl | 32 |
| cs-cv | 2 |
| cs-lg | 14 |
| mixed-source | 82 |

## 9. Length distribution

| Length bucket | Unique PDFs |
| --- | ---: |
| long:25-49 | 51 |
| medium:10-24 | 72 |
| short:<10 | 13 |
| very-long:50+ | 13 |

## 10. Per-PDF summary table

| # | arXiv ID | Wave | Category | Pages | GROBID refs | GROBID biblStructs | ODL status | Candidate edges | Corpus candidate edges |
| ---: | --- | ---: | --- | ---: | ---: | ---: | --- | ---: | ---: |
| 1 | `1703.04247` | 6 | mixed-source | 8 | 69 | 14 | success | 2 | 0 |
| 2 | `2107.03374` | 1 | cs-lg | 35 | 204 | 121 | success | 35 | 0 |
| 3 | `2108.07732` | 1 | mixed-source | 34 | 239 | 104 | success | 15 | 0 |
| 4 | `2203.13474` | 1 | cs-lg | 25 | 129 | 49 | success | 11 | 2 |
| 5 | `2204.01691` | 1 | mixed-source | 34 | 203 | 103 | success | 33 | 1 |
| 6 | `2207.05608` | 6 | mixed-source | 25 | 185 | 109 | success | 37 | 2 |
| 7 | `2210.14306` | 5 | mixed-source | 41 | 82 | 41 | success | 8 | 2 |
| 8 | `2211.12588` | 1 | cs-cl | 20 | 113 | 50 | success | 32 | 3 |
| 9 | `2302.06590` | 5 | mixed-source | 19 | 28 | 15 | success | 5 | 2 |
| 10 | `2303.03378` | 6 | mixed-source | 18 | 147 | 67 | success | 44 | 3 |
| 11 | `2304.05128` | 4 | cs-cl | 78 | 326 | 75 | success | 28 | 3 |
| 12 | `2307.01928` | 2 | mixed-source | 24 | 139 | 66 | success | 34 | 1 |
| 13 | `2307.04349` | 2 | cs-ai | 15 | 71 | 33 | success | 19 | 3 |
| 14 | `2307.05973` | 6 | mixed-source | 23 | 184 | 139 | success | 76 | 3 |
| 15 | `2308.03688` | 1 | cs-ai | 58 | 189 | 111 | success | 36 | 3 |
| 16 | `2310.02003` | 5 | mixed-source | 61 | 221 | 88 | success | 28 | 3 |
| 17 | `2310.06770` | 1 | cs-cl | 52 | 174 | 70 | success | 9 | 0 |
| 18 | `2310.08067` | 4 | cs-ai | 8 | 26 | 25 | success | 13 | 0 |
| 19 | `2310.08560` | 4 | cs-ai | 13 | 53 | 35 | success | 24 | 1 |
| 20 | `2310.10021` | 2 | mixed-source | 24 | 152 | 68 | success | 7 | 2 |
| 21 | `2312.04474` | 1 | cs-cl | 19 | 122 | 55 | success | 37 | 4 |
| 22 | `2401.03065` | 3 | mixed-source | 71 | 369 | 127 | success | 75 | 4 |
| 23 | `2401.08500` | 5 | cs-lg | 10 | 49 | 14 | success | 8 | 2 |
| 24 | `2402.01411` | 5 | mixed-source | 45 | 131 | 83 | success | 37 | 6 |
| 25 | `2403.07974` | 3 | mixed-source | 46 | 192 | 109 | success | 67 | 6 |
| 26 | `2403.08291` | 5 | cs-lg | 6 | 32 | 9 | success | 7 | 0 |
| 27 | `2404.02183` | 4 | mixed-source | 11 | 61 | 29 | success | 16 | 1 |
| 28 | `2406.01006` | 2 | cs-cl | 27 | 143 | 59 | success | 10 | 3 |
| 29 | `2407.01489` | 5 | mixed-source | 25 | 172 | 109 | success | 28 | 2 |
| 30 | `2408.01055` | 5 | mixed-source | 12 | 52 | 42 | success | 11 | 0 |
| 31 | `2408.03095` | 5 | mixed-source | 25 | 152 | 71 | success | 24 | 0 |
| 32 | `2409.03733` | 3 | cs-lg | 52 | 144 | 55 | success | 11 | 2 |
| 33 | `2409.16299` | 5 | mixed-source | 49 | 125 | 62 | success | 32 | 4 |
| 34 | `2410.02089` | 2 | cs-cl | 23 | 95 | 49 | success | 23 | 4 |
| 35 | `2410.06992` | 5 | mixed-source | 14 | 29 | 32 | success | 6 | 0 |
| 36 | `2410.13825` | 6 | mixed-source | 33 | 287 | 35 | success | 8 | 0 |
| 37 | `2411.13244` | 4 | cs-cl | 15 | 76 | 31 | success | 22 | 1 |
| 38 | `2412.07822` | 5 | mixed-source | 7 | 89 | 35 | success | 15 | 0 |
| 39 | `2412.15118` | 2 | cs-cl | 23 | 90 | 126 | success | 82 | 6 |
| 40 | `2412.15305` | 3 | mixed-source | 16 | 80 | 47 | success | 24 | 3 |
| 41 | `2501.07811` | 3 | mixed-source | 20 | 90 | 49 | success | 19 | 3 |
| 42 | `2501.13958` | 3 | cs-cl | 26 | 378 | 243 | success | 53 | 0 |
| 43 | `2501.17167` | 4 | mixed-source | 19 | 103 | 44 | success | 15 | 2 |
| 44 | `2501.18653` | 5 | mixed-source | 19 | 64 | 19 | success | 12 | 1 |
| 45 | `2502.04350` | 2 | cs-cl | 28 | 104 | 60 | success | 32 | 3 |
| 46 | `2502.12115` | 5 | cs-lg | 39 | 80 | 34 | success | 3 | 1 |
| 47 | `2503.05703` | 2 | cs-lg | 18 | 65 | 23 | success | 8 | 2 |
| 48 | `2503.09572` | 3 | cs-cl | 44 | 153 | 62 | success | 46 | 1 |
| 49 | `2503.13657` | 6 | mixed-source | 47 | 124 | 86 | success | 30 | 1 |
| 50 | `2503.15223` | 5 | mixed-source | 13 | 106 | 63 | success | 16 | 4 |
| 51 | `2504.10046` | 3 | mixed-source | 23 | 148 | 47 | success | 32 | 5 |
| 52 | `2504.11354` | 2 | cs-ai | 24 | 53 | 32 | success | 20 | 0 |
| 53 | `2504.15257` | 5 | cs-ai | 17 | 125 | 67 | success | 43 | 2 |
| 54 | `2504.15965` | 3 | mixed-source | 26 | 271 | 156 | success | 90 | 1 |
| 55 | `2504.19413` | 6 | mixed-source | 23 | 42 | 30 | success | 10 | 0 |
| 56 | `2504.21801` | 2 | cs-cl | 39 | 96 | 70 | success | 20 | 1 |
| 57 | `2505.00212` | 6 | mixed-source | 17 | 85 | 46 | success | 22 | 0 |
| 58 | `2505.03864` | 4 | mixed-source | 16 | 88 | 44 | success | 23 | 0 |
| 59 | `2505.10571` | 3 | cs-cl | 15 | 89 | 64 | success | 18 | 1 |
| 60 | `2505.10819` | 2 | cs-ai | 30 | 143 | 81 | success | 16 | 0 |
| 61 | `2505.16901` | 3 | mixed-source | 35 | 148 | 72 | success | 45 | 2 |
| 62 | `2505.17653` | 3 | cs-ai | 29 | 87 | 43 | success | 27 | 0 |
| 63 | `2505.18646` | 5 | mixed-source | 16 | 72 | 31 | success | 19 | 3 |
| 64 | `2505.19443` | 4 | mixed-source | 35 | 365 | 261 | success | 115 | 5 |
| 65 | `2505.23135` | 2 | cs-lg | 40 | 265 | 63 | success | 15 | 4 |
| 66 | `2506.02943` | 5 | mixed-source | 26 | 226 | 91 | success | 26 | 1 |
| 67 | `2506.11442` | 4 | mixed-source | 19 | 72 | 28 | success | 2 | 0 |
| 68 | `2506.18019` | 4 | cs-ai | 20 | 434 | 245 | success | 75 | 1 |
| 69 | `2506.18403` | 5 | mixed-source | 10 | 88 | 37 | success | 16 | 2 |
| 70 | `2507.05269` | 3 | mixed-source | 55 | 207 | 118 | success | 19 | 4 |
| 71 | `2507.06134` | 4 | cs-ai | 26 | 78 | 54 | success | 3 | 0 |
| 72 | `2507.07957` | 4 | cs-cl | 17 | 87 | 39 | success | 24 | 2 |
| 73 | `2507.23348` | 4 | mixed-source | 19 | 134 | 54 | success | 27 | 1 |
| 74 | `2507.23370` | 5 | mixed-source | 23 | 141 | 68 | success | 9 | 1 |
| 75 | `2508.00083` | 3 | mixed-source | 24 | 321 | 164 | success | 1 | 0 |
| 76 | `2508.03613` | 2 | cs-lg | 24 | 80 | 47 | success | 27 | 2 |
| 77 | `2508.04289` | 2 | cs-ai | 10 | 26 | 19 | success | 4 | 0 |
| 78 | `2508.07434` | 3 | cs-cl | 18 | 81 | 58 | success | 40 | 6 |
| 79 | `2508.07468` | 2 | cs-ai | 14 | 53 | 34 | low_or_unavailable | 6 | 0 |
| 80 | `2508.13732` | 4 | mixed-source | 12 | 64 | 32 | success | 12 | 0 |
| 81 | `2508.18675` | 3 | mixed-source | 6 | 45 | 48 | low_or_unavailable | 11 | 0 |
| 82 | `2509.02544` | 4 | cs-ai | 30 | 155 | 90 | success | 32 | 1 |
| 83 | `2509.03312` | 6 | mixed-source | 18 | 117 | 85 | success | 27 | 3 |
| 84 | `2509.10397` | 6 | mixed-source | 10 | 95 | 74 | success | 57 | 0 |
| 85 | `2509.16198` | 3 | cs-cl | 54 | 70 | 58 | success | 14 | 2 |
| 86 | `2509.16941` | 5 | mixed-source | 20 | 46 | 22 | success | 12 | 4 |
| 87 | `2509.18597` | 2 | mixed-source | 34 | 54 | 36 | success | 12 | 0 |
| 88 | `2509.24219` | 2 | mixed-source | 8 | 36 | 19 | success | 0 | 0 |
| 89 | `2509.25370` | 6 | mixed-source | 32 | 60 | 25 | success | 17 | 3 |
| 90 | `2510.03342` | 6 | mixed-source | 62 | 134 | 46 | success | 21 | 2 |
| 91 | `2510.03902` | 3 | mixed-source | 18 | 51 | 28 | success | 5 | 1 |
| 92 | `2510.05156` | 4 | mixed-source | 22 | 50 | 35 | success | 3 | 0 |
| 93 | `2510.10292` | 2 | cs-cv | 29 | 94 | 43 | success | 10 | 0 |
| 94 | `2510.11967` | 4 | cs-cl | 22 | 91 | 44 | success | 25 | 1 |
| 95 | `2510.12157` | 2 | cs-lg | 44 | 149 | 43 | success | 35 | 0 |
| 96 | `2510.18471` | 2 | mixed-source | 18 | 80 | 57 | success | 30 | 4 |
| 97 | `2510.20909` | 2 | cs-cl | 38 | 141 | 93 | success | 48 | 0 |
| 98 | `2510.23010` | 3 | mixed-source | 10 | 50 | 24 | success | 5 | 1 |
| 99 | `2510.26094` | 2 | cs-ai | 23 | 92 | 52 | success | 27 | 3 |
| 100 | `2511.01854` | 4 | cs-cl | 10 | 52 | 41 | success | 22 | 0 |
| 101 | `2511.03690` | 4 | mixed-source | 19 | 39 | 21 | success | 1 | 0 |
| 102 | `2511.10621` | 2 | cs-cl | 32 | 124 | 58 | low_or_unavailable | 38 | 0 |
| 103 | `2511.13646` | 3 | mixed-source | 20 | 108 | 48 | success | 21 | 6 |
| 104 | `2511.20857` | 4 | cs-cl | 27 | 90 | 50 | success | 22 | 2 |
| 105 | `2512.02002` | 2 | mixed-source | 8 | 84 | 40 | success | 9 | 0 |
| 106 | `2512.10563` | 2 | cs-ai | 22 | 26 | 36 | success | 6 | 0 |
| 107 | `2512.12806` | 6 | mixed-source | 7 | 30 | 23 | low_or_unavailable | 3 | 0 |
| 108 | `2512.17419` | 5 | mixed-source | 21 | 30 | 21 | success | 2 | 0 |
| 109 | `2512.23631` | 5 | cs-lg | 25 | 62 | 59 | low_or_unavailable | 19 | 3 |
| 110 | `2601.03515` | 1 | cs-cl | 34 | 142 | 45 | success | 16 | 2 |
| 111 | `2601.05808` | 1 | cs-cl | 32 | 102 | 62 | success | 18 | 0 |
| 112 | `2601.06789` | 1 | mixed-source | 17 | 52 | 37 | success | 16 | 2 |
| 113 | `2601.08816` | 6 | mixed-source | 30 | 145 | 68 | success | 8 | 1 |
| 114 | `2601.11655` | 5 | mixed-source | 26 | 233 | 214 | success | 135 | 13 |
| 115 | `2601.11868` | 5 | mixed-source | 84 | 87 | 94 | success | 9 | 0 |
| 116 | `2601.12762` | 4 | mixed-source | 24 | 98 | 12 | success | 4 | 0 |
| 117 | `2601.13247` | 3 | cs-cl | 19 | 140 | 122 | success | 83 | 1 |
| 118 | `2601.15709` | 3 | cs-ai | 9 | 76 | 38 | success | 25 | 2 |
| 119 | `2601.16746` | 4 | mixed-source | 30 | 147 | 90 | success | 43 | 5 |
| 120 | `2601.19510` | 2 | mixed-source | 8 | 43 | 25 | success | 12 | 2 |
| 121 | `2602.05842` | 2 | cs-cl | 20 | 149 | 83 | success | 8 | 0 |
| 122 | `2602.05892` | 5 | cs-lg | 34 | 81 | 42 | success | 21 | 7 |
| 123 | `2602.06052` | 3 | cs-cl | 83 | 1007 | 546 | success | 268 | 14 |
| 124 | `2602.09944` | 4 | mixed-source | 5 | 46 | 29 | success | 16 | 0 |
| 125 | `2602.10090` | 3 | cs-ai | 43 | 191 | 69 | success | 11 | 0 |
| 126 | `2602.11757` | 3 | cs-cv | 28 | 49 | 46 | success | 9 | 0 |
| 127 | `2602.13962` | 3 | mixed-source | 13 | 106 | 55 | success | 25 | 3 |
| 128 | `2602.14337` | 4 | mixed-source | 11 | 65 | 49 | success | 43 | 6 |
| 129 | `2602.23647` | 3 | mixed-source | 32 | 153 | 73 | success | 30 | 4 |
| 130 | `2603.03329` | 1 | cs-cl | 21 | 40 | 21 | success | 7 | 0 |
| 131 | `2603.03836` | 1 | mixed-source | 16 | 92 | 54 | success | 12 | 1 |
| 132 | `2603.04177` | 1 | mixed-source | 29 | 71 | 40 | success | 1 | 0 |
| 133 | `2603.04257` | 1 | cs-cl | 22 | 66 | 48 | success | 30 | 2 |
| 134 | `2603.05621` | 1 | mixed-source | 9 | 68 | 33 | success | 7 | 0 |
| 135 | `2603.11226` | 1 | mixed-source | 25 | 112 | 34 | success | 24 | 4 |
| 136 | `2603.13258` | 1 | cs-lg | 18 | 76 | 58 | success | 26 | 4 |
| 137 | `2603.19329` | 1 | mixed-source | 25 | 91 | 52 | success | 27 | 6 |
| 138 | `2603.21430` | 1 | cs-ai | 10 | 72 | 48 | success | 13 | 0 |
| 139 | `2603.21520` | 1 | cs-cl | 19 | 106 | 38 | success | 16 | 3 |
| 140 | `2603.24533` | 1 | cs-lg | 20 | 147 | 119 | success | 64 | 1 |
| 141 | `2603.25723` | 1 | cs-cl | 22 | 76 | 67 | success | 0 | 0 |
| 142 | `2603.26664` | 1 | mixed-source | 9 | 35 | 23 | success | 0 | 0 |
| 143 | `2603.28052` | 1 | cs-ai | 26 | 103 | 61 | success | 10 | 1 |
| 144 | `2603.28119` | 1 | mixed-source | 12 | 79 | 40 | success | 16 | 5 |
| 145 | `2604.08224` | 1 | mixed-source | 54 | 333 | 199 | success | 102 | 4 |
| 146 | `2604.11839` | 1 | mixed-source | 13 | 18 | 17 | low_or_unavailable | 5 | 0 |
| 147 | `2604.14228` | 1 | mixed-source | 46 | 148 | 106 | success | 39 | 6 |
| 148 | `2604.25850` | 1 | cs-cl | 35 | 135 | 54 | success | 0 | 0 |
| 149 | `2605.18747` | 1 | cs-cl | 102 | 958 | 479 | anchor-not-routed | 171 | 148 |

## 11. Routing recommendation

ADR-009 remains the correct parser routing rule after M056 scale-up.
M055deep showed fulltext-aware hybrid routing at 20-PDF scale, with 95% hybrid success under the operational criteria.
M056 extends that evidence to 149 unique PDFs: GROBID fulltext remains the citation and TEI structure source, while OpenDataLoader remains useful for body markdown when it is successful and non-low-quality.

Recommended parser routing for downstream graph-readiness work:

1. Use GROBID fulltext TEI for metadata, biblStruct citations, references, and native TEI structure.
2. Use OpenDataLoader body markdown only when its packet is successful, non-low-quality, and above the body evidence threshold.
3. Preserve candidate evidence as diagnostic JSON until a later ADR or gate explicitly authorizes graph import.
4. Treat 1-hop BFS from 2605.18747 as parser-scale evidence, not graph-readiness evidence.

## 11.1 Graph-readiness recommendation for M058

M058 should not use the M056 1-hop corpus as a graph-ready import set by itself.
The empirical target-set signal is saturated at only 7-8 edges over 149 nodes, which is too sparse for meaningful graph traversal, clustering, or candidate promotion decisions.
M058 should require either a 2-hop BFS expansion or a different anchor strategy before graph-readiness can be assessed fairly.

## 12. Safety defaults and authorization boundary

This evidence is not authorized for graph import or fact promotion.

### 12.1 Packet-compatible safety defaults

| Safety default | Value |
| --- | --- |
| `graph_import_allowed` | `false` |
| `graphdb_written` | `false` |
| `import_eligible` | `false` |
| `ladybugdb_written` | `false` |
| `production_import_attempted` | `false` |

### 12.2 Human-readable safety flags

| Safety flag | Value |
| --- | --- |
| `graph_writes` | `false` |
| `production_import_attempted` | `false` |
| `promotion_allowed` | `false` |
| `facts_promoted` | `false` |
| `external_mutation_allowed` | `false` |

No LadybugDB writes, graph writes, production import, fact promotion, or external mutation were performed by this report renderer.

## 13. Wave-by-wave narrative

### 13.1 Wave 1

- Requested references: 30
- Acquired PDFs: 30
- GROBID packets: 30
- OpenDataLoader packets: 30
- New target-set edges: 3
- Category distribution: cs-ai: 3, cs-cl: 10, cs-cv: 0, cs-lg: 4, mixed-source: 13
- Length distribution: long: 10, medium: 17, short: 3
- Safety: all default flags false; evidence is not authorized for graph import or fact promotion.

### 13.2 Wave 2

- Requested references: 30
- Acquired PDFs: 30
- GROBID packets: 30
- OpenDataLoader packets: 30
- New target-set edges: 2
- Category distribution: cs-ai: 7, cs-cl: 8, cs-cv: 1, cs-lg: 4, mixed-source: 10
- Length distribution: long_26_plus: 10, medium_11_25: 15, short_1_10: 5
- Safety: all default flags false; evidence is not authorized for graph import or fact promotion.

### 13.3 Wave 3

- Requested references: 30
- Acquired PDFs: 30
- GROBID packets: 30
- OpenDataLoader packets: 30
- New target-set edges: 1
- Category distribution: cs-ai: 4, cs-cl: 7, cs-cv: 1, cs-lg: 2, mixed-source: 16
- Length distribution: long_26_plus: 15, medium_11_25: 11, short_1_10: 4
- Safety: all default flags false; evidence is not authorized for graph import or fact promotion.

### 13.4 Wave 4

- Requested references: 30
- Acquired PDFs: 30
- GROBID packets: 30
- OpenDataLoader packets: 30
- New target-set edges: 2
- Category distribution: cs-ai: 5, cs-cl: 10, mixed-source: 15
- Length distribution: long: 20, medium: 8, short: 2
- Safety: all default flags false; evidence is not authorized for graph import or fact promotion.

### 13.5 Wave 5

- Requested references: 30
- Acquired PDFs: 30
- GROBID packets: 30
- OpenDataLoader packets: 30
- New target-set edges: 0
- Category distribution: cs-ai: 1, cs-cl: 2, cs-lg: 5, mixed-source: 22
- Length distribution: long: 21, medium: 7, short: 2
- Safety: all default flags false; evidence is not authorized for graph import or fact promotion.

### 13.6 Wave 6

- Requested references: 16
- Acquired PDFs: 16
- GROBID packets: 16
- OpenDataLoader packets: 16
- New target-set edges: 0
- Category distribution: mixed-source: 16
- Length distribution: 1-10: 3, 11-25: 8, 26+: 5
- Safety: all default flags false; evidence is not authorized for graph import or fact promotion.

## 14. Detailed per-PDF evidence appendix

This appendix intentionally expands one short evidence block per unique corpus PDF so the report remains reviewable without opening every packet.

### 14.1 `1703.04247`

- Title: Deepfm: a factorizationmachine based neural network for ctr prediction
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 8.
- GROBID evidence: refs 69; biblStructs 14; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 35849; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 2; corpus-internal candidates 0.

### 14.2 `2107.03374`

- Title: Evaluating large language models trained on code
- Corpus placement: first seen in wave 1; category cs-lg; estimated pages 35.
- GROBID evidence: refs 204; biblStructs 121; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 155457; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 35; corpus-internal candidates 0.

### 14.3 `2108.07732`

- Title: Program synthesis with large language models
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 34.
- GROBID evidence: refs 239; biblStructs 104; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 129132; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 15; corpus-internal candidates 0.

### 14.4 `2203.13474`

- Title: Codegen: An open large language model for code with multi-turn program synthesis
- Corpus placement: first seen in wave 1; category cs-lg; estimated pages 25.
- GROBID evidence: refs 129; biblStructs 49; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 77879; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 11; corpus-internal candidates 2.

### 14.5 `2204.01691`

- Title: Do as i can, not as i say: Grounding language in robotic affordances
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 34.
- GROBID evidence: refs 203; biblStructs 103; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 114778; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 33; corpus-internal candidates 1.

### 14.6 `2207.05608`

- Title: Inner monologue: Embodied reasoning through planning with language models
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 25.
- GROBID evidence: refs 185; biblStructs 109; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 104183; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 37; corpus-internal candidates 2.

### 14.7 `2210.14306`

- Title: Reading between the lines: Modeling user behavior and costs in AI-assisted programming
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 41.
- GROBID evidence: refs 82; biblStructs 41; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 94446; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 8; corpus-internal candidates 2.

### 14.8 `2211.12588`

- Title: Program of thoughts prompting: Disentangling computation from reasoning for numerical reasoning tasks
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 20.
- GROBID evidence: refs 113; biblStructs 50; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 55300; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 32; corpus-internal candidates 3.

### 14.9 `2302.06590`

- Title: The impact of ai on developer productivity: Evidence from github copilot
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 19.
- GROBID evidence: refs 28; biblStructs 15; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 22396; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 5; corpus-internal candidates 2.

### 14.10 `2303.03378`

- Title: Palm-e: An embodied multimodal language model
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 18.
- GROBID evidence: refs 147; biblStructs 67; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 79017; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 44; corpus-internal candidates 3.

### 14.11 `2304.05128`

- Title: Teaching large language models to self-debug
- Corpus placement: first seen in wave 4; category cs-cl; estimated pages 78.
- GROBID evidence: refs 326; biblStructs 75; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 193906; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 28; corpus-internal candidates 3.

### 14.12 `2307.01928`

- Title: Robots that ask for help: Uncertainty alignment for large language model planners
- Corpus placement: first seen in wave 2; category mixed-source; estimated pages 24.
- GROBID evidence: refs 139; biblStructs 66; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 101363; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 34; corpus-internal candidates 1.

### 14.13 `2307.04349`

- Title: Rltf: Reinforcement learning from unit test feedback
- Corpus placement: first seen in wave 2; category cs-ai; estimated pages 15.
- GROBID evidence: refs 71; biblStructs 33; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 51819; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 19; corpus-internal candidates 3.

### 14.14 `2307.05973`

- Title: Composable 3d value maps for robotic manipulation with language models
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 23.
- GROBID evidence: refs 184; biblStructs 139; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 78770; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 76; corpus-internal candidates 3.

### 14.15 `2308.03688`

- Title: Evaluating llms as agents
- Corpus placement: first seen in wave 1; category cs-ai; estimated pages 58.
- GROBID evidence: refs 189; biblStructs 111; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 179701; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 36; corpus-internal candidates 3.

### 14.16 `2310.02003`

- Title: L2MAC: Large language model automatic computer for extensive code generation
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 61.
- GROBID evidence: refs 221; biblStructs 88; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 294178; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 28; corpus-internal candidates 3.

### 14.17 `2310.06770`

- Title: Swe-bench: Can language models resolve real-world github issues? arXiv preprint
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 52.
- GROBID evidence: refs 174; biblStructs 70; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 152260; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 9; corpus-internal candidates 0.

### 14.18 `2310.08067`

- Title: Multi-agent collaborative framework for game development
- Corpus placement: first seen in wave 4; category cs-ai; estimated pages 8.
- GROBID evidence: refs 26; biblStructs 25; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 25765; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 13; corpus-internal candidates 0.

### 14.19 `2310.08560`

- Title: Towards llms as operating systems
- Corpus placement: first seen in wave 4; category cs-ai; estimated pages 13.
- GROBID evidence: refs 53; biblStructs 35; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 57392; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 24; corpus-internal candidates 1.

### 14.20 `2310.10021`

- Title: Bootstrap your own skills: Learning to solve new tasks with large language model guidance
- Corpus placement: first seen in wave 2; category mixed-source; estimated pages 24.
- GROBID evidence: refs 152; biblStructs 68; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 83438; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 7; corpus-internal candidates 2.

### 14.21 `2312.04474`

- Title: Chain of code: Reasoning with a language model-augmented code emulator
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 19.
- GROBID evidence: refs 122; biblStructs 55; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 75807; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 37; corpus-internal candidates 4.

### 14.22 `2401.03065`

- Title: Cruxeval: A benchmark for code reasoning, understanding and execution
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 71.
- GROBID evidence: refs 369; biblStructs 127; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 180279; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 75; corpus-internal candidates 4.

### 14.23 `2401.08500`

- Title: Code generation with AlphaCodium: From prompt engineering to flow engineering
- Corpus placement: first seen in wave 5; category cs-lg; estimated pages 10.
- GROBID evidence: refs 49; biblStructs 14; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 35882; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 8; corpus-internal candidates 2.

### 14.24 `2402.01411`

- Title: Codepori: Large-scale system for autonomous software development using multi-agent technology
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 45.
- GROBID evidence: refs 131; biblStructs 83; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 132239; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 37; corpus-internal candidates 6.

### 14.25 `2403.07974`

- Title: Armando Solar-Lezama, Koushik Sen, and Ion Stoica. Livecodebench: Holistic and contamination free evaluation of large language models for code
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 46.
- GROBID evidence: refs 192; biblStructs 109; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 103697; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 67; corpus-internal candidates 6.

### 14.26 `2403.08291`

- Title: CleanAgent: Automating data standardization with LLM-based agents
- Corpus placement: first seen in wave 5; category cs-lg; estimated pages 6.
- GROBID evidence: refs 32; biblStructs 9; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 29779; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 7; corpus-internal candidates 0.

### 14.27 `2404.02183`

- Title: Self-organized agents: A LLM multi-agent framework toward ultra large-scale code generation and optimization
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 11.
- GROBID evidence: refs 61; biblStructs 29; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 47630; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 1.

### 14.28 `2406.01006`

- Title: Training code language models with comprehensive semantics
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 27.
- GROBID evidence: refs 143; biblStructs 59; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 89240; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 10; corpus-internal candidates 3.

### 14.29 `2407.01489`

- Title: Agentless: Demystifying LLM-based software engineering agents
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 25.
- GROBID evidence: refs 172; biblStructs 109; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 110019; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 28; corpus-internal candidates 2.

### 14.30 `2408.01055`

- Title: Llm as runtime error handler: A promising pathway to adaptive self-healing of software systems
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 12.
- GROBID evidence: refs 52; biblStructs 42; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 47679; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 11; corpus-internal candidates 0.

### 14.31 `2408.03095`

- Title: Testart: Improving llm-based unit testing via co-evolution of automated generation and repair iteration
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 25.
- GROBID evidence: refs 152; biblStructs 71; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 84459; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 24; corpus-internal candidates 0.

### 14.32 `2409.03733`

- Title: Planning in natural language improves llm search for code generation
- Corpus placement: first seen in wave 3; category cs-lg; estimated pages 52.
- GROBID evidence: refs 144; biblStructs 55; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 108876; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 11; corpus-internal candidates 2.

### 14.33 `2409.16299`

- Title: HyperAgent: Generalist software engineering agents to solve coding tasks at scale
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 49.
- GROBID evidence: refs 125; biblStructs 62; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 139332; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 32; corpus-internal candidates 4.

### 14.34 `2410.02089`

- Title: Rlef: Grounding code llms in execution feedback with reinforcement learning
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 23.
- GROBID evidence: refs 95; biblStructs 49; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 74815; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 23; corpus-internal candidates 4.

### 14.35 `2410.06992`

- Title: Swe-bench+: Enhanced coding benchmark for llms
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 14.
- GROBID evidence: refs 29; biblStructs 32; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 54253; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 6; corpus-internal candidates 0.

### 14.36 `2410.13825`

- Title: A simple yet strong baseline for llm-based web agents
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 33.
- GROBID evidence: refs 287; biblStructs 35; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 145524; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 8; corpus-internal candidates 0.

### 14.37 `2411.13244`

- Title: Leveraging prior experience: An expandable auxiliary knowledge base for text-to-sql
- Corpus placement: first seen in wave 4; category cs-cl; estimated pages 15.
- GROBID evidence: refs 76; biblStructs 31; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 52303; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 22; corpus-internal candidates 1.

### 14.38 `2412.07822`

- Title: MAGE: A multi-agent engine for automated RTL code generation
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 7.
- GROBID evidence: refs 89; biblStructs 35; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 40871; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 15; corpus-internal candidates 0.

### 14.39 `2412.15118`

- Title: Reasoning through execution: Unifying process and outcome rewards for code generation
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 23.
- GROBID evidence: refs 90; biblStructs 126; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 90440; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 82; corpus-internal candidates 6.

### 14.40 `2412.15305`

- Title: Tree-of-code: A tree-structured exploring framework for end-to-end code generation and execution in complex task handling
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 16.
- GROBID evidence: refs 80; biblStructs 47; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 68156; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 24; corpus-internal candidates 3.

### 14.41 `2501.07811`

- Title: CodeCoR: An LLM-based self-reflective multi-agent framework for code generation
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 20.
- GROBID evidence: refs 90; biblStructs 49; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 65695; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 19; corpus-internal candidates 3.

### 14.42 `2501.13958`

- Title: A survey of graph retrieval-augmented generation for customized large language models
- Corpus placement: first seen in wave 3; category cs-cl; estimated pages 26.
- GROBID evidence: refs 378; biblStructs 243; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 185748; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 53; corpus-internal candidates 0.

### 14.43 `2501.17167`

- Title: QualityFlow: An agentic workflow for program synthesis controlled by LLM quality checks
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 19.
- GROBID evidence: refs 103; biblStructs 44; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 69679; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 15; corpus-internal candidates 2.

### 14.44 `2501.18653`

- Title: Cogito, ergo sum: A neurobiologically-inspired cognition-memory-growth system for code generation
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 19.
- GROBID evidence: refs 64; biblStructs 19; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 86086; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 12; corpus-internal candidates 1.

### 14.45 `2502.04350`

- Title: Codesteer: Symbolicaugmented language models via code/text guidance
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 28.
- GROBID evidence: refs 104; biblStructs 60; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 78675; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 32; corpus-internal candidates 3.

### 14.46 `2502.12115`

- Title: SWE-lancer: Can frontier LLMs earn $1 million from real-world freelance software engineering
- Corpus placement: first seen in wave 5; category cs-lg; estimated pages 39.
- GROBID evidence: refs 80; biblStructs 34; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 105473; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 3; corpus-internal candidates 1.

### 14.47 `2503.05703`

- Title: What i cannot execute, i do not understand: Training and evaluating llms on program execution traces
- Corpus placement: first seen in wave 2; category cs-lg; estimated pages 18.
- GROBID evidence: refs 65; biblStructs 23; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 62018; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 8; corpus-internal candidates 2.

### 14.48 `2503.09572`

- Title: Plan-and-act: Improving planning of agents for long-horizon tasks
- Corpus placement: first seen in wave 3; category cs-cl; estimated pages 44.
- GROBID evidence: refs 153; biblStructs 62; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 143690; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 46; corpus-internal candidates 1.

### 14.49 `2503.13657`

- Title: Why do multi-agent LLM systems fail? arXiv preprint
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 47.
- GROBID evidence: refs 124; biblStructs 86; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 139823; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 30; corpus-internal candidates 1.

### 14.50 `2503.15223`

- Title: Are" solved issues" in swe-bench really solved correctly? an empirical study
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 13.
- GROBID evidence: refs 106; biblStructs 63; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 83752; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 4.

### 14.51 `2504.10046`

- Title: Dual graph-guided llm agent for retrieval-augmented repo-level code generation
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 23.
- GROBID evidence: refs 148; biblStructs 47; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 94122; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 32; corpus-internal candidates 5.

### 14.52 `2504.11354`

- Title: Towards large formal reasoning models with reinforcement learning
- Corpus placement: first seen in wave 2; category cs-ai; estimated pages 24.
- GROBID evidence: refs 53; biblStructs 32; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 67611; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 20; corpus-internal candidates 0.

### 14.53 `2504.15257`

- Title: Reinforcing query-level meta-agents
- Corpus placement: first seen in wave 5; category cs-ai; estimated pages 17.
- GROBID evidence: refs 125; biblStructs 67; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 59192; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 43; corpus-internal candidates 2.

### 14.54 `2504.15965`

- Title: From human memory to ai memory: A survey on memory mechanisms in the era of llms
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 26.
- GROBID evidence: refs 271; biblStructs 156; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 103179; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 90; corpus-internal candidates 1.

### 14.55 `2504.19413`

- Title: Mem0: Building production-ready ai agents with scalable long-term memory
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 23.
- GROBID evidence: refs 42; biblStructs 30; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 69131; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 10; corpus-internal candidates 0.

### 14.56 `2504.21801`

- Title: Deepseek-prover-v2: Advancing formal mathematical reasoning via reinforcement learning for subgoal decomposition
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 39.
- GROBID evidence: refs 96; biblStructs 70; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 110490; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 20; corpus-internal candidates 1.

### 14.57 `2505.00212`

- Title: Which agent causes task failures and when? on automated failure attribution of LLM multi-agent systems
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 17.
- GROBID evidence: refs 85; biblStructs 46; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 68825; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 22; corpus-internal candidates 0.

### 14.58 `2505.03864`

- Title: From glue-code to protocols: A critical analysis of a2a and mcp integration for scalable agent systems
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 16.
- GROBID evidence: refs 88; biblStructs 44; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 38318; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 23; corpus-internal candidates 0.

### 14.59 `2505.10571`

- Title: On the failure of latent state persistence in large language models
- Corpus placement: first seen in wave 3; category cs-cl; estimated pages 15.
- GROBID evidence: refs 89; biblStructs 64; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 66217; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 18; corpus-internal candidates 1.

### 14.60 `2505.10819`

- Title: Poe-world: Compositional world modeling with products of programmatic experts
- Corpus placement: first seen in wave 2; category cs-ai; estimated pages 30.
- GROBID evidence: refs 143; biblStructs 81; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 99788; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 0.

### 14.61 `2505.16901`

- Title: Code graph model (cgm): A graph-integrated large language model for repository-level software engineering tasks
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 35.
- GROBID evidence: refs 148; biblStructs 72; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 109187; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 45; corpus-internal candidates 2.

### 14.62 `2505.17653`

- Title: Geogrambench: Benchmarking the geometric program reasoning in modern llms
- Corpus placement: first seen in wave 3; category cs-ai; estimated pages 29.
- GROBID evidence: refs 87; biblStructs 43; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 98361; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 27; corpus-internal candidates 0.

### 14.63 `2505.18646`

- Title: SEW: Self-evolving agentic workflows for automated code generation
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 16.
- GROBID evidence: refs 72; biblStructs 31; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 67244; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 19; corpus-internal candidates 3.

### 14.64 `2505.19443`

- Title: Vibe coding vs. agentic coding: Fundamentals and practical implications of agentic ai
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 35.
- GROBID evidence: refs 365; biblStructs 261; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 181017; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 115; corpus-internal candidates 5.

### 14.65 `2505.23135`

- Title: Verina: Benchmarking verifiable code generation
- Corpus placement: first seen in wave 2; category cs-lg; estimated pages 40.
- GROBID evidence: refs 265; biblStructs 63; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 128571; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 15; corpus-internal candidates 4.

### 14.66 `2506.02943`

- Title: Hallucination to consensus: Multi-agent llms for end-to-end junit test generation
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 26.
- GROBID evidence: refs 226; biblStructs 91; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 114030; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 26; corpus-internal candidates 1.

### 14.67 `2506.11442`

- Title: Reveal: Self-evolving code agents via reliable self-verification
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 19.
- GROBID evidence: refs 72; biblStructs 28; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 56365; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 2; corpus-internal candidates 0.

### 14.68 `2506.18019`

- Title: Graphs meet ai agents: Taxonomy, progress, and future opportunities
- Corpus placement: first seen in wave 4; category cs-ai; estimated pages 20.
- GROBID evidence: refs 434; biblStructs 245; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 147292; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 75; corpus-internal candidates 1.

### 14.69 `2506.18403`

- Title: The debugging decay index: Rethinking debugging strategies for code llms
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 10.
- GROBID evidence: refs 88; biblStructs 37; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 42582; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 2.

### 14.70 `2507.05269`

- Title: Core: Benchmarking llms code reasoning capabilities through static analysis tasks
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 55.
- GROBID evidence: refs 207; biblStructs 118; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 184729; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 19; corpus-internal candidates 4.

### 14.71 `2507.06134`

- Title: Openagentsafety: A comprehensive framework for evaluating real-world ai agent safety
- Corpus placement: first seen in wave 4; category cs-ai; estimated pages 26.
- GROBID evidence: refs 78; biblStructs 54; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 80139; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 3; corpus-internal candidates 0.

### 14.72 `2507.07957`

- Title: Mirix: Multi-agent memory system for llm-based agents
- Corpus placement: first seen in wave 4; category cs-cl; estimated pages 17.
- GROBID evidence: refs 87; biblStructs 39; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 54709; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 24; corpus-internal candidates 2.

### 14.73 `2507.23348`

- Title: Swe-debate: Competitive multi-agent debate for software issue resolution
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 19.
- GROBID evidence: refs 134; biblStructs 54; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 105930; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 27; corpus-internal candidates 1.

### 14.74 `2507.23370`

- Title: Trae agent: An llm-based agent for software engineering with test-time scaling
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 23.
- GROBID evidence: refs 141; biblStructs 68; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 83664; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 9; corpus-internal candidates 1.

### 14.75 `2508.00083`

- Title: A survey on code generation with llm-based agents
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 24.
- GROBID evidence: refs 321; biblStructs 164; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 141214; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 1; corpus-internal candidates 0.

### 14.76 `2508.03613`

- Title: Goedel-prover-v2: Scaling formal theorem proving with scaffolded data synthesis and self-correction
- Corpus placement: first seen in wave 2; category cs-lg; estimated pages 24.
- GROBID evidence: refs 80; biblStructs 47; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 74160; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 27; corpus-internal candidates 2.

### 14.77 `2508.04289`

- Title: Method-based reasoning for large language models: Extraction, reuse, and continuous improvement
- Corpus placement: first seen in wave 2; category cs-ai; estimated pages 10.
- GROBID evidence: refs 26; biblStructs 19; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 53876; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 4; corpus-internal candidates 0.

### 14.78 `2508.07434`

- Title: Let's revise step-by-step: A unified local search framework for code generation with llms
- Corpus placement: first seen in wave 3; category cs-cl; estimated pages 18.
- GROBID evidence: refs 81; biblStructs 58; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 63947; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 40; corpus-internal candidates 6.

### 14.79 `2508.07468`

- Title: Cp-agent: Agentic constraint programming
- Corpus placement: first seen in wave 2; category cs-ai; estimated pages 14.
- GROBID evidence: refs 53; biblStructs 34; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 47414; low_quality_source `true`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 6; corpus-internal candidates 0.

### 14.80 `2508.13732`

- Title: Self-organizing agent network for llm-based workflow automation
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 12.
- GROBID evidence: refs 64; biblStructs 32; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 56614; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 12; corpus-internal candidates 0.

### 14.81 `2508.18675`

- Title: Requirements development and formalization for reliable code generation: A multi-agent vision
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 6.
- GROBID evidence: refs 45; biblStructs 48; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 0; low_quality_source `true`; error `OpenDataLoaderProbeError: import API failed: Command '['java', '-Djava.awt.headless=true', '-Dapple.awt.UIElement=true', '-jar', '/root/daily-archive/.venv/lib/python3.13/site-packages/opendataloader_pdf/jar/opendataloader-pdf-cli.jar', 'data/article_catalog/article_catalog/arxiv/mixed-source/2508.18675/source/2508.18675.pdf', '--output-dir', '/root/daily-archive/artifacts/m056-bfs-graph/wave-3/opendataloader/2508.18675.yev3uy4u', '--format', 'json,markdown', '--quiet']' returned non-zero exit status 1.; subprocess failed: Error running opendataloader-pdf CLI.
Return code: 1`.
- Candidate citation evidence: outbound arXiv candidates 11; corpus-internal candidates 0.

### 14.82 `2509.02544`

- Title: Ui-tars-2 technical report: Advancing gui agent with multi-turn reinforcement learning
- Corpus placement: first seen in wave 4; category cs-ai; estimated pages 30.
- GROBID evidence: refs 155; biblStructs 90; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 105376; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 32; corpus-internal candidates 1.

### 14.83 `2509.03312`

- Title: Who is inducing failure in the llm agentic systems? arXiv preprint
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 18.
- GROBID evidence: refs 117; biblStructs 85; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 71813; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 27; corpus-internal candidates 3.

### 14.84 `2509.10397`

- Title: Building simulated environments for agentic recommender systems
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 10.
- GROBID evidence: refs 95; biblStructs 74; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 74327; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 57; corpus-internal candidates 0.

### 14.85 `2509.16198`

- Title: Rpg: A repository planning graph for unified and scalable codebase generation
- Corpus placement: first seen in wave 3; category cs-cl; estimated pages 54.
- GROBID evidence: refs 70; biblStructs 58; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 170587; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 14; corpus-internal candidates 2.

### 14.86 `2509.16941`

- Title: Swe-bench pro: Can ai agents solve long-horizon software engineering tasks
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 20.
- GROBID evidence: refs 46; biblStructs 22; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 59547; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 12; corpus-internal candidates 4.

### 14.87 `2509.18597`

- Title: Growing with your embodied agent: A human-in-the-loop lifelong code generation framework for long-horizon manipulation skills
- Corpus placement: first seen in wave 2; category mixed-source; estimated pages 34.
- GROBID evidence: refs 54; biblStructs 36; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 117165; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 12; corpus-internal candidates 0.

### 14.88 `2509.24219`

- Title: Vireskill: Vision-grounded replanning with skill memory for llm-based planning in lifelong robot learning
- Corpus placement: first seen in wave 2; category mixed-source; estimated pages 8.
- GROBID evidence: refs 36; biblStructs 19; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 36241; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 0; corpus-internal candidates 0.

### 14.89 `2509.25370`

- Title: Where llm agents fail and how they can learn from failures
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 32.
- GROBID evidence: refs 60; biblStructs 25; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 89839; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 17; corpus-internal candidates 3.

### 14.90 `2510.03342`

- Title: Gemini robotics 1.5: Pushing the frontier of generalist robots with advanced embodied reasoning, thinking, and motion transfer
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 62.
- GROBID evidence: refs 134; biblStructs 46; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 174000; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 21; corpus-internal candidates 2.

### 14.91 `2510.03902`

- Title: Multi-agent code-orchestrated generation for reliable infrastructure-as-code
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 18.
- GROBID evidence: refs 51; biblStructs 28; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 60750; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 5; corpus-internal candidates 1.

### 14.92 `2510.05156`

- Title: Enhancing llm agent safety via verified code generation
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 22.
- GROBID evidence: refs 50; biblStructs 35; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 57823; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 3; corpus-internal candidates 0.

### 14.93 `2510.10292`

- Title: From programs to poses: Factored real-world scene generation via learned program libraries
- Corpus placement: first seen in wave 2; category cs-cv; estimated pages 29.
- GROBID evidence: refs 94; biblStructs 43; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 88827; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 10; corpus-internal candidates 0.

### 14.94 `2510.11967`

- Title: Scaling long-horizon llm agent via context-folding
- Corpus placement: first seen in wave 4; category cs-cl; estimated pages 22.
- GROBID evidence: refs 91; biblStructs 44; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 64374; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 25; corpus-internal candidates 1.

### 14.95 `2510.12157`

- Title: Self-verifying reflection helps transformers with cot reasoning
- Corpus placement: first seen in wave 2; category cs-lg; estimated pages 44.
- GROBID evidence: refs 149; biblStructs 43; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 154103; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 35; corpus-internal candidates 0.

### 14.96 `2510.18471`

- Title: Coderl+: Improving code generation via reinforcement with execution semantics alignment
- Corpus placement: first seen in wave 2; category mixed-source; estimated pages 18.
- GROBID evidence: refs 80; biblStructs 57; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 69765; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 30; corpus-internal candidates 4.

### 14.97 `2510.20909`

- Title: Codeenabled language models can outperform reasoning models on diverse tasks
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 38.
- GROBID evidence: refs 141; biblStructs 93; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 126267; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 48; corpus-internal candidates 0.

### 14.98 `2510.23010`

- Title: Talm: Dynamic tree-structured multi-agent framework with long-term memory for scalable code generation
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 10.
- GROBID evidence: refs 50; biblStructs 24; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 52548; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 5; corpus-internal candidates 1.

### 14.99 `2510.26094`

- Title: Lean4physics: Comprehensive reasoning framework for college-level physics in lean4
- Corpus placement: first seen in wave 2; category cs-ai; estimated pages 23.
- GROBID evidence: refs 92; biblStructs 52; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 72499; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 27; corpus-internal candidates 3.

### 14.100 `2511.01854`

- Title: Tool-to-agent retrieval: Bridging tools and agents for scalable llm multi-agent systems
- Corpus placement: first seen in wave 4; category cs-cl; estimated pages 10.
- GROBID evidence: refs 52; biblStructs 41; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 25221; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 22; corpus-internal candidates 0.

### 14.101 `2511.03690`

- Title: The openhands software agent sdk: A composable and extensible foundation for production agents
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 19.
- GROBID evidence: refs 39; biblStructs 21; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 62270; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 1; corpus-internal candidates 0.

### 14.102 `2511.10621`

- Title: Socratic self-refine for large language model reasoning
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 32.
- GROBID evidence: refs 124; biblStructs 58; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 0; low_quality_source `true`; error `UnicodeEncodeError: 'utf-8' codec can't encode character '\ud835' in position 40403: surrogates not allowed`.
- Candidate citation evidence: outbound arXiv candidates 38; corpus-internal candidates 0.

### 14.103 `2511.13646`

- Title: Live-swe-agent: Can software engineering agents self-evolve on the fly? arXiv preprint
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 20.
- GROBID evidence: refs 108; biblStructs 48; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 77170; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 21; corpus-internal candidates 6.

### 14.104 `2511.20857`

- Title: Evo-memory: Benchmarking llm agent test-time learning with self-evolving memory
- Corpus placement: first seen in wave 4; category cs-cl; estimated pages 27.
- GROBID evidence: refs 90; biblStructs 50; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 85238; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 22; corpus-internal candidates 2.

### 14.105 `2512.02002`

- Title: Llm-driven corrective robot operation code generation with static text-based simulation
- Corpus placement: first seen in wave 2; category mixed-source; estimated pages 8.
- GROBID evidence: refs 84; biblStructs 40; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 46074; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 9; corpus-internal candidates 0.

### 14.106 `2512.10563`

- Title: Normcode: A semi-formal language for auditable ai planning
- Corpus placement: first seen in wave 2; category cs-ai; estimated pages 22.
- GROBID evidence: refs 26; biblStructs 36; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 81901; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 6; corpus-internal candidates 0.

### 14.107 `2512.12806`

- Title: Fault-tolerant sandboxing for AI coding agents: A transactional approach to safe autonomous execution
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 7.
- GROBID evidence: refs 30; biblStructs 23; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 27266; low_quality_source `true`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 3; corpus-internal candidates 0.

### 14.108 `2512.17419`

- Title: Swe-bench++: A framework for the scalable generation of software engineering benchmarks from open-source repositories
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 21.
- GROBID evidence: refs 30; biblStructs 21; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 63094; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 2; corpus-internal candidates 0.

### 14.109 `2512.23631`

- Title: Boad: Discovering hierarchical software engineering agents via bandit optimization
- Corpus placement: first seen in wave 5; category cs-lg; estimated pages 25.
- GROBID evidence: refs 62; biblStructs 59; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 96411; low_quality_source `true`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 19; corpus-internal candidates 3.

### 14.110 `2601.03515`

- Title: Mem-gallery: Benchmarking multimodal long-term conversational memory for mllm agents
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 34.
- GROBID evidence: refs 142; biblStructs 45; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 163097; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 2.

### 14.111 `2601.05808`

- Title: Envscaler: Scaling tool-interactive environments for llm agent via programmatic synthesis
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 32.
- GROBID evidence: refs 102; biblStructs 62; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 146527; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 18; corpus-internal candidates 0.

### 14.112 `2601.06789`

- Title: Enhancing code agents through learning from governed human experiences
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 17.
- GROBID evidence: refs 52; biblStructs 37; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 60635; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 2.

### 14.113 `2601.08816`

- Title: Memrec: Collaborative memory-augmented agentic recommender system
- Corpus placement: first seen in wave 6; category mixed-source; estimated pages 30.
- GROBID evidence: refs 145; biblStructs 68; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 112617; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 8; corpus-internal candidates 1.

### 14.114 `2601.11655`

- Title: Advances and frontiers of llm-based issue resolution in software engineering: A comprehensive survey
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 26.
- GROBID evidence: refs 233; biblStructs 214; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 123529; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 135; corpus-internal candidates 13.

### 14.115 `2601.11868`

- Title: Terminal-bench: Benchmarking agents on hard, realistic tasks in command line interfaces
- Corpus placement: first seen in wave 5; category mixed-source; estimated pages 84.
- GROBID evidence: refs 87; biblStructs 94; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 202264; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 9; corpus-internal candidates 0.

### 14.116 `2601.12762`

- Title: Teaching llms to learn tool trialing and execution through environment interaction
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 24.
- GROBID evidence: refs 98; biblStructs 12; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 93431; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 4; corpus-internal candidates 0.

### 14.117 `2601.13247`

- Title: Aligning agentic world models via knowledgeable experience learning
- Corpus placement: first seen in wave 3; category cs-cl; estimated pages 19.
- GROBID evidence: refs 140; biblStructs 122; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 80351; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 83; corpus-internal candidates 1.

### 14.118 `2601.15709`

- Title: Agentsm: Semantic memory for agentic text-to-sql
- Corpus placement: first seen in wave 3; category cs-ai; estimated pages 9.
- GROBID evidence: refs 76; biblStructs 38; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 54519; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 25; corpus-internal candidates 2.

### 14.119 `2601.16746`

- Title: Swe-pruner: Self-adaptive context pruning for coding agents
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 30.
- GROBID evidence: refs 147; biblStructs 90; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 100218; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 43; corpus-internal candidates 5.

### 14.120 `2601.19510`

- Title: Agentic llm for robotic manipulation
- Corpus placement: first seen in wave 2; category mixed-source; estimated pages 8.
- GROBID evidence: refs 43; biblStructs 25; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 45095; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 12; corpus-internal candidates 2.

### 14.121 `2602.05842`

- Title: Reinforcement world model learning for llm-based agents
- Corpus placement: first seen in wave 2; category cs-cl; estimated pages 20.
- GROBID evidence: refs 149; biblStructs 83; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 84141; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 8; corpus-internal candidates 0.

### 14.122 `2602.05892`

- Title: Contextbench: A benchmark for context retrieval in coding agents
- Corpus placement: first seen in wave 5; category cs-lg; estimated pages 34.
- GROBID evidence: refs 81; biblStructs 42; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 94045; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 21; corpus-internal candidates 7.

### 14.123 `2602.06052`

- Title: Rethinking memory mechanisms of foundation agents in the second half
- Corpus placement: first seen in wave 3; category cs-cl; estimated pages 83.
- GROBID evidence: refs 1007; biblStructs 546; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 367439; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 268; corpus-internal candidates 14.

### 14.124 `2602.09944`

- Title: Environment-in-the-loop: Rethinking code migration with llm-based agents
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 5.
- GROBID evidence: refs 46; biblStructs 29; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 27671; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 0.

### 14.125 `2602.10090`

- Title: Agent world model: Infinity synthetic environments for agentic reinforcement learning
- Corpus placement: first seen in wave 3; category cs-ai; estimated pages 43.
- GROBID evidence: refs 191; biblStructs 69; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 159506; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 11; corpus-internal candidates 0.

### 14.126 `2602.11757`

- Title: Code2worlds: Empowering coding llms for 4d world generation
- Corpus placement: first seen in wave 3; category cs-cv; estimated pages 28.
- GROBID evidence: refs 49; biblStructs 46; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 81623; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 9; corpus-internal candidates 0.

### 14.127 `2602.13962`

- Title: Codeglance: Understanding code reasoning challenges in llms through multi-dimensional feature analysis
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 13.
- GROBID evidence: refs 106; biblStructs 55; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 73806; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 25; corpus-internal candidates 3.

### 14.128 `2602.14337`

- Title: Longcli-bench: A preliminary benchmark and study for long-horizon agentic programming in command-line interfaces
- Corpus placement: first seen in wave 4; category mixed-source; estimated pages 11.
- GROBID evidence: refs 65; biblStructs 49; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 51648; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 43; corpus-internal candidates 6.

### 14.129 `2602.23647`

- Title: Suggestion-guided llm-based multi-agent framework for repository-level software repair
- Corpus placement: first seen in wave 3; category mixed-source; estimated pages 32.
- GROBID evidence: refs 153; biblStructs 73; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 125728; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 30; corpus-internal candidates 4.

### 14.130 `2603.03329`

- Title: Autoharness: improving llm agents by automatically synthesizing a code harness
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 21.
- GROBID evidence: refs 40; biblStructs 21; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 48847; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 7; corpus-internal candidates 0.

### 14.131 `2603.03836`

- Title: Skillvla: Tackling combinatorial diversity in dual-arm manipulation via skill reuse
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 16.
- GROBID evidence: refs 92; biblStructs 54; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 82365; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 12; corpus-internal candidates 1.

### 14.132 `2603.04177`

- Title: Can llms generate human-level code refactorings? arXiv preprint
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 29.
- GROBID evidence: refs 71; biblStructs 40; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 103371; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 1; corpus-internal candidates 0.

### 14.133 `2603.04257`

- Title: Memex (rl): Scaling long-horizon llm agents via indexed experience memory
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 22.
- GROBID evidence: refs 66; biblStructs 48; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 71997; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 30; corpus-internal candidates 2.

### 14.134 `2603.05621`

- Title: Eric Feron, and Jürgen Schmidhuber. Racas: Controlling diverse robots with a single agentic system
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 9.
- GROBID evidence: refs 68; biblStructs 33; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 43702; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 7; corpus-internal candidates 0.

### 14.135 `2603.11226`

- Title: Execverify: White-box rl with verifiable stepwise rewards for code execution reasoning
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 25.
- GROBID evidence: refs 112; biblStructs 34; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 83553; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 24; corpus-internal candidates 4.

### 14.136 `2603.13258`

- Title: Your code agent can grow alongside you with structured memory
- Corpus placement: first seen in wave 1; category cs-lg; estimated pages 18.
- GROBID evidence: refs 76; biblStructs 58; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 75826; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 26; corpus-internal candidates 4.

### 14.137 `2603.19329`

- Title: Goedel-code-prover: Hierarchical proof search for open state-of-the-art code verification
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 25.
- GROBID evidence: refs 91; biblStructs 52; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 83810; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 27; corpus-internal candidates 6.

### 14.138 `2603.21430`

- Title: Domagent: Leveraging knowledge graphs and case-based reasoning for domain-specific code generation
- Corpus placement: first seen in wave 1; category cs-ai; estimated pages 10.
- GROBID evidence: refs 72; biblStructs 48; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 59699; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 13; corpus-internal candidates 0.

### 14.139 `2603.21520`

- Title: Generalizable self-evolving memory for automatic prompt optimization
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 19.
- GROBID evidence: refs 106; biblStructs 38; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 69686; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 3.

### 14.140 `2603.24533`

- Title: Ui-voyager: A self-evolving gui agent learning via failed experience
- Corpus placement: first seen in wave 1; category cs-lg; estimated pages 20.
- GROBID evidence: refs 147; biblStructs 119; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 86123; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 64; corpus-internal candidates 1.

### 14.141 `2603.25723`

- Title: Natural-language agent harnesses
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 22.
- GROBID evidence: refs 76; biblStructs 67; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 77981; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 0; corpus-internal candidates 0.

### 14.142 `2603.26664`

- Title: Learning to commit: Generating organic pull requests via online repository memory
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 9.
- GROBID evidence: refs 35; biblStructs 23; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 35147; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 0; corpus-internal candidates 0.

### 14.143 `2603.28052`

- Title: Metaharness: End-to-end optimization of model harnesses
- Corpus placement: first seen in wave 1; category cs-ai; estimated pages 26.
- GROBID evidence: refs 103; biblStructs 61; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 85359; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 10; corpus-internal candidates 1.

### 14.144 `2603.28119`

- Title: Compressing code context for llm-based issue resolution
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 12.
- GROBID evidence: refs 79; biblStructs 40; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 64076; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 16; corpus-internal candidates 5.

### 14.145 `2604.08224`

- Title: Externalization in llm agents: A unified review of memory, skills, protocols and harness engineering
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 54.
- GROBID evidence: refs 333; biblStructs 199; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 222133; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 102; corpus-internal candidates 4.

### 14.146 `2604.11839`

- Title: Beyond static sandboxing: Learned capability governance for autonomous ai agents
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 13.
- GROBID evidence: refs 18; biblStructs 17; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 42586; low_quality_source `true`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 5; corpus-internal candidates 0.

### 14.147 `2604.14228`

- Title: Dive into claude code: The design space of today's and future ai agent systems
- Corpus placement: first seen in wave 1; category mixed-source; estimated pages 46.
- GROBID evidence: refs 148; biblStructs 106; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 189199; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 39; corpus-internal candidates 6.

### 14.148 `2604.25850`

- Title: Agentic harness engineering: Observability-driven automatic evolution of codingagent harnesses
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 35.
- GROBID evidence: refs 135; biblStructs 54; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 132042; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 0; corpus-internal candidates 0.

### 14.149 `2605.18747`

- Title: Code as Agent Harness ♢ Toward Executable, Verifiable, and Stateful Agent Systems ♢
- Corpus placement: first seen in wave 1; category cs-cl; estimated pages 102.
- GROBID evidence: refs 958; biblStructs 479; low_quality_source `false`.
- OpenDataLoader evidence: markdown bytes 0; low_quality_source `false`; error `none`.
- Candidate citation evidence: outbound arXiv candidates 171; corpus-internal candidates 148.

## 15. Closure statement

M056 S07 closes the 1-hop BFS synthesis loop with three durable artifacts: this report, `candidate-edges.json`, and ADR-010.
The artifacts preserve parser-scale evidence and a conservative graph-readiness recommendation while keeping the safety boundary intact.
The next milestone gate should decide whether to run 2-hop BFS or select an alternative anchor before any graph import path is considered.
