#!/usr/bin/env python3
"""M061 S01 1-anchor 2-hop BFS pilot for anchor 2605.18747.

The pilot is synchronous by design (ADR-017). It is diagnostic-only: graph writes
are not authorized, production import is not authorized, fact promotion is not
authorized, external network is disabled by default, and LLM calls are disabled
by default. Stage 7 records a scoped diagnostic-only M3 override by reusing the
M060g evidence bundle rather than making new live model calls.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import time
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parents[1]
ANCHOR_ARXIV_ID = "2605.18747"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m061-2hop" / "anchor-2605.18747"
M056_ROOT = ROOT / "artifacts" / "m056-bfs-graph"
M057_ROOT = ROOT / "artifacts" / "m057-fd-marker"
M058_ROOT = ROOT / "artifacts" / "m058-plotextractor"
M060G_ROOT = ROOT / "artifacts" / "m060g-judge"
GENERATED_BY = "scripts/m061_anchor_pilot.py"
NETWORK_HOST = "127.0.0.1"
GROBID_URL = f"http://{NETWORK_HOST}:8070/api/processFulltextDocument"
FD_URL = f"http://{NETWORK_HOST}:8000"
MANIFEST_SCHEMA_PATH = ROOT / "schemas" / "daily-archive.pdf-batch-manifest.v1.json"
GROBID_SCHEMA_PATH = ROOT / "schemas" / "grobid-tei.v1.json"
OPENDATALOADER_SCHEMA_PATH = ROOT / "schemas" / "opendataloader-pdf.v1.json"
PLOTEXTRACTOR_SCHEMA_PATH = ROOT / "schemas" / "m058-plotextractor-figure-caption.v1.json"
TABLE_SCHEMA_PATH = ROOT / "schemas" / "m057-fd-table-similarity.v1.json"

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}

DIAGNOSTIC_M3_OVERRIDE: dict[str, Any] = {
    "llm_calls_authorized": True,
    "scope": "M061 S01 M3 diagnostic-only evidence reuse from M060g",
    "reason": (
        "Live LLM calls are disabled by default; M3 diagnostic scores are reused "
        "from artifacts/m060g-judge. Graph writes is not authorized, production "
        "import is not authorized, and fact promotion is not authorized."
    ),
    "model": "MiniMax-M3",
    "binding_id": "figure-qa-judge-quality",
}

PARSER_EXPECTATIONS: list[dict[str, str]] = [
    {"name": "grobid-fulltext", "version": "existing-m056-or-skip", "mode": "sync", "expected_output_schema": "schemas/grobid-tei.v1.json"},
    {"name": "opendataloader", "version": "diagnostic-wrapper", "mode": "sync", "expected_output_schema": "schemas/opendataloader-pdf.v1.json"},
    {"name": "plotextractor", "version": "existing-m058-or-skip", "mode": "sync", "expected_output_schema": "schemas/m058-plotextractor-figure-caption.v1.json"},
]

ARXIV_ID_RE = re.compile(r"(?i)(?:arxiv\s*:\s*)?(\d{4}\.\d{4,5})(?:v\d+)?")
TEI_NS = {"tei": "http://www.tei-c.org/ns/1.0"}
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()


@dataclass(frozen=True)
class PipelinePaths:
    output_dir: Path
    acquisition_dir: Path
    parsing_dir: Path
    judgments_dir: Path
    graph_dir: Path
    paper_manifest_dir: Path
    parser_output_dir: Path


def utc_now() -> str:
    return dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def ensure_dirs(output_dir: Path) -> PipelinePaths:
    paths = PipelinePaths(
        output_dir=output_dir,
        acquisition_dir=output_dir / "acquisition",
        parsing_dir=output_dir / "parsing",
        judgments_dir=output_dir / "judgments",
        graph_dir=output_dir / "graph",
        paper_manifest_dir=output_dir / "parsing" / "paper-manifests",
        parser_output_dir=output_dir / "parsing" / "parser-outputs",
    )
    for directory in (
        paths.acquisition_dir,
        paths.parsing_dir,
        paths.judgments_dir,
        paths.graph_dir,
        paths.paper_manifest_dir,
        paths.parser_output_dir,
    ):
        directory.mkdir(parents=True, exist_ok=True)
    return paths


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def normalize_arxiv_id(value: str | None) -> str | None:
    if not value:
        return None
    match = ARXIV_ID_RE.search(value)
    return match.group(1) if match else None


def index_tei_files(root: Path = M056_ROOT) -> dict[str, Path]:
    index: dict[str, Path] = {}
    for path in root.rglob("*.tei.xml"):
        arxiv_id = path.name.removesuffix(".tei.xml")
        if normalize_arxiv_id(arxiv_id):
            index[arxiv_id] = path
    return index


def index_grobid_json(root: Path = M056_ROOT) -> dict[str, Path]:
    index: dict[str, Path] = {}
    for path in root.rglob("grobid-fulltext/per-pdf/*.json"):
        arxiv_id = normalize_arxiv_id(path.stem)
        if arxiv_id:
            index[arxiv_id] = path
    for path in root.rglob("anchor-grobid/per-pdf/*.json"):
        arxiv_id = normalize_arxiv_id(path.stem)
        if arxiv_id:
            index[arxiv_id] = path
    return index


def extract_arxiv_refs_from_tei(tei_path: Path, source_arxiv_id: str) -> list[str]:
    root = ET.parse(tei_path).getroot()
    refs: set[str] = set()
    for bibl in root.findall(".//tei:biblStruct", TEI_NS):
        texts: list[str] = []
        for element in bibl.iter():
            if element.text:
                texts.append(element.text)
        for text in texts:
            for match in ARXIV_ID_RE.finditer(text):
                candidate = match.group(1)
                if candidate != source_arxiv_id:
                    refs.add(candidate)
    return sorted(refs)


def load_one_hop_refs(cumulative_corpus: dict[str, Any], anchor_arxiv_id: str) -> list[str]:
    pdfs = cumulative_corpus.get("pdfs", [])
    refs = sorted({item["arxiv_id"] for item in pdfs if item.get("arxiv_id") != anchor_arxiv_id})
    expected = cumulative_corpus.get("unique_1hop_pdf_count")
    if expected is not None and expected != len(refs):
        raise RuntimeError(f"M056 1-hop ref count mismatch: expected {expected}, got {len(refs)}")
    return refs


def stage_1_anchor_acquisition(cumulative_corpus: dict[str, Any], anchor_arxiv_id: str) -> dict[str, Any]:
    anchor_pdf = next((item for item in cumulative_corpus.get("pdfs", []) if item.get("arxiv_id") == anchor_arxiv_id), None)
    verified = bool(anchor_pdf and (ROOT / anchor_pdf.get("path", "")).exists())
    return {
        "stage": 1,
        "name": "anchor_acquisition",
        "status": "complete" if verified else "failed",
        "anchor_arxiv_id": anchor_arxiv_id,
        "anchor_pdf_in_m056_corpus": bool(anchor_pdf),
        "anchor_pdf_path": anchor_pdf.get("path") if anchor_pdf else None,
        "anchor_pdf_exists": verified,
        "external_network_authorized": SAFETY_DEFAULTS["external_network_authorized"],
        "note": "Anchor PDF was reused from M056 corpus; live arXiv download is disabled by default.",
    }


def stage_2_one_hop_validation(
    cumulative_corpus: dict[str, Any], candidate_edges: dict[str, Any], one_hop_refs: list[str], anchor_arxiv_id: str
) -> dict[str, Any]:
    edge_neighbors = {
        edge["paper_b"]
        for edge in candidate_edges.get("edges", [])
        if edge.get("paper_a") == anchor_arxiv_id and normalize_arxiv_id(edge.get("paper_b"))
    }
    corpus_refs = set(one_hop_refs)
    return {
        "stage": 2,
        "name": "one_hop_validation",
        "status": "complete" if len(one_hop_refs) == cumulative_corpus.get("unique_1hop_pdf_count") else "failed",
        "anchor_arxiv_id": anchor_arxiv_id,
        "m056_unique_1hop_pdf_count": cumulative_corpus.get("unique_1hop_pdf_count"),
        "validated_1hop_count": len(one_hop_refs),
        "candidate_edge_direct_neighbor_count": len(edge_neighbors),
        "candidate_edges_match_corpus_subset": edge_neighbors.issubset(corpus_refs),
        "extra_candidate_edge_neighbors": sorted(edge_neighbors - corpus_refs)[:25],
        "safety_defaults": SAFETY_DEFAULTS,
    }


def stage_3_two_hop_bfs(
    one_hop_refs: list[str], tei_index: dict[str, Path], anchor_arxiv_id: str
) -> tuple[dict[str, Any], list[dict[str, Any]], list[str]]:
    one_hop_set = set(one_hop_refs)
    edges: list[dict[str, Any]] = []
    per_ref: list[dict[str, Any]] = []
    all_targets: set[str] = set()
    for source in one_hop_refs:
        tei_path = tei_index.get(source)
        refs = extract_arxiv_refs_from_tei(tei_path, source) if tei_path else []
        all_targets.update(refs)
        edges.extend(
            {
                "paper_a": source,
                "paper_b": target,
                "edge_type": "cites",
                "evidence": "grobid_tei_biblstruct",
                "source_tei": str(tei_path.relative_to(ROOT)) if tei_path else None,
            }
            for target in refs
        )
        per_ref.append(
            {
                "arxiv_id": source,
                "tei_available": tei_path is not None,
                "ref_count": len(refs),
                "new_2hop_ref_count": len(set(refs) - one_hop_set - {anchor_arxiv_id}),
            }
        )
    new_2hop_ids = sorted(all_targets - one_hop_set - {anchor_arxiv_id})
    report = {
        "stage": 3,
        "name": "two_hop_bfs_algorithm",
        "status": "complete",
        "anchor_arxiv_id": anchor_arxiv_id,
        "one_hop_input_count": len(one_hop_refs),
        "one_hop_with_tei_count": sum(1 for row in per_ref if row["tei_available"]),
        "candidate_2hop_edge_count": len(edges),
        "unique_2hop_target_count": len(all_targets),
        "new_2hop_arxiv_id_count": len(new_2hop_ids),
        "new_2hop_arxiv_ids_sample": new_2hop_ids[:50],
        "per_ref": per_ref,
        "safety_defaults": SAFETY_DEFAULTS,
    }
    return report, edges, new_2hop_ids


def build_manifest_item(arxiv_id: str, pdf_path: Path | None) -> dict[str, Any]:
    if pdf_path and pdf_path.exists():
        rel_path = str(pdf_path.relative_to(ROOT)) if pdf_path.is_absolute() and pdf_path.is_relative_to(ROOT) else str(pdf_path)
        size_bytes = pdf_path.stat().st_size
        content_sha256 = sha256_file(pdf_path)
        storage_provider = "local"
    else:
        rel_path = f"not-acquired/{arxiv_id}.pdf"
        size_bytes = 0
        content_sha256 = EMPTY_SHA256
        storage_provider = "unknown"
    return {
        "arxiv_id": arxiv_id,
        "source_uri": f"https://arxiv.org/pdf/{arxiv_id}",
        "storage_provider": storage_provider,
        "path": rel_path,
        "size_bytes": size_bytes,
        "content_sha256": content_sha256,
        "expected_parsers": PARSER_EXPECTATIONS,
    }


def find_existing_pdf_path(grobid_json_index: dict[str, Path], arxiv_id: str) -> Path | None:
    json_path = grobid_json_index.get(arxiv_id)
    if not json_path:
        return None
    payload = read_json(json_path)
    pdf_path = payload.get("pdf_path")
    if not pdf_path:
        return None
    candidate = ROOT / pdf_path
    return candidate if candidate.exists() else None


def validate_json(schema_path: Path, payload: dict[str, Any]) -> list[str]:
    schema = read_json(schema_path)
    validator = Draft7Validator(schema)
    return [error.message for error in sorted(validator.iter_errors(payload), key=lambda item: item.path)]


def write_parser_wrappers(
    paths: PipelinePaths,
    arxiv_id: str,
    manifest_batch_id: str,
    grobid_json_path: Path | None,
    plotextractor_path: Path | None,
    pdf_path: Path | None,
) -> dict[str, Any]:
    parser_dir = paths.parser_output_dir / arxiv_id
    parser_dir.mkdir(parents=True, exist_ok=True)

    grobid_payload = (
        read_json(grobid_json_path)
        if grobid_json_path and grobid_json_path.exists()
        else {
            "schema_version": "grobid-tei.v1",
            "tei_xml_sha256": EMPTY_SHA256,
            "header": {"title": "", "authors": []},
            "biblStruct": [],
            "abstract": "",
            "body_sections": [],
            "arxiv_id": arxiv_id,
            "status": "skipped_external_network_disabled",
            "pdf_path": str(pdf_path) if pdf_path else f"not-acquired/{arxiv_id}.pdf",
            "safety_defaults": SAFETY_DEFAULTS,
            "endpoint": GROBID_URL,
            "message": "GROBID live parsing is disabled by default for this diagnostic pilot.",
        }
    )
    grobid_payload.setdefault("safety_defaults", SAFETY_DEFAULTS)
    grobid_out = parser_dir / "grobid-fulltext.json"
    write_json(grobid_out, grobid_payload)

    opendataloader_payload = {
        "schema_version": "opendataloader-pdf.v1",
        "source_arxiv_id": arxiv_id,
        "manifest_batch_id": manifest_batch_id,
        "parser_version_pinned": "diagnostic-wrapper-local-only",
        "status": "skipped_parser_execution_disabled" if not pdf_path else "not_run_local_only_pilot",
        "pdf_path": str(pdf_path.relative_to(ROOT)) if pdf_path and pdf_path.is_relative_to(ROOT) else (str(pdf_path) if pdf_path else f"not-acquired/{arxiv_id}.pdf"),
        "safety_defaults": SAFETY_DEFAULTS,
        "message": "OpenDataLoader execution is disabled by default; wrapper records sync stage outcome.",
    }
    opendataloader_out = parser_dir / "opendataloader.json"
    write_json(opendataloader_out, opendataloader_payload)

    plot_payload = (
        read_json(plotextractor_path)
        if plotextractor_path and plotextractor_path.exists()
        else {
            "schema_version": "m058-plotextractor-figure-caption.v1",
            "per_pdf": [
                {
                    "arxiv_id": arxiv_id,
                    "tex_status": "skipped_external_network_disabled",
                    "figures": [],
                    "caption_count": 0,
                    "figure_count": 0,
                    "message": "TeX acquisition and PlotExtractor execution is disabled by default for this diagnostic pilot.",
                }
            ],
            "safety_defaults": SAFETY_DEFAULTS,
        }
    )
    if "per_pdf" not in plot_payload:
        plot_payload = {
            "schema_version": "m058-plotextractor-figure-caption.v1",
            "per_pdf": [plot_payload],
            "safety_defaults": SAFETY_DEFAULTS,
        }
    plot_out = parser_dir / "plotextractor.json"
    write_json(plot_out, plot_payload)

    validations = {
        "grobid": validate_json(GROBID_SCHEMA_PATH, grobid_payload),
        "opendataloader": validate_json(OPENDATALOADER_SCHEMA_PATH, opendataloader_payload),
        "plotextractor": validate_json(PLOTEXTRACTOR_SCHEMA_PATH, plot_payload),
    }
    return {
        "parser_output_dir": str(parser_dir.relative_to(paths.output_dir)),
        "grobid_output": str(grobid_out.relative_to(paths.output_dir)),
        "opendataloader_output": str(opendataloader_out.relative_to(paths.output_dir)),
        "plotextractor_output": str(plot_out.relative_to(paths.output_dir)),
        "validation_errors": validations,
        "validation_passed": all(not errors for errors in validations.values()),
    }


def stage_4_to_8_per_paper(
    paths: PipelinePaths,
    selected_ids: list[str],
    grobid_json_index: dict[str, Path],
    plotextractor_index: dict[str, Path],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    manifest_schema_errors: dict[str, list[str]] = {}
    papers: list[dict[str, Any]] = []
    for arxiv_id in selected_ids:
        pdf_path = find_existing_pdf_path(grobid_json_index, arxiv_id)
        manifest_batch_id = f"m061-s01-{arxiv_id}"
        manifest = {
            "schema_version": "daily-archive.pdf-batch-manifest.v1",
            "batch_id": manifest_batch_id,
            "created_at": utc_now(),
            "generated_by": GENERATED_BY,
            "source_artifacts": [
                "artifacts/m056-bfs-graph/candidate-edges.json",
                "artifacts/m056-bfs-graph/cumulative-corpus.json",
            ],
            "source_uris": [f"https://arxiv.org/pdf/{arxiv_id}", f"https://arxiv.org/e-print/{arxiv_id}"],
            "pdfs": [build_manifest_item(arxiv_id, pdf_path)],
            "safety_defaults": SAFETY_DEFAULTS,
            "sync_execution": True,
            "queue_execution": False,
            "network_host_reference": NETWORK_HOST,
        }
        errors = validate_json(MANIFEST_SCHEMA_PATH, manifest)
        manifest_schema_errors[arxiv_id] = errors
        manifest_path = paths.paper_manifest_dir / f"{arxiv_id}.json"
        write_json(manifest_path, manifest)

        parser_result = write_parser_wrappers(
            paths=paths,
            arxiv_id=arxiv_id,
            manifest_batch_id=manifest_batch_id,
            grobid_json_path=grobid_json_index.get(arxiv_id),
            plotextractor_path=plotextractor_index.get(arxiv_id),
            pdf_path=pdf_path,
        )
        stage_records = [
            {"stage": 1, "name": "acquisition", "status": "complete" if pdf_path else "skipped_external_network_disabled"},
            {"stage": 2, "name": "one_hop_validation", "status": "complete"},
            {"stage": 3, "name": "two_hop_bfs", "status": "complete"},
            {"stage": 4, "name": "real_arxiv_acquisition", "status": "complete" if pdf_path else "skipped_external_network_disabled"},
            {"stage": 5, "name": "parsing", "status": "complete" if parser_result["validation_passed"] else "validation_failed"},
            {"stage": 6, "name": "fdembed", "status": "skipped_fd_service_disabled", "fd_url": FD_URL},
            {"stage": 7, "name": "m3_judge", "status": "complete_reused_m060g_diagnostic"},
            {"stage": 8, "name": "manifest_validation", "status": "complete" if not errors else "validation_failed"},
        ]
        fully_processed_real_paper = bool(pdf_path and parser_result["validation_passed"] and not errors)
        papers.append(
            {
                "arxiv_id": arxiv_id,
                "manifest_path": str(manifest_path.relative_to(paths.output_dir)),
                "pdf_available_locally": pdf_path is not None,
                "fully_processed_real_paper": fully_processed_real_paper,
                "stage_records": stage_records,
                "parser_result": parser_result,
                "manifest_validation_errors": errors,
            }
        )
    report = {
        "stages": [4, 5, 6, 8],
        "name": "per_paper_acquisition_parsing_fdembed_manifest_validation",
        "status": "complete",
        "selected_paper_count": len(selected_ids),
        "locally_available_pdf_count": sum(1 for paper in papers if paper["pdf_available_locally"]),
        "fully_processed_real_paper_count": sum(1 for paper in papers if paper["fully_processed_real_paper"]),
        "manifest_validation_passed_count": sum(1 for errors in manifest_schema_errors.values() if not errors),
        "manifest_validation_success_rate": (
            sum(1 for errors in manifest_schema_errors.values() if not errors) / len(selected_ids) if selected_ids else 0.0
        ),
        "external_network_authorized": SAFETY_DEFAULTS["external_network_authorized"],
        "note": "Live arXiv PDF/TeX downloads are disabled by default; missing papers are recorded as explicit skips.",
        "papers": papers,
        "safety_defaults": SAFETY_DEFAULTS,
    }
    return report, papers


def stage_7_m3_judge(paths: PipelinePaths) -> dict[str, Any]:
    comparison_path = M060G_ROOT / "comparison.json"
    comparison = read_json(comparison_path)
    quality_stats = comparison.get("aggregate", {}).get("model_stats", {}).get("figure-qa-judge-quality", {})
    figure_count = quality_stats.get("passed_count", 0) + quality_stats.get("failed_count", 0)
    success_rate = (quality_stats.get("passed_count", 0) / figure_count) if figure_count else 0.0
    per_figure_files = sorted(M060G_ROOT.glob("per-figure/*.json"))
    report = {
        "stage": 7,
        "name": "m3_judge",
        "status": "complete_reused_m060g_diagnostic",
        "source_artifact": str(comparison_path.relative_to(ROOT)),
        "per_figure_evidence_count": len(per_figure_files),
        "figure_count": figure_count,
        "passed_count": quality_stats.get("passed_count", 0),
        "failed_count": quality_stats.get("failed_count", 0),
        "success_rate": success_rate,
        "latency_avg_ms": quality_stats.get("latency_avg_ms"),
        "model_used": quality_stats.get("model_used"),
        "diagnostic_llm_calls_override": DIAGNOSTIC_M3_OVERRIDE,
        "safety_defaults": SAFETY_DEFAULTS,
    }
    write_json(paths.judgments_dir / "m3-judgments.json", report)
    return report


def validate_layer_payload(schema_path: Path, payload_path: Path) -> list[str]:
    payload = read_json(payload_path)
    return validate_json(schema_path, payload)


def stage_9_graph_manifest(
    paths: PipelinePaths,
    bfs_edges: list[dict[str, Any]],
    new_2hop_ids: list[str],
    m3_report: dict[str, Any],
) -> dict[str, Any]:
    citation_payload = read_json(M056_ROOT / "candidate-edges.json")
    table_payload = read_json(M057_ROOT / "table-similarity" / "edges.json")
    figure_v1_payload = read_json(M057_ROOT / "figure-links" / "edges.json")
    figure_v2_payload = read_json(M058_ROOT / "edges.json")
    layers = [
        {
            "name": "citation_m056_plus_m061_2hop",
            "source_artifacts": [
                "artifacts/m056-bfs-graph/candidate-edges.json",
                "artifacts/m061-2hop/anchor-2605.18747/acquisition/two-hop-bfs.json",
            ],
            "edge_count": len(citation_payload.get("edges", [])) + len(bfs_edges),
            "node_count": len({node.get("arxiv_id") for node in citation_payload.get("nodes", []) if node.get("arxiv_id")} | set(new_2hop_ids)),
        },
        {
            "name": "table_similarity_m057",
            "source_artifacts": ["artifacts/m057-fd-marker/table-similarity/edges.json"],
            "edge_count": len(table_payload.get("edges", [])),
            "node_count": len({edge.get("paper_a") for edge in table_payload.get("edges", [])} | {edge.get("paper_b") for edge in table_payload.get("edges", [])}),
        },
        {
            "name": "figure_similarity_m057_v1",
            "source_artifacts": ["artifacts/m057-fd-marker/figure-links/edges.json"],
            "edge_count": len(figure_v1_payload.get("edges", [])),
            "node_count": len({edge.get("figure_a_id") for edge in figure_v1_payload.get("edges", [])} | {edge.get("figure_b_id") for edge in figure_v1_payload.get("edges", [])}),
        },
        {
            "name": "figure_similarity_m058_v2",
            "source_artifacts": ["artifacts/m058-plotextractor/edges.json"],
            "edge_count": len(figure_v2_payload.get("edges", [])),
            "node_count": len({edge.get("figure_a_id") for edge in figure_v2_payload.get("edges", [])} | {edge.get("figure_b_id") for edge in figure_v2_payload.get("edges", [])}),
        },
        {
            "name": "m3_judge_m060g_diagnostic",
            "source_artifacts": ["artifacts/m060g-judge/comparison.json"],
            "edge_count": m3_report.get("figure_count", 0),
            "node_count": m3_report.get("figure_count", 0),
        },
    ]
    manifest = {
        "schema_version": "m061-2hop.5-layer-graph-manifest.v1",
        "generated_at": utc_now(),
        "generated_by": GENERATED_BY,
        "anchor_arxiv_id": ANCHOR_ARXIV_ID,
        "diagnostic_only": True,
        "sync_execution": True,
        "queue_execution": False,
        "safety_defaults": SAFETY_DEFAULTS,
        "diagnostic_llm_calls_override": DIAGNOSTIC_M3_OVERRIDE,
        "layers": layers,
        "layer_count": len(layers),
        "total_edge_count": sum(layer["edge_count"] for layer in layers),
        "total_node_count_by_layer_sum": sum(layer["node_count"] for layer in layers),
        "validation": {
            "table_layer_errors": validate_layer_payload(TABLE_SCHEMA_PATH, M057_ROOT / "table-similarity" / "edges.json"),
            "figure_v2_layer_errors": validate_layer_payload(PLOTEXTRACTOR_SCHEMA_PATH, M058_ROOT / "edges.json"),
        },
    }
    write_json(paths.graph_dir / "5-layer-graph-manifest.json", manifest)
    return manifest


def build_decision(summary: dict[str, Any]) -> str:
    go_new_papers = summary["two_hop_new_arxiv_id_count"] >= 100
    go_m3 = summary["m3_judge_success_rate"] >= 0.80
    go_throughput = summary["real_paper_throughput_per_min"] >= 1.0
    decision = "GO to S02" if go_new_papers and go_m3 and go_throughput else "STOP before S02"
    rationale = (
        "All quantitative gates passed."
        if decision.startswith("GO")
        else "The 1-anchor pilot did not meet all quantitative gates with safety defaults enabled."
    )
    lines = [
        "# M061 S01 Decision: 1-anchor pilot (2605.18747)",
        "",
        f"Generated: {summary['generated_at']}",
        "",
        "## Decision",
        "",
        f"**{decision}.** {rationale}",
        "",
        "## Gates",
        "",
        "| Gate | Threshold | Observed | Result |",
        "|---|---:|---:|---|",
        f"| New 2-hop papers | >= 100 | {summary['two_hop_new_arxiv_id_count']} | {'pass' if go_new_papers else 'fail'} |",
        f"| M3 judge success rate | >= 80% | {summary['m3_judge_success_rate']:.1%} | {'pass' if go_m3 else 'fail'} |",
        f"| Real-paper throughput | >= 1 paper/min | {summary['real_paper_throughput_per_min']:.2f} | {'pass' if go_throughput else 'fail'} |",
        "",
        "## Safety posture",
        "",
        "External network is disabled, graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default.",
        "Stage 7 uses a diagnostic-only M3 override by reusing M060g evidence; no new live LLM call is made by this S01 pilot.",
        "",
        "## Rationale",
        "",
        f"- 1-hop validation matched M056 with {summary['one_hop_validated_count']} references.",
        f"- 2-hop BFS produced {summary['two_hop_new_arxiv_id_count']} new arXiv IDs from available TEI files.",
        f"- {summary['papers_audited_count']} papers were audited through stage records; {summary['fully_processed_real_paper_count']} were fully processed as real acquired papers.",
        f"- M3 diagnostic evidence covered {summary['m3_judge_figure_count']} figures with {summary['m3_judge_success_rate']:.1%} success.",
        "- Because live arXiv acquisition is disabled by default, this pilot should not be treated as proof that network acquisition capacity is production-ready.",
        "",
    ]
    return "\n".join(lines)


def run_pilot(output_dir: Path = DEFAULT_OUTPUT_DIR, anchor_arxiv_id: str = ANCHOR_ARXIV_ID, max_papers: int = 30) -> dict[str, Any]:
    started = time.perf_counter()
    paths = ensure_dirs(output_dir)
    cumulative_corpus = read_json(M056_ROOT / "cumulative-corpus.json")
    candidate_edges = read_json(M056_ROOT / "candidate-edges.json")
    one_hop_refs = load_one_hop_refs(cumulative_corpus, anchor_arxiv_id)
    tei_index = index_tei_files(M056_ROOT)
    grobid_json_index = index_grobid_json(M056_ROOT)
    plotextractor_index = {path.stem: path for path in (M058_ROOT / "per-pdf").glob("*.json")}

    stage1 = stage_1_anchor_acquisition(cumulative_corpus, anchor_arxiv_id)
    if stage1["status"] != "complete":
        raise RuntimeError("Anchor PDF is missing from M056 corpus")
    stage2 = stage_2_one_hop_validation(cumulative_corpus, candidate_edges, one_hop_refs, anchor_arxiv_id)
    stage3, bfs_edges, new_2hop_ids = stage_3_two_hop_bfs(one_hop_refs, tei_index, anchor_arxiv_id)
    selected_ids = new_2hop_ids[:max_papers]
    stage4_8, papers = stage_4_to_8_per_paper(paths, selected_ids, grobid_json_index, plotextractor_index)
    m3_report = stage_7_m3_judge(paths)
    graph_manifest = stage_9_graph_manifest(paths, bfs_edges, new_2hop_ids, m3_report)

    elapsed_seconds = time.perf_counter() - started
    fully_processed_real_papers = stage4_8["fully_processed_real_paper_count"]
    real_paper_throughput_per_min = fully_processed_real_papers / (elapsed_seconds / 60) if elapsed_seconds else 0.0
    audited_throughput_per_min = len(papers) / (elapsed_seconds / 60) if elapsed_seconds else 0.0

    write_json(paths.acquisition_dir / "anchor-acquisition.json", stage1)
    write_json(paths.acquisition_dir / "one-hop-validation.json", stage2)
    write_json(paths.acquisition_dir / "two-hop-bfs.json", {**stage3, "edges": bfs_edges})
    write_json(paths.acquisition_dir / "selected-2hop-papers.json", {"selected_arxiv_ids": selected_ids, "count": len(selected_ids)})
    write_json(paths.parsing_dir / "per-paper-stage-report.json", stage4_8)

    summary = {
        "schema_version": "m061-2hop.anchor-pilot-summary.v1",
        "generated_at": utc_now(),
        "generated_by": GENERATED_BY,
        "anchor_arxiv_id": anchor_arxiv_id,
        "sync_execution": True,
        "queue_execution": False,
        "network_host_reference": NETWORK_HOST,
        "safety_defaults": SAFETY_DEFAULTS,
        "diagnostic_m3_override": DIAGNOSTIC_M3_OVERRIDE,
        "one_hop_validated_count": stage2["validated_1hop_count"],
        "two_hop_candidate_edge_count": stage3["candidate_2hop_edge_count"],
        "two_hop_new_arxiv_id_count": len(new_2hop_ids),
        "papers_audited_count": len(papers),
        "fully_processed_real_paper_count": fully_processed_real_papers,
        "manifest_validation_success_rate": stage4_8["manifest_validation_success_rate"],
        "m3_judge_figure_count": m3_report["figure_count"],
        "m3_judge_success_rate": m3_report["success_rate"],
        "elapsed_seconds": elapsed_seconds,
        "real_paper_throughput_per_min": real_paper_throughput_per_min,
        "audited_stage_record_throughput_per_min": audited_throughput_per_min,
        "graph_layer_count": graph_manifest["layer_count"],
        "graph_node_count_per_layer": {layer["name"]: layer["node_count"] for layer in graph_manifest["layers"]},
        "graph_edge_count_per_layer": {layer["name"]: layer["edge_count"] for layer in graph_manifest["layers"]},
        "artifacts": {
            "anchor_acquisition": display_path(paths.acquisition_dir / "anchor-acquisition.json"),
            "one_hop_validation": display_path(paths.acquisition_dir / "one-hop-validation.json"),
            "two_hop_bfs": display_path(paths.acquisition_dir / "two-hop-bfs.json"),
            "per_paper_stage_report": display_path(paths.parsing_dir / "per-paper-stage-report.json"),
            "m3_judgments": display_path(paths.judgments_dir / "m3-judgments.json"),
            "graph_manifest": display_path(paths.graph_dir / "5-layer-graph-manifest.json"),
        },
    }
    write_json(paths.output_dir / "pipeline-summary.json", summary)
    decision = build_decision(summary)
    decision_path = output_dir.parent / "s01-decision.md"
    decision_path.parent.mkdir(parents=True, exist_ok=True)
    decision_path.write_text(decision)
    return summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run M061 S01 1-anchor 2-hop BFS pilot.")
    parser.add_argument("--anchor", default=ANCHOR_ARXIV_ID)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--max-papers", type=int, default=30)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    summary = run_pilot(output_dir=args.output_dir, anchor_arxiv_id=args.anchor, max_papers=args.max_papers)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
