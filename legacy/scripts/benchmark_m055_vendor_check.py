#!/usr/bin/env python3
"""Vendor-source integrity check for M055 parser benchmark S01.

Checks the local vendor snapshots used by the parser benchmark. This script is
read-only: it does not mutate vendor trees, GraphDB, LadybugDB, or production
catalog data.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import subprocess
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055-parser-benchmark.vendor-check.v1"
DEFAULT_VENDOR_DIR = Path("/root/vendor-source")
DEFAULT_OUTPUT = Path("artifacts/m055-parser-benchmark/vendor-check.json")
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


def _utc_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def _has_named_file(path: Path, prefixes: tuple[str, ...]) -> bool:
    if not path.exists() or not path.is_dir():
        return False
    lowered_prefixes = tuple(prefix.lower() for prefix in prefixes)
    return any(
        child.is_file() and child.name.lower().startswith(lowered_prefixes)
        for child in path.iterdir()
    )


def _gitnexus_indexed(path: Path) -> bool:
    """Best-effort GitNexus index detection that never raises."""
    try:
        if (path / ".gitnexus").is_dir():
            return True
    except OSError:
        return False

    # Fallback for environments where the index is registered outside the
    # vendor checkout. The CLI may not exist; failures are intentionally ignored.
    commands = (
        ("gitnexus", "list-repos"),
        ("npx", "gitnexus", "list-repos"),
    )
    for command in commands:
        try:
            completed = subprocess.run(
                command,
                check=False,
                capture_output=True,
                text=True,
                timeout=5,
            )
        except (OSError, subprocess.SubprocessError):
            continue
        output = f"{completed.stdout}\n{completed.stderr}"
        if str(path) in output or path.name in output:
            return True
    return False


def _check_vendor_source(name: str, path: str) -> dict[str, Any]:
    vendor_path = Path(path)
    present = vendor_path.exists() and vendor_path.is_dir()
    result: dict[str, Any] = {
        "name": name,
        "present": present,
        "path": str(vendor_path),
        "has_changelog": _has_named_file(vendor_path, ("CHANGELOG",)),
        "has_readme": _has_named_file(vendor_path, ("README", "Readme")),
        "has_license": _has_named_file(vendor_path, ("LICENSE", "License")),
        "indexed": _gitnexus_indexed(vendor_path) if present else False,
        "status": "ok" if present else "missing",
    }
    return result


def check_grobid_vendor(vendor_dir: str = str(DEFAULT_VENDOR_DIR / "grobid")) -> dict[str, Any]:
    result = _check_vendor_source("grobid", vendor_dir)
    vendor_path = Path(vendor_dir)
    result["has_dockerfile"] = bool(
        result["present"] and (vendor_path / "Dockerfile.crf").is_file()
    )
    if result["present"] and not result["has_dockerfile"]:
        result["status"] = "missing_dockerfile"
    return result


def check_opendataloader_vendor(
    vendor_dir: str = str(DEFAULT_VENDOR_DIR / "opendataloader-pdf"),
) -> dict[str, Any]:
    result = _check_vendor_source("opendataloader-pdf", vendor_dir)
    result["has_dockerfile"] = False
    return result


def run_vendor_check(vendor_dir: str | Path = DEFAULT_VENDOR_DIR) -> dict[str, Any]:
    root = Path(vendor_dir)
    grobid = check_grobid_vendor(str(root / "grobid"))
    opendataloader = check_opendataloader_vendor(str(root / "opendataloader-pdf"))
    vendors = {"grobid": grobid, "opendataloader": opendataloader}
    all_present = all(vendor["present"] for vendor in vendors.values())
    all_indexed = all(vendor["indexed"] for vendor in vendors.values())
    all_required_files = grobid["has_dockerfile"] and all(
        vendor["has_readme"] and vendor["has_license"] and vendor["has_changelog"]
        for vendor in vendors.values()
    )
    status = "ok" if all_present and all_indexed and all_required_files else "incomplete"
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "vendor_dir": str(root),
        "status": status,
        "safety": dict(SAFETY_DEFAULTS),
        "vendors": vendors,
    }


def write_vendor_check(payload: dict[str, Any], output_path: Path = DEFAULT_OUTPUT) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output_path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vendor-dir", default=str(DEFAULT_VENDOR_DIR))
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT))
    args = parser.parse_args(argv)

    payload = run_vendor_check(args.vendor_dir)
    write_vendor_check(payload, Path(args.output))
    return 0 if payload["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
