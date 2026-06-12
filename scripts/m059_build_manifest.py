#!/usr/bin/env python3
"""Build M059 retroactive PDF batch manifests for M054-M058 artifacts.

The generated manifests are diagnostic-only contracts. They do not authorize
network calls, graph writes, production import, fact promotion, or LLM calls.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = "daily-archive.pdf-batch-manifest.v1"
GENERATED_BY = "scripts/m059_build_manifest.py"

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}

MANIFEST_SCHEMA = "schemas/daily-archive.pdf-batch-manifest.v1.json"
GROBID_SCHEMA = "schemas/grobid-tei.v1.json"
OPENDATALOADER_SCHEMA = "schemas/opendataloader-pdf.v1.json"
TABLE_SCHEMA = "schemas/m057-fd-table-similarity.v1.json"
PLOT_SCHEMA = "schemas/m058-plotextractor-figure-caption.v1.json"


def utc_now() -> str:
    """Return an ISO-8601 timestamp for manifest creation."""
    return dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()


def read_json(path: Path) -> Any:
    """Read a JSON file relative to the repository root when needed."""
    actual = path if path.is_absolute() else ROOT / path
    return json.loads(actual.read_text())


def sha256_file(path: Path) -> str:
    """Compute a SHA-256 digest for a local file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def rel(path: Path) -> str:
    """Return a stable POSIX repository-relative path."""
    actual = path if path.is_absolute() else ROOT / path
    return actual.relative_to(ROOT).as_posix()


def arxiv_pdf_uri(arxiv_id: str) -> str:
    """Return the canonical arXiv PDF URI for an identifier."""
    return f"https://arxiv.org/pdf/{arxiv_id}"


def find_pdf(arxiv_id: str) -> Path:
    """Find a local PDF by arXiv identifier under the article catalog."""
    matches = sorted((ROOT / "data" / "article_catalog" / "article_catalog" / "arxiv").glob(f"**/{arxiv_id}/source/{arxiv_id}.pdf"))
    if not matches:
        raise FileNotFoundError(f"No local PDF found for {arxiv_id}")
    return matches[0]


def pdf_entry(
    *,
    arxiv_id: str,
    path: str | Path,
    source_uri: str | None = None,
    category: str | None = None,
    article_key: str | None = None,
    expected_parsers: list[dict[str, Any]],
    source_artifacts: list[str],
    metadata: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build one manifest PDF item and compute local byte metadata."""
    pdf_path = Path(path)
    actual_path = pdf_path if pdf_path.is_absolute() else ROOT / pdf_path
    if not actual_path.exists():
        raise FileNotFoundError(f"Missing local PDF for {arxiv_id}: {actual_path}")
    item: dict[str, Any] = {
        "arxiv_id": arxiv_id,
        "source_uri": source_uri or arxiv_pdf_uri(arxiv_id),
        "storage_provider": "local",
        "path": rel(actual_path),
        "size_bytes": actual_path.stat().st_size,
        "content_sha256": sha256_file(actual_path),
        "expected_parsers": expected_parsers,
        "source_artifacts": source_artifacts,
    }
    if article_key:
        item["article_key"] = article_key
    if category:
        item["category"] = category
    if metadata:
        item["metadata"] = metadata
    return item


def parser_expectation(
    name: str,
    *,
    version: str,
    mode: str,
    expected_output_schema: str,
    output_path_template: str | None = None,
    batch_output_path: str | None = None,
    required_for_validation: bool = True,
) -> dict[str, Any]:
    """Return a parser expectation embedded in a PDF item."""
    expectation: dict[str, Any] = {
        "name": name,
        "version": version,
        "mode": mode,
        "expected_output_schema": expected_output_schema,
        "required_for_validation": required_for_validation,
        "diagnostic_only": True,
    }
    if output_path_template:
        expectation["output_path_template"] = output_path_template
    if batch_output_path:
        expectation["batch_output_path"] = batch_output_path
    return expectation


def finalize_manifest(
    *,
    batch_id: str,
    source_artifacts: list[str],
    pdfs: list[dict[str, Any]],
    created_at: str,
    output_path: Path,
    notes: list[str] | None = None,
) -> dict[str, Any]:
    """Create, persist, and return a complete manifest object."""
    source_uris = sorted({str(pdf["source_uri"]) for pdf in pdfs})
    parser_names = sorted({parser["name"] for pdf in pdfs for parser in pdf["expected_parsers"]})
    manifest: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "manifest_schema": MANIFEST_SCHEMA,
        "batch_id": batch_id,
        "created_at": created_at,
        "generated_by": GENERATED_BY,
        "source_artifacts": source_artifacts,
        "source_uris": source_uris,
        "ingest_commit": None,
        "safety_defaults": dict(SAFETY_DEFAULTS),
        "pdfs": sorted(pdfs, key=lambda item: item["arxiv_id"]),
        "aggregate": {
            "pdf_count": len(pdfs),
            "total_size_bytes": sum(int(pdf["size_bytes"]) for pdf in pdfs),
            "missing_pdf_count": 0,
            "parser_count": len(parser_names),
            "parser_names": parser_names,
        },
        "notes": notes or [
            "Retroactive manifest generated by M059 S01.",
            "All safety defaults are explicit false; production import is not authorized.",
        ],
    }
    actual_output = output_path if output_path.is_absolute() else ROOT / output_path
    actual_output.parent.mkdir(parents=True, exist_ok=True)
    actual_output.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def build_m054(created_at: str) -> dict[str, Any]:
    target_path = Path("artifacts/m054-pdf-acquisition/target-subset.json")
    log_path = Path("artifacts/m054-pdf-acquisition/acquisition-log.json")
    target = read_json(target_path)
    log = read_json(log_path)
    log_by_id = {entry["article_key"]: entry for entry in log["entries"]}
    pdfs: list[dict[str, Any]] = []
    for record in target["records"]:
        arxiv_id = record["article_key"]
        entry = log_by_id[arxiv_id]
        pdfs.append(
            pdf_entry(
                arxiv_id=arxiv_id,
                article_key=record.get("article_key"),
                category=record.get("category"),
                source_uri=entry.get("url") or record.get("expected_arxiv_url"),
                path=entry["local_path"],
                expected_parsers=[
                    parser_expectation(
                        "grobid",
                        version="m055-retroactive",
                        mode="header",
                        expected_output_schema=GROBID_SCHEMA,
                        output_path_template="artifacts/m055-parser-benchmark/grobid-only/per-pdf/{arxiv_id}.json",
                    ),
                    parser_expectation(
                        "opendataloader",
                        version="m055-retroactive",
                        mode="layout",
                        expected_output_schema=OPENDATALOADER_SCHEMA,
                        output_path_template="artifacts/m055-parser-benchmark/opendataloader-only/per-pdf/{arxiv_id}.json",
                    ),
                ],
                source_artifacts=[target_path.as_posix(), log_path.as_posix()],
                metadata={"target_index": record.get("index"), "acquisition_status": entry.get("status")},
            )
        )
    return finalize_manifest(
        batch_id="m054-pdf-acquisition",
        source_artifacts=[target_path.as_posix(), log_path.as_posix()],
        pdfs=pdfs,
        created_at=created_at,
        output_path=Path("artifacts/m054-pdf-acquisition/manifest.json"),
    )


def build_m055(created_at: str) -> dict[str, Any]:
    manifest_path = Path("artifacts/m055-parser-benchmark/corpus-manifest.json")
    corpus = read_json(manifest_path)
    pdfs = [
        pdf_entry(
            arxiv_id=row["arxiv_id"],
            article_key=row.get("article_key"),
            category=row.get("category"),
            path=row["path"],
            expected_parsers=[
                parser_expectation("grobid", version="m055", mode="header", expected_output_schema=GROBID_SCHEMA, output_path_template="artifacts/m055-parser-benchmark/grobid-only/per-pdf/{arxiv_id}.json"),
                parser_expectation("opendataloader", version="m055", mode="layout", expected_output_schema=OPENDATALOADER_SCHEMA, output_path_template="artifacts/m055-parser-benchmark/opendataloader-only/per-pdf/{arxiv_id}.json"),
                parser_expectation("hybrid-routing", version="m055", mode="header-or-layout", expected_output_schema="schemas/daily-archive.parser-op.v1.json", output_path_template="artifacts/m055-parser-benchmark/hybrid-routing/per-pdf/{arxiv_id}.json", required_for_validation=False),
            ],
            source_artifacts=[manifest_path.as_posix()],
            metadata={"source_schema_version": corpus.get("schema_version"), "target_index": row.get("target_index")},
        )
        for row in corpus["pdfs"]
    ]
    return finalize_manifest(batch_id="m055-parser-benchmark", source_artifacts=[manifest_path.as_posix()], pdfs=pdfs, created_at=created_at, output_path=Path("artifacts/m055-parser-benchmark/manifest.json"))


def build_m055deep(created_at: str) -> dict[str, Any]:
    manifest_path = Path("artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")
    corpus = read_json(manifest_path)
    pdfs = [
        pdf_entry(
            arxiv_id=row["arxiv_id"],
            category=row.get("category"),
            path=row["path"],
            expected_parsers=[
                parser_expectation("grobid", version="m055deep", mode="fulltext", expected_output_schema=GROBID_SCHEMA, output_path_template="artifacts/m055deep-parser-benchmark/grobid-fulltext-20/per-pdf/{arxiv_id}.json"),
                parser_expectation("opendataloader", version="m055deep", mode="layout", expected_output_schema=OPENDATALOADER_SCHEMA, output_path_template="artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf/{arxiv_id}.json"),
                parser_expectation("hybrid-routing", version="m055deep", mode="fulltext-or-layout", expected_output_schema="schemas/daily-archive.parser-op.v1.json", output_path_template="artifacts/m055deep-parser-benchmark/hybrid-routing-20/per-pdf/{arxiv_id}.json", required_for_validation=False),
            ],
            source_artifacts=[manifest_path.as_posix()],
            metadata={"source_schema_version": corpus.get("schema_version"), "source_milestone": row.get("source_milestone")},
        )
        for row in corpus["pdfs"]
    ]
    return finalize_manifest(batch_id="m055deep-parser-benchmark", source_artifacts=[manifest_path.as_posix()], pdfs=pdfs, created_at=created_at, output_path=Path("artifacts/m055deep-parser-benchmark/manifest.json"))


def build_m056(created_at: str) -> dict[str, Any]:
    corpus_path = Path("artifacts/m056-bfs-graph/cumulative-corpus.json")
    edges_path = Path("artifacts/m056-bfs-graph/candidate-edges.json")
    corpus = read_json(corpus_path)
    pdfs = [
        pdf_entry(
            arxiv_id=row["arxiv_id"],
            path=row["path"],
            expected_parsers=[
                parser_expectation("grobid", version="m056", mode="fulltext", expected_output_schema=GROBID_SCHEMA, output_path_template="artifacts/m056-bfs-graph/**/grobid-fulltext/per-pdf/{arxiv_id}.json", required_for_validation=False),
                parser_expectation("opendataloader", version="m056", mode="layout", expected_output_schema=OPENDATALOADER_SCHEMA, output_path_template="artifacts/m056-bfs-graph/**/opendataloader/per-pdf/{arxiv_id}.json", required_for_validation=False),
            ],
            source_artifacts=[corpus_path.as_posix(), edges_path.as_posix()],
            metadata={"source_schema_version": corpus.get("schema_version"), "source_milestone": row.get("source_milestone")},
        )
        for row in corpus["pdfs"]
    ]
    return finalize_manifest(batch_id="m056-bfs-graph", source_artifacts=[corpus_path.as_posix(), edges_path.as_posix()], pdfs=pdfs, created_at=created_at, output_path=Path("artifacts/m056-bfs-graph/manifest.json"))


def build_m057(created_at: str) -> dict[str, Any]:
    corpus_path = Path("artifacts/m056-bfs-graph/cumulative-corpus.json")
    table_summary = Path("artifacts/m057-fd-marker/table-similarity/summary.json")
    figure_summary = Path("artifacts/m057-fd-marker/figure-links/summary.json")
    corpus = read_json(corpus_path)
    pdfs = [
        pdf_entry(
            arxiv_id=row["arxiv_id"],
            path=row["path"],
            expected_parsers=[
                parser_expectation("fd-table-similarity", version="m057", mode="batch-summary", expected_output_schema=TABLE_SCHEMA, batch_output_path="artifacts/m057-fd-marker/table-similarity/edges.json", required_for_validation=False),
                parser_expectation("fd-figure-links", version="m057", mode="batch-summary", expected_output_schema="schemas/daily-archive.parser-op.v1.json", batch_output_path="artifacts/m057-fd-marker/figure-links/edges.json", required_for_validation=False),
            ],
            source_artifacts=[corpus_path.as_posix(), table_summary.as_posix(), figure_summary.as_posix()],
            metadata={"source_schema_version": corpus.get("schema_version"), "source_milestone": row.get("source_milestone")},
        )
        for row in corpus["pdfs"]
    ]
    return finalize_manifest(batch_id="m057-fd-marker", source_artifacts=[corpus_path.as_posix(), table_summary.as_posix(), figure_summary.as_posix()], pdfs=pdfs, created_at=created_at, output_path=Path("artifacts/m057-fd-marker/manifest.json"))


def build_m058(created_at: str) -> dict[str, Any]:
    summary_path = Path("artifacts/m058-plotextractor/summary.json")
    summary = read_json(summary_path)
    pdfs = []
    for row in summary["per_pdf"]:
        arxiv_id = row["arxiv_id"]
        pdfs.append(
            pdf_entry(
                arxiv_id=arxiv_id,
                category=row.get("category"),
                path=find_pdf(arxiv_id),
                expected_parsers=[
                    parser_expectation("plotextractor", version="m058", mode="figure-caption", expected_output_schema=PLOT_SCHEMA, output_path_template="artifacts/m058-plotextractor/per-pdf/{arxiv_id}.json"),
                    parser_expectation("plotextractor-summary", version="m058", mode="batch-summary", expected_output_schema=PLOT_SCHEMA, batch_output_path=summary_path.as_posix(), required_for_validation=False),
                ],
                source_artifacts=[summary_path.as_posix()],
                metadata={"tex_status": row.get("tex_status"), "figure_count": row.get("figure_count"), "caption_count": row.get("caption_count")},
            )
        )
    return finalize_manifest(batch_id="m058-plotextractor", source_artifacts=[summary_path.as_posix()], pdfs=pdfs, created_at=created_at, output_path=Path("artifacts/m058-plotextractor/manifest.json"))


def build_all() -> dict[str, dict[str, Any]]:
    """Build every retroactive manifest and return them by batch id."""
    created_at = utc_now()
    builders = [build_m054, build_m055, build_m055deep, build_m056, build_m057, build_m058]
    return {manifest["batch_id"]: manifest for manifest in (builder(created_at) for builder in builders)}


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build M059 retroactive PDF batch manifests.")
    parser.add_argument("--batch", choices=["m054", "m055", "m055deep", "m056", "m057", "m058", "all"], default="all")
    args = parser.parse_args(list(argv) if argv is not None else None)
    created_at = utc_now()
    selected = {
        "m054": [build_m054],
        "m055": [build_m055],
        "m055deep": [build_m055deep],
        "m056": [build_m056],
        "m057": [build_m057],
        "m058": [build_m058],
        "all": [build_m054, build_m055, build_m055deep, build_m056, build_m057, build_m058],
    }[args.batch]
    for builder in selected:
        manifest = builder(created_at)
        print(f"{manifest['batch_id']}: {manifest['aggregate']['pdf_count']} PDFs -> artifacts/{manifest['batch_id']}/manifest.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
