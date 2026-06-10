#!/usr/bin/env python3
"""Compare M054 GROBID header-only packets with M055deep fulltext packets."""

from __future__ import annotations

import argparse
import datetime as dt
import json
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055deep-parser-benchmark.header-vs-fulltext-delta.v1"
DEFAULT_HEADER_DIR = Path("artifacts/m055-parser-benchmark/grobid-only/per-pdf")
DEFAULT_FULLTEXT_DIR = Path("artifacts/m055deep-parser-benchmark/grobid-fulltext/per-pdf")
DEFAULT_OUTPUT_PATH = Path("artifacts/m055deep-parser-benchmark/header-vs-fulltext-delta.json")
DELTA_FIELDS = ("body", "ref", "bibl", "equation", "figure")


def _utc_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_name(f".{path.name}.tmp")
    tmp_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp_path.replace(path)


def _metric(packet: dict[str, Any], metric_name: str) -> int:
    return int(packet.get(metric_name) or 0)


def _packet_map(packet_dir: Path) -> dict[str, dict[str, Any]]:
    packets: dict[str, dict[str, Any]] = {}
    for path in sorted(packet_dir.glob("*.json")):
        packet = _read_json(path)
        arxiv_id = str(packet.get("arxiv_id") or path.stem)
        packets[arxiv_id] = packet
    return packets


def compare_header_vs_fulltext(
    header_dir: Path = DEFAULT_HEADER_DIR,
    fulltext_dir: Path = DEFAULT_FULLTEXT_DIR,
    output_path: Path = DEFAULT_OUTPUT_PATH,
) -> dict[str, Any]:
    header_packets = _packet_map(header_dir)
    fulltext_packets = _packet_map(fulltext_dir)
    arxiv_ids = sorted(set(header_packets) | set(fulltext_packets))
    per_pdf: list[dict[str, Any]] = []

    for arxiv_id in arxiv_ids:
        header = header_packets.get(arxiv_id, {})
        fulltext = fulltext_packets.get(arxiv_id, {})
        row = {
            "arxiv_id": arxiv_id,
            "header_status": header.get("status"),
            "fulltext_status": fulltext.get("status"),
            "header_body_element_count": _metric(header, "body_element_count"),
            "fulltext_body_element_count": _metric(fulltext, "body_element_count"),
            "body_delta": _metric(fulltext, "body_element_count") - _metric(header, "body_element_count"),
            "header_ref_count": _metric(header, "ref_count"),
            "fulltext_ref_count": _metric(fulltext, "ref_count"),
            "ref_delta": _metric(fulltext, "ref_count") - _metric(header, "ref_count"),
            "header_bibl_count": _metric(header, "bibl_count"),
            "fulltext_bibl_count": _metric(fulltext, "bibl_count"),
            "bibl_delta": _metric(fulltext, "bibl_count") - _metric(header, "bibl_count"),
            "header_equation_count": _metric(header, "equation_count"),
            "fulltext_equation_count": _metric(fulltext, "equation_count"),
            "equation_delta": _metric(fulltext, "equation_count") - _metric(header, "equation_count"),
            "header_figure_count": _metric(header, "figure_count"),
            "fulltext_figure_count": _metric(fulltext, "figure_count"),
            "figure_delta": _metric(fulltext, "figure_count") - _metric(header, "figure_count"),
        }
        per_pdf.append(row)

    aggregate = {
        "total_pdfs": len(per_pdf),
        "fulltext_body_positive_count": sum(1 for row in per_pdf if row["fulltext_body_element_count"] > 0),
        "body_delta_total": sum(row["body_delta"] for row in per_pdf),
        "ref_delta_total": sum(row["ref_delta"] for row in per_pdf),
        "bibl_delta_total": sum(row["bibl_delta"] for row in per_pdf),
        "equation_delta_total": sum(row["equation_delta"] for row in per_pdf),
        "figure_delta_total": sum(row["figure_delta"] for row in per_pdf),
    }
    payload = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "header_dir": str(header_dir),
        "fulltext_dir": str(fulltext_dir),
        "aggregate": aggregate,
        "per_pdf": per_pdf,
    }
    _atomic_write_json(output_path, payload)
    return payload


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--header-dir", type=Path, default=DEFAULT_HEADER_DIR)
    parser.add_argument("--fulltext-dir", type=Path, default=DEFAULT_FULLTEXT_DIR)
    parser.add_argument("--output-path", type=Path, default=DEFAULT_OUTPUT_PATH)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    payload = compare_header_vs_fulltext(args.header_dir, args.fulltext_dir, args.output_path)
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
