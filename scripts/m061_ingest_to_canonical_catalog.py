#!/usr/bin/env python3
"""Ingest M061 acquired PDFs into the canonical article catalog.

The script is intentionally narrow: it copies locally acquired M061 PDFs into
``data/article_catalog/article_catalog/arxiv/<category>/<arxiv_id>/source/`` and
creates minimal catalog records so the existing index verifier can validate the
new lookup surface. External network use is limited to arxiv API metadata lookup
for category/title detection and is explicitly paced at one request per three
seconds.
"""

from __future__ import annotations

import argparse
import dataclasses
import email.utils
import hashlib
import json
import shutil
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import Counter, defaultdict
from collections.abc import Callable, Iterable
from pathlib import Path
from typing import Any

import feedparser

ROOT = Path(__file__).resolve().parents[1]
M061_ROOT = ROOT / "artifacts" / "m061-2hop"
CATALOG_ROOT = ROOT / "data" / "article_catalog"
CATALOG_MANIFEST_PATH = CATALOG_ROOT / "catalog.json"
CANONICAL_ARXIV_ROOT = CATALOG_ROOT / "article_catalog" / "arxiv"
INDEX_PATH = CATALOG_ROOT / "article_catalog" / "index.json"
REPORT_PATH = M061_ROOT / "s04-ingest-report.md"
ARXIV_API_URL = "https://export.arxiv.org/api/query"
ARXIV_USER_AGENT = "daily-archive/1.0 (mailto: contact)"
ARXIV_API_MIN_INTERVAL_SECONDS = 3.0
ARXIV_MAX_RETRY_ATTEMPTS = 3
ARXIV_BACKOFF_SECONDS = (1.0, 5.0, 15.0, 60.0, 300.0)
FALLBACK_CATEGORY = "mixed-source"
KNOWN_REPORT_BUCKETS = ("cs-cl", "cs-lg", "cs-cv", "cs-ai", "mixed-source")

SAFETY_OVERRIDE = {
    "external_network_authorized": True,
    "reason": "User explicit authorization for M064-wqfgfa S04 catalog ingestion; arxiv API rate limit respected (1 req/3s, retry+backoff, 429 honors Retry-After)",
    "scope": "M064-wqfgfa S04 only, ~32 unique arxiv_ids for category lookup, no graph writes, no production import",
}

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}

CATALOG_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_manifests_embed_raw_text": False,
    "metadata_manifests_embed_raw_binary": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
    "network_fetch_required_for_pipeline_phase": False,
}


@dataclasses.dataclass(frozen=True)
class ArxivMetadata:
    arxiv_id: str
    category: str
    title: str
    source: str
    fallback: bool = False
    error: str | None = None


@dataclasses.dataclass
class ApiMetrics:
    requests_made: int = 0
    rate_limit_429s: int = 0
    pacing_delay_seconds: float = 0.0
    retry_delay_seconds: float = 0.0
    failures: int = 0


@dataclasses.dataclass
class IngestRecord:
    arxiv_id: str
    anchor_ids: list[str]
    source_pdf: Path
    dest_pdf: Path
    category: str
    title: str
    status: str
    fallback: bool
    source_sha256: str
    dest_sha256: str
    message: str


@dataclasses.dataclass
class IngestResult:
    records: list[IngestRecord]
    selected_total: int
    discovered_pdf_total: int
    unique_arxiv_ids: int
    before_catalog_pdf_count: int
    after_catalog_pdf_count: int
    api_metrics: ApiMetrics
    index_updated: bool
    index_entries: int | None
    index_diagnostics: list[dict[str, Any]]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize_arxiv_id(value: str) -> str:
    arxiv_id = value.strip()
    if arxiv_id.endswith(".pdf"):
        arxiv_id = arxiv_id[:-4]
    return arxiv_id


def normalize_category(value: str | None) -> str:
    if not value:
        return FALLBACK_CATEGORY
    category = value.strip().lower().replace(".", "-").replace("_", "-")
    return category or FALLBACK_CATEGORY


def report_bucket(category: str) -> str:
    return category if category in KNOWN_REPORT_BUCKETS else "other"


def catalog_pdf_count(catalog_root: Path = CANONICAL_ARXIV_ROOT) -> int:
    if not catalog_root.exists():
        return 0
    return sum(1 for _ in catalog_root.glob("*/**/source/*.pdf"))


def load_selected_ids(m061_root: Path = M061_ROOT) -> dict[str, list[str]]:
    anchors: dict[str, list[str]] = {}
    for selected_path in sorted(m061_root.glob("anchor-*/acquisition/selected-2hop-papers.json")):
        anchor_id = selected_path.parents[1].name.removeprefix("anchor-")
        payload = json.loads(selected_path.read_text(encoding="utf-8"))
        selected = payload.get("selected_arxiv_ids")
        if not isinstance(selected, list):
            raise ValueError(f"{selected_path} selected_arxiv_ids must be a list")
        anchors[anchor_id] = [normalize_arxiv_id(str(item)) for item in selected]
    if not anchors:
        raise FileNotFoundError(f"No M061 selected-2hop-papers.json files under {m061_root}")
    return anchors


def load_pdf_paths(m061_root: Path = M061_ROOT) -> dict[str, list[Path]]:
    pdfs: dict[str, list[Path]] = defaultdict(list)
    for pdf_path in sorted(m061_root.glob("anchor-*/acquisition/pdfs/*.pdf")):
        pdfs[normalize_arxiv_id(pdf_path.name)].append(pdf_path)
    if not pdfs:
        raise FileNotFoundError(f"No M061 PDFs under {m061_root}")
    return dict(pdfs)


def invert_anchor_membership(anchor_ids: dict[str, list[str]]) -> dict[str, list[str]]:
    membership: dict[str, list[str]] = defaultdict(list)
    for anchor_id, selected_ids in anchor_ids.items():
        for arxiv_id in selected_ids:
            membership[arxiv_id].append(anchor_id)
    return {arxiv_id: sorted(anchors) for arxiv_id, anchors in membership.items()}


def existing_catalog_pdf(arxiv_root: Path, arxiv_id: str) -> Path | None:
    matches = sorted(arxiv_root.glob(f"*/{arxiv_id}/source/{arxiv_id}.pdf"))
    return matches[0] if matches else None


def parse_retry_after(value: str | None) -> float | None:
    if not value:
        return None
    try:
        return max(0.0, float(value))
    except ValueError:
        parsed = email.utils.parsedate_to_datetime(value)
        if parsed is None:
            return None
        return max(0.0, parsed.timestamp() - time.time())


class RequestPacer:
    def __init__(self, min_interval_seconds: float = ARXIV_API_MIN_INTERVAL_SECONDS, sleep: Callable[[float], None] = time.sleep) -> None:
        self.min_interval_seconds = min_interval_seconds
        self.sleep = sleep
        self.last_request_started_at: float | None = None
        self.total_delay_seconds = 0.0

    def wait(self) -> None:
        if self.last_request_started_at is None:
            return
        elapsed = time.monotonic() - self.last_request_started_at
        remaining = self.min_interval_seconds - elapsed
        if remaining > 0:
            self.sleep(remaining)
            self.total_delay_seconds += remaining

    def mark_request_started(self) -> None:
        self.last_request_started_at = time.monotonic()


def arxiv_query_url(arxiv_id: str) -> str:
    params = urllib.parse.urlencode({"id_list": arxiv_id, "start": 0, "max_results": 1})
    return f"{ARXIV_API_URL}?{params}"


def fetch_arxiv_metadata(
    arxiv_id: str,
    *,
    pacer: RequestPacer,
    metrics: ApiMetrics,
    sleep: Callable[[float], None] = time.sleep,
) -> ArxivMetadata:
    """Fetch arxiv category/title using the same API and parser shape as ArxivClient._fetch_category."""

    # Import here so tests can exercise pure filesystem helpers without loading optional code paths.
    from arxiv_archive.arxiv_client import ArxivClient

    client = ArxivClient()
    request = urllib.request.Request(arxiv_query_url(arxiv_id), headers={"User-Agent": ARXIV_USER_AGENT})
    last_error: str | None = None
    for attempt in range(ARXIV_MAX_RETRY_ATTEMPTS + 1):
        pacer.wait()
        pacer.mark_request_started()
        metrics.requests_made += 1
        try:
            with urllib.request.urlopen(request, timeout=60.0) as response:
                body = response.read().decode("utf-8", errors="replace")
            feed = feedparser.parse(body)
            if not feed.entries:
                raise RuntimeError(f"arxiv API returned no entries for {arxiv_id}")
            paper = client._parse_entry(feed.entries[0])
            category = normalize_category(paper.categories[0] if paper.categories else None)
            if category == FALLBACK_CATEGORY:
                raise RuntimeError(f"arxiv API returned no category for {arxiv_id}")
            return ArxivMetadata(arxiv_id=arxiv_id, category=category, title=paper.title.strip(), source="arxiv_api")
        except urllib.error.HTTPError as exc:
            last_error = f"HTTP {exc.code}: {exc.reason}"
            if exc.code == 429:
                metrics.rate_limit_429s += 1
                retry_after = parse_retry_after(exc.headers.get("Retry-After"))
                delay = retry_after if retry_after is not None else ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
            else:
                delay = ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
        except (urllib.error.URLError, TimeoutError, RuntimeError, ValueError) as exc:
            last_error = str(exc)
            delay = ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]

        if attempt >= ARXIV_MAX_RETRY_ATTEMPTS:
            break
        sleep(delay)
        metrics.retry_delay_seconds += delay

    metrics.failures += 1
    return ArxivMetadata(
        arxiv_id=arxiv_id,
        category=FALLBACK_CATEGORY,
        title=f"arXiv {arxiv_id} (M061 catalog ingestion)",
        source="fallback",
        fallback=True,
        error=last_error or "unknown arxiv API failure",
    )


def build_article_record(arxiv_id: str, category: str, title: str, dest_pdf: Path) -> dict[str, Any]:
    article_ref = f"arxiv/{category}/{arxiv_id}"
    try:
        rel_pdf_path = dest_pdf.relative_to(CATALOG_ROOT).as_posix()
    except ValueError:
        rel_pdf_path = dest_pdf.as_posix()
    return {
        "schema_version": "article.v00.01",
        "article_key": arxiv_id,
        "catalog_path": article_ref,
        "source_code": "arxiv",
        "source_type": "preprint_server",
        "publisher": "arxiv",
        "coarse_topic_code": category,
        "topic_tags": ["m061-2hop", "catalog-ingestion"],
        "identity": {
            "arxiv_id": arxiv_id,
            "title": title,
            "canonical_url": f"https://arxiv.org/abs/{arxiv_id}",
            "abs_url": f"https://arxiv.org/abs/{arxiv_id}",
            "pdf_url": f"https://arxiv.org/pdf/{arxiv_id}",
            "normalized_identity": f"arxiv:{arxiv_id}",
            "source_kind": "arxiv_api_metadata_and_local_pdf",
        },
        "source_strategy": {
            "primary_source_variant_id": f"{arxiv_id}:source:arxiv-api-metadata",
            "preferred_content_order": ["arxiv_pdf"],
            "metadata_order": ["arxiv_api_metadata"],
            "pdf_policy": "local_pdf_ingested_from_m061_acquisition",
            "fallback_policy": "use local PDF only; graph writes is not authorized and production import is not authorized",
            "parser_readiness": "not_claimed",
            "chunk_readiness": "not_claimed",
            "graph_readiness": "not_claimed",
        },
        "source_variants": [
            {
                "variant_id": f"{arxiv_id}:source:arxiv-api-metadata",
                "source_role": "arxiv_api_metadata",
                "source_format": "json_metadata",
                "source_origin": "provider_api",
                "is_primary": True,
                "is_content_bearing": False,
                "is_metadata_only": True,
                "path": None,
                "url": f"https://arxiv.org/abs/{arxiv_id}",
                "media_type": "application/json",
                "capture_status": "metadata_detected",
                "capture_policy": "category_detection_only_network_override_documented",
                "loader_outcome": "not_loaded_metadata_only",
                "requires_conversion": False,
                "conversion_hint": None,
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": True,
                "parser_readiness_claimed": False,
                "chunk_readiness_claimed": False,
                "graph_readiness_claimed": False,
            },
            {
                "variant_id": f"{arxiv_id}:source:arxiv-pdf",
                "source_role": "arxiv_pdf",
                "source_format": "pdf",
                "source_origin": "m061_local_acquisition",
                "is_primary": False,
                "is_content_bearing": True,
                "is_metadata_only": False,
                "path": rel_pdf_path,
                "url": f"https://arxiv.org/pdf/{arxiv_id}",
                "media_type": "application/pdf",
                "capture_status": "captured_local",
                "capture_policy": "local_copy_from_artifacts_m061_2hop_no_additional_pdf_download",
                "loader_outcome": "not_loaded",
                "requires_conversion": True,
                "conversion_hint": "future_pdf_to_markdown_conversion_before_parser_or_graph_use",
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
                "parser_readiness_claimed": False,
                "chunk_readiness_claimed": False,
                "graph_readiness_claimed": False,
            },
        ],
        "expected_profile": {
            "should_load": False,
            "should_parse_text": False,
            "should_chunk": False,
            "graph_ready": False,
            "parser_ready": False,
            "chunk_ready": False,
            "known_risks": [
                "pdf_registered_without_parser_artifacts",
                "future_loader_must_replay_source_before_parser_or_graph_use",
                "not_safe_for_ladybugdb_or_production_import",
            ],
        },
        "safety_flags": dict(CATALOG_SAFETY_FLAGS),
        "safety_defaults": dict(SAFETY_DEFAULTS),
        "safety_override": dict(SAFETY_OVERRIDE),
    }


def write_article_record(article_path: Path, article: dict[str, Any]) -> None:
    article_path.parent.mkdir(parents=True, exist_ok=True)
    article_path.write_text(json.dumps(article, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def update_index_if_exists(catalog_root: Path = CATALOG_ROOT) -> tuple[bool, int | None, list[dict[str, Any]]]:
    index_path = catalog_root / "article_catalog" / "index.json"
    if not index_path.exists():
        return False, None, []
    scripts_dir = ROOT / "scripts"
    if str(scripts_dir) not in sys.path:
        sys.path.insert(0, str(scripts_dir))
    from verify_m025_article_catalog import rebuild_index_from_articles

    existing = json.loads(index_path.read_text(encoding="utf-8"))
    catalog_manifest_path = catalog_root / "catalog.json"
    rebuilt, diagnostics = rebuild_index_from_articles(catalog_manifest_path, existing)
    index_path.write_text(json.dumps(rebuilt, indent=2, sort_keys=False) + "\n", encoding="utf-8")
    return True, len(rebuilt.get("articles", [])) if isinstance(rebuilt.get("articles"), list) else None, diagnostics


def ingest_catalog(
    *,
    m061_root: Path = M061_ROOT,
    arxiv_root: Path = CANONICAL_ARXIV_ROOT,
    fetcher: Callable[[str], ArxivMetadata] | None = None,
    sleep: Callable[[float], None] = time.sleep,
    update_index: bool = True,
) -> IngestResult:
    anchor_ids = load_selected_ids(m061_root)
    pdf_paths = load_pdf_paths(m061_root)
    membership = invert_anchor_membership(anchor_ids)
    unique_ids = sorted(membership)
    missing_pdfs = [arxiv_id for arxiv_id in unique_ids if arxiv_id not in pdf_paths]
    if missing_pdfs:
        raise FileNotFoundError(f"Missing M061 PDFs for selected arxiv_ids: {', '.join(missing_pdfs)}")

    before_count = catalog_pdf_count(arxiv_root)
    metrics = ApiMetrics()
    pacer = RequestPacer(sleep=sleep)

    def default_fetcher(arxiv_id: str) -> ArxivMetadata:
        return fetch_arxiv_metadata(arxiv_id, pacer=pacer, metrics=metrics, sleep=sleep)

    metadata_fetcher = fetcher or default_fetcher
    records: list[IngestRecord] = []

    for arxiv_id in unique_ids:
        source_pdf = sorted(pdf_paths[arxiv_id])[0]
        source_hash = sha256_file(source_pdf)
        existing_pdf = existing_catalog_pdf(arxiv_root, arxiv_id)

        if existing_pdf is not None:
            existing_hash = sha256_file(existing_pdf)
            category = existing_pdf.parents[2].name
            dest_pdf = existing_pdf
            article_path = dest_pdf.parents[1] / "article.json"
            if existing_hash == source_hash:
                title = f"arXiv {arxiv_id}"
                status = "skipped"
                message = "already present with matching SHA256"
                if not article_path.exists():
                    article = build_article_record(arxiv_id, category, title, dest_pdf)
                    write_article_record(article_path, article)
                    status = "metadata_created"
                    message = "PDF already present; article.json created"
                records.append(
                    IngestRecord(
                        arxiv_id=arxiv_id,
                        anchor_ids=membership[arxiv_id],
                        source_pdf=source_pdf,
                        dest_pdf=dest_pdf,
                        category=category,
                        title=title,
                        status=status,
                        fallback=False,
                        source_sha256=source_hash,
                        dest_sha256=existing_hash,
                        message=message,
                    )
                )
                continue
            metadata = ArxivMetadata(arxiv_id=arxiv_id, category=category, title=f"arXiv {arxiv_id}", source="existing_catalog_category")
            status = "updated"
        else:
            metadata = metadata_fetcher(arxiv_id)
            status = "ingested"

        category = normalize_category(metadata.category)
        dest_pdf = arxiv_root / category / arxiv_id / "source" / f"{arxiv_id}.pdf"
        dest_pdf.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_pdf, dest_pdf)
        dest_hash = sha256_file(dest_pdf)
        article = build_article_record(arxiv_id, category, metadata.title, dest_pdf)
        write_article_record(dest_pdf.parents[1] / "article.json", article)
        records.append(
            IngestRecord(
                arxiv_id=arxiv_id,
                anchor_ids=membership[arxiv_id],
                source_pdf=source_pdf,
                dest_pdf=dest_pdf,
                category=category,
                title=metadata.title,
                status=status,
                fallback=metadata.fallback,
                source_sha256=source_hash,
                dest_sha256=dest_hash,
                message=metadata.error or metadata.source,
            )
        )

    metrics.pacing_delay_seconds = pacer.total_delay_seconds
    if update_index:
        index_updated, index_entries, index_diagnostics = update_index_if_exists(CATALOG_ROOT)
    else:
        index_updated, index_entries, index_diagnostics = False, None, []
    after_count = catalog_pdf_count(arxiv_root)
    return IngestResult(
        records=records,
        selected_total=sum(len(ids) for ids in anchor_ids.values()),
        discovered_pdf_total=sum(len(paths) for paths in pdf_paths.values()),
        unique_arxiv_ids=len(unique_ids),
        before_catalog_pdf_count=before_count,
        after_catalog_pdf_count=after_count,
        api_metrics=metrics,
        index_updated=index_updated,
        index_entries=index_entries,
        index_diagnostics=index_diagnostics,
    )


def per_anchor_counts(records: Iterable[IngestRecord]) -> dict[str, Counter[str]]:
    counters: dict[str, Counter[str]] = defaultdict(Counter)
    for record in records:
        for anchor_id in record.anchor_ids:
            counters[anchor_id]["total"] += 1
            if record.status in {"ingested", "updated", "metadata_created"}:
                counters[anchor_id]["ingested"] += 1
            if record.status == "skipped":
                counters[anchor_id]["skipped"] += 1
            if record.fallback:
                counters[anchor_id]["fallback"] += 1
    return dict(sorted(counters.items()))


def render_report(result: IngestResult, report_path: Path = REPORT_PATH) -> str:
    status_counts = Counter(record.status for record in result.records)
    fallback_count = sum(1 for record in result.records if record.fallback)
    category_counts = Counter(record.category for record in result.records)
    bucket_counts = Counter(report_bucket(record.category) for record in result.records)
    anchor_counts = per_anchor_counts(result.records)
    lines: list[str] = []
    lines.append("# M061 S04 canonical catalog ingestion report")
    lines.append("")
    lines.append("## 0. Резюме")
    lines.append("")
    lines.append(
        f"Заявленный объём в задаче: 151 PDFs processed; фактически обнаружено и обработано локальных PDF-копий: "
        f"{result.discovered_pdf_total}. Уникальных arxiv_id: {result.unique_arxiv_ids}."
    )
    lines.append(
        f"Уникальные записи: ingested={status_counts.get('ingested', 0)}, updated={status_counts.get('updated', 0)}, "
        f"metadata_created={status_counts.get('metadata_created', 0)}, skipped={status_counts.get('skipped', 0)}, fallback={fallback_count}."
    )
    lines.append("Graph writes is not authorized; production import is not authorized; LLM calls are disabled.")
    lines.append("")
    lines.append("## 1. Per-arxiv_id")
    lines.append("")
    lines.append("| arxiv_id | anchors | source | dest | category | status | fallback |")
    lines.append("|---|---|---|---|---|---|---|")
    for record in sorted(result.records, key=lambda item: item.arxiv_id):
        source_rel = record.source_pdf.relative_to(ROOT).as_posix() if record.source_pdf.is_absolute() else record.source_pdf.as_posix()
        dest_rel = record.dest_pdf.relative_to(ROOT).as_posix() if record.dest_pdf.is_absolute() else record.dest_pdf.as_posix()
        lines.append(
            f"| {record.arxiv_id} | {', '.join(record.anchor_ids)} | `{source_rel}` | `{dest_rel}` | "
            f"{record.category} | {record.status} | {str(record.fallback).lower()} |"
        )
    lines.append("")
    lines.append("## 2. Per-anchor")
    lines.append("")
    lines.append("| anchor | total | ingested | skipped | fallback |")
    lines.append("|---|---:|---:|---:|---:|")
    for anchor_id, counts in anchor_counts.items():
        lines.append(
            f"| {anchor_id} | {counts.get('total', 0)} | {counts.get('ingested', 0)} | "
            f"{counts.get('skipped', 0)} | {counts.get('fallback', 0)} |"
        )
    lines.append("")
    lines.append("## 3. arxiv API metrics")
    lines.append("")
    lines.append(f"- requests made: {result.api_metrics.requests_made}")
    lines.append(f"- 429s: {result.api_metrics.rate_limit_429s}")
    lines.append(f"- pacing delay seconds: {result.api_metrics.pacing_delay_seconds:.1f}")
    lines.append(f"- retry delay seconds: {result.api_metrics.retry_delay_seconds:.1f}")
    lines.append(f"- failures: {result.api_metrics.failures}")
    lines.append(f"- user agent: `{ARXIV_USER_AGENT}`")
    lines.append("")
    lines.append("## 4. Канонический каталог")
    lines.append("")
    lines.append(f"- PDF count: {result.before_catalog_pdf_count} -> {result.after_catalog_pdf_count}")
    lines.append(f"- index.json updated: {str(result.index_updated).lower()}")
    lines.append(f"- index entries: {result.index_entries if result.index_entries is not None else 'n/a'}")
    lines.append("- category distribution:")
    for category, count in sorted(category_counts.items()):
        lines.append(f"  - {category}: {count}")
    lines.append("- report buckets:")
    for bucket in [*KNOWN_REPORT_BUCKETS, "other"]:
        lines.append(f"  - {bucket}: {bucket_counts.get(bucket, 0)}")
    if result.index_diagnostics:
        lines.append("- index rebuild diagnostics:")
        for diagnostic in result.index_diagnostics[:20]:
            lines.append(f"  - {diagnostic}")
    lines.append("")
    lines.append("## 5. Lessons + next steps")
    lines.append("")
    lines.append("- Входные selected JSON содержат 150 выбранных позиций, не 151; расхождение сохранено как S04 deviation.")
    lines.append("- Повторы между anchor-ациями схлопнуты в 32 уникальных arxiv_id; повторный запуск безопасен и пропускает matching SHA256 без сети.")
    lines.append("- Следующий шаг: отдельным milestone/slice запускать parser/chunker; текущий S04 не заявляет parser, chunk или graph readiness.")
    text = "\n".join(lines) + "\n"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(text, encoding="utf-8")
    return text


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-index", action="store_true", help="Do not update index.json after ingestion")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    result = ingest_catalog(update_index=not args.no_index)
    render_report(result)
    status_counts = Counter(record.status for record in result.records)
    fallback_count = sum(1 for record in result.records if record.fallback)
    print(f"processed_pdf_copies={result.discovered_pdf_total}")
    print(f"unique_arxiv_ids={result.unique_arxiv_ids}")
    print(f"ingested={status_counts.get('ingested', 0) + status_counts.get('updated', 0) + status_counts.get('metadata_created', 0)}")
    print(f"skipped={status_counts.get('skipped', 0)}")
    print(f"fallback={fallback_count}")
    print(f"arxiv_api_requests={result.api_metrics.requests_made}")
    print(f"arxiv_api_429s={result.api_metrics.rate_limit_429s}")
    print(f"catalog_pdf_count={result.before_catalog_pdf_count}->{result.after_catalog_pdf_count}")
    print(f"report={REPORT_PATH.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
