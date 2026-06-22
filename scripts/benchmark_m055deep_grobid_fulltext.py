#!/usr/bin/env python3
"""GROBID fulltext probe for M055deep S01.

Runs a bounded, fail-closed GROBID /api/processFulltextDocument probe over the
five-PDF M055 corpus manifest. The script emits per-PDF diagnostic packets, raw
TEI XML, and an aggregate summary only. It never writes graph data, never
attempts a production import, and keeps all five safety defaults false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import time
import urllib.error
import urllib.request
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055deep-parser-benchmark.grobid-fulltext.v1"
DEFAULT_CORPUS_MANIFEST = Path("artifacts/m055-parser-benchmark/corpus-manifest.json")
DEFAULT_OUTPUT_DIR = Path("artifacts/m055deep-parser-benchmark/grobid-fulltext")
DEFAULT_GROBID_URL = "http://127.0.0.1:8070"
DEFAULT_MAX_RETRIES = 3
DEFAULT_TIMEOUT_SECONDS = 120
USER_AGENT = "daily-archive-m055deep-grobid-fulltext/1.0"
LOW_QUALITY_MIN_TEI_BYTES = 1024
AGGREGATE_STATUSES = ("success", "low_quality_source", "blocked")


def _utc_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def _normalize_grobid_url(grobid_url: str) -> str:
    return grobid_url.rstrip("/")


def _safety_defaults() -> dict[str, bool]:
    return {
        "graph_import_allowed": False,
        "graphdb_written": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def _atomic_write_bytes(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
    tmp_path.write_bytes(payload)
    tmp_path.replace(path)


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    encoded = json.dumps(payload, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    _atomic_write_bytes(path, encoded)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _multipart_pdf_request(endpoint: str, pdf_path: Path) -> urllib.request.Request:
    boundary = f"----daily-archive-m055deep-{uuid.uuid4().hex}"
    pdf_bytes = pdf_path.read_bytes()
    filename = pdf_path.name.encode("utf-8")
    body = b"".join(
        [
            f"--{boundary}\r\n".encode("ascii"),
            b'Content-Disposition: form-data; name="input"; filename="' + filename + b'"\r\n',
            b"Content-Type: application/pdf\r\n\r\n",
            pdf_bytes,
            b"\r\n",
            f"--{boundary}--\r\n".encode("ascii"),
        ]
    )
    request = urllib.request.Request(endpoint, data=body, method="POST")
    request.add_header("Content-Type", f"multipart/form-data; boundary={boundary}")
    request.add_header("Content-Length", str(len(body)))
    request.add_header("User-Agent", USER_AGENT)
    return request


def _probe_grobid_fulltext(
    pdf_path: Path, endpoint: str, timeout: int = DEFAULT_TIMEOUT_SECONDS
) -> dict[str, Any]:
    """POST one PDF to GROBID fulltext and return a fail-closed result dict."""

    started = time.monotonic()
    try:
        request = _multipart_pdf_request(endpoint, pdf_path)
        with urllib.request.urlopen(request, timeout=timeout) as response:  # noqa: S310 - local benchmark endpoint
            response_body = response.read()
            status = getattr(response, "status", None) or getattr(response, "code", None)
        duration_ms = int((time.monotonic() - started) * 1000)
        tei_text = response_body.decode("utf-8", errors="replace")
        return {
            "tei_text": tei_text,
            "http_status": status,
            "bytes": len(response_body),
            "duration_ms": duration_ms,
            "error": None,
        }
    except urllib.error.HTTPError as exc:
        duration_ms = int((time.monotonic() - started) * 1000)
        try:
            error_body = exc.read().decode("utf-8", errors="replace")[:500]
        except Exception:  # pragma: no cover - defensive only
            error_body = ""
        return {
            "tei_text": "",
            "http_status": exc.code,
            "bytes": 0,
            "duration_ms": duration_ms,
            "error": f"HTTPError:{exc.code}:{error_body}",
        }
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        duration_ms = int((time.monotonic() - started) * 1000)
        return {
            "tei_text": "",
            "http_status": None,
            "bytes": 0,
            "duration_ms": duration_ms,
            "error": f"{type(exc).__name__}:{exc}",
        }


def _local_name(tag: str) -> str:
    if "}" in tag:
        return tag.rsplit("}", 1)[1]
    return tag


def _text_present(element: ET.Element | None) -> bool:
    if element is None:
        return False
    return bool("".join(element.itertext()).strip())


def _iter_elements(root: ET.Element, local_name: str) -> list[ET.Element]:
    return [element for element in root.iter() if _local_name(element.tag) == local_name]


def _extract_fulltext_metrics(tei_text: str) -> dict[str, Any]:
    """Extract parser-quality metrics from GROBID TEI XML."""

    metrics: dict[str, Any] = {
        "tei_size_bytes": len(tei_text.encode("utf-8")),
        "ref_count": 0,
        "bibl_count": 0,
        "body_element_count": 0,
        "equation_count": 0,
        "figure_count": 0,
        "header_title_present": False,
        "header_author_count": 0,
        "abstract_present": False,
        "section_count": 0,
        "sections": [],
        "parse_error": None,
    }
    if not tei_text:
        return metrics
    try:
        root = ET.fromstring(tei_text)
    except ET.ParseError as exc:
        metrics["parse_error"] = f"ParseError:{exc}"
        return metrics

    refs = _iter_elements(root, "ref")
    bibls = [
        element for element in root.iter() if _local_name(element.tag) in {"biblStruct", "bibl"}
    ]
    formulas = _iter_elements(root, "formula")
    figures = _iter_elements(root, "figure")
    titles = _iter_elements(root, "title")
    authors = _iter_elements(root, "author")
    abstracts = _iter_elements(root, "abstract")
    body = next((element for element in root.iter() if _local_name(element.tag) == "body"), None)
    body_divs = (
        [element for element in body.iter() if _local_name(element.tag) == "div"]
        if body is not None
        else []
    )
    section_divs = [
        element
        for element in body_divs
        if element.attrib.get("type") == "section"
        or (
            element.attrib.get("type") in {None, ""}
            and any(_local_name(child.tag) == "head" for child in element)
        )
    ]

    sections = []
    for div in section_divs:
        head = next((child for child in div if _local_name(child.tag) == "head"), None)
        sections.append(" ".join("".join(head.itertext()).split()) if head is not None else "")

    metrics.update(
        {
            "ref_count": len(refs),
            "bibl_count": len(bibls),
            "body_element_count": max(0, sum(1 for _ in body.iter()) - 1)
            if body is not None
            else 0,
            "equation_count": len(formulas),
            "figure_count": len(figures),
            "header_title_present": any(_text_present(title) for title in titles),
            "header_author_count": len(authors),
            "abstract_present": any(_text_present(abstract) for abstract in abstracts),
            "section_count": len(section_divs),
            "sections": sections,
        }
    )
    return metrics


def _low_quality_source_criteria(metrics: dict[str, Any]) -> bool:
    return (
        int(metrics.get("tei_size_bytes") or 0) < LOW_QUALITY_MIN_TEI_BYTES
        or int(metrics.get("ref_count") or 0) == 0
        or int(metrics.get("body_element_count") or 0) == 0
        or int(metrics.get("section_count") or 0) == 0
    )


def _empty_packet_metrics() -> dict[str, Any]:
    return {
        "tei_size_bytes": 0,
        "ref_count": 0,
        "bibl_count": 0,
        "body_element_count": 0,
        "equation_count": 0,
        "figure_count": 0,
        "header_title_present": False,
        "header_author_count": 0,
        "abstract_present": False,
        "section_count": 0,
        "parse_error": None,
    }


def _packet_base(entry: dict[str, Any], *, grobid_url: str) -> dict[str, Any]:
    arxiv_id = (
        entry.get("arxiv_id")
        or entry.get("article_key")
        or Path(str(entry.get("path", "unknown"))).stem
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "arxiv_id": arxiv_id,
        "article_key": entry.get("article_key"),
        "category": entry.get("category"),
        "pdf_path": entry.get("path"),
        "manifest_sha256": entry.get("sha256"),
        "grobid_url": grobid_url,
        "endpoint": f"{_normalize_grobid_url(grobid_url)}/api/processFulltextDocument",
        "safety_defaults": _safety_defaults(),
    }


def _packet_for_missing_pdf(
    entry: dict[str, Any], *, grobid_url: str, pdf_path: Path
) -> dict[str, Any]:
    packet = _packet_base(entry, grobid_url=grobid_url)
    packet.update(_empty_packet_metrics())
    packet.update(
        {
            "status": "blocked",
            "low_quality_source": False,
            "m022_repair_candidate": False,
            "http_status": None,
            "attempts": 0,
            "duration_ms": 0,
            "bytes": 0,
            "error": f"pdf_missing:{pdf_path}",
            "tei_path": None,
            "sha256_actual": None,
            "sha256_matches_manifest": False,
        }
    )
    return packet


def _write_low_level_outputs(
    output_dir: Path, arxiv_id: str, packet: dict[str, Any], tei_text: str
) -> None:
    if tei_text:
        _atomic_write_bytes(output_dir / "tei" / f"{arxiv_id}.tei.xml", tei_text.encode("utf-8"))
    _atomic_write_json(output_dir / "per-pdf" / f"{arxiv_id}.json", packet)


def _probe_manifest_entry(
    entry: dict[str, Any],
    *,
    output_dir: Path,
    grobid_url: str,
    max_retries: int,
    timeout: int,
) -> dict[str, Any]:
    arxiv_id = str(
        entry.get("arxiv_id")
        or entry.get("article_key")
        or Path(str(entry.get("path", "unknown"))).stem
    )
    pdf_path = Path(str(entry.get("path", "")))
    if not pdf_path.exists():
        packet = _packet_for_missing_pdf(entry, grobid_url=grobid_url, pdf_path=pdf_path)
        _write_low_level_outputs(output_dir, arxiv_id, packet, "")
        return packet

    endpoint = f"{_normalize_grobid_url(grobid_url)}/api/processFulltextDocument"
    attempts = 0
    total_duration_ms = 0
    result: dict[str, Any] = {
        "tei_text": "",
        "http_status": None,
        "bytes": 0,
        "duration_ms": 0,
        "error": "not_attempted",
    }
    for attempt in range(1, max(1, max_retries) + 1):
        attempts = attempt
        result = _probe_grobid_fulltext(pdf_path, endpoint, timeout=timeout)
        total_duration_ms += int(result.get("duration_ms") or 0)
        if (
            result.get("error") is None
            and result.get("http_status") == 200
            and result.get("tei_text")
        ):
            break

    tei_text = str(result.get("tei_text") or "")
    metrics = _extract_fulltext_metrics(tei_text)
    low_quality = _low_quality_source_criteria(metrics)
    blocked = (
        bool(result.get("error"))
        or result.get("http_status") != 200
        or bool(metrics.get("parse_error"))
    )
    status = "blocked" if blocked else "low_quality_source" if low_quality else "success"
    tei_path = output_dir / "tei" / f"{arxiv_id}.tei.xml"
    sha256_actual = _sha256_file(pdf_path)

    packet = _packet_base(entry, grobid_url=grobid_url)
    packet.update(metrics)
    packet.update(
        {
            "status": status,
            "low_quality_source": low_quality,
            "m022_repair_candidate": low_quality,
            "http_status": result.get("http_status"),
            "attempts": attempts,
            "duration_ms": total_duration_ms,
            "bytes": int(result.get("bytes") or 0),
            "error": result.get("error") or metrics.get("parse_error"),
            "tei_path": str(tei_path) if tei_text else None,
            "sha256_actual": sha256_actual,
            "sha256_matches_manifest": sha256_actual == entry.get("sha256"),
        }
    )
    _write_low_level_outputs(output_dir, arxiv_id, packet, tei_text)
    return packet


def _build_summary(
    *,
    corpus_manifest_path: Path,
    output_dir: Path,
    grobid_url: str,
    packets: list[dict[str, Any]],
) -> dict[str, Any]:
    counts = dict.fromkeys(AGGREGATE_STATUSES, 0)
    for packet in packets:
        status = str(packet.get("status", "blocked"))
        counts[status] = counts.get(status, 0) + 1
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "corpus_manifest_path": str(corpus_manifest_path),
        "output_dir": str(output_dir),
        "grobid_url": grobid_url,
        "total_pdfs": len(packets),
        "aggregate_counts": counts,
        "success_count": counts.get("success", 0),
        "low_quality_source_count": counts.get("low_quality_source", 0),
        "blocked_count": counts.get("blocked", 0),
        "body_positive_count": sum(
            1 for packet in packets if int(packet.get("body_element_count") or 0) > 0
        ),
        "section_positive_count": sum(
            1 for packet in packets if int(packet.get("section_count") or 0) > 0
        ),
        "ref_positive_count": sum(1 for packet in packets if int(packet.get("ref_count") or 0) > 0),
        "total_ref_count": sum(int(packet.get("ref_count") or 0) for packet in packets),
        "total_bibl_count": sum(int(packet.get("bibl_count") or 0) for packet in packets),
        "total_body_element_count": sum(
            int(packet.get("body_element_count") or 0) for packet in packets
        ),
        "total_equation_count": sum(int(packet.get("equation_count") or 0) for packet in packets),
        "total_figure_count": sum(int(packet.get("figure_count") or 0) for packet in packets),
        "safety_defaults": _safety_defaults(),
        "packets": [
            str(output_dir / "per-pdf" / f"{packet['arxiv_id']}.json") for packet in packets
        ],
    }


def probe_grobid_fulltext(
    corpus_manifest_path: Path,
    output_dir: Path,
    *,
    grobid_url: str = DEFAULT_GROBID_URL,
    max_retries: int = DEFAULT_MAX_RETRIES,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
) -> dict[str, Any]:
    manifest = json.loads(corpus_manifest_path.read_text(encoding="utf-8"))
    packets = [
        _probe_manifest_entry(
            entry,
            output_dir=output_dir,
            grobid_url=grobid_url,
            max_retries=max_retries,
            timeout=timeout,
        )
        for entry in manifest.get("pdfs", [])
    ]
    summary = _build_summary(
        corpus_manifest_path=corpus_manifest_path,
        output_dir=output_dir,
        grobid_url=grobid_url,
        packets=packets,
    )
    _atomic_write_json(output_dir / "summary.json", summary)
    return summary


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-manifest", type=Path, default=DEFAULT_CORPUS_MANIFEST)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--grobid-url", default=DEFAULT_GROBID_URL)
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    summary = probe_grobid_fulltext(
        args.corpus_manifest,
        args.output_dir,
        grobid_url=args.grobid_url,
        max_retries=args.max_retries,
        timeout=args.timeout,
    )
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if summary.get("blocked_count") == 0 else 2


if __name__ == "__main__":
    raise SystemExit(main())
