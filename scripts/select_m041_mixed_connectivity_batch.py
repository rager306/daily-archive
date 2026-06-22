#!/usr/bin/env python3
"""Build a mixed 20-30 article manifest for the no-write connectivity smoke.

The script keeps the current normalized baseline, adds reference-linked arXiv
candidates when discoverable, and fills any remaining slots with fresh arXiv
metadata/source candidates. It writes only metadata records and arXiv abstract
page source artifacts; it never writes graph/import/promotion state.
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Any

# pyrefly: ignore [missing-import]
from select_m036_real_corpus_smoke_batch import ROOT, candidate_entry

BASE_DIR = ROOT / "artifacts" / "m041-mixed-connectivity-smoke"
DEFAULT_OUTPUT = BASE_DIR / "manifest.json"
DEFAULT_DISCOVERY = BASE_DIR / "discovery.json"
DEFAULT_REPORT = BASE_DIR / "report.md"
ARTICLE_ROOT = ROOT / "data" / "article_catalog" / "article_catalog"
M040_MANIFEST = ROOT / "artifacts" / "m036-real-corpus-no-write-smoke" / "manifest.json"
HERMES_DIGEST = (
    ROOT
    / "data"
    / "article_corpora"
    / "m028-universal-loader-runtime-smoke-v1"
    / "hermes-digest-projection.json"
)
MIN_TARGET = 20
MAX_TARGET = 30
BASELINE_COUNT = 10
ARXIV_RE = re.compile(r"(?<!\d)(\d{4}\.\d{4,5})(?:v\d+)?")
ARXIV_NS = {"atom": "http://www.w3.org/2005/Atom", "arxiv": "http://arxiv.org/schemas/atom"}
FALSE_CATALOG_FLAGS = {
    "production_graphdb_write_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
}
SMOKE_SAFETY_FLAGS = {
    "graph_write_allowed": False,
    "promotion_allowed": False,
    "production_import_attempted": False,
    "import_eligible": False,
}
USER_AGENT = "daily-archive-m041-connectivity-smoke/0.1"
NETWORK_TIMEOUT_SECONDS = 60
NETWORK_RETRIES = 3


@dataclass(frozen=True)
class ArxivRecord:
    arxiv_id: str
    title: str
    summary: str
    primary_category: str
    published: str
    updated: str
    abs_url: str
    pdf_url: str
    metadata_status: str = "fetched"


def emit(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


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


def topic_from_category(category: str) -> str:
    return category.strip().lower().replace(".", "-") or "mixed-source"


def known_arxiv_ids() -> set[str]:
    known: set[str] = set()
    for article_path in ARTICLE_ROOT.rglob("article.json"):
        article = load_json(article_path)
        identity = article.get("identity") if isinstance(article.get("identity"), dict) else {}
        if identity.get("arxiv_id"):  # ty:ignore[unresolved-attribute]
            known.add(normalize_arxiv_id(str(identity["arxiv_id"])))  # ty:ignore[not-subscriptable]
        key = str(article.get("article_key") or "")
        if key:
            known.add(normalize_arxiv_id(key))
    return known


def load_hermes_review_candidates(*, known: set[str]) -> dict[str, Any]:
    if not HERMES_DIGEST.exists():
        return {
            "hermes_digest_ref": f"artifact:{HERMES_DIGEST.relative_to(ROOT).as_posix()}",
            "hermes_candidate_count": 0,
            "hermes_candidates": [],
            "hermes_refs_by_id": {},
            "fallback_reason": "missing_hermes_digest_projection",
        }
    digest = load_json(HERMES_DIGEST)
    candidates: list[str] = []
    refs_by_id: dict[str, list[str]] = {}
    for item in digest.get("items", []):
        if not isinstance(item, dict):
            continue
        identity = str(item.get("normalized_identity") or "")
        if not identity.startswith("arxiv:"):
            continue
        arxiv_id = normalize_arxiv_id(identity.split(":", 1)[1])
        if not arxiv_id or arxiv_id in known:
            continue
        if arxiv_id not in candidates:
            candidates.append(arxiv_id)
        ref_id = str(item.get("ref_id") or "")
        if ref_id:
            refs_by_id.setdefault(arxiv_id, []).append(ref_id)
    return {
        "hermes_digest_ref": f"artifact:{HERMES_DIGEST.relative_to(ROOT).as_posix()}",
        "hermes_candidate_count": len(candidates),
        "hermes_candidates": candidates,
        "hermes_refs_by_id": refs_by_id,
        "fallback_reason": None if candidates else "no_new_hermes_arxiv_candidates",
    }


def discover_reference_ids(*, known: set[str]) -> dict[str, Any]:
    refs_by_source: dict[str, list[str]] = {}
    for path in sorted(ARTICLE_ROOT.rglob("source/*")):
        if not path.is_file() or path.suffix.lower() not in {
            ".html",
            ".md",
            ".txt",
            ".xml",
            ".json",
        }:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        found = sorted({normalize_arxiv_id(item) for item in ARXIV_RE.findall(text)} - known)
        if found:
            refs_by_source[str(path.relative_to(ROOT))] = found[:50]
    ordered: list[str] = []
    for refs in refs_by_source.values():
        for ref in refs:
            if ref not in ordered:
                ordered.append(ref)
    return {
        "reference_source_count": len(refs_by_source),
        "reference_candidate_count": len(ordered),
        "reference_candidates": ordered,
        "references_by_source": refs_by_source,
    }


def fetch_url(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    last_error: Exception | None = None
    for attempt in range(NETWORK_RETRIES):
        try:
            with urllib.request.urlopen(request, timeout=NETWORK_TIMEOUT_SECONDS) as response:
                return response.read()
        except TimeoutError as exc:
            last_error = exc
            if attempt < NETWORK_RETRIES - 1:
                time.sleep(2 * (attempt + 1))
        except (urllib.error.HTTPError, urllib.error.URLError, OSError) as exc:
            last_error = exc
            if attempt < NETWORK_RETRIES - 1:
                time.sleep(2 * (attempt + 1))
    if last_error is not None:
        raise last_error
    raise RuntimeError("unreachable fetch_url state")


def stub_arxiv_record(arxiv_id: str, *, reason: str) -> ArxivRecord:
    return ArxivRecord(
        arxiv_id=arxiv_id,
        title=f"Referenced arXiv article {arxiv_id}",
        summary=f"Metadata fetch deferred: {reason}. Identifier was discovered from local evidence.",
        primary_category="cs.AI",
        published="",
        updated="",
        abs_url=f"https://arxiv.org/abs/{arxiv_id}",
        pdf_url=f"https://arxiv.org/pdf/{arxiv_id}",
        metadata_status=f"deferred:{reason}",
    )


def fetch_arxiv_records(arxiv_ids: list[str]) -> list[ArxivRecord]:
    if not arxiv_ids:
        return []
    query = ",".join(arxiv_ids)
    url = "https://export.arxiv.org/api/query?" + urllib.parse.urlencode(
        {"id_list": query, "max_results": len(arxiv_ids)}
    )
    try:
        root = ET.fromstring(fetch_url(url))
    except Exception as exc:
        return [stub_arxiv_record(arxiv_id, reason=type(exc).__name__) for arxiv_id in arxiv_ids]
    records_by_id: dict[str, ArxivRecord] = {}
    for entry in root.findall("atom:entry", ARXIV_NS):
        raw_id = entry.findtext("atom:id", default="", namespaces=ARXIV_NS).rsplit("/", 1)[-1]
        arxiv_id = normalize_arxiv_id(raw_id)
        title = " ".join(
            entry.findtext("atom:title", default="untitled", namespaces=ARXIV_NS).split()
        )
        summary = " ".join(entry.findtext("atom:summary", default="", namespaces=ARXIV_NS).split())
        primary = entry.find("arxiv:primary_category", ARXIV_NS)
        primary_category = primary.attrib.get("term", "cs.AI") if primary is not None else "cs.AI"
        records_by_id[arxiv_id] = ArxivRecord(
            arxiv_id=arxiv_id,
            title=title,
            summary=summary,
            primary_category=primary_category,
            published=entry.findtext("atom:published", default="", namespaces=ARXIV_NS),
            updated=entry.findtext("atom:updated", default="", namespaces=ARXIV_NS),
            abs_url=f"https://arxiv.org/abs/{arxiv_id}",
            pdf_url=f"https://arxiv.org/pdf/{arxiv_id}",
        )
    return [
        records_by_id.get(arxiv_id) or stub_arxiv_record(arxiv_id, reason="missing_api_entry")
        for arxiv_id in arxiv_ids
    ]


def fetch_fresh_arxiv_ids(*, exclude: set[str], count: int) -> list[str]:
    url = "https://export.arxiv.org/api/query?" + urllib.parse.urlencode(
        {
            "search_query": "cat:cs.AI OR cat:cs.CL OR cat:cs.IR",
            "sortBy": "submittedDate",
            "sortOrder": "descending",
            "start": 0,
            "max_results": max(count * 3, 10),
        }
    )
    root = ET.fromstring(fetch_url(url))
    ids: list[str] = []
    for entry in root.findall("atom:entry", ARXIV_NS):
        arxiv_id = normalize_arxiv_id(
            entry.findtext("atom:id", default="", namespaces=ARXIV_NS).rsplit("/", 1)[-1]
        )
        if arxiv_id and arxiv_id not in exclude and arxiv_id not in ids:
            ids.append(arxiv_id)
        if len(ids) >= count:
            break
    return ids


def article_path_for(record: ArxivRecord) -> Path:
    return (
        ARTICLE_ROOT
        / "arxiv"
        / topic_from_category(record.primary_category)
        / record.arxiv_id
        / "article.json"
    )


def render_abs_stub(record: ArxivRecord) -> str:
    escaped_title = html.escape(record.title)
    escaped_summary = html.escape(record.summary)
    return (
        "<!doctype html>\n"
        "<html><head><meta charset='utf-8'>"
        f"<title>{escaped_title}</title></head><body>"
        f"<h1>{escaped_title}</h1>"
        f"<p data-source='arxiv-api-summary'>{escaped_summary}</p>"
        f"<p><a href='{record.abs_url}'>arXiv abstract page</a></p>"
        "</body></html>\n"
    )


def write_arxiv_article(record: ArxivRecord, *, category: str, linked_from: list[str]) -> Path:
    path = article_path_for(record)
    article_dir = path.parent
    abs_path = article_dir / "source" / "abs.html"
    article = {
        "schema_version": "article.v00.01",
        "article_key": record.arxiv_id,
        "catalog_path": str(article_dir.relative_to(ARTICLE_ROOT)),
        "source_code": "arxiv",
        "source_type": "preprint_server",
        "publisher": "arxiv",
        "coarse_topic_code": topic_from_category(record.primary_category),
        "topic_tags": [record.primary_category, category],
        "identity": {
            "arxiv_id": record.arxiv_id,
            "title": record.title,
            "canonical_url": record.abs_url,
            "pdf_url": record.pdf_url,
            "published": record.published,
            "updated": record.updated,
            "metadata_status": record.metadata_status,
        },
        "source_strategy": {
            "primary_source_variant_id": f"{record.arxiv_id}:source:arxiv-abs",
            "preferred_content_order": ["arxiv_abs_page"],
            "metadata_order": ["arxiv_api_metadata"],
            "fallback_policy": "metadata_only_connectivity_smoke",
        },
        "source_variants": [
            {
                "variant_id": f"{record.arxiv_id}:source:arxiv-abs",
                "source_role": "arxiv_abs_page",
                "source_format": "html_metadata",
                "source_origin": "arxiv_api_summary_stub",
                "path": str(abs_path.relative_to(ROOT)),
                "url": record.abs_url,
            }
        ],
        "connectivity_smoke": {
            "category": category,
            "linked_from": linked_from,
            "metadata_only": True,
            "metadata_status": record.metadata_status,
        },
        "safety_flags": dict(FALSE_CATALOG_FLAGS),
    }
    write_json(path, article)
    write_text(abs_path, render_abs_stub(record))
    return path


def load_baseline_entries() -> list[dict[str, Any]]:
    manifest = load_json(M040_MANIFEST)
    entries = manifest.get("articles")
    if not isinstance(entries, list) or len(entries) < BASELINE_COUNT:
        raise ValueError("M040 manifest must contain at least 10 baseline articles")
    baseline: list[dict[str, Any]] = []
    for entry in entries[:BASELINE_COUNT]:
        copied = dict(entry)
        copied["m041_category"] = "baseline"
        copied["connectivity_role"] = "retained_normalized_baseline"
        baseline.append(copied)
    return baseline


def entry_from_article_path(
    article_path: Path, *, category: str, linked_from: list[str]
) -> dict[str, Any]:
    entry = candidate_entry(article_path)
    if entry is None:
        raise ValueError(f"article is not selectable: {article_path}")
    entry["m041_category"] = category
    entry["connectivity_role"] = category
    entry["linked_from"] = linked_from
    return entry


def build_mixed_manifest(
    *, target_count: int, no_network: bool = False
) -> tuple[dict[str, Any], dict[str, Any]]:
    if target_count < MIN_TARGET or target_count > MAX_TARGET:
        raise ValueError(f"target count must be between {MIN_TARGET} and {MAX_TARGET}")
    baseline = load_baseline_entries()
    known = known_arxiv_ids()
    reference_discovery = discover_reference_ids(known=known)
    hermes_discovery = load_hermes_review_candidates(known=known)
    selected: list[dict[str, Any]] = list(baseline)
    selected_ids = {normalize_arxiv_id(str(entry["article_key"])) for entry in selected}
    acquisition: list[dict[str, Any]] = []

    def add_records(
        records: list[ArxivRecord],
        *,
        category: str,
        linked_from_map: dict[str, list[str]] | None = None,
    ) -> None:
        for record in records:
            if len(selected) >= target_count:
                break
            if record.arxiv_id in selected_ids:
                continue
            linked_from = linked_from_map.get(record.arxiv_id, []) if linked_from_map else []
            article_path = write_arxiv_article(record, category=category, linked_from=linked_from)
            selected.append(
                entry_from_article_path(article_path, category=category, linked_from=linked_from)
            )
            selected_ids.add(record.arxiv_id)
            acquisition.append(
                {
                    "article_key": record.arxiv_id,
                    "category": category,
                    "path": display_path(article_path),
                }
            )

    if not no_network:
        reference_ids = list(reference_discovery["reference_candidates"])
        required_reference_count = min(5, max(0, target_count - len(selected)))
        reference_to_fetch = [ref for ref in reference_ids if ref not in selected_ids][
            :required_reference_count
        ]
        linked_from_map: dict[str, list[str]] = {ref: [] for ref in reference_to_fetch}
        for source_path, refs in reference_discovery["references_by_source"].items():
            for ref in refs:
                if ref in linked_from_map:
                    linked_from_map[ref].append(source_path)
        if len(reference_to_fetch) < required_reference_count:
            raise ValueError(
                f"only found {len(reference_to_fetch)} reference-linked articles; need {required_reference_count}"
            )
        add_records(
            fetch_arxiv_records(reference_to_fetch),
            category="reference_linked",
            linked_from_map=linked_from_map,
        )

        hermes_ids = [
            arxiv_id
            for arxiv_id in hermes_discovery["hermes_candidates"]
            if arxiv_id not in selected_ids
        ]
        needed_hermes = max(0, min(target_count - len(selected), 10))
        hermes_to_fetch = hermes_ids[:needed_hermes]
        hermes_refs_by_id = {
            arxiv_id: [
                f"hermes:{ref_id}"
                for ref_id in hermes_discovery["hermes_refs_by_id"].get(arxiv_id, [])
            ]
            for arxiv_id in hermes_to_fetch
        }
        if hermes_to_fetch:
            time.sleep(3)
            add_records(
                fetch_arxiv_records(hermes_to_fetch),
                category="hermes_review_section",
                linked_from_map=hermes_refs_by_id,
            )

        remaining = target_count - len(selected)
        if remaining > 0:
            fresh_ids = fetch_fresh_arxiv_ids(exclude=known | selected_ids, count=remaining)
            if fresh_ids:
                time.sleep(3)
            add_records(fetch_arxiv_records(fresh_ids), category="fresh", linked_from_map=None)
    else:
        reference_discovery["network_skipped"] = True
        hermes_discovery["network_skipped"] = True

    if len(selected) < target_count:
        raise ValueError(f"only built {len(selected)} articles; need {target_count}")

    category_counts: dict[str, int] = {}
    for entry in selected:
        category = str(entry.get("m041_category") or "unknown")
        category_counts[category] = category_counts.get(category, 0) + 1

    manifest = {
        "schema_version": "m041-mixed-connectivity-smoke-manifest.v1",
        "inherits_smoke_schema": "m036-real-corpus-smoke-manifest.v1",
        "catalog_ref": "artifact:data/article_catalog/catalog.json",
        "article_count": len(selected),
        "target_count": target_count,
        "category_counts": category_counts,
        "hermes_review_selection": {
            "digest_ref": hermes_discovery["hermes_digest_ref"],
            "candidate_count": hermes_discovery["hermes_candidate_count"],
            "used_count": category_counts.get("hermes_review_section", 0),
            "fallback_reason": None
            if category_counts.get("hermes_review_section", 0)
            else hermes_discovery.get("fallback_reason")
            or "no_hermes_candidates_used_or_network_skipped",
        },
        "reference_discovery": {
            "candidate_count": reference_discovery["reference_candidate_count"],
            "source_count": reference_discovery["reference_source_count"],
            "used_reference_linked_count": category_counts.get("reference_linked", 0),
            "fallback_reason": None
            if category_counts.get("reference_linked", 0)
            else "reference_candidates_not_needed_or_network_skipped",
        },
        "articles": selected,
        "safety_flags": dict(SMOKE_SAFETY_FLAGS),
        "diagnostics": sorted(
            {diagnostic for entry in selected for diagnostic in entry.get("diagnostics", [])}
        ),
    }
    discovery = {
        "hermes": hermes_discovery,
        "references": reference_discovery,
        "acquired_articles": acquisition,
        "category_counts": category_counts,
    }
    return manifest, discovery


def write_report(path: Path, manifest: dict[str, Any], discovery: dict[str, Any]) -> None:
    lines = [
        "# M041 Mixed Connectivity Smoke Selection",
        "",
        f"- Articles: {manifest['article_count']}",
        f"- Category counts: {manifest['category_counts']}",
        f"- Hermes review candidates discovered: {discovery.get('hermes', {}).get('hermes_candidate_count')}",
        f"- Hermes review-section articles used: {manifest['category_counts'].get('hermes_review_section', 0)}",
        f"- Reference candidates discovered: {discovery.get('references', {}).get('reference_candidate_count')}",
        f"- Reference-linked articles used: {manifest['category_counts'].get('reference_linked', 0)}",
        f"- Fresh articles used: {manifest['category_counts'].get('fresh', 0)}",
        "- Graph write/import/promotion: false",
        "",
        "## Notes",
        "",
        "This is a no-write connectivity smoke selection. Article metadata/source artifacts are local evidence only and do not authorize graph import or fact promotion.",
        "",
    ]
    write_text(path, "\n".join(lines))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-count", type=int, default=20)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--discovery-output", type=Path, default=DEFAULT_DISCOVERY)
    parser.add_argument("--report-output", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--discovery-only", action="store_true")
    parser.add_argument("--no-network", action="store_true")
    args = parser.parse_args()

    if args.discovery_only:
        discovery = discover_reference_ids(known=known_arxiv_ids())
        write_json(args.discovery_output, discovery)
        emit(f"reference_candidates={discovery['reference_candidate_count']}")
        return 0

    manifest, discovery = build_mixed_manifest(
        target_count=args.target_count, no_network=args.no_network
    )
    write_json(args.output, manifest)
    write_json(args.discovery_output, discovery)
    write_report(args.report_output, manifest, discovery)
    emit(f"article_count={manifest['article_count']}")
    emit(f"category_counts={manifest['category_counts']}")
    emit(
        "graph_write_allowed=false promotion_allowed=false production_import_attempted=false import_eligible=false"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
