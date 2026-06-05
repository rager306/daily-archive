# OpenDataLoader PDF Input Manifest

| Article | Role | PDF | Size | SHA256 | Rationale |
|---|---|---|---:|---|---|
| 2605.26525v1 — ReCA: Multi-Shot Long Video Extrapolation via Recursive Context Allocation | `layout_figure_heavy` | `data/article_catalog/article_catalog/arxiv/cs-cv/2605.26525v1/source/original.pdf` | 23568022 | `a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8` | Large computer-vision/video paper with likely figures, layouts, and visual elements; stresses reading order and figure/layout extraction. |
| 2512.24601 — Recursive Language Models | `text_section_heavy` | `data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/source/original.pdf` | 9942446 | `8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca` | Long AI paper suited to section hierarchy, reading order, bibliography, and markdown/json structure assessment. |
| 2507.19457 — GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning | `fallback_problem_case` | `data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/original.pdf` | 2975112 | `ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec` | Catalog HTML capture was low/HTTPError-like while PDF exists, making it useful as a fallback/parser-quality case. |

Network fetch avoided for all entries.
