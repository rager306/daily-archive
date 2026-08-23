# M179 Script Wave Scope

## Decision

Select two exact residual script families for M179: **M057 structure extraction outputs** and **M060 graph and figure benchmark outputs**.

## Expected movement

```text
script-only: 170 -> 142
m057-structure-extraction-output: 0 -> 15
m060-graph-figure-benchmark-output: 0 -> 13
total movement: 28
```

## Exact source paths

### M057 structure extraction outputs

- `scripts/m057_compare_marker_opendataloader.py`
- `scripts/m057_figure_similarity.py`
- `scripts/m057_marker_extract_5.py`
- `scripts/m057_table_similarity.py`
- `scripts/legacy/m057_table_embed.py`
- `scripts/m057_build_graph_manifest.py`
- `scripts/m057_compare_marker_opendataloader_1pdf.py`
- `scripts/m057_fd_validate.py`
- `scripts/m057_figure_caption_build.py`
- `scripts/m057_figure_embed.py`
- `scripts/m057_table_text_build.py`

### M060 graph and figure benchmark outputs

- `scripts/m060g_figure_judge.py`
- `scripts/m060b_graph_stats.py`
- `scripts/m060b_graph_validate.py`
- `scripts/m060c_applicability_matrix.py`
- `scripts/m060c_benchmark.py`
- `scripts/m060b_graph_visualize.py`
- `scripts/m060b_two_hop_preview.py`

## Rejected alternatives

- `verify_m031` and `verify_m033` remain good future candidates, but selecting M057 plus M060 produces a larger 28-record wave while preserving exact path matching.
- Generic output target rules remain rejected.
- Broad `m057*` or `m060*` prefix rules remain rejected; implementation must list exact paths.
