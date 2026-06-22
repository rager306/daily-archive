# Formerly: src/arxiv_archive/chunk_baseline_measurement.py

"""Baseline chunk import-readiness measurement.

This module maps the current full-text → PageIndex → SemanticChunk path into the
M005 import-ready chunk contract. It is intentionally conservative: current
section chunks are represented as retrieval-only baseline chunks unless later
M005 slices add graph-grade source spans, route typing, and evidence semantics.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from research_graph.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.papers.indexing import PageIndexNode, build_page_index
from research_graph.papers.semantic_chunks import build_evidence_path, build_semantic_chunks
from research_graph.repair.chunk_import_contract import (
    validate_import_ready_package,
    validation_to_dict,
)

PACKAGE_SCHEMA_VERSION = "m005-import-ready-chunk-package.v1"
CONTRACT_VERSION = "import-ready-chunk-contract.v1"
BASELINE_RUN_SCHEMA = "m005-baseline-chunk-measurement.v1"


@dataclass(frozen=True)
class BaselineMeasurement:
    """One paper's baseline package and validator result."""

    paper_id: str
    package: dict[str, Any]
    validation: dict[str, Any]


@dataclass(frozen=True)
class BaselineRunResult:
    """Aggregate baseline measurement result."""

    measurements: list[BaselineMeasurement]
    summary: dict[str, Any]


def build_baseline_package(paper: dict[str, Any], *, run_id: str) -> dict[str, Any]:
    """Build a conservative import-contract package for one manifest paper."""
    paper_id = str(paper["paper_id"])
    source_path = _select_full_text_path(paper)
    if source_path is None:
        return _missing_artifact_package(paper, run_id=run_id, reason="missing_full_text_artifact")

    ingestion = ingest_full_text(
        FullTextSource(paper_id=paper_id, source_type="markdown", source_path=source_path)
    )
    if ingestion.extraction_mode in {"missing_source", "empty_source", "low_quality_source"}:
        return _missing_artifact_package(
            paper, run_id=run_id, reason=f"full_text_{ingestion.extraction_mode}"
        )

    document = build_page_index(ingestion)
    semantic_chunks = build_semantic_chunks(document)
    evidence_paths = [build_evidence_path(document, chunk) for chunk in semantic_chunks]
    elements = [_element_from_node(node) for node in document.nodes]
    chunks = [_chunk_from_semantic_chunk(chunk) for chunk in semantic_chunks]
    diagnostics = _diagnostics_for_chunks(chunks=chunks, package_state="ok_for_retrieval_only")
    return {
        "schema_version": PACKAGE_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "run_id": run_id,
        "created_at": _now_iso(),
        "paper_id": paper_id,
        "paper": _paper_identity(paper),
        "conversion": {
            "conversion_id": f"conversion:{paper_id}:baseline",
            "converter": str(ingestion.provenance.get("parser", ingestion.extraction_mode)),
            "converter_version": None,
            "source_artifact": _source_artifact_for_paper(paper, source_path=source_path),
            "quality_state": "ok_for_retrieval_only",
            "warnings": [
                _warning(code=warning, object_id=f"conversion:{paper_id}:baseline", severity="warn")
                for warning in [*ingestion.warnings, *document.validation_warnings]
            ],
            "raw_text_included": False,
            "embeddings_included": False,
        },
        "elements": elements,
        "chunks": chunks,
        "annotations": [],
        "evidence_paths": [_evidence_path_to_contract(path) for path in evidence_paths],
        "diagnostics": diagnostics,
    }


def measure_manifest(manifest_path: Path) -> BaselineRunResult:
    """Run baseline measurement for every paper in a gold-corpus manifest."""
    manifest = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
    run_id = f"m005-s02-baseline:{manifest.get('milestone', 'unknown')}"
    measurements: list[BaselineMeasurement] = []
    for paper in manifest.get("papers", []):
        if not isinstance(paper, dict):
            continue
        package = build_baseline_package(paper, run_id=run_id)
        validation = validation_to_dict(validate_import_ready_package(package))
        measurements.append(
            BaselineMeasurement(
                paper_id=str(package["paper_id"]), package=package, validation=validation
            )
        )
    return BaselineRunResult(
        measurements=measurements, summary=_summary_for_measurements(measurements)
    )


def write_baseline_run(result: BaselineRunResult, output_dir: Path) -> None:
    """Write redacted JSON/JSONL baseline measurement artifacts."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    diagnostics_path = output_dir / "baseline-package-diagnostics.jsonl"
    diagnostics_path.write_text(
        "".join(
            json.dumps(_measurement_to_record(measurement), sort_keys=True) + "\n"
            for measurement in result.measurements
        ),
        encoding="utf-8",
    )
    (output_dir / "baseline-summary.json").write_text(
        json.dumps(result.summary, indent=2, sort_keys=True),
        encoding="utf-8",
    )


def write_review_samples(
    result: BaselineRunResult, manifest_path: Path, *, review_path: Path, index_path: Path
) -> None:
    """Write bounded human review samples plus a redacted machine index."""
    manifest = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
    papers_by_id = {
        str(paper["paper_id"]): paper
        for paper in manifest.get("papers", [])
        if isinstance(paper, dict)
    }
    inner_review_ids = [str(paper_id) for paper_id in manifest.get("inner_review_minimum", [])]
    measurements_by_id = {measurement.paper_id: measurement for measurement in result.measurements}
    review_path = Path(review_path)
    index_path = Path(index_path)
    review_path.parent.mkdir(parents=True, exist_ok=True)
    index_path.parent.mkdir(parents=True, exist_ok=True)

    review_lines = [
        "# M005/S02 Baseline Review Samples",
        "",
        "These samples are bounded human-review snippets. Machine JSON/JSONL artifacts remain redacted and must not include raw chunk text.",
        "",
    ]
    index_records: list[dict[str, Any]] = []
    for paper_id in inner_review_ids:
        paper = papers_by_id.get(paper_id, {"paper_id": paper_id})
        measurement = measurements_by_id.get(paper_id)
        sample = _review_sample_for_paper(paper, measurement)
        index_records.append(sample["index_record"])
        review_lines.extend(sample["markdown_lines"])
        review_lines.append("")

    index_payload = {
        "schema_version": "m005-baseline-review-sample-index.v1",
        "paper_count": len(index_records),
        "records": index_records,
        "raw_text_in_machine_logs": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }
    review_path.write_text("\n".join(review_lines).rstrip() + "\n", encoding="utf-8")
    index_path.write_text(json.dumps(index_payload, indent=2, sort_keys=True), encoding="utf-8")


def _measurement_to_record(measurement: BaselineMeasurement) -> dict[str, Any]:
    package = measurement.package
    return {
        "schema_version": "m005-baseline-package-diagnostic.v1",
        "paper_id": measurement.paper_id,
        "package_state": package["diagnostics"]["package_state"],
        "valid_package": measurement.validation["valid_package"],
        "passed": measurement.validation["passed"],
        "has_import_eligible_chunks": measurement.validation["has_import_eligible_chunks"],
        "import_ready": measurement.validation["import_ready"],
        "import_eligible_chunk_count": measurement.validation["import_eligible_chunk_count"],
        "refused_chunk_count": measurement.validation["refused_chunk_count"],
        "refusal_counts": _merged_refusal_counts(
            package["diagnostics"].get("refusal_counts", {}),
            measurement.validation["refusal_counts"],
        ),
        "counts_by_state": package["diagnostics"]["counts_by_state"],
        "counts_by_route": package["diagnostics"]["counts_by_route"],
        "counts_by_chunk_type": package["diagnostics"]["counts_by_chunk_type"],
        "source_span_coverage": package["diagnostics"]["source_span_coverage"],
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _summary_for_measurements(measurements: list[BaselineMeasurement]) -> dict[str, Any]:
    refusal_counts: dict[str, int] = {}
    counts_by_state: dict[str, int] = {}
    counts_by_route: dict[str, int] = {}
    counts_by_chunk_type: dict[str, int] = {}
    for measurement in measurements:
        package_refusals = measurement.package["diagnostics"].get("refusal_counts", {})
        for reason, count in _merged_refusal_counts(
            package_refusals, measurement.validation["refusal_counts"]
        ).items():
            refusal_counts[reason] = refusal_counts.get(reason, 0) + int(count)
        _merge_counts(
            counts_by_state, measurement.package["diagnostics"].get("counts_by_state", {})
        )
        _merge_counts(
            counts_by_route, measurement.package["diagnostics"].get("counts_by_route", {})
        )
        _merge_counts(
            counts_by_chunk_type, measurement.package["diagnostics"].get("counts_by_chunk_type", {})
        )
    return {
        "schema_version": BASELINE_RUN_SCHEMA,
        "paper_count": len(measurements),
        "valid_package_count": sum(1 for item in measurements if item.validation["valid_package"]),
        "import_ready_count": sum(1 for item in measurements if item.validation["import_ready"]),
        "import_eligible_chunk_count": sum(
            int(item.validation["import_eligible_chunk_count"]) for item in measurements
        ),
        "refused_chunk_count": sum(
            int(item.validation["refused_chunk_count"]) for item in measurements
        ),
        "refusal_counts": dict(sorted(refusal_counts.items())),
        "counts_by_state": dict(sorted(counts_by_state.items())),
        "counts_by_route": dict(sorted(counts_by_route.items())),
        "counts_by_chunk_type": dict(sorted(counts_by_chunk_type.items())),
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "claims": [
            "baseline_measurement_only",
            "current_chunks_are_not_claimed_import_ready",
            "missing_artifacts_are_reported_as_blockers",
        ],
    }


def _review_sample_for_paper(
    paper: dict[str, Any], measurement: BaselineMeasurement | None
) -> dict[str, Any]:
    paper_id = str(paper["paper_id"])
    title = str(paper.get("title") or "Untitled")
    hard_case_tags = (
        [str(tag) for tag in paper.get("hard_case_tags", [])]
        if isinstance(paper.get("hard_case_tags"), list)
        else []
    )
    markdown_lines = [
        f"## {paper_id} — {title}",
        "",
        f"Hard-case tags: {', '.join(hard_case_tags) or 'none'}",
        "",
    ]
    if measurement is None:
        markdown_lines.extend(["Status: blocked — no baseline measurement record.", ""])
        return {
            "markdown_lines": markdown_lines,
            "index_record": _review_index_record(
                paper_id=paper_id,
                status="blocked",
                sample_count=0,
                blocker_reason="missing_measurement",
            ),
        }

    package = measurement.package
    chunks = package.get("chunks", [])
    markdown_lines.extend(
        [
            f"Package state: `{package['diagnostics']['package_state']}`",
            f"Import ready: `{measurement.validation['import_ready']}`",
            f"Refused chunks: `{measurement.validation['refused_chunk_count']}`",
            "",
        ]
    )
    if not chunks:
        refusal_counts = package["diagnostics"].get("refusal_counts", {})
        blocker_reason = next(iter(refusal_counts), "no_chunks_available")
        markdown_lines.extend([f"Status: blocked — `{blocker_reason}`.", ""])
        return {
            "markdown_lines": markdown_lines,
            "index_record": _review_index_record(
                paper_id=paper_id, status="blocked", sample_count=0, blocker_reason=blocker_reason
            ),
        }

    source_path = _select_full_text_path(paper)
    chunk_text_by_id = (
        _sample_chunk_text_by_id(paper, max_samples=2) if source_path is not None else {}
    )
    markdown_lines.append("Sampled baseline chunks:")
    markdown_lines.append("")
    sample_count = 0
    for chunk in chunks[:2]:
        chunk_id = str(chunk["chunk_id"])
        snippet = chunk_text_by_id.get(
            chunk_id, "[snippet unavailable from current source artifact]"
        )
        markdown_lines.extend(
            [
                f"- Chunk: `{chunk_id}`",
                f"  - Route/state/type: `{chunk['route']}` / `{chunk['state']}` / `{chunk['chunk_type']}`",
                f"  - Source artifact: `{chunk['source_artifact']}`",
                "  - Bounded snippet:",
                f"    > {snippet}",
                "",
            ]
        )
        sample_count += 1
    return {
        "markdown_lines": markdown_lines,
        "index_record": _review_index_record(
            paper_id=paper_id, status="sampled", sample_count=sample_count, blocker_reason=None
        ),
    }


def _sample_chunk_text_by_id(paper: dict[str, Any], *, max_samples: int) -> dict[str, str]:
    paper_id = str(paper["paper_id"])
    source_path = _select_full_text_path(paper)
    if source_path is None:
        return {}
    ingestion = ingest_full_text(
        FullTextSource(paper_id=paper_id, source_type="markdown", source_path=source_path)
    )
    if ingestion.extraction_mode in {"missing_source", "empty_source", "low_quality_source"}:
        return {}
    document = build_page_index(ingestion)
    chunks = build_semantic_chunks(document)
    return {chunk.id: _bounded_snippet(chunk.text) for chunk in chunks[:max_samples]}


def _bounded_snippet(text: str, *, limit: int = 240) -> str:
    normalized = " ".join(text.split())
    if len(normalized) <= limit:
        return normalized
    return f"{normalized[:limit].rstrip()}…"


def _review_index_record(
    *, paper_id: str, status: str, sample_count: int, blocker_reason: str | None
) -> dict[str, Any]:
    return {
        "paper_id": paper_id,
        "status": status,
        "sample_count": sample_count,
        "blocker_reason": blocker_reason,
        "raw_text_in_machine_logs": False,
        "embeddings_included": False,
    }


def _missing_artifact_package(paper: dict[str, Any], *, run_id: str, reason: str) -> dict[str, Any]:
    paper_id = str(paper["paper_id"])
    diagnostics = _diagnostics_for_chunks(chunks=[], package_state="reject")
    diagnostics["refusal_counts"] = {reason: 1}
    return {
        "schema_version": PACKAGE_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "run_id": run_id,
        "created_at": _now_iso(),
        "paper_id": paper_id,
        "paper": _paper_identity(paper),
        "conversion": {
            "conversion_id": f"conversion:{paper_id}:missing",
            "converter": "missing_artifact",
            "converter_version": None,
            "source_artifact": _source_artifact_for_paper(paper, source_path=None),
            "quality_state": "reject",
            "warnings": [
                _warning(
                    code=reason, object_id=f"conversion:{paper_id}:missing", severity="blocker"
                )
            ],
            "raw_text_included": False,
            "embeddings_included": False,
        },
        "elements": [],
        "chunks": [],
        "annotations": [],
        "evidence_paths": [],
        "diagnostics": diagnostics,
    }


def _paper_identity(paper: dict[str, Any]) -> dict[str, Any]:
    paper_id = str(paper["paper_id"])
    return {
        "paper_id": paper_id,
        "title": paper.get("title"),
        "categories": list(paper.get("categories", []))
        if isinstance(paper.get("categories"), list)
        else [],
        "source_artifacts": list(paper.get("source_artifacts", []))
        or [f"normalized_markdown:{paper_id}"],
    }


def _element_from_node(node: PageIndexNode) -> dict[str, Any]:
    return {
        "element_id": node.id,
        "paper_id": node.paper_id,
        "element_type": "section_heading" if node.text.strip() == "" else "paragraph",
        "parent_element_id": node.parent_id,
        "section_path": list(node.path),
        "order_index": node.order,
        "source_span": None,
        "quality_state": "ok_for_retrieval_only",
        "warnings": [],
    }


def _chunk_from_semantic_chunk(chunk: Any) -> dict[str, Any]:
    return {
        "chunk_id": chunk.id,
        "paper_id": chunk.paper_id,
        "parent_chunk_id": None,
        "parent_element_ids": [chunk.page_index_node_id],
        "section_path": list(chunk.page_index_path),
        "chunk_type": "retrieval_context",
        "route": "retrieval_only",
        "state": "ok_for_retrieval_only",
        "allowed_uses": ["retrieval_diagnostics", "review_only"],
        "excluded_uses": [
            "trusted_kg_import",
            "claim_extraction",
            "entity_extraction",
            "relation_extraction",
        ],
        "order_index": chunk.order,
        "source_span": {
            "coordinate_space": "semantic_chunk_text",
            "char_start": chunk.char_start,
            "char_end": chunk.char_end,
            "page_start": None,
            "page_end": None,
        },
        "source_artifact": str(
            chunk.provenance.get("source_path", f"normalized_markdown:{chunk.paper_id}")
        ),
        "evidence_path_id": None,
        "quality_warnings": [
            _warning(
                code="baseline_chunk_retrieval_only_no_graph_grade_source_span",
                object_id=chunk.id,
                severity="warn",
                route="retrieval_only",
                blocks_import=False,
            )
        ],
        "redaction": {
            "raw_text_included": False,
            "chunk_text_included": False,
            "embeddings_included": False,
            "vectors_included": False,
            "secrets_included": False,
        },
    }


def _evidence_path_to_contract(path: Any) -> dict[str, Any]:
    return {
        "evidence_path_id": f"evidence:{path.semantic_chunk_id}",
        "paper_id": path.paper_id,
        "chunk_id": path.semantic_chunk_id,
        "source_element_ids": [path.page_index_node_id],
        "source_artifact": str(
            path.provenance.get("source_path", f"normalized_markdown:{path.paper_id}")
        ),
        "source_span": {
            "coordinate_space": "semantic_chunk_text",
            "char_start": 0,
            "char_end": 1,
            "page_start": None,
            "page_end": None,
        },
        "provenance_chain": [path.page_index_node_id, path.semantic_chunk_id],
    }


def _diagnostics_for_chunks(*, chunks: list[dict[str, Any]], package_state: str) -> dict[str, Any]:
    counts_by_state = _counts(str(chunk["state"]) for chunk in chunks)
    counts_by_route = _counts(str(chunk["route"]) for chunk in chunks)
    counts_by_chunk_type = _counts(str(chunk["chunk_type"]) for chunk in chunks)
    return {
        "package_state": package_state,
        "valid_package": True,
        "import_eligible_chunk_count": 0,
        "refused_chunk_count": len(chunks),
        "counts_by_state": counts_by_state,
        "counts_by_route": counts_by_route,
        "counts_by_chunk_type": counts_by_chunk_type,
        "refusal_counts": {"baseline_retrieval_only_not_import_ready": len(chunks)}
        if chunks
        else {},
        "source_span_coverage": 1.0 if chunks else 0.0,
        "parent_reference_resolution_rate": 1.0 if chunks else 0.0,
        "evidence_path_resolution_rate": 0.0,
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _warning(
    *,
    code: str,
    object_id: str,
    severity: str,
    route: str | None = None,
    blocks_import: bool = True,
) -> dict[str, Any]:
    return {
        "code": code,
        "severity": severity,
        "message": code.replace("_", " "),
        "object_id": object_id,
        "route": route,
        "blocks_import": blocks_import,
    }


def _select_full_text_path(paper: dict[str, Any]) -> Path | None:
    for raw_path in paper.get("required_paths", []):
        path = Path(str(raw_path))
        if path.name == "full_text.md" and path.exists():
            return path
    paper_id = str(paper["paper_id"])
    fallback = Path("/root/.research/papers") / paper_id / "full_text.md"
    return fallback if fallback.exists() else None


def _source_artifact_for_paper(paper: dict[str, Any], *, source_path: Path | None) -> str:
    if source_path is not None:
        return str(source_path)
    artifacts = paper.get("source_artifacts")
    if isinstance(artifacts, list) and artifacts:
        return str(artifacts[0])
    return f"normalized_markdown:{paper['paper_id']}"


def _merged_refusal_counts(*refusal_maps: Any) -> dict[str, int]:
    merged: dict[str, int] = {}
    for refusal_map in refusal_maps:
        if not isinstance(refusal_map, dict):
            continue
        for reason, count in refusal_map.items():
            merged[str(reason)] = merged.get(str(reason), 0) + int(count)
    return dict(sorted(merged.items()))


def _merge_counts(target: dict[str, int], source: Any) -> None:
    if not isinstance(source, dict):
        return
    for key, value in source.items():
        target[str(key)] = target.get(str(key), 0) + int(value)


def _counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        counts[value] = counts.get(value, 0) + 1
    return dict(sorted(counts.items()))


def _now_iso() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Measure current chunks against the M005 import-ready contract."
    )
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--review-output", required=False, type=Path)
    parser.add_argument("--review-index", required=False, type=Path)
    args = parser.parse_args(argv)
    result = measure_manifest(args.manifest)
    write_baseline_run(result, args.output_dir)
    if args.review_output is not None or args.review_index is not None:
        if args.review_output is None or args.review_index is None:
            parser.error("--review-output and --review-index must be provided together")
        write_review_samples(
            result, args.manifest, review_path=args.review_output, index_path=args.review_index
        )
    sys.stdout.write(json.dumps(result.summary, indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
