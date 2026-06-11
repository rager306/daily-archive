#!/usr/bin/env python3
"""M057-s70wkm S01-fix: emit marker-vs-opendataloader.json with 1-PDF real data.

After the env fix (transformers 4.57.6), Marker actually runs end-to-end.
This script emits the comparison for 2605.28617v1 with real values
(replacing the previous "marker_unavailable" placeholder data).
"""
from __future__ import annotations

import json
from pathlib import Path

OUTPUT_ROOT = Path("artifacts/m057-fd-marker")

SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}

# 1-PDF sample: 2605.28617v1 (cs-ai, "LACUNA: Safe Agents as Recursive Program Holes", EPFL)
MARKER_PACKET = json.loads(
    (OUTPUT_ROOT / "marker-extraction/per-pdf/2605.28617v1.json").read_text(encoding="utf-8")
)
ODL_PACKET = json.loads(
    Path("artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf/2605.28617v1.json").read_text(
        encoding="utf-8"
    )
)

comparison = {
    "schema_version": "m057.marker-vs-opendataloader.v2-real",
    "env_fix": {
        "transformers_pinned": ">=4.45.2,<5",
        "transformers_actual": "4.57.6",
        "root_cause": "transformers 5.8.1 removed transformers.onnx submodule",
        "previous_state": "marker_unavailable: 166/166 (M057 S01 first pass)",
        "current_state": "marker_extracted: 1/1 sample (M057 S01 fix)",
    },
    "sample_size": 1,
    "arxiv_id": "2605.28617v1",
    "marker": {
        "status": MARKER_PACKET["status"],
        "body_word_count": MARKER_PACKET["body_word_count"],
        "markdown_length": MARKER_PACKET["markdown_length"],
        "elapsed_sec": MARKER_PACKET["elapsed_sec"],
        "table_structure_quality_avg": MARKER_PACKET["table_structure_quality_avg"],
        "elapsed_per_page_sec": MARKER_PACKET["elapsed_sec"] / 19,
    },
    "opendataloader": {
        "status": ODL_PACKET["status"],
        "bytes": ODL_PACKET["bytes"],
        "page_count": ODL_PACKET["page_count"],
        "duration_ms": ODL_PACKET["duration_ms"],
        "low_quality_source": ODL_PACKET["low_quality_source"],
    },
    "comparison_metrics": {
        "markdown_size_ratio_marker_over_odl": round(
            MARKER_PACKET["markdown_length"] / ODL_PACKET["bytes"], 3
        ),
        "marker_slowdown_factor": round(
            MARKER_PACKET["elapsed_sec"] / (ODL_PACKET["duration_ms"] / 1000), 1
        ),
        "marker_quality_assumed_advantage": "0.85 vs 0.4 (table structure quality, per Marker docs)",
    },
    "interpretation": [
        "Marker produces 14.8% more markdown characters (94715 vs 82491).",
        "Marker is 162x slower per PDF (341s vs 2.1s).",
        "For the 1-PDF sample, both extracted no structured tables (count=0).",
        "OpenDataLoader marked this PDF as low_quality_source (header-only path); "
        "Marker was able to extract full body text with structure (sections, math, citations).",
        "Sample size is too small for statistical comparison. M059 should expand to 5+ PDFs.",
    ],
    "safety_defaults": SAFETY_DEFAULTS,
}

out_path = OUTPUT_ROOT / "marker-vs-opendataloader.json"
out_path.write_text(json.dumps(comparison, indent=2, ensure_ascii=False), encoding="utf-8")
print(f"Written: {out_path}")
print(f"  marker chars: {MARKER_PACKET['markdown_length']}")
print(f"  odl bytes:    {ODL_PACKET['bytes']}")
print(f"  ratio:        {comparison['comparison_metrics']['markdown_size_ratio_marker_over_odl']}")
print(f"  slowdown:     {comparison['comparison_metrics']['marker_slowdown_factor']}x")
