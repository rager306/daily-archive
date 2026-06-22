#!/usr/bin/env python3
"""Bounded GROBID pilot probe for M053 S01.

Emits per-PDF diagnostic packets for the five M051 acquired PDFs. This script
never writes graph data, never attempts a production import, and keeps all five
safety flags false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import socket
import time
import urllib.error
import urllib.request
import uuid
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGET_SUBSET = REPO_ROOT / "artifacts" / "m054-pdf-acquisition" / "target-subset.json"
DEFAULT_OUTPUT_DIR = REPO_ROOT / "artifacts" / "m053-grobid-pilot"
DEFAULT_GROBID_URL = "http://localhost:8070"
DEFAULT_MAX_RETRIES = 3
DEFAULT_TIMEOUT_SECONDS = 60
SERVICE_TIMEOUT_SECONDS = 5
SCHEMA_VERSION = "m053-grobid-pilot.v1"
USER_AGENT = "daily-archive-grobid-pilot/1.0"
LOW_QUALITY_MIN_TEI_BYTES = 1024
VALID_STATUSES = {
    "success",
    "low_quality_source",
    "blocked",
    "grobid_unavailable",
    "network_error",
    "timeout",
}
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


@dataclass(frozen=True)
class PdfTarget:
    paper_id: str
    pdf_path: Path


def utc_now() -> str:
    return dt.datetime.now(tz=dt.timezone.utc).isoformat()


def safety_defaults() -> dict[str, bool]:
    return dict(SAFETY_DEFAULTS)


def normalize_grobid_url(grobid_url: str | None) -> str:
    return (grobid_url or os.environ.get("GROBID_URL") or DEFAULT_GROBID_URL).rstrip("/")


def atomic_write_bytes(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    tmp_path.write_bytes(payload)
    tmp_path.replace(path)


def atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    body = json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False).encode("utf-8")
    atomic_write_bytes(path, body + b"\n")


def paper_id_from_path(pdf_path: Path) -> str:
    return pdf_path.stem


def load_default_targets(target_subset_path: Path = DEFAULT_TARGET_SUBSET) -> list[PdfTarget]:
    payload = json.loads(target_subset_path.read_text(encoding="utf-8"))
    targets: list[PdfTarget] = []
    for record in payload.get("records", []):
        raw_path = record.get("expected_local_pdf_path")
        if not raw_path:
            continue
        pdf_path = Path(raw_path)
        if not pdf_path.is_absolute():
            pdf_path = REPO_ROOT / pdf_path
        targets.append(
            PdfTarget(str(record.get("article_key") or paper_id_from_path(pdf_path)), pdf_path)
        )
    return targets


def collect_pdf_targets(
    *,
    pdf_paths: list[Path] | None,
    pdf_dir: Path | None,
    target_subset_path: Path = DEFAULT_TARGET_SUBSET,
) -> list[PdfTarget]:
    targets: list[PdfTarget] = []
    if pdf_paths:
        targets.extend(PdfTarget(paper_id_from_path(path), path.resolve()) for path in pdf_paths)
    if pdf_dir:
        targets.extend(
            PdfTarget(paper_id_from_path(path), path.resolve())
            for path in sorted(pdf_dir.glob("*.pdf"))
        )
    return targets or load_default_targets(target_subset_path)


def is_timeout_exception(exc: BaseException) -> bool:
    if isinstance(exc, (TimeoutError, socket.timeout)):
        return True
    return isinstance(getattr(exc, "reason", None), (TimeoutError, socket.timeout))


def classify_exception(exc: BaseException) -> str:
    if isinstance(exc, urllib.error.HTTPError):
        return "blocked" if 400 <= exc.code < 500 else "network_error"
    if is_timeout_exception(exc):
        return "timeout"
    if isinstance(exc, (urllib.error.URLError, ConnectionError, OSError)):
        return "network_error"
    return "network_error"


def check_grobid_available(grobid_url: str, *, timeout: int = SERVICE_TIMEOUT_SECONDS) -> bool:
    request = urllib.request.Request(
        f"{normalize_grobid_url(grobid_url)}/api/isalive",
        method="GET",
        headers={"User-Agent": USER_AGENT},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return int(getattr(response, "status", 0)) == 200
    except BaseException:  # noqa: BLE001 - availability is a fail-closed boolean
        return False


def iter_local_names(root: ET.Element, local_name: str) -> int:
    suffix = "}" + local_name
    return sum(
        1 for element in root.iter() if element.tag == local_name or element.tag.endswith(suffix)
    )


def count_tei_elements(tei: bytes) -> tuple[int, int]:
    if not tei.strip():
        return 0, 0
    try:
        root = ET.fromstring(tei)
    except ET.ParseError:
        return 0, 0
    return iter_local_names(root, "ref"), iter_local_names(root, "body")


def is_low_quality_source(tei: bytes, *, ref_count: int, body_element_count: int) -> bool:
    return len(tei) < LOW_QUALITY_MIN_TEI_BYTES or ref_count == 0 or body_element_count == 0


def build_multipart_body(pdf_path: Path) -> tuple[bytes, str]:
    boundary = f"----m053-grobid-pilot-{uuid.uuid4().hex}"
    parts: list[bytes] = []

    def field(name: str, value: str) -> None:
        parts.extend(
            [
                f"--{boundary}\r\n".encode(),
                f'Content-Disposition: form-data; name="{name}"\r\n\r\n'.encode(),
                value.encode() + b"\r\n",
            ]
        )

    field("consolidateHeader", "0")
    field("consolidateCitations", "0")
    parts.extend(
        [
            f"--{boundary}\r\n".encode(),
            (
                f'Content-Disposition: form-data; name="input"; filename="{pdf_path.name}"\r\n'
                "Content-Type: application/pdf\r\n\r\n"
            ).encode(),
            pdf_path.read_bytes() + b"\r\n",
            f"--{boundary}--\r\n".encode(),
        ]
    )
    return b"".join(parts), boundary


def post_grobid_header(
    pdf_path: Path, grobid_url: str, *, timeout: int = DEFAULT_TIMEOUT_SECONDS
) -> tuple[bytes, int]:
    body, boundary = build_multipart_body(pdf_path)
    request = urllib.request.Request(
        f"{normalize_grobid_url(grobid_url)}/api/processHeaderDocument",
        data=body,
        method="POST",
        headers={
            "User-Agent": USER_AGENT,
            "Content-Type": f"multipart/form-data; boundary={boundary}",
            "Content-Length": str(len(body)),
        },
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read(), int(getattr(response, "status", 0))


def make_packet(
    target: PdfTarget,
    *,
    status: str,
    grobid_url: str,
    attempts: list[dict[str, Any]] | None = None,
    http_status: int | None = None,
    tei_path: Path | None = None,
    tei_size_bytes: int = 0,
    ref_count: int = 0,
    body_element_count: int = 0,
    note: str | None = None,
) -> dict[str, Any]:
    packet: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": utc_now(),
        "paper_id": target.paper_id,
        "pdf_path": str(target.pdf_path),
        "grobid_url": normalize_grobid_url(grobid_url),
        "status": status,
        "tei_path": str(tei_path) if tei_path else None,
        "tei_size_bytes": tei_size_bytes,
        "ref_count": ref_count,
        "body_element_count": body_element_count,
        "low_quality_source": status == "low_quality_source",
        "http_status": http_status,
        "attempts": attempts or [],
        "m022_repair_candidate": status in {"low_quality_source", "grobid_unavailable"},
        "safety_defaults": safety_defaults(),
    }
    if note:
        packet["note"] = note
    return packet


def probe_pdf(
    target: PdfTarget,
    *,
    grobid_url: str,
    output_dir: Path,
    max_retries: int = DEFAULT_MAX_RETRIES,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
    service_available: bool = True,
    dry_run: bool = False,
) -> dict[str, Any]:
    packet_path = output_dir / f"{target.paper_id}.json"
    tei_path = output_dir / f"{target.paper_id}.tei.xml"
    if dry_run:
        packet = make_packet(
            target,
            status="grobid_unavailable",
            grobid_url=grobid_url,
            note="dry_run_skipped_grobid_call",
        )
        atomic_write_json(packet_path, packet)
        return packet
    if not target.pdf_path.exists():
        packet = make_packet(target, status="blocked", grobid_url=grobid_url, note="pdf_missing")
        atomic_write_json(packet_path, packet)
        return packet
    if not service_available:
        packet = make_packet(
            target, status="grobid_unavailable", grobid_url=grobid_url, note="grobid_isalive_failed"
        )
        atomic_write_json(packet_path, packet)
        return packet

    attempts: list[dict[str, Any]] = []
    final_status = "network_error"
    final_http_status: int | None = None
    final_tei = b""
    final_ref_count = 0
    final_body_count = 0
    for attempt_index in range(1, max_retries + 1):
        started_at = utc_now()
        try:
            tei, http_status = post_grobid_header(target.pdf_path, grobid_url, timeout=timeout)
            ref_count, body_count = count_tei_elements(tei)
            final_status = (
                "low_quality_source"
                if is_low_quality_source(tei, ref_count=ref_count, body_element_count=body_count)
                else "success"
            )
            final_http_status = http_status
            final_tei = tei
            final_ref_count = ref_count
            final_body_count = body_count
            attempts.append(
                {
                    "attempt": attempt_index,
                    "started_at": started_at,
                    "completed_at": utc_now(),
                    "bytes": len(tei),
                    "http_status": http_status,
                    "outcome": final_status,
                }
            )
            atomic_write_bytes(tei_path, tei)
            break
        except BaseException as exc:  # noqa: BLE001 - typed fail-closed diagnostics
            final_status = classify_exception(exc)
            final_http_status = getattr(exc, "code", None)
            attempts.append(
                {
                    "attempt": attempt_index,
                    "started_at": started_at,
                    "completed_at": utc_now(),
                    "bytes": 0,
                    "http_status": final_http_status,
                    "outcome": final_status,
                    "exception": type(exc).__name__,
                    "exception_message": str(exc)[:500],
                }
            )
            if final_status == "blocked" or attempt_index >= max_retries:
                break
            time.sleep(min(2 ** (attempt_index - 1), 5))

    packet = make_packet(
        target,
        status=final_status,
        grobid_url=grobid_url,
        attempts=attempts,
        http_status=final_http_status,
        tei_path=tei_path if final_tei else None,
        tei_size_bytes=len(final_tei),
        ref_count=final_ref_count,
        body_element_count=final_body_count,
    )
    atomic_write_json(packet_path, packet)
    return packet


def build_summary(
    *,
    packets: list[dict[str, Any]],
    output_dir: Path,
    grobid_url: str,
    dry_run: bool,
    max_retries: int,
    timeout: int,
) -> dict[str, Any]:
    counts: dict[str, int] = dict.fromkeys(sorted(VALID_STATUSES), 0)
    for packet in packets:
        status = str(packet.get("status", "network_error"))
        counts[status] = counts.get(status, 0) + 1
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": utc_now(),
        "grobid_url": normalize_grobid_url(grobid_url),
        "dry_run": dry_run,
        "max_retries": max_retries,
        "timeout_seconds": timeout,
        "total_pdfs": len(packets),
        "counts": counts,
        "packets": [
            {
                "paper_id": packet["paper_id"],
                "status": packet["status"],
                "packet_path": str(output_dir / f"{packet['paper_id']}.json"),
                "tei_path": packet.get("tei_path"),
                "m022_repair_candidate": packet["m022_repair_candidate"],
            }
            for packet in sorted(packets, key=lambda item: str(item["paper_id"]))
        ],
        "safety_defaults": safety_defaults(),
    }


def run_probe(
    targets: list[PdfTarget],
    *,
    output_dir: Path,
    grobid_url: str,
    max_retries: int = DEFAULT_MAX_RETRIES,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
    dry_run: bool = False,
) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    service_available = False if dry_run else check_grobid_available(grobid_url)
    packets = [
        probe_pdf(
            target,
            grobid_url=grobid_url,
            output_dir=output_dir,
            max_retries=max_retries,
            timeout=timeout,
            service_available=service_available,
            dry_run=dry_run,
        )
        for target in targets
    ]
    summary = build_summary(
        packets=packets,
        output_dir=output_dir,
        grobid_url=grobid_url,
        dry_run=dry_run,
        max_retries=max_retries,
        timeout=timeout,
    )
    atomic_write_json(output_dir / "summary.json", summary)
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pdf-path", type=Path, action="append", default=None, help="PDF path; may be repeated"
    )
    parser.add_argument(
        "--pdf-dir", type=Path, default=None, help="Directory containing PDFs to probe"
    )
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument(
        "--grobid-url", default=None, help="GROBID base URL; defaults to GROBID_URL or localhost"
    )
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--dry-run", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    grobid_url = normalize_grobid_url(args.grobid_url)
    targets = collect_pdf_targets(pdf_paths=args.pdf_path, pdf_dir=args.pdf_dir)
    summary = run_probe(
        targets,
        output_dir=args.output_dir,
        grobid_url=grobid_url,
        max_retries=args.max_retries,
        timeout=args.timeout,
        dry_run=args.dry_run,
    )
    print(
        json.dumps(
            {"summary_path": str(args.output_dir / "summary.json"), "counts": summary["counts"]},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
