#!/usr/bin/env python3
"""Parser availability probe for M055 parser benchmark S01.

Checks a live GROBID endpoint and the local OpenDataLoader Python import. The
optional OpenDataLoader install attempt is best-effort and never raises.
"""

from __future__ import annotations

import argparse
import datetime as dt
import importlib.metadata
import importlib.util
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055-parser-benchmark.availability.v1"
DEFAULT_GROBID_URL = "http://localhost:8070"
DEFAULT_OUTPUT = Path("artifacts/m055-parser-benchmark/availability.json")
USER_AGENT = "daily-archive-m055-availability/1.0"
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}
LAST_INSTALL_ATTEMPT: dict[str, Any] | None = None


def _utc_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def _isalive_url(endpoint: str) -> str:
    trimmed = endpoint.rstrip("/")
    if trimmed.endswith("/api/isalive"):
        return trimmed
    return f"{trimmed}/api/isalive"


def _parse_grobid_version(body: str) -> str | None:
    stripped = body.strip()
    if not stripped or stripped.lower() == "true":
        return None
    try:
        parsed = json.loads(stripped)
    except json.JSONDecodeError:
        return stripped if "version" in stripped.lower() else None
    if isinstance(parsed, dict):
        version = parsed.get("version") or parsed.get("grobid_version")
        return str(version) if version else None
    return None


def _probe_grobid(endpoint: str, timeout: int = 5) -> dict[str, Any]:
    url = _isalive_url(endpoint)
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    started = time.perf_counter()
    http_status: int | None = None
    body = ""
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            http_status = getattr(response, "status", None) or response.getcode()
            body = response.read().decode("utf-8", errors="replace")
        available = http_status == 200 and body.strip().lower() == "true"
        error = None
    except urllib.error.HTTPError as exc:
        http_status = exc.code
        available = False
        error = str(exc)
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        available = False
        error = str(exc)
    latency_ms = int((time.perf_counter() - started) * 1000)
    return {
        "available": available,
        "http_status": http_status,
        "version": _parse_grobid_version(body),
        "latency_ms": latency_ms,
        "endpoint": endpoint,
        "isalive_url": url,
        "error": error,
    }


def _metadata_version() -> str | None:
    for distribution_name in ("opendataloader-pdf", "opendataloader_pdf"):
        try:
            return importlib.metadata.version(distribution_name)
        except importlib.metadata.PackageNotFoundError:
            continue
    return None


def _probe_opendataloader() -> dict[str, Any]:
    spec = importlib.util.find_spec("opendataloader_pdf")
    if spec is None:
        return {
            "installed": False,
            "version": _metadata_version(),
            "import_error": "module opendataloader_pdf is not importable",
        }
    return {
        "installed": True,
        "version": _metadata_version(),
        "import_error": None,
    }


def _tail(text: str, limit: int = 4000) -> str:
    return text[-limit:] if len(text) > limit else text


def _attempt_install_opendataloader() -> bool:
    global LAST_INSTALL_ATTEMPT
    commands = [
        ["uv", "pip", "install", "--system", "opendataloader-pdf"],
        [sys.executable, "-m", "pip", "install", "--user", "opendataloader-pdf"],
    ]
    attempts: list[dict[str, Any]] = []
    for command in commands:
        started = time.perf_counter()
        try:
            completed = subprocess.run(
                command,
                check=False,
                capture_output=True,
                text=True,
                timeout=120,
            )
            attempt = {
                "command": " ".join(command),
                "exit_code": completed.returncode,
                "duration_ms": int((time.perf_counter() - started) * 1000),
                "stdout_tail": _tail(completed.stdout),
                "stderr_tail": _tail(completed.stderr),
            }
        except (OSError, subprocess.SubprocessError) as exc:
            attempt = {
                "command": " ".join(command),
                "exit_code": None,
                "duration_ms": int((time.perf_counter() - started) * 1000),
                "stdout_tail": "",
                "stderr_tail": str(exc),
            }
        attempts.append(attempt)
        if attempt["exit_code"] == 0:
            LAST_INSTALL_ATTEMPT = {"attempted": True, "succeeded": True, "attempts": attempts}
            return True
    LAST_INSTALL_ATTEMPT = {"attempted": True, "succeeded": False, "attempts": attempts}
    return False


def probe_availability(grobid_url: str, auto_install: bool = False) -> dict[str, Any]:
    global LAST_INSTALL_ATTEMPT
    LAST_INSTALL_ATTEMPT = None
    grobid = _probe_grobid(grobid_url)
    opendataloader = _probe_opendataloader()
    if auto_install and not opendataloader["installed"]:
        _attempt_install_opendataloader()
        opendataloader = _probe_opendataloader()
    install_attempt = LAST_INSTALL_ATTEMPT or {
        "attempted": False,
        "succeeded": False,
        "attempts": [],
    }
    opendataloader["install_attempt"] = install_attempt

    if not grobid["available"]:
        status = "grobid_unavailable"
    elif not opendataloader["installed"]:
        status = "opendataloader_missing"
    else:
        status = "ok"

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "status": status,
        "safety": dict(SAFETY_DEFAULTS),
        "grobid": grobid,
        "opendataloader": opendataloader,
    }


def write_availability(payload: dict[str, Any], output_path: Path = DEFAULT_OUTPUT) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output_path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--grobid-url", default=DEFAULT_GROBID_URL)
    parser.add_argument("--auto-install", action="store_true", default=False)
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT))
    args = parser.parse_args(argv)

    payload = probe_availability(args.grobid_url, auto_install=args.auto_install)
    write_availability(payload, Path(args.output))
    return 0 if payload["grobid"]["available"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
