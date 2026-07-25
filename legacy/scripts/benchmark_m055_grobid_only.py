#!/usr/bin/env python3
"""GROBID-only baseline probe for M055 parser hybrid benchmark S02.

Runs a bounded, fail-closed GROBID header probe over the five-PDF M055 corpus
manifest. The script emits per-PDF diagnostic packets and a summary only; it
never writes graph data, never attempts a production import, and keeps all five
safety defaults false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import sys
import time
import urllib.error
import urllib.request
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055-parser-benchmark.grobid-only.v1"
DEFAULT_CORPUS_MANIFEST = Path("artifacts/m055-parser-benchmark/corpus-manifest.json")
DEFAULT_OUTPUT_DIR = Path("artifacts/m055-parser-benchmark/grobid-only")
DEFAULT_GROBID_URL = "http://localhost:8070"
DEFAULT_MAX_RETRIES = 3
DEFAULT_TIMEOUT_SECONDS = 60
USER_AGENT = "daily-archive-m055-grobid-only/1.0"
LOW_QUALITY_MIN_TEI_BYTES = 1024
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}
AGGREGATE_STATUSES = (
    "success",
    "low_quality_source",
    "blocked",
    "grobid_unavailable",
    "network_error",
    "timeout",
)


def _utc_now() -> str:
    return dt.datetime.now(tz=dt.UTC).isoformat()


def _normalize_grobid_url(grobid_url: str) -> str:
    return grobid_url.rstrip("/")


def _safety_defaults() -> dict[str, bool]:
    return dict(SAFETY_DEFAULTS)


def _atomic_write_bytes(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + f".{uuid.uuid4().hex}.tmp")
    tmp_path.write_bytes(payload)
    tmp_path.replace(path)


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    body = json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False).encode("utf-8")
    _atomic_write_bytes(path, body + b"\n")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1] if "}" in tag else tag


def _has_text(element: ET.Element) -> bool:
    return "".join(element.itertext()).strip() != ""


def _iter_descendants_by_local_name(root: ET.Element, local_name: str) -> list[ET.Element]:
    return [element for element in root.iter() if _local_name(element.tag) == local_name]


def _multipart_body(pdf_path: Path) -> tuple[bytes, str]:
    boundary = f"----daily-archive-m055-{uuid.uuid4().hex}"
    filename = pdf_path.name
    pdf_bytes = pdf_path.read_bytes()
    parts = [
        f"--{boundary}\r\n".encode(),
        (
            'Content-Disposition: form-data; name="input"; '
            f'filename="{filename}"\r\n'
            "Content-Type: application/pdf\r\n\r\n"
        ).encode(),
        pdf_bytes,
        b"\r\n",
        f"--{boundary}--\r\n".encode(),
    ]
    return b"".join(parts), f"multipart/form-data; boundary={boundary}"


def _classify_exception(exc: BaseException) -> str:
    if isinstance(exc, TimeoutError):
        return "timeout"
    if isinstance(exc, urllib.error.HTTPError):
        return "blocked" if 400 <= exc.code < 500 else "network_error"
    if isinstance(exc, urllib.error.URLError):
        reason = getattr(exc, "reason", None)
        if isinstance(reason, TimeoutError):
            return "timeout"
        return "network_error"
    return "network_error"


def _probe_grobid_pdf(pdf_path: Path, endpoint: str, timeout: int) -> dict[str, Any]:
    """POST one PDF to GROBID /api/processHeaderDocument."""
    started = time.monotonic()
    try:
        body, content_type = _multipart_body(pdf_path)
        request = urllib.request.Request(
            endpoint,
            data=body,
            method="POST",
            headers={
                "Content-Type": content_type,
                "Accept": "application/xml,text/xml,*/*",
                "User-Agent": USER_AGENT,
            },
        )
        with urllib.request.urlopen(request, timeout=timeout) as response:
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
    except (urllib.error.URLError, TimeoutError, ConnectionError) as exc:
        duration_ms = int((time.monotonic() - started) * 1000)
        return {
            "tei_text": "",
            "http_status": None,
            "bytes": 0,
            "duration_ms": duration_ms,
            "error": f"{type(exc).__name__}:{exc}",
        }


def _extract_tei_metrics(tei_text: str) -> dict[str, Any]:
    """Extract lightweight quality metrics from TEI XML using stdlib XML."""
    tei_size_bytes = len(tei_text.encode("utf-8"))
    empty_metrics: dict[str, Any] = {
        "tei_size_bytes": tei_size_bytes,
        "ref_count": 0,
        "bibl_count": 0,
        "body_element_count": 0,
        "header_title_present": False,
        "header_author_count": 0,
        "abstract_present": False,
    }
    if not tei_text.strip():
        return empty_metrics
    try:
        root = ET.fromstring(tei_text)
    except ET.ParseError:
        return empty_metrics

    refs = _iter_descendants_by_local_name(root, "ref")
    bibls = [
        element
        for element in root.iter()
        if _local_name(element.tag) in {"bibl", "biblStruct", "listBibl"}
    ]
    bodies = _iter_descendants_by_local_name(root, "body")
    body_element_count = sum(1 for body in bodies for element in body.iter() if element is not body)
    headers = _iter_descendants_by_local_name(root, "teiHeader")
    header_roots = headers or [root]

    title_present = any(
        _has_text(title)
        for header in header_roots
        for title in _iter_descendants_by_local_name(header, "title")
    )
    author_count = sum(
        1
        for header in header_roots
        for author in _iter_descendants_by_local_name(header, "author")
        if _has_text(author)
    )
    abstract_present = any(
        _has_text(abstract) for abstract in _iter_descendants_by_local_name(root, "abstract")
    )

    return {
        "tei_size_bytes": tei_size_bytes,
        "ref_count": len(refs),
        "bibl_count": len(bibls),
        "body_element_count": body_element_count,
        "header_title_present": title_present,
        "header_author_count": author_count,
        "abstract_present": abstract_present,
    }


def _low_quality_source_criteria(tei_metrics: dict[str, Any]) -> bool:
    return (
        int(tei_metrics.get("tei_size_bytes") or 0) < LOW_QUALITY_MIN_TEI_BYTES
        or int(tei_metrics.get("ref_count") or 0) == 0
        or int(tei_metrics.get("body_element_count") or 0) == 0
    )


def _load_corpus_manifest(corpus_manifest_path: Path) -> list[dict[str, Any]]:
    payload = json.loads(corpus_manifest_path.read_text(encoding="utf-8"))
    pdfs = payload.get("pdfs")
    if not isinstance(pdfs, list) or not pdfs:
        raise ValueError(f"corpus manifest has no pdfs list: {corpus_manifest_path}")
    return pdfs


def _empty_packet_metrics() -> dict[str, Any]:
    return {
        "tei_size_bytes": 0,
        "ref_count": 0,
        "bibl_count": 0,
        "body_element_count": 0,
        "header_title_present": False,
        "header_author_count": 0,
        "abstract_present": False,
    }


def _packet_base(entry: dict[str, Any], *, grobid_url: str) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "arxiv_id": entry.get("arxiv_id")
        or entry.get("article_key")
        or Path(str(entry.get("path", "unknown"))).stem,
        "article_key": entry.get("article_key"),
        "category": entry.get("category"),
        "pdf_path": entry.get("path"),
        "manifest_sha256": entry.get("sha256"),
        "grobid_url": grobid_url,
        "endpoint": f"{_normalize_grobid_url(grobid_url)}/api/processHeaderDocument",
        "safety_defaults": _safety_defaults(),
    }


def _write_low_level_outputs(
    output_dir: Path,
    arxiv_id: str,
    packet: dict[str, Any],
    tei_text: str,
) -> None:
    if tei_text:
        _atomic_write_bytes(output_dir / "tei" / f"{arxiv_id}.tei.xml", tei_text.encode("utf-8"))
    _atomic_write_json(output_dir / "per-pdf" / f"{arxiv_id}.json", packet)


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


def _packet_for_dry_run(entry: dict[str, Any], *, grobid_url: str) -> dict[str, Any]:
    packet = _packet_base(entry, grobid_url=grobid_url)
    packet.update(_empty_packet_metrics())
    packet.update(
        {
            "status": "grobid_unavailable",
            "low_quality_source": False,
            "m022_repair_candidate": False,
            "http_status": None,
            "attempts": 0,
            "duration_ms": 0,
            "bytes": 0,
            "error": "dry_run_skipped_grobid_call",
            "tei_path": None,
            "sha256_actual": None,
            "sha256_matches_manifest": None,
        }
    )
    return packet


def _packet_from_probe_result(
    entry: dict[str, Any],
    *,
    grobid_url: str,
    pdf_path: Path,
    probe_result: dict[str, Any],
    attempts: int,
    sha256_actual: str | None,
) -> tuple[dict[str, Any], str]:
    tei_text = str(probe_result.get("tei_text") or "")
    metrics = _extract_tei_metrics(tei_text)
    status = "success"
    error = probe_result.get("error")
    if error:
        error_text = str(error)
        status = _classify_exception(urllib.error.URLError(error_text))
        if error_text.startswith("HTTPError:"):
            status = "blocked"
        elif error_text.startswith("TimeoutError:"):
            status = "timeout"
    elif _low_quality_source_criteria(metrics):
        status = "low_quality_source"

    arxiv_id = entry.get("arxiv_id") or entry.get("article_key") or pdf_path.stem
    packet = _packet_base(entry, grobid_url=grobid_url)
    packet.update(metrics)
    packet.update(
        {
            "status": status,
            "low_quality_source": status == "low_quality_source",
            "m022_repair_candidate": status == "low_quality_source",
            "http_status": probe_result.get("http_status"),
            "attempts": attempts,
            "duration_ms": probe_result.get("duration_ms") or 0,
            "bytes": probe_result.get("bytes") or 0,
            "error": error,
            "tei_path": str(Path("tei") / f"{arxiv_id}.tei.xml") if tei_text else None,
            "sha256_actual": sha256_actual,
            "sha256_matches_manifest": sha256_actual == entry.get("sha256")
            if entry.get("sha256")
            else None,
        }
    )
    return packet, tei_text


def _probe_one_entry(
    entry: dict[str, Any],
    *,
    output_dir: Path,
    grobid_url: str,
    max_retries: int,
    timeout: int,
    dry_run: bool,
) -> dict[str, Any]:
    arxiv_id = (
        entry.get("arxiv_id")
        or entry.get("article_key")
        or Path(str(entry.get("path", "unknown"))).stem
    )
    pdf_path = Path(str(entry.get("path", "")))
    if dry_run:
        packet = _packet_for_dry_run(entry, grobid_url=grobid_url)
        _write_low_level_outputs(output_dir, str(arxiv_id), packet, "")
        return packet
    if not pdf_path.exists():
        packet = _packet_for_missing_pdf(entry, grobid_url=grobid_url, pdf_path=pdf_path)
        _write_low_level_outputs(output_dir, str(arxiv_id), packet, "")
        return packet

    sha256_actual = _sha256(pdf_path)
    endpoint = f"{_normalize_grobid_url(grobid_url)}/api/processHeaderDocument"
    attempts = max(1, max_retries)
    last_result: dict[str, Any] | None = None
    for attempt in range(1, attempts + 1):
        last_result = _probe_grobid_pdf(pdf_path, endpoint, timeout)
        if not last_result.get("error") and last_result.get("tei_text"):
            packet, tei_text = _packet_from_probe_result(
                entry,
                grobid_url=grobid_url,
                pdf_path=pdf_path,
                probe_result=last_result,
                attempts=attempt,
                sha256_actual=sha256_actual,
            )
            _write_low_level_outputs(output_dir, str(arxiv_id), packet, tei_text)
            return packet
        if attempt < attempts:
            time.sleep(min(2 ** (attempt - 1), 5))

    assert last_result is not None
    packet, tei_text = _packet_from_probe_result(
        entry,
        grobid_url=grobid_url,
        pdf_path=pdf_path,
        probe_result=last_result,
        attempts=attempts,
        sha256_actual=sha256_actual,
    )
    if packet["status"] not in AGGREGATE_STATUSES:
        packet["status"] = "blocked"
    _write_low_level_outputs(output_dir, str(arxiv_id), packet, tei_text)
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
        counts[str(packet["status"])] = counts.get(str(packet["status"]), 0) + 1
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
        "grobid_unavailable_count": counts.get("grobid_unavailable", 0),
        "network_error_count": counts.get("network_error", 0),
        "timeout_count": counts.get("timeout", 0),
        "per_pdf_statuses": {packet["arxiv_id"]: packet["status"] for packet in packets},
        "safety_defaults": _safety_defaults(),
    }


def probe_grobid_only(
    corpus_manifest_path: Path,
    output_dir: Path,
    *,
    grobid_url: str = DEFAULT_GROBID_URL,
    max_retries: int = DEFAULT_MAX_RETRIES,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
    dry_run: bool = False,
) -> dict[str, Any]:
    """Run the GROBID-only probe and emit per-PDF packets plus summary.json."""
    entries = _load_corpus_manifest(corpus_manifest_path)
    normalized_url = _normalize_grobid_url(grobid_url)
    output_dir.mkdir(parents=True, exist_ok=True)
    packets = [
        _probe_one_entry(
            entry,
            output_dir=output_dir,
            grobid_url=normalized_url,
            max_retries=max_retries,
            timeout=timeout,
            dry_run=dry_run,
        )
        for entry in entries
    ]
    summary = _build_summary(
        corpus_manifest_path=corpus_manifest_path,
        output_dir=output_dir,
        grobid_url=normalized_url,
        packets=packets,
    )
    _atomic_write_json(output_dir / "summary.json", summary)
    return summary


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-manifest", type=Path, default=DEFAULT_CORPUS_MANIFEST)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--grobid-url", default=DEFAULT_GROBID_URL)
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--dry-run", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    summary = probe_grobid_only(
        args.corpus_manifest,
        args.output_dir,
        grobid_url=args.grobid_url,
        max_retries=args.max_retries,
        timeout=args.timeout,
        dry_run=args.dry_run,
    )
    sys.stdout.write(json.dumps(summary, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
