#!/usr/bin/env python3
"""Audit M053 GROBID pilot packets.

Reads the M053 summary plus per-PDF diagnostic packets and emits a
fail-closed markdown audit surface. This script does not call GROBID, does
not write graph data, and keeps the five safety defaults false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMMARY_PATH = REPO_ROOT / "artifacts" / "m053-grobid-pilot" / "summary.json"
DEFAULT_PER_PDF_DIR = REPO_ROOT / "artifacts" / "m053-grobid-pilot"
DEFAULT_AUDIT_PATH = REPO_ROOT / "artifacts" / "m053-grobid-pilot" / "audit.md"

SCHEMA_VERSION = "m053-grobid-pilot-audit.v1"
STATUS_ORDER = (
    "success",
    "low_quality_source",
    "blocked",
    "grobid_unavailable",
    "network_error",
    "timeout",
)
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


def utc_now() -> str:
    return dt.datetime.now(tz=dt.UTC).isoformat()


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _relative(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def _summary_packet_path(summary_packet: dict[str, Any], per_pdf_dir: Path) -> Path:
    raw_path = summary_packet.get("packet_path")
    if raw_path:
        packet_path = Path(raw_path)
        if not packet_path.is_absolute():
            packet_path = REPO_ROOT / packet_path
        if packet_path.exists():
            return packet_path
    paper_id = str(summary_packet.get("paper_id") or summary_packet.get("arxiv_id"))
    return per_pdf_dir / f"{paper_id}.json"


def load_packets(summary: dict[str, Any], per_pdf_dir: Path) -> list[dict[str, Any]]:
    packets: list[dict[str, Any]] = []
    for summary_packet in summary.get("packets", []):
        packet_path = _summary_packet_path(summary_packet, per_pdf_dir)
        packet = _read_json(packet_path)
        packet.setdefault("packet_path", _relative(packet_path))
        packets.append(packet)
    return sorted(
        packets, key=lambda packet: str(packet.get("paper_id") or packet.get("arxiv_id") or "")
    )


def count_statuses(packets: list[dict[str, Any]]) -> dict[str, int]:
    counts = dict.fromkeys(STATUS_ORDER, 0)
    for packet in packets:
        status = str(packet.get("status", "unknown"))
        counts[status] = counts.get(status, 0) + 1
    return counts


def _attempt_count(packet: dict[str, Any]) -> int:
    attempts = packet.get("attempts")
    if isinstance(attempts, list):
        return len(attempts)
    if isinstance(attempts, int):
        return attempts
    return 0


def _error_text(packet: dict[str, Any]) -> str:
    for key in ("error", "error_reason", "note"):
        value = packet.get(key)
        if value:
            return str(value).replace("\n", " ")
    return "—"


def per_pdf_table(packets: list[dict[str, Any]]) -> str:
    lines = [
        "| arxiv_id | status | tei_size_bytes | ref_count | body_element_count | m022_repair_candidate | attempts | error |",
        "| --- | --- | ---: | ---: | ---: | --- | ---: | --- |",
    ]
    for packet in packets:
        paper_id = packet.get("paper_id") or packet.get("arxiv_id") or "unknown"
        lines.append(
            "| "
            f"`{paper_id}` | "
            f"{packet.get('status', 'unknown')} | "
            f"{int(packet.get('tei_size_bytes') or 0)} | "
            f"{int(packet.get('ref_count') or 0)} | "
            f"{int(packet.get('body_element_count') or 0)} | "
            f"{str(bool(packet.get('m022_repair_candidate', False))).lower()} | "
            f"{_attempt_count(packet)} | "
            f"{_error_text(packet)} |"
        )
    return "\n".join(lines)


def status_counts_block(counts: dict[str, int]) -> str:
    lines = ["| status | count |", "| --- | ---: |"]
    for status in STATUS_ORDER:
        lines.append(f"| {status} | {counts.get(status, 0)} |")
    extra_statuses = sorted(set(counts) - set(STATUS_ORDER))
    for status in extra_statuses:
        lines.append(f"| {status} | {counts[status]} |")
    return "\n".join(lines)


def safety_block() -> str:
    return "\n".join(["```json", json.dumps(SAFETY_DEFAULTS, indent=2, sort_keys=True), "```"])


def m022_candidates_block(packets: list[dict[str, Any]]) -> str:
    candidates = [packet for packet in packets if bool(packet.get("m022_repair_candidate", False))]
    if not candidates:
        return "No M022 chunk repair candidates were emitted by M053."
    lines = [
        "The following PDFs are M022 chunk repair candidates because the GROBID pilot did not produce a usable TEI body/reference surface:",
        "",
    ]
    for packet in candidates:
        paper_id = packet.get("paper_id") or packet.get("arxiv_id") or "unknown"
        lines.append(f"- `{paper_id}` — status `{packet.get('status', 'unknown')}`")
    return "\n".join(lines)


def build_audit(
    summary: dict[str, Any], packets: list[dict[str, Any]], *, summary_path: Path, per_pdf_dir: Path
) -> str:
    counts = count_statuses(packets)
    parts: list[str] = []
    parts.append("# M053 GROBID Pilot Audit")
    parts.append("")
    parts.append(f"**Schema version:** `{SCHEMA_VERSION}`")
    parts.append(f"**Generated at:** {utc_now()}")
    parts.append("")
    parts.append("## Inputs")
    parts.append("")
    parts.append(f"- Summary: `{_relative(summary_path)}`")
    parts.append(f"- Per-PDF directory: `{_relative(per_pdf_dir)}`")
    parts.append(f"- Total PDFs: {len(packets)}")
    parts.append(f"- Source schema version: `{summary.get('schema_version', 'unknown')}`")
    parts.append("")
    parts.append("## Status counts")
    parts.append("")
    parts.append(status_counts_block(counts))
    parts.append("")
    parts.append("## Per-PDF packets")
    parts.append("")
    parts.append(per_pdf_table(packets))
    parts.append("")
    parts.append("## Safety defaults")
    parts.append("")
    parts.append(
        "Production import is not authorized by this audit; all safety defaults remain false."
    )
    parts.append("")
    parts.append(safety_block())
    parts.append("")
    parts.append("## M022 chunk repair candidates")
    parts.append("")
    parts.append(m022_candidates_block(packets))
    parts.append("")
    return "\n".join(parts)


def write_audit(summary_path: Path, per_pdf_dir: Path, output_path: Path) -> str:
    summary = _read_json(summary_path)
    packets = load_packets(summary, per_pdf_dir)
    audit = build_audit(summary, packets, summary_path=summary_path, per_pdf_dir=per_pdf_dir)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(audit, encoding="utf-8")
    return audit


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audit M053 GROBID pilot packets")
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY_PATH)
    parser.add_argument("--per-pdf-dir", type=Path, default=None)
    parser.add_argument("--output", type=Path, default=DEFAULT_AUDIT_PATH)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    summary_path = args.summary
    per_pdf_dir = args.per_pdf_dir or summary_path.parent
    if not summary_path.exists():
        print(f"M053 summary not found at {summary_path}")
        return 1
    if not per_pdf_dir.exists():
        print(f"M053 per-PDF directory not found at {per_pdf_dir}")
        return 1
    write_audit(summary_path, per_pdf_dir, args.output)
    print(f"M053 GROBID pilot audit written: {_relative(args.output)}")
    print(f"Safety defaults: {json.dumps(SAFETY_DEFAULTS, sort_keys=True)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
