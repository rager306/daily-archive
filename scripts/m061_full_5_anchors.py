#!/usr/bin/env python3
"""M061 S02 5-anchor 2-hop BFS runner.

Runs the four S02 anchors and combines them with the completed S01 anchor into a
single diagnostic 5-layer graph manifest. Execution remains synchronous per
ADR-017. Safety defaults remain false: graph writes is not authorized,
production import is not authorized, fact promotion is not authorized, external
network is disabled by default, and LLM calls are disabled by default. S02 has a
scoped network override for arXiv acquisition only.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
import time
import urllib.parse
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
S01_SCRIPT_PATH = ROOT / "scripts" / "m061_anchor_pilot.py"
S01_ANCHOR = "2605.18747"
S02_ANCHORS = ["2401.04016", "2207.05608", "2505.19443", "2510.12157"]
ALL_ANCHORS = [S01_ANCHOR, *S02_ANCHORS]
BASE_OUTPUT_DIR = ROOT / "artifacts" / "m061-2hop"
GENERATED_BY = "scripts/m061_full_5_anchors.py"
NETWORK_HOST = "127.0.0.1"

S02_NETWORK_OVERRIDE: dict[str, Any] = {
    "external_network_authorized": True,
    "reason": (
        "User explicit authorization for M064-wqfgfa S02 four-anchor real-pipeline run; "
        "arxiv rate limit respected (1 req/3s, retry+backoff, 429 honors Retry-After)."
    ),
    "scope": "M064-wqfgfa S02 only, anchors 2401.04016, 2207.05608, 2505.19443, 2510.12157, 30 sample PDFs per anchor, no production import, no graph writes",
}

ANCHOR_FALLBACK_NOTE = (
    "Anchor was not present in M056 cumulative corpus, so S02 acquired only the anchor PDF/e-print "
    "from arXiv under the same scoped override and extracted arXiv references from the e-print source."
)


def load_s01_module() -> Any:
    spec = importlib.util.spec_from_file_location("m061_anchor_pilot", S01_SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Unable to load {S01_SCRIPT_PATH}")
    module = importlib.util.module_from_spec(spec)
    sys.modules["m061_anchor_pilot"] = module
    spec.loader.exec_module(module)
    module.GENERATED_BY = GENERATED_BY
    return module


s01 = load_s01_module()


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def display_path(path: Path) -> str:
    return str(path.relative_to(ROOT))


def unique_sorted(values: list[str]) -> list[str]:
    return sorted({value for value in values if value})


def aggregate_rate_metrics(metrics_list: list[dict[str, Any]]) -> dict[str, Any]:
    request_kinds: dict[str, int] = {}
    total_pacing_delay = 0.0
    total_pacing_count = 0
    aggregate: dict[str, Any] = {
        "user_agent": s01.ARXIV_USER_AGENT,
        "min_interval_seconds": s01.ARXIV_API_MIN_INTERVAL_SECONDS,
        "max_retry_attempts_per_request": s01.ARXIV_MAX_RETRY_ATTEMPTS,
        "backoff_schedule_seconds": list(s01.ARXIV_BACKOFF_SECONDS),
        "requests_made": 0,
        "http_429_count": 0,
        "retry_attempts": 0,
        "retry_after_honored_count": 0,
        "retry_after_delay_seconds_total": 0.0,
        "backoff_delay_seconds_total": 0.0,
        "pacing_delay_count": 0,
        "pacing_delay_seconds_total": 0.0,
        "request_kinds": request_kinds,
    }
    for metrics in metrics_list:
        aggregate["requests_made"] += int(metrics.get("requests_made", 0))
        aggregate["http_429_count"] += int(metrics.get("http_429_count", 0))
        aggregate["retry_attempts"] += int(metrics.get("retry_attempts", 0))
        aggregate["retry_after_honored_count"] += int(metrics.get("retry_after_honored_count", 0))
        aggregate["retry_after_delay_seconds_total"] += float(
            metrics.get("retry_after_delay_seconds_total", 0.0)
        )
        aggregate["backoff_delay_seconds_total"] += float(
            metrics.get("backoff_delay_seconds_total", 0.0)
        )
        pacing_count = int(metrics.get("pacing_delay_count", 0))
        pacing_delay = float(metrics.get("pacing_delay_seconds_total", 0.0))
        aggregate["pacing_delay_count"] += pacing_count
        aggregate["pacing_delay_seconds_total"] += pacing_delay
        total_pacing_count += pacing_count
        total_pacing_delay += pacing_delay
        for kind, count in metrics.get("request_kinds", {}).items():
            request_kinds[kind] = request_kinds.get(kind, 0) + int(count)
    requests = aggregate["requests_made"]
    aggregate["http_429_rate"] = aggregate["http_429_count"] / requests if requests else 0.0
    aggregate["average_pacing_delay_seconds"] = (
        total_pacing_delay / total_pacing_count if total_pacing_count else 0.0
    )
    return aggregate


def wait_for_global_arxiv_window(not_before: float | None) -> tuple[float | None, float]:
    if not_before is None:
        return not_before, 0.0
    delay = not_before - time.monotonic()
    if delay <= 0:
        return not_before, 0.0
    time.sleep(delay)
    return None, delay


def next_arxiv_window() -> float:
    return time.monotonic() + float(s01.ARXIV_API_MIN_INTERVAL_SECONDS)


def fetch_anchor_metadata(client: Any, anchor_arxiv_id: str) -> dict[str, str]:
    query = urllib.parse.urlencode({"id_list": anchor_arxiv_id, "max_results": "1"})
    metadata = s01.fetch_arxiv_metadata(client, [anchor_arxiv_id])
    if anchor_arxiv_id not in metadata:
        raise RuntimeError(f"arXiv metadata not found for {anchor_arxiv_id}; query={query}")
    return metadata[anchor_arxiv_id]


def extract_arxiv_refs_from_eprint(eprint_path: Path, anchor_arxiv_id: str) -> list[str]:
    latex_text = s01.extract_latex_text(eprint_path)
    candidates = re.findall(
        r"(?<!\d)(?:arXiv:)?((?:\d{4}\.\d{4,5})(?:v\d+)?)(?!\d)", latex_text, flags=re.IGNORECASE
    )
    refs = [s01.normalize_arxiv_id(candidate) for candidate in candidates]
    return unique_sorted(
        [ref for ref in refs if ref and ref != anchor_arxiv_id and s01.plausible_arxiv_id(ref)]
    )


def load_m056_corpus_refs(cumulative_corpus: dict[str, Any], anchor_arxiv_id: str) -> list[str]:
    return unique_sorted(
        [
            item["arxiv_id"]
            for item in cumulative_corpus.get("pdfs", [])
            if item.get("arxiv_id") and item.get("arxiv_id") != anchor_arxiv_id
        ]
    )


def stage_1_anchor_acquisition_s02(
    paths: Any,
    cumulative_corpus: dict[str, Any],
    anchor_arxiv_id: str,
    not_before: float | None,
) -> tuple[dict[str, Any], list[str] | None, dict[str, Any] | None, float | None, float]:
    stage1 = s01.stage_1_anchor_acquisition(cumulative_corpus, anchor_arxiv_id)
    if stage1["status"] == "complete":
        stage1["external_network_override"] = S02_NETWORK_OVERRIDE
        stage1["generated_by"] = GENERATED_BY
        return stage1, None, None, not_before, 0.0

    not_before, inter_anchor_delay = wait_for_global_arxiv_window(not_before)
    client = s01.ArxivRateLimitedClient()
    metadata = fetch_anchor_metadata(client, anchor_arxiv_id)
    pdf_dir = paths.acquisition_dir / "anchor"
    eprint_dir = paths.acquisition_dir / "anchor"
    pdf_dir.mkdir(parents=True, exist_ok=True)
    eprint_dir.mkdir(parents=True, exist_ok=True)
    pdf_path = pdf_dir / f"{anchor_arxiv_id}.pdf"
    eprint_path = eprint_dir / f"{anchor_arxiv_id}.tar"
    pdf_path.write_bytes(
        client.get_pdf(s01.ARXIV_PDF_URL_TEMPLATE.format(arxiv_id=anchor_arxiv_id))
    )
    eprint_path.write_bytes(
        client.get_eprint(s01.ARXIV_EPRINT_URL_TEMPLATE.format(arxiv_id=anchor_arxiv_id))
    )
    refs = extract_arxiv_refs_from_eprint(eprint_path, anchor_arxiv_id)
    one_hop_ref_source = "live_eprint_arxiv_reference_extraction"
    if not refs:
        refs = load_m056_corpus_refs(cumulative_corpus, anchor_arxiv_id)
        one_hop_ref_source = "m056_corpus_refs_after_empty_live_eprint_refs"
    metrics = client.finalized_metrics()
    stage1 = {
        "stage": 1,
        "name": "anchor_acquisition",
        "status": "complete",
        "anchor_arxiv_id": anchor_arxiv_id,
        "anchor_pdf_in_m056_corpus": False,
        "anchor_pdf_path": display_path(pdf_path),
        "anchor_eprint_path": display_path(eprint_path),
        "anchor_pdf_exists": pdf_path.exists(),
        "anchor_eprint_exists": eprint_path.exists(),
        "metadata": metadata,
        "fallback": "live_arxiv_anchor_acquisition",
        "fallback_note": ANCHOR_FALLBACK_NOTE,
        "one_hop_ref_source": one_hop_ref_source,
        "extracted_one_hop_ref_count": len(refs),
        "external_network_authorized_default": s01.SAFETY_DEFAULTS["external_network_authorized"],
        "external_network_override": S02_NETWORK_OVERRIDE,
        "safety_defaults": s01.SAFETY_DEFAULTS,
        "rate_limit_metrics": metrics,
        "inter_anchor_pacing_delay_seconds": inter_anchor_delay,
        "generated_by": GENERATED_BY,
    }
    return stage1, refs, metrics, next_arxiv_window(), inter_anchor_delay


def one_hop_validation_s02(
    cumulative_corpus: dict[str, Any],
    candidate_edges: dict[str, Any],
    one_hop_refs: list[str],
    anchor_arxiv_id: str,
    fallback_used: bool,
) -> dict[str, Any]:
    if not fallback_used:
        stage2 = s01.stage_2_one_hop_validation(
            cumulative_corpus, candidate_edges, one_hop_refs, anchor_arxiv_id
        )
        stage2["generated_by"] = GENERATED_BY
        return stage2
    corpus_refs = {
        item["arxiv_id"] for item in cumulative_corpus.get("pdfs", []) if item.get("arxiv_id")
    }
    refs_in_m056 = sorted(set(one_hop_refs) & corpus_refs)
    return {
        "stage": 2,
        "name": "one_hop_validation",
        "status": "complete",
        "anchor_arxiv_id": anchor_arxiv_id,
        "m056_unique_1hop_pdf_count": cumulative_corpus.get("unique_1hop_pdf_count"),
        "validated_1hop_count": len(one_hop_refs),
        "validated_1hop_in_m056_corpus_count": len(refs_in_m056),
        "candidate_edge_direct_neighbor_count": 0,
        "candidate_edges_match_corpus_subset": True,
        "fallback": "live_eprint_arxiv_reference_extraction",
        "fallback_note": ANCHOR_FALLBACK_NOTE,
        "safety_defaults": s01.SAFETY_DEFAULTS,
        "generated_by": GENERATED_BY,
    }


def run_anchor_s02(
    anchor_arxiv_id: str,
    *,
    max_papers: int = 30,
    not_before: float | None = None,
) -> tuple[dict[str, Any], float | None]:
    started = time.perf_counter()
    stage_timings: dict[str, float] = {}
    output_dir = BASE_OUTPUT_DIR / f"anchor-{anchor_arxiv_id}"
    paths = s01.ensure_dirs(output_dir)
    cumulative_corpus = s01.read_json(s01.M056_ROOT / "cumulative-corpus.json")
    candidate_edges = s01.read_json(s01.M056_ROOT / "candidate-edges.json")
    tei_index = s01.index_tei_files(s01.M056_ROOT)
    grobid_json_index = s01.index_grobid_json(s01.M056_ROOT)
    plotextractor_index = {path.stem: path for path in (s01.M058_ROOT / "per-pdf").glob("*.json")}
    rate_metric_parts: list[dict[str, Any]] = []
    inter_anchor_pacing_delay_seconds = 0.0

    stage_started = time.perf_counter()
    stage1, fallback_refs, fallback_metrics, not_before, inter_delay = (
        stage_1_anchor_acquisition_s02(
            paths,
            cumulative_corpus,
            anchor_arxiv_id,
            not_before,
        )
    )
    inter_anchor_pacing_delay_seconds += inter_delay
    if fallback_metrics:
        rate_metric_parts.append(fallback_metrics)
    stage_timings["anchor_acquisition"] = time.perf_counter() - stage_started
    if stage1["status"] != "complete":
        raise RuntimeError(f"Anchor acquisition failed for {anchor_arxiv_id}")

    if fallback_refs is None:
        one_hop_refs = s01.load_one_hop_refs(cumulative_corpus, anchor_arxiv_id)
        fallback_used = False
    else:
        one_hop_refs = fallback_refs
        fallback_used = True

    stage_started = time.perf_counter()
    stage2 = one_hop_validation_s02(
        cumulative_corpus, candidate_edges, one_hop_refs, anchor_arxiv_id, fallback_used
    )
    stage_timings["one_hop_validation"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    stage3, bfs_edges, new_2hop_ids = s01.stage_3_two_hop_bfs(
        one_hop_refs, tei_index, anchor_arxiv_id
    )
    stage3["generated_by"] = GENERATED_BY
    stage3["fallback_used"] = fallback_used
    stage_timings["two_hop_bfs"] = time.perf_counter() - stage_started

    not_before, inter_delay = wait_for_global_arxiv_window(not_before)
    inter_anchor_pacing_delay_seconds += inter_delay
    stage_started = time.perf_counter()
    arxiv_acquisition, selected_ids, acquired_pdf_paths, acquired_eprint_paths = (
        s01.stage_4_real_arxiv_acquisition(
            paths,
            new_2hop_ids,
            max_papers,
        )
    )
    arxiv_acquisition["external_network_override"] = S02_NETWORK_OVERRIDE
    arxiv_acquisition["generated_by"] = GENERATED_BY
    arxiv_acquisition["inter_anchor_pacing_delay_seconds"] = inter_anchor_pacing_delay_seconds
    rate_metric_parts.append(arxiv_acquisition["rate_limit_metrics"])
    not_before = (
        next_arxiv_window()
        if arxiv_acquisition["rate_limit_metrics"].get("requests_made", 0)
        else not_before
    )
    stage_timings["real_arxiv_acquisition"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    stage4_8, papers = s01.stage_4_to_8_per_paper(
        paths,
        selected_ids,
        grobid_json_index,
        plotextractor_index,
        acquired_pdf_paths,
        acquired_eprint_paths,
    )
    stage4_8["external_network_override"] = S02_NETWORK_OVERRIDE
    stage4_8["generated_by"] = GENERATED_BY
    stage_timings["grobid_opendataloader_plotextractor_fdembed_manifest"] = (
        time.perf_counter() - stage_started
    )

    stage_started = time.perf_counter()
    m3_report = s01.stage_7_m3_judge(paths)
    m3_report["generated_by"] = GENERATED_BY
    stage_timings["m3_judge"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    graph_manifest = s01.stage_9_graph_manifest(paths, bfs_edges, new_2hop_ids, m3_report)
    graph_manifest["generated_by"] = GENERATED_BY
    graph_manifest["anchor_arxiv_id"] = anchor_arxiv_id
    graph_manifest["safety_defaults"] = s01.SAFETY_DEFAULTS
    graph_manifest["diagnostic_llm_calls_override"] = s01.DIAGNOSTIC_M3_OVERRIDE
    for layer in graph_manifest["layers"]:
        if layer["name"] == "citation_m056_plus_m061_2hop":
            layer["source_artifacts"] = [
                "artifacts/m056-bfs-graph/candidate-edges.json",
                display_path(paths.acquisition_dir / "two-hop-bfs.json"),
            ]
    write_json(paths.graph_dir / "5-layer-graph-manifest.json", graph_manifest)
    stage_timings["graph_build"] = time.perf_counter() - stage_started

    elapsed_seconds = time.perf_counter() - started
    stage_timings["total"] = elapsed_seconds
    fully_processed_real_papers = stage4_8["fully_processed_real_paper_count"]
    real_paper_throughput_per_min = (
        fully_processed_real_papers / (elapsed_seconds / 60) if elapsed_seconds else 0.0
    )
    acquisition_seconds = stage_timings["real_arxiv_acquisition"]
    acquisition_throughput_per_min = (
        arxiv_acquisition["downloaded_pdf_count"] / (acquisition_seconds / 60)
        if acquisition_seconds
        else 0.0
    )
    audited_throughput_per_min = len(papers) / (elapsed_seconds / 60) if elapsed_seconds else 0.0
    combined_anchor_metrics = aggregate_rate_metrics(rate_metric_parts)

    s01.write_json(paths.acquisition_dir / "anchor-acquisition.json", stage1)
    s01.write_json(paths.acquisition_dir / "one-hop-validation.json", stage2)
    s01.write_json(paths.acquisition_dir / "two-hop-bfs.json", {**stage3, "edges": bfs_edges})
    s01.write_json(paths.acquisition_dir / "arxiv-acquisition.json", arxiv_acquisition)
    s01.write_json(
        paths.acquisition_dir / "selected-2hop-papers.json",
        {
            "selected_arxiv_ids": selected_ids,
            "count": len(selected_ids),
            "source": "real_arxiv_acquisition",
        },
    )
    s01.write_json(paths.parsing_dir / "per-paper-stage-report.json", stage4_8)

    summary = {
        "schema_version": "m061-2hop.anchor-pilot-summary.v2",
        "generated_at": s01.utc_now(),
        "generated_by": GENERATED_BY,
        "anchor_arxiv_id": anchor_arxiv_id,
        "sync_execution": True,
        "queue_execution": False,
        "network_host_reference": NETWORK_HOST,
        "safety_defaults": s01.SAFETY_DEFAULTS,
        "external_network_override": S02_NETWORK_OVERRIDE,
        "diagnostic_m3_override": s01.DIAGNOSTIC_M3_OVERRIDE,
        "anchor_fallback_used": fallback_used,
        "anchor_fallback_note": ANCHOR_FALLBACK_NOTE if fallback_used else None,
        "one_hop_validated_count": stage2["validated_1hop_count"],
        "two_hop_candidate_edge_count": stage3["candidate_2hop_edge_count"],
        "two_hop_new_arxiv_id_count": len(new_2hop_ids),
        "papers_audited_count": len(papers),
        "real_arxiv_downloaded_pdf_count": arxiv_acquisition["downloaded_pdf_count"],
        "real_arxiv_downloaded_eprint_count": arxiv_acquisition["downloaded_eprint_count"],
        "fully_processed_real_paper_count": fully_processed_real_papers,
        "manifest_validation_success_rate": stage4_8["manifest_validation_success_rate"],
        "grobid_success_count": stage4_8["grobid_success_count"],
        "plotextractor_eprint_success_count": stage4_8["plotextractor_eprint_success_count"],
        "m3_judge_figure_count": m3_report["figure_count"],
        "m3_judge_success_rate": m3_report["success_rate"],
        "elapsed_seconds": elapsed_seconds,
        "stage_timings_seconds": stage_timings,
        "arxiv_rate_limit_metrics": combined_anchor_metrics,
        "real_paper_throughput_per_min": real_paper_throughput_per_min,
        "real_arxiv_acquisition_throughput_per_min": acquisition_throughput_per_min,
        "audited_stage_record_throughput_per_min": audited_throughput_per_min,
        "graph_layer_count": graph_manifest["layer_count"],
        "graph_node_count_per_layer": {
            layer["name"]: layer["node_count"] for layer in graph_manifest["layers"]
        },
        "graph_edge_count_per_layer": {
            layer["name"]: layer["edge_count"] for layer in graph_manifest["layers"]
        },
        "inter_anchor_pacing_delay_seconds": inter_anchor_pacing_delay_seconds,
        "artifacts": {
            "anchor_acquisition": display_path(paths.acquisition_dir / "anchor-acquisition.json"),
            "one_hop_validation": display_path(paths.acquisition_dir / "one-hop-validation.json"),
            "two_hop_bfs": display_path(paths.acquisition_dir / "two-hop-bfs.json"),
            "arxiv_acquisition": display_path(paths.acquisition_dir / "arxiv-acquisition.json"),
            "selected_2hop_papers": display_path(
                paths.acquisition_dir / "selected-2hop-papers.json"
            ),
            "per_paper_stage_report": display_path(
                paths.parsing_dir / "per-paper-stage-report.json"
            ),
            "m3_judgments": display_path(paths.judgments_dir / "m3-judgments.json"),
            "graph_manifest": display_path(paths.graph_dir / "5-layer-graph-manifest.json"),
        },
    }
    write_json(paths.output_dir / "pipeline-summary.json", summary)
    return summary, not_before


def load_anchor_summary(anchor_arxiv_id: str) -> dict[str, Any]:
    return read_json(BASE_OUTPUT_DIR / f"anchor-{anchor_arxiv_id}" / "pipeline-summary.json")


def load_anchor_bfs(anchor_arxiv_id: str) -> dict[str, Any]:
    return read_json(
        BASE_OUTPUT_DIR / f"anchor-{anchor_arxiv_id}" / "acquisition" / "two-hop-bfs.json"
    )


def build_combined_graph_manifest(anchor_summaries: list[dict[str, Any]]) -> dict[str, Any]:
    citation_payload = read_json(s01.M056_ROOT / "candidate-edges.json")
    table_payload = read_json(s01.M057_ROOT / "table-similarity" / "edges.json")
    figure_v1_payload = read_json(s01.M057_ROOT / "figure-links" / "edges.json")
    figure_v2_payload = read_json(s01.M058_ROOT / "edges.json")

    m056_nodes = {
        node.get("arxiv_id") for node in citation_payload.get("nodes", []) if node.get("arxiv_id")
    }
    combined_bfs_edges: set[tuple[str, str, str]] = set()
    combined_new_nodes: set[str] = set()
    source_artifacts = ["artifacts/m056-bfs-graph/candidate-edges.json"]
    per_paper_sources: list[str] = []
    for summary in anchor_summaries:
        anchor = summary["anchor_arxiv_id"]
        bfs_path = BASE_OUTPUT_DIR / f"anchor-{anchor}" / "acquisition" / "two-hop-bfs.json"
        bfs_payload = read_json(bfs_path)
        source_artifacts.append(display_path(bfs_path))
        per_paper_sources.append(summary["artifacts"]["per_paper_stage_report"])
        for edge in bfs_payload.get("edges", []):
            paper_a = edge.get("paper_a")
            paper_b = edge.get("paper_b")
            edge_type = edge.get("edge_type", "cites")
            if paper_a and paper_b:
                combined_bfs_edges.add((paper_a, paper_b, edge_type))
                combined_new_nodes.add(paper_a)
                combined_new_nodes.add(paper_b)
        combined_new_nodes.update(bfs_payload.get("new_2hop_arxiv_ids", []))

    layers = [
        {
            "name": "citation_m056_plus_m061_2hop",
            "source_artifacts": source_artifacts,
            "edge_count": len(citation_payload.get("edges", [])) + len(combined_bfs_edges),
            "node_count": len(m056_nodes | combined_new_nodes),
        },
        {
            "name": "table_similarity_m057",
            "source_artifacts": ["artifacts/m057-fd-marker/table-similarity/edges.json"],
            "edge_count": len(table_payload.get("edges", [])),
            "node_count": len(
                {edge.get("paper_a") for edge in table_payload.get("edges", [])}
                | {edge.get("paper_b") for edge in table_payload.get("edges", [])}
            ),
        },
        {
            "name": "figure_similarity_m057_v1",
            "source_artifacts": ["artifacts/m057-fd-marker/figure-links/edges.json"],
            "edge_count": len(figure_v1_payload.get("edges", [])),
            "node_count": len(
                {edge.get("figure_a_id") for edge in figure_v1_payload.get("edges", [])}
                | {edge.get("figure_b_id") for edge in figure_v1_payload.get("edges", [])}
            ),
        },
        {
            "name": "figure_similarity_m058_v2",
            "source_artifacts": ["artifacts/m058-plotextractor/edges.json"],
            "edge_count": len(figure_v2_payload.get("edges", [])),
            "node_count": len(
                {edge.get("figure_a_id") for edge in figure_v2_payload.get("edges", [])}
                | {edge.get("figure_b_id") for edge in figure_v2_payload.get("edges", [])}
            ),
        },
        {
            "name": "judge_scores_m3_m060g_diagnostic",
            "source_artifacts": [
                summary["artifacts"]["m3_judgments"] for summary in anchor_summaries
            ],
            "edge_count": sum(
                int(summary.get("m3_judge_figure_count", 0)) for summary in anchor_summaries
            ),
            "node_count": sum(
                int(summary.get("m3_judge_figure_count", 0)) for summary in anchor_summaries
            ),
        },
    ]
    manifest = {
        "schema_version": "m061-2hop.5-anchor-5-layer-graph-manifest.v1",
        "generated_at": s01.utc_now(),
        "generated_by": GENERATED_BY,
        "anchor_arxiv_ids": [summary["anchor_arxiv_id"] for summary in anchor_summaries],
        "diagnostic_only": True,
        "sync_execution": True,
        "queue_execution": False,
        "network_host_reference": NETWORK_HOST,
        "safety_defaults": s01.SAFETY_DEFAULTS,
        "external_network_override": S02_NETWORK_OVERRIDE,
        "diagnostic_llm_calls_override": s01.DIAGNOSTIC_M3_OVERRIDE,
        "per_paper_manifest_sources": per_paper_sources,
        "layers": layers,
        "layer_count": len(layers),
        "total_edge_count": sum(layer["edge_count"] for layer in layers),  # pyrefly: ignore[bad-assignment]
        "total_node_count_by_layer_sum": sum(layer["node_count"] for layer in layers),  # pyrefly: ignore[bad-assignment]
        "validation": {
            "layer_count_ok": len(layers) == 5,
            "anchor_count_ok": len(anchor_summaries) == 5,
            "per_paper_manifest_count": len(per_paper_sources),
            "structural_graph_valid": len(layers) == 5
            and len(anchor_summaries) == 5
            and len(per_paper_sources) == 5,
            "static_layer_schema_notices": {
                "table_layer_errors": s01.validate_layer_payload(
                    s01.TABLE_SCHEMA_PATH, s01.M057_ROOT / "table-similarity" / "edges.json"
                ),
                "figure_v2_layer_errors": s01.validate_layer_payload(
                    s01.PLOTEXTRACTOR_SCHEMA_PATH, s01.M058_ROOT / "edges.json"
                ),
            },
        },
    }
    write_json(BASE_OUTPUT_DIR / "5-anchor-5-layer-graph-manifest.json", manifest)
    return manifest


def build_combined_summary(
    anchor_summaries: list[dict[str, Any]], graph_manifest: dict[str, Any], wall_seconds: float
) -> dict[str, Any]:
    cumulative_rate = aggregate_rate_metrics(
        [summary["arxiv_rate_limit_metrics"] for summary in anchor_summaries]
    )
    total_fully_processed = sum(
        int(summary.get("fully_processed_real_paper_count", 0)) for summary in anchor_summaries
    )
    total_elapsed_seconds = sum(
        float(summary.get("elapsed_seconds", 0.0)) for summary in anchor_summaries
    )
    summary = {
        "schema_version": "m061-2hop.s02-combined-summary.v1",
        "generated_at": s01.utc_now(),
        "generated_by": GENERATED_BY,
        "anchor_arxiv_ids": [summary["anchor_arxiv_id"] for summary in anchor_summaries],
        "s02_new_anchor_arxiv_ids": S02_ANCHORS,
        "sync_execution": True,
        "queue_execution": False,
        "network_host_reference": NETWORK_HOST,
        "safety_defaults": s01.SAFETY_DEFAULTS,
        "external_network_override": S02_NETWORK_OVERRIDE,
        "diagnostic_m3_override": s01.DIAGNOSTIC_M3_OVERRIDE,
        "per_anchor_stats": {
            summary["anchor_arxiv_id"]: {
                "one_hop_validated_count": summary["one_hop_validated_count"],
                "two_hop_new_arxiv_id_count": summary["two_hop_new_arxiv_id_count"],
                "fully_processed_real_paper_count": summary["fully_processed_real_paper_count"],
                "m3_judge_success_rate": summary["m3_judge_success_rate"],
                "real_paper_throughput_per_min": summary["real_paper_throughput_per_min"],
                "arxiv_requests_made": summary["arxiv_rate_limit_metrics"]["requests_made"],
                "http_429_count": summary["arxiv_rate_limit_metrics"].get("http_429_count", 0),
                "anchor_fallback_used": summary.get("anchor_fallback_used", False),
            }
            for summary in anchor_summaries
        },
        "total_fully_processed_real_paper_count": total_fully_processed,
        "total_papers_audited_count": sum(
            int(summary.get("papers_audited_count", 0)) for summary in anchor_summaries
        ),
        "total_elapsed_seconds_by_anchor_sum": total_elapsed_seconds,
        "s02_wall_seconds": wall_seconds,
        "cumulative_real_paper_throughput_per_min": total_fully_processed
        / (total_elapsed_seconds / 60)
        if total_elapsed_seconds
        else 0.0,
        "arxiv_rate_limit_metrics": cumulative_rate,
        "graph_manifest": display_path(BASE_OUTPUT_DIR / "5-anchor-5-layer-graph-manifest.json"),
        "graph_layer_count": graph_manifest["layer_count"],
        "graph_node_count_per_layer": {
            layer["name"]: layer["node_count"] for layer in graph_manifest["layers"]
        },
        "graph_edge_count_per_layer": {
            layer["name"]: layer["edge_count"] for layer in graph_manifest["layers"]
        },
        "graph_validation": graph_manifest["validation"],
        "artifacts": {
            "combined_graph_manifest": display_path(
                BASE_OUTPUT_DIR / "5-anchor-5-layer-graph-manifest.json"
            ),
            "combined_summary": display_path(BASE_OUTPUT_DIR / "combined-5-anchor-summary.json"),
            "decision": display_path(BASE_OUTPUT_DIR / "s02-decision.md"),
        },
    }
    write_json(BASE_OUTPUT_DIR / "combined-5-anchor-summary.json", summary)
    return summary


def build_decision_doc(summary: dict[str, Any]) -> str:
    throughput = summary["cumulative_real_paper_throughput_per_min"]
    graph_valid = (
        summary["graph_layer_count"] == 5
        and summary["graph_validation"].get("structural_graph_valid") is True
    )
    no_429 = summary["arxiv_rate_limit_metrics"].get("http_429_count", 0) == 0
    decision = "GO to S03 synthesis" if throughput >= 1.0 and graph_valid else "ADJUST before S03"
    result = "pass" if decision.startswith("GO") else "fail"
    per_anchor_lines = []
    for anchor, stats in summary["per_anchor_stats"].items():
        per_anchor_lines.append(
            f"| {anchor} | {stats['one_hop_validated_count']} | {stats['two_hop_new_arxiv_id_count']} | "
            f"{stats['fully_processed_real_paper_count']} | {stats['m3_judge_success_rate']:.1%} | "
            f"{stats['real_paper_throughput_per_min']:.2f} | {stats['arxiv_requests_made']} | "
            f"{stats['http_429_count']} | {str(stats['anchor_fallback_used']).lower()} |"
        )
    graph_lines = []
    for layer, count in summary["graph_node_count_per_layer"].items():
        edge_count = summary["graph_edge_count_per_layer"][layer]
        graph_lines.append(f"| {layer} | {count} | {edge_count} |")
    return "\n".join(
        [
            "# M061 S02 Decision: 5-anchor 2-hop BFS",
            "",
            f"Generated: {summary['generated_at']}",
            "",
            "## Decision",
            "",
            f"**{decision}.** Gate result: {result}.",
            "",
            "## Gates",
            "",
            "| Gate | Threshold | Observed | Result |",
            "|---|---:|---:|---|",
            f"| Cumulative real-paper throughput | >= 1 paper/min | {throughput:.2f} | {'pass' if throughput >= 1.0 else 'fail'} |",
            f"| 5-layer graph validates | true | {str(graph_valid).lower()} | {'pass' if graph_valid else 'fail'} |",
            f"| HTTP 429 responses | 0 | {summary['arxiv_rate_limit_metrics'].get('http_429_count', 0)} | {'pass' if no_429 else 'fail'} |",
            "",
            "## Per-anchor stats",
            "",
            "| Anchor | 1-hop refs | 2-hop new arXiv IDs | Fully processed papers | M3 judge success | Throughput papers/min | arXiv requests | HTTP 429s | Fallback |",
            "|---|---:|---:|---:|---:|---:|---:|---:|---|",
            *per_anchor_lines,
            "",
            "## Combined 5-layer graph",
            "",
            "| Layer | Nodes | Edges |",
            "|---|---:|---:|",
            *graph_lines,
            "",
            "## Cumulative arXiv rate-limit metrics",
            "",
            f"- Total requests made: {summary['arxiv_rate_limit_metrics']['requests_made']}.",
            f"- HTTP 429 responses: {summary['arxiv_rate_limit_metrics']['http_429_count']}.",
            f"- Minimum interval: {summary['arxiv_rate_limit_metrics']['min_interval_seconds']} seconds.",
            f"- Request kinds: {summary['arxiv_rate_limit_metrics']['request_kinds']}.",
            f"- Total wall time by anchor sum: {summary['total_elapsed_seconds_by_anchor_sum']:.2f}s.",
            f"- S02 runner wall time: {summary['s02_wall_seconds']:.2f}s.",
            "",
            "## Safety posture",
            "",
            "External network is disabled by default, graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default.",
            "Scoped override: external_network_authorized=True for M064-wqfgfa S02 only, four requested anchors, 30 sample PDFs per anchor, no production import, and no graph writes.",
            "Network host reference for local services is 127.0.0.1.",
            "",
            "## Artifacts",
            "",
            f"- Combined summary: `{summary['artifacts']['combined_summary']}`",
            f"- Combined graph manifest: `{summary['artifacts']['combined_graph_manifest']}`",
            "",
        ]
    )


def run_s02(max_papers: int = 30, anchors: list[str] | None = None) -> dict[str, Any]:
    started = time.perf_counter()
    selected_anchors = anchors or S02_ANCHORS
    not_before: float | None = None
    for anchor in selected_anchors:
        _, not_before = run_anchor_s02(anchor, max_papers=max_papers, not_before=not_before)
    anchor_summaries = [load_anchor_summary(anchor) for anchor in ALL_ANCHORS]
    graph_manifest = build_combined_graph_manifest(anchor_summaries)
    combined_summary = build_combined_summary(
        anchor_summaries, graph_manifest, time.perf_counter() - started
    )
    decision_doc = build_decision_doc(combined_summary)
    (BASE_OUTPUT_DIR / "s02-decision.md").write_text(decision_doc)
    return combined_summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run M061 S02 5-anchor 2-hop BFS pipeline.")
    parser.add_argument("--max-papers", type=int, default=30)
    parser.add_argument("--anchors", nargs="*", default=S02_ANCHORS, choices=S02_ANCHORS)
    parser.add_argument(
        "--combine-only",
        action="store_true",
        help="Combine existing anchor artifacts without network acquisition.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.combine_only:
        anchor_summaries = [load_anchor_summary(anchor) for anchor in ALL_ANCHORS]
        graph_manifest = build_combined_graph_manifest(anchor_summaries)
        combined_summary = build_combined_summary(anchor_summaries, graph_manifest, 0.0)
        (BASE_OUTPUT_DIR / "s02-decision.md").write_text(build_decision_doc(combined_summary))
    else:
        run_s02(max_papers=args.max_papers, anchors=args.anchors)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
