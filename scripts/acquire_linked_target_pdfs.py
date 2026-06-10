#!/usr/bin/env python3
"""Bounded local PDF acquisition for linked target records (M054 / M051-aaw9j7).

Per M045 next gate: bounded local PDF acquisition for the five linked target
records. M044 lesson: only 1/6 had local PDF, so 0-5 successful acquisitions
is expected; the goal is to record explicit statuses per record, not to
maximize successful downloads.

Per M048 patterns-review 01 (ActiveGraph-style bounded operations):

- bounded retry: max 3 attempts with exponential backoff
- bounded timeout: 30s per HTTP request
- bounded concurrency: max_workers=2 (NOT distributed)
- fail-closed: explicit statuses per record (acquired / blocked /
  low_quality_source / network_error)
- content-addressed logs: artifacts/m054-pdf-acquisition/acquisition-log.json

This script never blocks on unknown status — every record has an explicit
verdict. It does NOT write to any GraphDB.

Usage:
    uv run python scripts/acquire_linked_target_pdfs.py
    uv run python scripts/acquire_linked_target_pdfs.py --target-subset artifacts/m054-pdf-acquisition/target-subset.json
    uv run python scripts/acquire_linked_target_pdfs.py --max-workers 2
    uv run python scripts/acquire_linked_target_pdfs.py --dry-run   # do not download, just probe URLs
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

DEFAULT_TARGET_SUBSET = Path("artifacts/m054-pdf-acquisition/target-subset.json")
DEFAULT_LOG_PATH = Path("artifacts/m054-pdf-acquisition/acquisition-log.json")
DEFAULT_MAX_RETRIES = 3
DEFAULT_TIMEOUT_SECONDS = 30
DEFAULT_BACKOFF_BASE_SECONDS = 2.0
DEFAULT_MAX_WORKERS = 2

VALID_STATUSES = {"acquired", "blocked", "low_quality_source", "network_error", "dry_run"}


def _classify_exception(exc: BaseException) -> str:
    """Classify a download exception into one of our valid statuses."""
    # HTTPError is a subclass of URLError, so check it first.
    if isinstance(exc, urllib.error.HTTPError):
        # arXiv returns 404 for missing versions, 403 for rate limit
        if 400 <= exc.code < 500:
            return "blocked"
        return "network_error"
    if isinstance(exc, (urllib.error.URLError, TimeoutError, ConnectionError)):
        return "network_error"
    return "network_error"


def _head_probe(url: str, *, timeout: int = 10) -> tuple[str, int, dict[str, str]]:
    """Probe URL via HEAD request. Returns (status, http_code, headers)."""
    request = urllib.request.Request(url, method="HEAD", headers={"User-Agent": "daily-archive-acquire/1.0"})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return ("ok", response.status, dict(response.headers))
    except urllib.error.HTTPError as exc:
        return ("http_error", exc.code, dict(exc.headers) if exc.headers else {})
    except (urllib.error.URLError, TimeoutError, ConnectionError) as exc:
        return ("network_error", 0, {"error": str(exc)})


def _download_with_retry(
    url: str,
    dest_path: Path,
    *,
    max_retries: int = DEFAULT_MAX_RETRIES,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
    backoff_base: float = DEFAULT_BACKOFF_BASE_SECONDS,
) -> dict[str, Any]:
    """Download URL to dest_path with bounded retry. Returns attempt summary."""
    attempt_log: list[dict[str, Any]] = []
    last_exc: BaseException | None = None

    for attempt in range(1, max_retries + 1):
        attempt_started = datetime.datetime.now(tz=datetime.timezone.utc).isoformat()
        try:
            request = urllib.request.Request(
                url,
                headers={"User-Agent": "daily-archive-acquire/1.0"},
            )
            with urllib.request.urlopen(request, timeout=timeout) as response:
                body = response.read()
                status = response.status
                headers = dict(response.headers)
            attempt_log.append(
                {
                    "attempt": attempt,
                    "started_at": attempt_started,
                    "http_status": status,
                    "bytes": len(body),
                    "outcome": "ok",
                }
            )
            # Atomic write to dest_path.
            dest_path.parent.mkdir(parents=True, exist_ok=True)
            tmp_path = dest_path.with_suffix(dest_path.suffix + ".tmp")
            tmp_path.write_bytes(body)
            tmp_path.replace(dest_path)
            sha256 = hashlib.sha256(body).hexdigest()
            return {
                "status": "acquired",
                "http_status": status,
                "bytes": len(body),
                "sha256": sha256,
                "content_type": headers.get("Content-Type", ""),
                "attempts": attempt_log,
            }
        except BaseException as exc:  # noqa: BLE001 - we re-classify
            attempt_log.append(
                {
                    "attempt": attempt,
                    "started_at": attempt_started,
                    "exception": type(exc).__name__,
                    "exception_message": str(exc)[:500],
                    "outcome": "failed",
                }
            )
            last_exc = exc
            if attempt < max_retries:
                time.sleep(backoff_base ** attempt)

    classified = _classify_exception(last_exc) if last_exc else "network_error"
    return {
        "status": classified,
        "attempts": attempt_log,
        "last_exception": {
            "type": type(last_exc).__name__ if last_exc else None,
            "message": str(last_exc)[:500] if last_exc else None,
        },
    }


def _probe_only(url: str, *, timeout: int = 10) -> dict[str, Any]:
    """HEAD probe only, no download. Status: dry_run."""
    status, http_code, headers = _head_probe(url, timeout=timeout)
    return {
        "status": "dry_run",
        "probe_status": status,
        "probe_http_code": http_code,
        "probe_content_type": headers.get("Content-Type", ""),
        "probe_content_length": headers.get("Content-Length", ""),
    }


def _process_record(
    record: dict[str, Any],
    *,
    storage_root: Path,
    max_retries: int,
    timeout: int,
    dry_run: bool,
) -> dict[str, Any]:
    """Process a single record. Returns log entry with explicit status."""
    article_key = record["article_key"]
    url = record["expected_arxiv_url"]
    log_entry: dict[str, Any] = {
        "article_key": article_key,
        "category": record.get("category"),
        "url": url,
        "started_at": datetime.datetime.now(tz=datetime.timezone.utc).isoformat(),
    }

    if dry_run:
        log_entry.update(_probe_only(url, timeout=10))
        return log_entry

    # Skip if local PDF already present.
    expected_local = Path(record["expected_local_pdf_path"])
    if expected_local.exists():
        log_entry.update(
            {
                "status": "acquired",
                "bytes": expected_local.stat().st_size,
                "sha256": hashlib.sha256(expected_local.read_bytes()).hexdigest(),
                "local_path": str(expected_local),
                "note": "local_pdf_already_present",
            }
        )
        return log_entry

    result = _download_with_retry(
        url,
        expected_local,
        max_retries=max_retries,
        timeout=timeout,
    )
    log_entry.update(result)
    log_entry["local_path"] = str(expected_local)
    log_entry["completed_at"] = datetime.datetime.now(tz=datetime.timezone.utc).isoformat()
    return log_entry


def acquire_all(
    target_subset_path: Path,
    *,
    log_path: Path,
    max_retries: int = DEFAULT_MAX_RETRIES,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
    max_workers: int = DEFAULT_MAX_WORKERS,
    dry_run: bool = False,
) -> dict[str, Any]:
    """Run bounded acquisition over all records in target-subset.json."""
    if not target_subset_path.exists():
        raise FileNotFoundError(f"target-subset.json not found: {target_subset_path}")

    target_subset = json.loads(target_subset_path.read_text(encoding="utf-8"))
    records = target_subset.get("records", [])
    storage_root = Path("data")

    started_at = datetime.datetime.now(tz=datetime.timezone.utc).isoformat()
    log_entries: list[dict[str, Any]] = []

    if max_workers <= 1:
        for record in records:
            log_entries.append(
                _process_record(
                    record,
                    storage_root=storage_root,
                    max_retries=max_retries,
                    timeout=timeout,
                    dry_run=dry_run,
                )
            )
    else:
        # Bounded ThreadPoolExecutor (network-bound; ThreadPool is fine).
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = {
                executor.submit(
                    _process_record,
                    record,
                    storage_root=storage_root,
                    max_retries=max_retries,
                    timeout=timeout,
                    dry_run=dry_run,
                ): record
                for record in records
            }
            for future in futures:
                log_entries.append(future.result())

    completed_at = datetime.datetime.now(tz=datetime.timezone.utc).isoformat()

    # Aggregate stats.
    counts: dict[str, int] = {status: 0 for status in VALID_STATUSES}
    for entry in log_entries:
        counts[entry.get("status", "unknown")] = counts.get(entry.get("status", "unknown"), 0) + 1

    log_payload = {
        "schema_version": "m054-pdf-acquisition-log.v1",
        "target_subset_path": str(target_subset_path),
        "started_at": started_at,
        "completed_at": completed_at,
        "max_retries": max_retries,
        "timeout_seconds": timeout,
        "max_workers": max_workers,
        "dry_run": dry_run,
        "counts": counts,
        "entries": log_entries,
    }

    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_path.write_text(
        json.dumps(log_payload, indent=2, sort_keys=True, ensure_ascii=False),
        encoding="utf-8",
    )

    return log_payload


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-subset", type=Path, default=DEFAULT_TARGET_SUBSET)
    parser.add_argument("--log", type=Path, default=DEFAULT_LOG_PATH)
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--max-workers", type=int, default=DEFAULT_MAX_WORKERS)
    parser.add_argument("--dry-run", action="store_true", help="probe URLs only, do not download")
    args = parser.parse_args()

    try:
        log = acquire_all(
            args.target_subset,
            log_path=args.log,
            max_retries=args.max_retries,
            timeout=args.timeout,
            max_workers=args.max_workers,
            dry_run=args.dry_run,
        )
    except FileNotFoundError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    counts = log["counts"]
    print(
        f"M054 acquisition log written to {args.log}: "
        f"acquired={counts.get('acquired', 0)}, "
        f"blocked={counts.get('blocked', 0)}, "
        f"low_quality_source={counts.get('low_quality_source', 0)}, "
        f"network_error={counts.get('network_error', 0)}, "
        f"dry_run={counts.get('dry_run', 0)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
