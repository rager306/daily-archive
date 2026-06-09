#!/usr/bin/env python3
"""Run a bounded live GROBID candidate probe for the M043 target subset.

Only local PDFs are submitted. The output stores candidate-only summaries
(counts, hashes, status, blockers), never raw TEI or paper text.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from collections.abc import Callable
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGET = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "target-subset.json"
DEFAULT_SOURCE = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "source-readiness.json"
DEFAULT_RUNTIME_UPDATE = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "grobid-runtime-update.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m044-grobid-architecture-guardrail"
DEFAULT_GUARDRAIL = ROOT / "scripts" / "verify_m044_sidecar_architecture_guardrail.py"
FORBIDDEN_KEYS = {"raw_text", "full_text", "tei_xml", "markdown_body", "embedding", "vector", "prompt", "completion"}
FALSE_KEYS = ("graph_write_allowed", "promotion_allowed", "production_import_attempted", "import_eligible")
GrobidSubmitter = Callable[[Path, str, int], bytes]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def run_guardrail(command: Path = DEFAULT_GUARDRAIL) -> None:
    result = subprocess.run([sys.executable, str(command)], cwd=ROOT, text=True, capture_output=True, timeout=30)
    if result.returncode != 0:
        raise RuntimeError(f"architecture guardrail failed: {result.stderr or result.stdout}")


def submit_pdf_to_grobid(pdf_path: Path, service_url: str, timeout: int) -> bytes:
    boundary = "----m044grobidboundary"
    data = pdf_path.read_bytes()
    body = b"\r\n".join(
        [
            f"--{boundary}".encode(),
            b'Content-Disposition: form-data; name="input"; filename="input.pdf"',
            b"Content-Type: application/pdf",
            b"",
            data,
            f"--{boundary}--".encode(),
            b"",
        ]
    )
    request = urllib.request.Request(
        f"{service_url.rstrip('/')}/api/processFulltextDocument",
        data=body,
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}", "Accept": "application/xml"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read()


def summarize_tei(tei_bytes: bytes) -> dict[str, Any]:
    root = ET.fromstring(tei_bytes)
    names: list[str] = []
    for elem in root.iter():
        tag = elem.tag.rsplit("}", 1)[-1]
        names.append(tag)
    counts = {name: names.count(name) for name in sorted(set(names))}
    return {
        "tei_sha256": hashlib.sha256(tei_bytes).hexdigest(),
        "tei_byte_count": len(tei_bytes),
        "element_counts": {
            "biblStruct": counts.get("biblStruct", 0),
            "div": counts.get("div", 0),
            "ref": counts.get("ref", 0),
            "figure": counts.get("figure", 0),
            "table": counts.get("table", 0),
            "title": counts.get("title", 0),
        },
    }


def assert_no_forbidden_fields(value: Any) -> None:
    if isinstance(value, dict):
        overlap = FORBIDDEN_KEYS.intersection(value)
        if overlap:
            raise ValueError(f"forbidden fields present: {sorted(overlap)}")
        for child in value.values():
            assert_no_forbidden_fields(child)
    elif isinstance(value, list):
        for child in value:
            assert_no_forbidden_fields(child)


def build_live_grobid_packets(
    *,
    target: dict[str, Any],
    source_readiness: dict[str, Any],
    runtime_update: dict[str, Any],
    submitter: GrobidSubmitter = submit_pdf_to_grobid,
    run_guardrail_first: bool = True,
    timeout: int = 120,
) -> dict[str, Any]:
    if run_guardrail_first:
        run_guardrail()
    service_url = str(runtime_update.get("service_url", ""))
    service_live = runtime_update.get("current_grobid_status") == "live_ready"
    source_by_key = {record["article_key"]: record for record in source_readiness.get("records", [])}
    packets: list[dict[str, Any]] = []
    for entry in target.get("articles", []):
        key = entry["article_key"]
        source = source_by_key.get(key)
        if source is None:
            raise ValueError(f"missing source readiness for {key}")
        pdf_files = [ROOT / path for path in source.get("pdf_files", [])]
        packet: dict[str, Any] = {
            "article_key": key,
            "category": entry.get("m041_category"),
            "candidate_only": True,
            "graph_write_allowed": False,
            "promotion_allowed": False,
            "production_import_attempted": False,
            "import_eligible": False,
            "local_pdf_count": len(pdf_files),
        }
        if not service_live:
            packet.update({"status": "service_blocked", "blockers": ["grobid_service_not_live"]})
        elif not pdf_files:
            packet.update({"status": "missing_pdf", "blockers": ["local_pdf_missing"]})
        else:
            pdf_path = pdf_files[0]
            try:
                tei_bytes = submitter(pdf_path, service_url, timeout)
                packet.update({"status": "live_success", "blockers": [], "pdf_ref": str(pdf_path.relative_to(ROOT))})
                packet.update(summarize_tei(tei_bytes))
            except (ET.ParseError, urllib.error.URLError, TimeoutError, OSError, ValueError) as exc:
                packet.update({"status": "live_error", "blockers": [f"{type(exc).__name__}: {exc}"], "pdf_ref": str(pdf_path.relative_to(ROOT))})
        packets.append(packet)

    status_counts: dict[str, int] = {}
    for packet in packets:
        status = packet["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    result = {
        "schema_version": "m044.live-grobid-candidate-packets.v1",
        "target_subset": "artifacts/m043-combined-sidecar-probe/target-subset.json",
        "service_url": service_url,
        "article_count": len(packets),
        "status_counts": status_counts,
        "packets": packets,
        "forbidden_payload_fields_absent": True,
        "candidate_only": True,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    assert_no_forbidden_fields(result)
    return result


def render_markdown(result: dict[str, Any]) -> str:
    lines = [
        "# M044 Live GROBID Candidate Packets",
        "",
        f"- Service URL: `{result['service_url']}`",
        f"- Article count: {result['article_count']}",
        f"- Status counts: {result['status_counts']}",
        "- Candidate only: true",
        "- Graph writes: disabled",
        "- Production import: disabled",
        "- Fact promotion: disabled",
        "- Raw TEI/full text persisted: false",
        "",
        "| Article | Status | PDF count | biblStruct | div | ref | TEI bytes | Blockers |",
        "|---|---|---:|---:|---:|---:|---:|---|",
    ]
    for packet in result["packets"]:
        counts = packet.get("element_counts", {})
        blockers = ", ".join(packet.get("blockers", [])) or "none"
        lines.append(
            f"| {packet['article_key']} | {packet['status']} | {packet['local_pdf_count']} | "
            f"{counts.get('biblStruct', 0)} | {counts.get('div', 0)} | {counts.get('ref', 0)} | "
            f"{packet.get('tei_byte_count', 0)} | {blockers} |"
        )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-subset", type=Path, default=DEFAULT_TARGET)
    parser.add_argument("--source-readiness", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--runtime-update", type=Path, default=DEFAULT_RUNTIME_UPDATE)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    result = build_live_grobid_packets(
        target=load_json(args.target_subset),
        source_readiness=load_json(args.source_readiness),
        runtime_update=load_json(args.runtime_update),
        timeout=args.timeout,
    )
    write_json(args.output_dir / "live-grobid-candidate-packets.json", result)
    write_text(args.output_dir / "live-grobid-candidate-packets.md", render_markdown(result))
    sys.stdout.write(f"m044 live grobid probe complete: {result['status_counts']}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
