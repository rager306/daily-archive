#!/usr/bin/env python3
"""Recover arXiv PDFs into the canonical article catalog.

Layout:
  data/article_catalog/article_catalog/arxiv/<category>/<id>/source/<id>.pdf

Network access is fail-closed: callers must pass ``--allow-network``. Each result
is checksum-bound and the report path is caller-controlled so the same tool can
serve any bounded corpus wave without project-specific artifact directories.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import TypedDict

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "data" / "article_catalog" / "article_catalog" / "arxiv"
DEFAULT_REPORT = ROOT / "artifacts" / "source_recovery" / "download_results.json"
API = "https://export.arxiv.org/api/query?id_list={ids}"
ATOM = "{http://www.w3.org/2005/Atom}"
ARXIV_NS = "{http://arxiv.org/schemas/atom}"
UA = "daily-archive/source-recovery (local research bot)"
MODERN_ID = re.compile(r"^\d{4}\.\d{4,5}$")
LEGACY_ID = re.compile(r"^[a-z][a-z0-9.-]+/\d{7}$", re.IGNORECASE)
MAX_TEX_ARCHIVE_BYTES = 100 << 20
MAX_TEX_MEMBERS = 1000
MAX_TEX_MEMBER_BYTES = 20 << 20
MAX_TEX_TOTAL_BYTES = 100 << 20


class PdfArtifact(TypedDict):
    status: str
    path: str
    sha256: str
    byte_size: int
    source_url: str | None


def normalize_arxiv_id(value: str) -> str:
    """Return a canonical versionless arXiv ID or reject unsafe input."""
    candidate = value.strip()
    candidate = re.sub(r"^arxiv:\s*", "", candidate, flags=re.IGNORECASE)
    candidate = re.sub(r"^https?://(?:export\.)?arxiv\.org/(?:abs|pdf)/", "", candidate)
    candidate = re.sub(r"\.pdf$", "", candidate, flags=re.IGNORECASE)
    candidate = re.sub(r"v\d+$", "", candidate, flags=re.IGNORECASE)
    if not (MODERN_ID.fullmatch(candidate) or LEGACY_ID.fullmatch(candidate)):
        raise ValueError(f"invalid arXiv id: {value!r}")
    return candidate


def normalize_category(value: str) -> str:
    """Map an arXiv category to the canonical lowercase filesystem form."""
    normalized = value.strip().lower().replace(".", "-")
    return normalized if re.fullmatch(r"[a-z0-9-]+", normalized) else "unknown"


def report_path(path: Path) -> str:
    """Use repository-relative paths for durable evidence when possible."""
    resolved = path.resolve()
    try:
        return str(resolved.relative_to(ROOT))
    except ValueError:
        return str(resolved)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def inspect_pdf(path: Path) -> dict[str, str | int]:
    """Validate a durable PDF and return checksum-bound metadata."""
    byte_size = path.stat().st_size
    with path.open("rb") as handle:
        magic = handle.read(5)
    if byte_size < 1000 or magic != b"%PDF-":
        raise ValueError(f"invalid PDF artifact at {path} (bytes={byte_size})")
    return {"sha256": sha256_file(path), "byte_size": byte_size}


def inspect_tex_source(path: Path) -> dict[str, object]:
    """Inventory an arXiv source tar without extracting it; reject unsafe archives."""
    archive_bytes = path.stat().st_size
    if archive_bytes == 0 or archive_bytes > MAX_TEX_ARCHIVE_BYTES:
        raise ValueError(f"invalid TeX archive size at {path} (bytes={archive_bytes})")
    files: list[dict[str, object]] = []
    seen_names: set[str] = set()
    member_count = directory_count = tex_count = bib_count = 0
    total_bytes = 0
    with tarfile.open(path, mode="r:*") as archive:
        for member in archive.getmembers():
            member_count += 1
            if member_count > MAX_TEX_MEMBERS:
                raise ValueError(f"TeX archive member cap exceeded at {path}")
            name = member.name
            parts = name.split("/")
            if (
                not name or name.startswith(("/", "\\"))
                or any(part in {"", ".", ".."} for part in parts)
                or name.startswith("-")
                or any(not char.isprintable() for char in name)
                or "\\" in name or "\x00" in name
                or member.issym() or member.islnk()
                or member.isdev() or member.isfifo()
            ):
                raise ValueError(f"unsafe TeX archive member: {name!r}")
            if name in seen_names:
                raise ValueError(f"duplicate TeX archive member: {name!r}")
            seen_names.add(name)
            if member.isdir():
                directory_count += 1
                continue
            if not member.isfile():
                raise ValueError(f"unsupported TeX archive member: {name!r}")
            if member.size > MAX_TEX_MEMBER_BYTES:
                raise ValueError(f"TeX archive member too large: {name!r}")
            total_bytes += member.size
            if total_bytes > MAX_TEX_TOTAL_BYTES:
                raise ValueError(f"TeX archive expansion cap exceeded at {path}")
            handle = archive.extractfile(member)
            if handle is None:
                raise ValueError(f"unreadable TeX archive member: {name!r}")
            digest = hashlib.sha256()
            observed = 0
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                observed += len(chunk)
                if observed > member.size or observed > MAX_TEX_MEMBER_BYTES:
                    raise ValueError(f"TeX archive stream cap exceeded: {name!r}")
                digest.update(chunk)
            if observed != member.size:
                raise ValueError(f"TeX archive member size mismatch: {name!r}")
            suffix = Path(name).suffix.lower()
            tex_count += int(suffix == ".tex")
            bib_count += int(suffix == ".bib")
            files.append({
                "path": name, "byte_size": observed, "sha256": digest.hexdigest(),
                "suffix": suffix,
            })
    files.sort(key=lambda item: str(item["path"]))
    return {
        "sha256": sha256_file(path), "byte_size": archive_bytes,
        "member_count": member_count, "file_count": len(files),
        "directory_count": directory_count, "tex_file_count": tex_count,
        "bib_file_count": bib_count, "expanded_bytes": total_bytes,
        "files": files,
    }


def inspect_html(path: Path) -> dict[str, str | int]:
    """Validate a durable HTML article rather than accepting an HTTP error page."""
    byte_size = path.stat().st_size
    prefix = path.read_bytes()[:4096].lower()
    if byte_size < 1000 or (b"<html" not in prefix and b"<!doctype html" not in prefix):
        raise ValueError(f"invalid HTML artifact at {path} (bytes={byte_size})")
    return {"sha256": sha256_file(path), "byte_size": byte_size}


def find_local_versioned_pdf(catalog: Path, arxiv_id: str) -> Path | None:
    """Resolve one local versioned PDF; reject competing versions explicitly."""
    candidates = sorted(
        path for path in catalog.rglob(f"{arxiv_id}v*.pdf")
        if re.fullmatch(rf"{re.escape(arxiv_id)}v\d+", path.stem)
    )
    if not candidates:
        return None
    checksums = {sha256_file(path) for path in candidates}
    if len(checksums) != 1:
        raise RuntimeError(
            f"conflicting local PDF versions for {arxiv_id}: "
            + ", ".join(str(path) for path in candidates)
        )
    return candidates[-1]


def category_from_catalog_pdf(catalog: Path, source: Path) -> str:
    relative = source.relative_to(catalog)
    return normalize_category(relative.parts[0]) if len(relative.parts) >= 1 else "unknown"


def materialize_local_pdf(source: Path, destination: Path) -> PdfArtifact:
    source_metadata = inspect_pdf(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination_existed = destination.exists()
    if not destination_existed:
        temporary = destination.with_name(f".{destination.name}.{os.getpid()}.part")
        try:
            shutil.copyfile(source, temporary)
            inspect_pdf(temporary)
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)
    destination_metadata = inspect_pdf(destination)
    if destination_metadata["sha256"] != source_metadata["sha256"]:
        raise RuntimeError(f"local PDF checksum mismatch: {source} -> {destination}")
    return {
        "status": "skip" if destination_existed else "ok",
        "path": report_path(destination),
        "sha256": str(destination_metadata["sha256"]),
        "byte_size": int(destination_metadata["byte_size"]),
        "source_url": None,
    }


def http_get(url: str, timeout: int = 90) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "*/*"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read()


def fetch_meta_one(arxiv_id: str, retries: int = 6) -> dict[str, str]:
    """Fetch primary category and title; return an explicit fallback on failure."""
    url = API.format(ids=arxiv_id)
    last = "unknown"
    for attempt in range(retries):
        try:
            root = ET.fromstring(http_get(url, timeout=60))
            entry = root.find(f"{ATOM}entry")
            if entry is None:
                return {"category": "unknown", "title": "", "metadata_status": "not_found"}
            title_el = entry.find(f"{ATOM}title")
            title = re.sub(r"\s+", " ", (title_el.text or "").strip()) if title_el is not None else ""
            primary = entry.find(f"{ARXIV_NS}primary_category")
            category = "unknown"
            if primary is not None:
                category = normalize_category(primary.get("term") or "unknown")
            return {"category": category, "title": title, "metadata_status": "resolved"}
        except urllib.error.HTTPError as error:
            last = f"HTTP {error.code}"
            if error.code not in (429, 503):
                break
            time.sleep(8 * (attempt + 1))
        except Exception as error:  # network/XML boundary; retained in redacted report
            last = f"{type(error).__name__}: {error}"
            time.sleep(3 * (attempt + 1))
    return {"category": "unknown", "title": "", "metadata_status": f"fallback: {last}"}


def curl_download(url: str, destination: Path) -> tuple[bool, str]:
    if not shutil.which("curl"):
        return False, "curl not found"
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_name(f".{destination.name}.{os.getpid()}.part")
    command = [
        "curl", "-fsSL", "--retry", "5", "--retry-delay", "5",
        "--retry-all-errors", "--max-time", "300", "-A", UA,
        "-o", str(temporary), url,
    ]
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=330)
        if result.returncode != 0:
            return False, f"curl exit {result.returncode}: {(result.stderr or '')[:120]}"
        inspect_pdf(temporary)
        temporary.replace(destination)
        return True, "downloaded"
    except Exception as error:
        return False, f"{type(error).__name__}: {error}"
    finally:
        temporary.unlink(missing_ok=True)


def download_html(arxiv_id: str, destination: Path, *, allow_network: bool) -> PdfArtifact:
    """Download or verify canonical arXiv HTML with checksum evidence."""
    arxiv_id = normalize_arxiv_id(arxiv_id)
    if destination.exists():
        metadata = inspect_html(destination)
        return {
            "status": "skip", "path": report_path(destination),
            "sha256": str(metadata["sha256"]), "byte_size": int(metadata["byte_size"]),
            "source_url": None,
        }
    if not allow_network:
        raise PermissionError("network recovery requires explicit --allow-network")
    destination.parent.mkdir(parents=True, exist_ok=True)
    urls = [f"https://arxiv.org/html/{arxiv_id}", f"https://ar5iv.labs.arxiv.org/html/{arxiv_id}"]
    failures: list[str] = []
    for url in urls:
        temporary = destination.with_name(f".{destination.name}.{os.getpid()}.part")
        command = [
            "curl", "-fsSL", "--retry", "5", "--retry-delay", "5",
            "--retry-all-errors", "--max-time", "300", "-A", UA,
            "-o", str(temporary), url,
        ]
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=330)
            if result.returncode != 0:
                failures.append(f"{url}: curl exit {result.returncode}")
                continue
            metadata = inspect_html(temporary)
            temporary.replace(destination)
            return {
                "status": "ok", "path": report_path(destination),
                "sha256": str(metadata["sha256"]), "byte_size": int(metadata["byte_size"]),
                "source_url": url,
            }
        except Exception as error:
            failures.append(f"{url}: {type(error).__name__}: {error}")
        finally:
            temporary.unlink(missing_ok=True)
    raise RuntimeError("; ".join(failures))


def download_pdf(arxiv_id: str, destination: Path, *, allow_network: bool) -> PdfArtifact:
    """Return an idempotent checksum-bound PDF result."""
    arxiv_id = normalize_arxiv_id(arxiv_id)
    if destination.exists():
        metadata = inspect_pdf(destination)
        return {
            "status": "skip", "path": report_path(destination),
            "sha256": str(metadata["sha256"]), "byte_size": int(metadata["byte_size"]),
            "source_url": None,
        }
    if not allow_network:
        raise PermissionError("network recovery requires explicit --allow-network")

    urls = [
        f"https://export.arxiv.org/pdf/{arxiv_id}",
        f"https://arxiv.org/pdf/{arxiv_id}.pdf",
        f"https://arxiv.org/pdf/{arxiv_id}",
    ]
    failures: list[str] = []
    for url in urls:
        ok, message = curl_download(url, destination)
        if ok:
            metadata = inspect_pdf(destination)
            return {
                "status": "ok", "path": report_path(destination),
                "sha256": str(metadata["sha256"]), "byte_size": int(metadata["byte_size"]),
                "source_url": url,
            }
        failures.append(f"{url}: {message}")
        time.sleep(2)
    raise RuntimeError("; ".join(failures))


def download_tex_source(
    arxiv_id: str,
    destination: Path,
    inventory_path: Path,
    *,
    allow_network: bool,
) -> dict[str, object]:
    """Download/verify TeX source and atomically publish its safe inventory."""
    arxiv_id = normalize_arxiv_id(arxiv_id)
    status = "skip"
    source_url: str | None = None
    if not destination.exists():
        if not allow_network:
            raise PermissionError("network recovery requires explicit --allow-network")
        if not shutil.which("curl"):
            raise RuntimeError("curl not found")
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_name(f".{destination.name}.{os.getpid()}.part")
        command = [
            "curl", "-fsSL", "--retry", "5", "--retry-delay", "5",
            "--retry-all-errors", "--max-time", "300", "-A", UA,
            "-o", str(temporary), f"https://arxiv.org/src/{arxiv_id}",
        ]
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=330)
            if result.returncode != 0:
                raise RuntimeError(f"curl exit {result.returncode}: {(result.stderr or '')[:120]}")
            inspect_tex_source(temporary)
            temporary.replace(destination)
            status = "ok"
            source_url = f"https://arxiv.org/src/{arxiv_id}"
        finally:
            temporary.unlink(missing_ok=True)
    inventory = inspect_tex_source(destination)
    durable_inventory = {
        "schema_version": "arxiv-tex-source-inventory.v1",
        "article_id": arxiv_id,
        "import_eligible": False,
        "archive_path": report_path(destination),
        **inventory,
    }
    inventory_path.parent.mkdir(parents=True, exist_ok=True)
    temporary_inventory = inventory_path.with_suffix(inventory_path.suffix + ".tmp")
    temporary_inventory.write_text(json.dumps(durable_inventory, indent=2, sort_keys=True) + "\n")
    temporary_inventory.replace(inventory_path)
    return {
        "status": status,
        "path": report_path(destination),
        "inventory_path": report_path(inventory_path),
        "inventory_sha256": sha256_file(inventory_path),
        "source_url": source_url,
        **{key: value for key, value in inventory.items() if key != "files"},
    }


def parse_ids(values: list[str], ids_file: Path | None) -> list[str]:
    candidates = list(values)
    if ids_file:
        candidates.extend(
            line.strip() for line in ids_file.read_text().splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        )
    return list(dict.fromkeys(normalize_arxiv_id(value) for value in candidates))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ids", nargs="*", help="arXiv IDs or canonical arXiv URLs")
    parser.add_argument("--ids-file", type=Path)
    parser.add_argument("--catalog", type=Path, default=CATALOG)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--sleep", type=float, default=4.0, help="seconds between items")
    parser.add_argument("--skip-meta", action="store_true", help="store under category=unknown")
    parser.add_argument("--allow-network", action="store_true", help="authorize external fetches")
    parser.add_argument(
        "--include-tex-source", action="store_true",
        help="download and safely inventory the official arXiv source archive",
    )
    args = parser.parse_args()

    try:
        arxiv_ids = parse_ids(args.ids, args.ids_file)
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 2
    if not arxiv_ids:
        print("no ids", file=sys.stderr)
        return 2

    results: list[dict[str, object]] = []
    for index, arxiv_id in enumerate(arxiv_ids, 1):
        metadata = (
            {"category": "unknown", "title": "", "metadata_status": "skipped"}
            if args.skip_meta else fetch_meta_one(arxiv_id)
        )
        try:
            local_pdf = find_local_versioned_pdf(args.catalog, arxiv_id)
            category = (
                category_from_catalog_pdf(args.catalog, local_pdf)
                if local_pdf is not None else str(metadata["category"])
            )
            article_root = args.catalog / category / arxiv_id
            pdf_destination = article_root / "source" / f"{arxiv_id}.pdf"
            html_destination = article_root / "article.html"
            pdf = (
                materialize_local_pdf(local_pdf, pdf_destination)
                if local_pdf is not None
                else download_pdf(arxiv_id, pdf_destination, allow_network=args.allow_network)
            )
            html = download_html(arxiv_id, html_destination, allow_network=args.allow_network)
            representations: dict[str, object] = {"pdf": pdf, "html": html}
            if args.include_tex_source:
                representations["tex_source"] = download_tex_source(
                    arxiv_id,
                    article_root / "source" / f"{arxiv_id}.tar.gz",
                    article_root / "source" / f"{arxiv_id}.tex-inventory.json",
                    allow_network=args.allow_network,
                )
            result: dict[str, object] = {
                "id": arxiv_id,
                **metadata,
                "category": category,
                "status": "ok" if "ok" in (pdf["status"], html["status"]) else "skip",
                "representations": representations,
            }
        except Exception as error:
            result = {
                "id": arxiv_id, **metadata, "status": "fail", "representations": {},
                "error": f"{type(error).__name__}: {error}",
            }
        results.append(result)
        print(
            f"[{index}/{len(arxiv_ids)}] {arxiv_id} {result['status']} "
            f"category={result.get('category', metadata['category'])}"
        )
        if index != len(arxiv_ids):
            time.sleep(args.sleep)

    summary = {
        "schema_version": "arxiv-source-recovery-v4-pdf-html-tex",
        "network_authorized": args.allow_network,
        "catalog_root": report_path(args.catalog),
        "total": len(results),
        "ok": sum(result["status"] == "ok" for result in results),
        "skip": sum(result["status"] == "skip" for result in results),
        "fail": sum(result["status"] == "fail" for result in results),
        "results": results,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    temporary_report = args.report.with_suffix(args.report.suffix + ".tmp")
    temporary_report.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    temporary_report.replace(args.report)
    print(f"summary ok={summary['ok']} skip={summary['skip']} fail={summary['fail']} -> {args.report}")
    return 0 if summary["fail"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
