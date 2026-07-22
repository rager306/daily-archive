#!/usr/bin/env python3
"""Build the M029 unified article corpus selection registry.

This command is intentionally local-only. It reads earlier milestone selection
artifacts plus the M028 roadmap URL list, normalizes URL/article identities, and
writes a deduped selection with explicit provenance and catalog resolution
counters. It does not fetch network content and does not create catalog records.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from collections import Counter, defaultdict
from collections.abc import Iterable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit, urlunsplit

SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "article-corpus-selection.v00.01"
CATALOG_SCHEMA_VERSION = "article-catalog.v00.01"
ARTICLE_SCHEMA_VERSION = "article.v00.01"
PROVENANCE_SCHEMA_VERSION = "article-selection-provenance.v00.01"
SUMMARY_SCHEMA_VERSION = "article-selection-summary.v00.01"

M025_SOURCE_ID = "M025"
M027_SOURCE_ID = "M027"
M028_SOURCE_ID = "M028"

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_manifests_embed_raw_text": False,
    "metadata_manifests_embed_raw_binary": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
}

ARXIV_URL_RE = re.compile(
    r"^https://arxiv\.org/(abs|pdf|html)/(\d{4}\.\d{4,5})(v\d+)?(?:\.pdf)?/?$"
)
URL_RE = re.compile(r"https?://[^\s`)>\"]+")
M028_EXPANSION_HEADER = "Newly accepted expansion refs:"
M028_NEXT_HEADER_RE = re.compile(r"^\s{4}[A-Z].*:$")


@dataclass(frozen=True)
class SourceObservation:
    source_id: str
    source_path: str
    source_subset: str
    url: str
    url_role: str
    selection_role: str | None = None
    article_ref_hint: str | None = None
    source_code_hint: str | None = None


@dataclass
class RegistryEntry:
    identity_key: str
    source_code: str
    article_key: str
    canonical_url: str
    selected_seed_url: str
    source_strategy: str
    catalog_resolution: str
    article_ref: str | None
    article_path: str | None
    provenance: list[SourceObservation] = field(default_factory=list)


def _read_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"required input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False
    ) as handle:
        json.dump(payload, handle, indent=2, sort_keys=True)
        handle.write("\n")
        temp_name = handle.name
    Path(temp_name).replace(path)


def _clean_url(raw_url: str) -> str:
    raw_url = raw_url.strip().rstrip(".,")
    parts = urlsplit(raw_url)
    return urlunsplit(
        (parts.scheme.lower(), parts.netloc.lower(), parts.path.rstrip("/"), parts.query, "")
    )


def _arxiv_match(url: str) -> re.Match[str] | None:
    return ARXIV_URL_RE.match(_clean_url(url))


def _canonical_url(url: str) -> str:
    clean = _clean_url(url)
    match = _arxiv_match(clean)
    if match:
        _, arxiv_id, version = match.groups()
        suffix = version or ""
        return f"https://arxiv.org/abs/{arxiv_id}{suffix}"
    return clean


def _identity_from_url(url: str) -> tuple[str, str, str, str]:
    clean = _clean_url(url)
    match = _arxiv_match(clean)
    if match:
        _, arxiv_id, version = match.groups()
        article_key = f"{arxiv_id}{version or ''}"
        return "arxiv", article_key, f"arxiv:{arxiv_id}{version or ''}", _canonical_url(clean)
    if "nature.com/articles/" in clean:
        article_key = clean.split("/articles/", 1)[1]
        return "nature", article_key, f"nature:{article_key}", clean
    if clean == "https://pageindex.ai/blog/pageindex-intro":
        return (
            "company_blog",
            "pageindex_zhang2025pageindex",
            "company_blog:pageindex_zhang2025pageindex",
            clean,
        )
    if "developer.nvidia.com/blog/" in clean:
        article_key = (
            clean.split("/blog/", 1)[1]
            .split("?", 1)[0]
            .strip("/")
            .replace("/", "_")
            .replace("-", "_")
        )
        return "company_blog", article_key, f"company_blog:{article_key}", clean
    host_path = urlsplit(clean).netloc + urlsplit(clean).path.rstrip("/")
    article_key = re.sub(r"[^a-zA-Z0-9]+", "_", host_path).strip("_").lower()
    return "web", article_key, f"web:{article_key}", clean


def _source_strategy(url: str, source_code: str, catalog_entry: dict[str, Any] | None) -> str:
    clean = _clean_url(url)
    if catalog_entry and catalog_entry.get("primary_source_role"):
        return str(catalog_entry["primary_source_role"])
    match = _arxiv_match(clean)
    if match:
        role = match.group(1)
        return {"abs": "arxiv_abs_page", "pdf": "arxiv_pdf", "html": "arxiv_html"}[role]
    if source_code == "nature":
        return "publisher_html"
    if source_code == "company_blog":
        return "web_article_html"
    return "web_url"


def _load_catalog_index(catalog_root: Path) -> dict[str, Any]:
    index_path = catalog_root / "index.json"
    try:
        index = _read_json(index_path)
    except ValueError as exc:
        raise ValueError(f"catalog index resolution failed: {exc}") from exc
    entries = index.get("articles")
    if not isinstance(entries, list):
        raise ValueError(f"catalog index has no articles list: {index_path}")
    by_ref = {entry.get("article_ref"): entry for entry in entries if entry.get("article_ref")}
    by_url = index.get("indexes", {}).get("by_canonical_url", {})
    by_key = index.get("indexes", {}).get("by_article_key", {})
    return {"by_ref": by_ref, "by_url": by_url, "by_key": by_key}


def _catalog_entry_for(
    catalog: dict[str, Any], url: str, article_ref_hint: str | None, article_key: str
) -> dict[str, Any] | None:
    by_ref = catalog["by_ref"]
    by_url = catalog["by_url"]
    by_key = catalog["by_key"]
    if article_ref_hint and article_ref_hint in by_ref:
        return by_ref[article_ref_hint]
    for candidate_url in (_clean_url(url), _canonical_url(url)):
        ref = by_url.get(candidate_url)
        if ref in by_ref:
            return by_ref[ref]
    ref = by_key.get(article_key)
    if ref in by_ref:
        return by_ref[ref]
    return None


def _selection_observations(selection_path: Path, source_id: str) -> list[SourceObservation]:
    selection = _read_json(selection_path)
    articles = selection.get("articles")
    if not isinstance(articles, list):
        raise ValueError(f"selection has no articles list: {selection_path}")
    observations: list[SourceObservation] = []
    for article in articles:
        seed_url = article.get("seed_url")
        if not isinstance(seed_url, str):
            raise ValueError(f"selection article missing seed_url in {selection_path}: {article!r}")
        observations.append(
            SourceObservation(
                source_id=source_id,
                source_path=str(selection_path),
                source_subset="selection.articles",
                url=_clean_url(seed_url),
                url_role="seed_url",
                selection_role=article.get("selection_role"),
                article_ref_hint=article.get("article_ref"),
                source_code_hint=article.get("source_code"),
            )
        )
        canonical = article.get("canonical_url")
        if isinstance(canonical, str) and _clean_url(canonical) != _clean_url(seed_url):
            observations.append(
                SourceObservation(
                    source_id=source_id,
                    source_path=str(selection_path),
                    source_subset="selection.articles",
                    url=_clean_url(canonical),
                    url_role="canonical_url",
                    selection_role=article.get("selection_role"),
                    article_ref_hint=article.get("article_ref"),
                    source_code_hint=article.get("source_code"),
                )
            )
    return observations


def _extract_m028_urls(
    roadmap_path: Path,
) -> tuple[list[SourceObservation], list[SourceObservation]]:
    try:
        lines = roadmap_path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise ValueError(f"required input is missing: {roadmap_path}") from exc
    all_observations: list[SourceObservation] = []
    expansion_observations: list[SourceObservation] = []
    in_expansion = False
    for line in lines:
        stripped = line.strip()
        if stripped == M028_EXPANSION_HEADER:
            in_expansion = True
            continue
        if in_expansion and M028_NEXT_HEADER_RE.match(line):
            in_expansion = False
        for raw_url in URL_RE.findall(line):
            url = _clean_url(raw_url)
            obs = SourceObservation(
                source_id=M028_SOURCE_ID,
                source_path=str(roadmap_path),
                source_subset="newly_accepted_expansion_refs"
                if in_expansion
                else "roadmap_url_refs",
                url=url,
                url_role="roadmap_url_ref",
            )
            all_observations.append(obs)
            if in_expansion:
                expansion_observations.append(obs)
    if len(expansion_observations) != 7:
        raise ValueError(
            f"expected 7 M028 newly accepted expansion refs, found {len(expansion_observations)}"
        )
    return all_observations, expansion_observations


def _selected_observations(
    m025_path: Path, m027_path: Path, m028_roadmap_path: Path
) -> tuple[list[SourceObservation], list[SourceObservation]]:
    m025 = _selection_observations(m025_path, M025_SOURCE_ID)
    m027 = _selection_observations(m027_path, M027_SOURCE_ID)
    m028_all, m028_expansion = _extract_m028_urls(m028_roadmap_path)
    selected = [*m025, *m027, *m028_expansion]
    provenance_pool = [*m025, *m027, *m028_all]
    return selected, provenance_pool


def _build_entries(
    selected: Iterable[SourceObservation],
    provenance_pool: Iterable[SourceObservation],
    catalog: dict[str, Any],
) -> tuple[list[RegistryEntry], dict[str, Any]]:
    selected_by_identity: dict[str, SourceObservation] = {}
    selected_order: list[str] = []
    for obs in selected:
        source_code, article_key, identity_key, _ = _identity_from_url(obs.url)
        if identity_key not in selected_by_identity:
            selected_by_identity[identity_key] = obs
            selected_order.append(identity_key)

    provenance_by_identity: dict[str, list[SourceObservation]] = defaultdict(list)
    url_occurrences: Counter[str] = Counter()
    for obs in provenance_pool:
        _, _, identity_key, _ = _identity_from_url(obs.url)
        provenance_by_identity[identity_key].append(obs)
        url_occurrences[obs.url] += 1

    entries: list[RegistryEntry] = []
    resolution_counter: Counter[str] = Counter()
    source_counter: Counter[str] = Counter()
    for identity_key in selected_order:
        selected_obs = selected_by_identity[identity_key]
        source_code, article_key, _, canonical_url = _identity_from_url(selected_obs.url)
        catalog_entry = _catalog_entry_for(
            catalog, selected_obs.url, selected_obs.article_ref_hint, article_key
        )
        article_ref = catalog_entry.get("article_ref") if catalog_entry else None
        article_path = catalog_entry.get("article_path") if catalog_entry else None
        resolution = "resolved" if catalog_entry else "unresolved"
        resolution_counter[resolution] += 1
        source_counter[source_code] += 1
        entries.append(
            RegistryEntry(
                identity_key=identity_key,
                source_code=source_code,
                article_key=article_key,
                canonical_url=str(catalog_entry.get("canonical_url", canonical_url))
                if catalog_entry
                else canonical_url,
                selected_seed_url=selected_obs.url,
                source_strategy=_source_strategy(selected_obs.url, source_code, catalog_entry),
                catalog_resolution=resolution,
                article_ref=str(article_ref) if article_ref else None,
                article_path=str(article_path) if article_path else None,
                provenance=sorted(
                    provenance_by_identity.get(identity_key, [selected_obs]),
                    key=lambda item: (item.source_id, item.source_subset, item.url_role, item.url),
                ),
            )
        )

    duplicate_urls = {url: count for url, count in sorted(url_occurrences.items()) if count > 1}
    counters = {
        "selected_observation_count": len(list(selected_by_identity.values())),
        "unique_article_count": len(entries),
        "provenance_url_observation_count": sum(url_occurrences.values()),
        "duplicate_url_count": len(duplicate_urls),
        "duplicate_urls": duplicate_urls,
        "index_resolution": dict(sorted(resolution_counter.items())),
        "source_code_counts": dict(sorted(source_counter.items())),
    }
    return entries, counters


def _entry_payload(entry: RegistryEntry) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "identity_key": entry.identity_key,
        "source_code": entry.source_code,
        "article_key": entry.article_key,
        "canonical_url": entry.canonical_url,
        "seed_url": entry.selected_seed_url,
        "source_strategy": entry.source_strategy,
        "catalog_resolution": entry.catalog_resolution,
        "provenance_sources": sorted({obs.source_id for obs in entry.provenance}),
        "provenance_url_count": len({obs.url for obs in entry.provenance}),
    }
    if entry.article_ref:
        payload["article_ref"] = entry.article_ref
    if entry.article_path:
        payload["article_path"] = entry.article_path
    return payload


def _provenance_payload(entries: list[RegistryEntry], counters: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": PROVENANCE_SCHEMA_VERSION,
        "selection_id": SELECTION_ID,
        "unique_article_count": len(entries),
        "duplicate_url_count": counters["duplicate_url_count"],
        "duplicate_urls": counters["duplicate_urls"],
        "articles": [
            {
                "identity_key": entry.identity_key,
                "article_ref": entry.article_ref,
                "canonical_url": entry.canonical_url,
                "source_strategy": entry.source_strategy,
                "catalog_resolution": entry.catalog_resolution,
                "observations": [obs.__dict__ for obs in entry.provenance],
            }
            for entry in entries
        ],
    }


def _selection_payload(entries: list[RegistryEntry]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "selection_id": SELECTION_ID,
        "catalog_schema_version": CATALOG_SCHEMA_VERSION,
        "article_schema_version": ARTICLE_SCHEMA_VERSION,
        "purpose": "M029 unified source-of-truth selection for M025, M027, and M028 accepted expansion URL provenance.",
        "selection_mode": "manual_url_seed_unified_with_identity_dedupe",
        "network_policy": {
            "capture_phase_may_fetch": False,
            "registration_command_fetches_network": False,
            "test_phase_must_not_fetch": True,
            "pipeline_phase_reads_catalog_only": True,
        },
        "articles": [_entry_payload(entry) for entry in entries],
        "safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
    }


def _summary_payload(
    entries: list[RegistryEntry],
    counters: dict[str, Any],
    selection_path: Path,
    provenance_path: Path,
) -> dict[str, Any]:
    return {
        "schema_version": SUMMARY_SCHEMA_VERSION,
        "selection_id": SELECTION_ID,
        "status": "registered",
        "selection_path": str(selection_path),
        "provenance_path": str(provenance_path),
        "unique_article_count": counters["unique_article_count"],
        "duplicate_url_count": counters["duplicate_url_count"],
        "duplicate_urls": counters["duplicate_urls"],
        "index_resolution": counters["index_resolution"],
        "source_code_counts": counters["source_code_counts"],
        "provenance_mapping": {
            entry.identity_key: {
                "canonical_url": entry.canonical_url,
                "article_ref": entry.article_ref,
                "catalog_resolution": entry.catalog_resolution,
                "source_strategy": entry.source_strategy,
                "sources": sorted({obs.source_id for obs in entry.provenance}),
                "urls": sorted({obs.url for obs in entry.provenance}),
            }
            for entry in entries
        },
        "network_fetch_attempted": False,
        "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
    }


def build_registry(
    m025_path: Path, m027_path: Path, m028_roadmap_path: Path, catalog_root: Path
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    selected, provenance_pool = _selected_observations(m025_path, m027_path, m028_roadmap_path)
    catalog = _load_catalog_index(catalog_root)
    entries, counters = _build_entries(selected, provenance_pool, catalog)
    if len(entries) != 18:
        raise ValueError(
            f"M029 provisional registry expected 18 unique article identities, found {len(entries)}"
        )
    selection_path = Path("data/article_corpora") / SELECTION_ID / "selection.json"
    provenance_path = Path("data/article_corpora") / SELECTION_ID / "selection-provenance.json"
    return (
        _selection_payload(entries),
        _provenance_payload(entries, counters),
        _summary_payload(entries, counters, selection_path, provenance_path),
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="Write the M029 selection, provenance, and summary artifacts.",
    )
    parser.add_argument(
        "--m025-selection",
        type=Path,
        default=Path("data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json"),
    )
    parser.add_argument(
        "--m027-selection",
        type=Path,
        default=Path("data/article_corpora/m027-mixed-source-corpus-v1/selection.json"),
    )
    parser.add_argument(
        "--m028-roadmap",
        type=Path,
        # Flat-phase layout (nested .gsd/milestones/M028-... removed by GSD migration).
        default=Path(
            ".gsd/phases/28-8hwqjk-m028-8hwqjk-universal-loader-runtime-smo/28-ROADMAP.md"
        ),
    )
    parser.add_argument("--catalog-root", type=Path, default=Path("data/article_catalog"))
    parser.add_argument(
        "--output-dir", type=Path, default=Path("data/article_corpora") / SELECTION_ID
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        selection, provenance, summary = build_registry(
            args.m025_selection, args.m027_selection, args.m028_roadmap, args.catalog_root
        )
        selection_path = args.output_dir / "selection.json"
        provenance_path = args.output_dir / "selection-provenance.json"
        summary_path = args.output_dir / "selection-summary.json"
        summary["selection_path"] = str(selection_path)
        summary["provenance_path"] = str(provenance_path)
        if args.write:
            _atomic_write_json(selection_path, selection)
            _atomic_write_json(provenance_path, provenance)
            _atomic_write_json(summary_path, summary)
        print(json.dumps(summary, sort_keys=True))
        return 0
    except Exception as exc:
        diagnostic = {
            "level": "error",
            "code": "m029_unified_registry_failed",
            "selection_id": SELECTION_ID,
            "message": str(exc),
            "network_fetch_attempted": False,
            "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        }
        print(json.dumps(diagnostic, sort_keys=True), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
