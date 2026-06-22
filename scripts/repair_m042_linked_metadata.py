#!/usr/bin/env python3
"""Repair/report M041 reference-linked arXiv metadata without graph writes.

The command is intentionally metadata-only. It preserves M041 corpus semantics,
keeps linked-from evidence, and produces M042 repair reports. When a linked
article record already has fetched identity metadata, the command records that
no repair was needed. When metadata is missing/deferred, it retries the arXiv
Atom API with bounded timeout/retries and either updates the article metadata or
persists an explicit deferred reason.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "artifacts" / "m041-mixed-connectivity-smoke" / "manifest.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m042-linked-metadata-readiness"
ARXIV_NS = {"atom": "http://www.w3.org/2005/Atom", "arxiv": "http://arxiv.org/schemas/atom"}
NETWORK_TIMEOUT_SECONDS = 60
NETWORK_RETRIES = 3
USER_AGENT = "daily-archive-m042-linked-metadata-repair/0.1"
FALSE_SAFETY_KEYS = (
    "graph_write_allowed",
    "import_eligible",
    "production_import_attempted",
    "promotion_allowed",
)


@dataclass(frozen=True)
class ArxivMetadata:
    arxiv_id: str
    title: str
    summary: str
    primary_category: str
    published: str
    updated: str
    abs_url: str
    pdf_url: str


FetchBytes = Callable[[str], bytes]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def normalize_arxiv_id(value: str) -> str:
    return value.strip().removeprefix("arXiv:").split("v", 1)[0]


def article_path_from_ref(article_ref: str) -> Path:
    if not article_ref.startswith("artifact:"):
        raise ValueError(f"unsupported article_ref: {article_ref}")
    path = ROOT / article_ref.removeprefix("artifact:")
    resolved = path.resolve()
    root = ROOT.resolve()
    if root not in resolved.parents and resolved != root:
        raise ValueError(f"article_ref escapes repository: {article_ref}")
    return path


def is_reference_linked(entry: dict[str, Any]) -> bool:
    return entry.get("m041_category") == "reference_linked"


def linked_from_count(entry: dict[str, Any], article: dict[str, Any]) -> int:
    linked_from = (
        entry.get("linked_from") or article.get("connectivity_smoke", {}).get("linked_from") or []
    )
    if isinstance(linked_from, list):
        return len(linked_from)
    return 1 if linked_from else 0


def identity_is_fetched(article: dict[str, Any]) -> bool:
    identity = article.get("identity") if isinstance(article.get("identity"), dict) else {}
    return bool(
        identity.get("title")
        and identity.get("canonical_url")
        and identity.get("metadata_status") == "fetched"
    )


def fetch_url(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    last_error: BaseException | None = None
    for attempt in range(NETWORK_RETRIES):
        try:
            with urllib.request.urlopen(request, timeout=NETWORK_TIMEOUT_SECONDS) as response:
                return response.read()
        except TimeoutError as exc:
            last_error = exc
        except (urllib.error.HTTPError, urllib.error.URLError, OSError) as exc:
            last_error = exc
        if attempt < NETWORK_RETRIES - 1:
            time.sleep(2 * (attempt + 1))
    if last_error is not None:
        raise last_error
    raise RuntimeError("unreachable fetch_url state")


def fetch_arxiv_metadata(arxiv_id: str, *, fetcher: FetchBytes = fetch_url) -> ArxivMetadata:
    query = urllib.parse.urlencode({"id_list": normalize_arxiv_id(arxiv_id)})
    url = f"https://export.arxiv.org/api/query?{query}"
    return parse_arxiv_atom(fetcher(url), arxiv_id=arxiv_id)


def parse_arxiv_atom(payload: bytes, *, arxiv_id: str) -> ArxivMetadata:
    root = ET.fromstring(payload)
    entry = root.find("atom:entry", ARXIV_NS)
    if entry is None:
        raise ValueError(f"arXiv metadata not found for {arxiv_id}")

    def text(name: str) -> str:
        node = entry.find(f"atom:{name}", ARXIV_NS)
        return " ".join((node.text or "").split()) if node is not None else ""

    primary = entry.find("arxiv:primary_category", ARXIV_NS)
    primary_category = primary.attrib.get("term", "") if primary is not None else ""
    abs_url = f"https://arxiv.org/abs/{normalize_arxiv_id(arxiv_id)}"
    pdf_url = f"https://arxiv.org/pdf/{normalize_arxiv_id(arxiv_id)}"
    for link in entry.findall("atom:link", ARXIV_NS):
        href = link.attrib.get("href", "")
        if link.attrib.get("rel") == "alternate" and href:
            abs_url = href
        if link.attrib.get("title") == "pdf" and href:
            pdf_url = href
    return ArxivMetadata(
        arxiv_id=normalize_arxiv_id(arxiv_id),
        title=text("title"),
        summary=text("summary"),
        primary_category=primary_category or "unknown",
        published=text("published"),
        updated=text("updated"),
        abs_url=abs_url,
        pdf_url=pdf_url,
    )


def apply_metadata(article: dict[str, Any], metadata: ArxivMetadata) -> dict[str, Any]:
    updated = dict(article)
    identity = dict(updated.get("identity") if isinstance(updated.get("identity"), dict) else {})
    identity.update(
        {
            "arxiv_id": metadata.arxiv_id,
            "canonical_url": metadata.abs_url,
            "metadata_status": "fetched",
            "pdf_url": metadata.pdf_url,
            "published": metadata.published,
            "title": metadata.title,
            "updated": metadata.updated,
        }
    )
    updated["identity"] = identity
    connectivity = dict(
        updated.get("connectivity_smoke")
        if isinstance(updated.get("connectivity_smoke"), dict)
        else {}
    )
    connectivity["metadata_status"] = "fetched"
    connectivity["metadata_only"] = True
    updated["connectivity_smoke"] = connectivity
    updated.setdefault("safety_flags", {})
    return updated


def repair_linked_metadata(
    *,
    manifest_path: Path,
    output_dir: Path,
    no_network: bool = False,
    write_records: bool = True,
    fetcher: FetchBytes = fetch_url,
) -> dict[str, Any]:
    manifest = load_json(manifest_path)
    articles = manifest.get("articles")
    if not isinstance(articles, list):
        raise ValueError("manifest articles must be a list")
    category_counts: dict[str, int] = {}
    for entry in articles:
        key = str(entry.get("m041_category"))
        category_counts[key] = category_counts.get(key, 0) + 1
    safety_flags = (
        manifest.get("safety_flags") if isinstance(manifest.get("safety_flags"), dict) else {}
    )
    if any(safety_flags.get(key) is not False for key in FALSE_SAFETY_KEYS):
        raise ValueError("M041 manifest safety flags must remain false")

    records: list[dict[str, Any]] = []
    status_counts: dict[str, int] = {}
    for entry in articles:
        if not is_reference_linked(entry):
            continue
        article_path = article_path_from_ref(str(entry.get("article_ref", "")))
        article = load_json(article_path)
        arxiv_id = str(
            entry.get("article_key") or article.get("identity", {}).get("arxiv_id") or ""
        )
        if not arxiv_id:
            raise ValueError(f"missing arxiv id for {article_path}")
        linked_count = linked_from_count(entry, article)
        if linked_count < 1:
            raise ValueError(f"reference-linked entry lacks linked_from evidence: {arxiv_id}")

        before_status = "fetched" if identity_is_fetched(article) else "deferred"
        action = "already_fetched"
        deferred_reason = ""
        after_status = before_status
        if before_status != "fetched":
            if no_network:
                action = "deferred"
                after_status = "deferred"
                deferred_reason = "network_disabled"
            else:
                try:
                    metadata = fetch_arxiv_metadata(arxiv_id, fetcher=fetcher)
                    article = apply_metadata(article, metadata)
                    if write_records:
                        write_json(article_path, article)
                    action = "repaired"
                    after_status = "fetched"
                except Exception as exc:  # noqa: BLE001 - persisted diagnostic, not suppression
                    action = "deferred"
                    after_status = "deferred"
                    deferred_reason = f"{type(exc).__name__}: {exc}"

        status_counts[after_status] = status_counts.get(after_status, 0) + 1
        records.append(
            {
                "article_key": normalize_arxiv_id(arxiv_id),
                "article_ref": entry.get("article_ref"),
                "catalog_path": entry.get("catalog_path"),
                "linked_from_count": linked_count,
                "before_status": before_status,
                "after_status": after_status,
                "action": action,
                "deferred_reason": deferred_reason,
            }
        )

    report = {
        "source_manifest": str(manifest_path),
        "article_count": manifest.get("article_count"),
        "category_counts": category_counts,
        "reference_linked_count": len(records),
        "status_counts": status_counts,
        "records": records,
        "safety_flags": safety_flags,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    write_json(output_dir / "repair-report.json", report)
    write_text(output_dir / "repair-report.md", render_report(report))
    return report


def render_report(report: dict[str, Any]) -> str:
    lines = [
        "# M042 Linked Metadata Repair Report",
        "",
        f"- Source manifest: `{report['source_manifest']}`",
        f"- Article count: {report['article_count']}",
        f"- Category counts: {report['category_counts']}",
        f"- Reference-linked records: {report['reference_linked_count']}",
        f"- Status counts: {report['status_counts']}",
        "- Graph writes: disabled",
        "- Production import: disabled",
        "- Fact promotion: disabled",
        "",
        "| Article | Before | After | Action | Linked-from evidence | Deferred reason |",
        "|---|---|---|---|---:|---|",
    ]
    for record in report["records"]:
        lines.append(
            "| {article_key} | {before_status} | {after_status} | {action} | {linked_from_count} | {deferred_reason} |".format(
                **record
            )
        )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument(
        "--no-network",
        action="store_true",
        help="Do not call arXiv; persist deferred diagnostics instead",
    )
    parser.add_argument(
        "--no-write-records",
        action="store_true",
        help="Write reports only, even when metadata could be repaired",
    )
    args = parser.parse_args()
    report = repair_linked_metadata(
        manifest_path=args.manifest,
        output_dir=args.output_dir,
        no_network=args.no_network,
        write_records=not args.no_write_records,
    )
    sys.stdout.write(
        "m042 linked metadata repair complete: "
        f"reference_linked={report['reference_linked_count']} status_counts={report['status_counts']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
