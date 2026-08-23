# OpenDataLoader Quality Report

This review evaluates the three M033 S03 OpenDataLoader hybrid/docling-fast outputs against the S01 baseline. It does not claim graph readiness, production import eligibility, or LadybugDB write readiness.

## Runtime and cache cost

- Total full-run duration: 576872 ms.
- Hybrid used Hugging Face model cache:
  - `docling-project/docling-layout-heron` at `/root/.cache/huggingface/hub/models--docling-project--docling-layout-heron` snapshot `8f39ad3c0b4c58e9c2d2c84a38465abf757272d8` size `171764747` bytes
  - `docling-project/docling-models` at `/root/.cache/huggingface/hub/models--docling-project--docling-models` snapshot `None` size `358236863` bytes
- If cache is absent, hybrid may require network/model downloads.

## Per-paper review

### 2605.26525v1: ReCA: Multi-Shot Long Video Extrapolation via Recursive Context Allocation

- Challenge role: `layout_figure_heavy`
- Backend: `hybrid-docling-fast`; fallback used: `False`; duration: `256026` ms

| Dimension | Rating | Evidence |
|---|---|---|
| `section_hierarchy` | `medium` | 57 markdown headings detected; JSON types include [('paragraph', 898), ('table cell', 159), ('heading', 57), ('image', 55), ('list item', 55)] |
| `reading_order` | `medium` | Markdown/text are coherent enough for review, but no independent ground-truth reading-order score was computed. |
| `tables` | `medium` | 242 table-like signals across JSON/Markdown; requires manual table fidelity review. |
| `figures_captions` | `medium` | 147 figure/caption/image text signals; image semantic descriptions are not independently validated. |
| `bibliography` | `medium` | 5 bibliography/reference section signals; citation graph readiness is not inferred. |
| `ocr_quality` | `not_applicable_or_not_proven` | These local arXiv PDFs appear digitally generated; OCR/scanned performance was not proven by this three-PDF run. |
| `coordinate_layout_metadata` | `high` | JSON has bounding boxes=True, page metadata=True. |
| `markdown_usefulness` | `high` | Markdown/text produced 28526 words and 57 headings. |
| `json_usefulness` | `high` | JSON contains 1299 nested objects with top keys [('type', 1298), ('page number', 1264), ('bounding box', 1264), ('pdfua_tag', 1255), ('id', 1031), ('content', 1023), ('font', 1019), ('font size', 1019)]. |
| `failure_diagnostics` | `high` | Run metadata captured status=passed, exit_code=0, duration_ms=256026, fallback_used=False. |

Representative examples (truncated):
- headings: # ReCA: Multi-Shot Long Video Extrapolation via Recursive Context Allocation; # 3 Context Allocation, Not Context Length; ## 4 Recursive Context Allocation
- tables: G(p, b; d), d ≤ τG, |p| ≤ BG, m ℓ=1 ReCA(S(ℓ),gℓ,dℓ,bℓ), otherwise,; [ϵ − qsup(z | v0)]+, (2); qsup(z | v0) · wk(z;gk) · fk(z) − λredRed(C), (3)
- figures: The second operator addresses the shot-level dilution bottleneck (Fig. 3, phase 2). From the candidate pool Ωk = Sk ∪ gk ∪ bk (persistent state, current shot goal, visual boundary), ReCA selects a compact slice; Figure 10 User-study interface. Participants view each generated long video and rate it along six dimensions: visual appeal, script faithfulness, character consistency, background consistency, physical law, and narrative coherence.; Figure 11 Tianti portrait references.

### 2512.24601: Recursive Language Models

- Challenge role: `text_section_heavy`
- Backend: `hybrid-docling-fast`; fallback used: `False`; duration: `215298` ms

| Dimension | Rating | Evidence |
|---|---|---|
| `section_hierarchy` | `medium` | 58 markdown headings detected; JSON types include [('paragraph', 861), ('table cell', 179), ('heading', 55), ('table row', 33), ('image', 32)] |
| `reading_order` | `medium` | Markdown/text are coherent enough for review, but no independent ground-truth reading-order score was computed. |
| `tables` | `medium` | 254 table-like signals across JSON/Markdown; requires manual table fidelity review. |
| `figures_captions` | `medium` | 114 figure/caption/image text signals; image semantic descriptions are not independently validated. |
| `bibliography` | `medium` | 3 bibliography/reference section signals; citation graph readiness is not inferred. |
| `ocr_quality` | `not_applicable_or_not_proven` | These local arXiv PDFs appear digitally generated; OCR/scanned performance was not proven by this three-PDF run. |
| `coordinate_layout_metadata` | `high` | JSON has bounding boxes=True, page metadata=True. |
| `markdown_usefulness` | `high` | Markdown/text produced 23511 words and 58 headings. |
| `json_usefulness` | `high` | JSON contains 1184 nested objects with top keys [('type', 1183), ('page number', 1150), ('bounding box', 1150), ('pdfua_tag', 1136), ('id', 967), ('font', 934), ('font size', 934), ('content', 934)]. |
| `failure_diagnostics` | `high` | Run metadata captured status=passed, exit_code=0, duration_ms=215298, fallback_used=False. |

Representative examples (truncated):
- headings: # Recursive Language Models; # 2 Recursive Language Models; # 3 Scaling Long Context Tasks
- tables: Table 1 reports our main evaluation results. We additionally explore how vanilla frontier model and RLM performance degrade as input contexts grow in Figure 1.; |Model|CodeQA|BrowseComp+|(1K) OOLONG|OOLONG-Pairs|; |---|---|---|---|---|
- figures: Table 1 reports our main evaluation results. We additionally explore how vanilla frontier model and RLM performance degrade as input contexts grow in Figure 1.; Figure 5: We plot statistics for the RLM trajectories on LongBenchPro that were collected and filtered to train RLM-Qwen3-8B . The left plots show the unfiltered trajectories, and right plots show the post-filtering trajectories.; Below, we provide plots for the runtime speed-up of training in Figure 6.

### 2507.19457: GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning

- Challenge role: `fallback_problem_case`
- Backend: `hybrid-docling-fast`; fallback used: `False`; duration: `105548` ms

| Dimension | Rating | Evidence |
|---|---|---|
| `section_hierarchy` | `medium` | 124 markdown headings detected; JSON types include [('paragraph', 5913), ('table cell', 211), ('list item', 202), ('heading', 109), ('list', 60)] |
| `reading_order` | `medium` | Markdown/text are coherent enough for review, but no independent ground-truth reading-order score was computed. |
| `tables` | `medium` | 297 table-like signals across JSON/Markdown; requires manual table fidelity review. |
| `figures_captions` | `medium` | 92 figure/caption/image text signals; image semantic descriptions are not independently validated. |
| `bibliography` | `medium` | 13 bibliography/reference section signals; citation graph readiness is not inferred. |
| `ocr_quality` | `not_applicable_or_not_proven` | These local arXiv PDFs appear digitally generated; OCR/scanned performance was not proven by this three-PDF run. |
| `coordinate_layout_metadata` | `high` | JSON has bounding boxes=True, page metadata=True. |
| `markdown_usefulness` | `high` | Markdown/text produced 42363 words and 124 headings. |
| `json_usefulness` | `high` | JSON contains 6570 nested objects with top keys [('type', 6569), ('page number', 6538), ('bounding box', 6538), ('pdfua_tag', 6537), ('content', 6226), ('font', 6225), ('font size', 6225), ('id', 6075)]. |
| `failure_diagnostics` | `high` | Run metadata captured status=passed, exit_code=0, duration_ms=105548, fallback_used=False. |

Representative examples (truncated):
- headings: # GEPA: R EFLECTIVE P ROMPT E VOLUTION C AN O UTPER FORM R EINFORCEMENT L EARNING; # Seed Prompt for Second-Hop of Multi-Hop QA System; # GEPA’s Optimized Prompt for Second-Hop of Multi-Hop QA System, GPT-4.1 Mini
- tables: maximize utility while ensuring auditable privacy—zero leakage tolerated.; |Qwen3 8B|HotpotQA|IFBench|Hover PUPA|PUPA|AIME-2025|LiveBench-Math|Aggregate|Improvement|; |---|---|---|---|---|---|---|---|---|
- figures: Figure C shows the meta-prompt used by GEPA, which guides the LLM to reflectively refine its current instruction based on input–output examples and corresponding feedback from the environment.; Figure 4 presents the core GEPA Algorithm, along with the algorithm for Pareto-based candidate selection.; Figure 9: Details of System Aware Merge. r represents a seeded stochastic sampler.

## Remaining gaps

- OCR quality remains unproven for scanned/image-only PDFs.
- Table fidelity needs ground-truth comparison.
- All outputs remain candidate evidence only.
