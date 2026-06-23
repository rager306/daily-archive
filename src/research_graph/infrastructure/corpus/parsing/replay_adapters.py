"""Infrastructure adapters for parser replay application ports.

The adapters preserve current R024/M121 parser replay behavior while keeping
filesystem, PDF extraction, parser, and page-index details outside the
application use case.
"""

from __future__ import annotations

import json
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import fitz

from research_graph.application.corpus.parser_replay import ParserReplayResult
from research_graph.domain.corpus import (
    ParserReplayArticle,
    ParserReplayChunkWriteResult,
    ParserReplayDiagnosticCode,
    ParserReplayParsedArticle,
    ParserReplayRecord,
    ParserReplaySource,
    ParserReplaySourceOutcome,
    ParserReplayStatus,
)
from research_graph.infrastructure.corpus.ingestion import (
    FullTextSource,
    assess_full_text_quality,
    ingest_full_text,
)
from research_graph.infrastructure.corpus.parsing.parser import parse_article
from research_graph.infrastructure.corpus.parsing.structure import ParsedArticle
from research_graph.infrastructure.papers.indexing.parsed_page_index import (
    build_page_index_from_parsed,
)


class CatalogIndexArticleSelector:
    """Select parser replay articles from canonical article_catalog/index.json."""

    def __init__(self, index_path: Path | str) -> None:
        self.index_path = Path(index_path)

    def selected_articles(self) -> list[ParserReplayArticle]:
        payload = _load_json(self.index_path)
        articles = payload.get("articles", [])
        if not isinstance(articles, list):
            raise ValueError(f"{self.index_path} articles must be a list")
        selected: list[ParserReplayArticle] = []
        for item in articles:
            if not isinstance(item, dict):
                continue
            selected.append(
                ParserReplayArticle(
                    article_id=str(item["article_key"]),
                    article_ref=str(item["article_ref"]),
                    article_path=str(item["article_path"]),
                )
            )
        return selected


class SelectionJsonArticleSelector:
    """Select parser replay articles from an R024 selection.json file."""

    def __init__(self, selection_path: Path | str) -> None:
        self.selection_path = Path(selection_path)

    def selected_articles(self) -> list[ParserReplayArticle]:
        payload = _load_json(self.selection_path)
        articles = payload.get("articles", [])
        if not isinstance(articles, list):
            raise ValueError(f"{self.selection_path} articles must be a list")
        selected: list[ParserReplayArticle] = []
        for item in articles:
            if not isinstance(item, dict):
                continue
            article_ref = str(item["article_ref"])
            selected.append(
                ParserReplayArticle(
                    article_id=str(item["article_key"]),
                    article_ref=article_ref,
                    article_path=str(item.get("article_path") or f"article_catalog/{article_ref}/article.json"),
                    source_kind_hint=str(item["source_kind"]) if "source_kind" in item else None,
                    text_path_hint=str(item["text_path"]) if "text_path" in item else None,
                )
            )
        return selected


class FilesystemParserReplaySourceLoader:
    """Resolve local PDF, Markdown, HTML, or text sources for parser replay."""

    def __init__(
        self,
        *,
        catalog_parent: Path | str,
        cache_dir: Path | str,
        repo_root: Path | str | None = None,
        prefer_pdf: bool = True,
    ) -> None:
        self.catalog_parent = Path(catalog_parent)
        self.cache_dir = Path(cache_dir)
        self.repo_root = Path(repo_root) if repo_root is not None else self.catalog_parent.parents[1]
        self.prefer_pdf = prefer_pdf

    def load_source(self, article: ParserReplayArticle) -> ParserReplaySourceOutcome:
        if article.text_path_hint:
            source_path = self._resolve_repo_path(article.text_path_hint)
            text_chars = len(source_path.read_text(encoding="utf-8", errors="replace"))
            return self._completed_or_low_quality(
                article=article,
                source_kind=article.source_kind_hint or "html_native",
                source_path=source_path,
                cache_reused=True,
                text_chars=text_chars,
                pdf_pages=0,
            )

        cache_path = self.cache_dir / f"{article.article_id}.txt"
        if article.source_kind_hint == "pdf_converted" and cache_path.exists():
            text = cache_path.read_text(encoding="utf-8", errors="replace")
            return self._completed_or_low_quality(
                article=article,
                source_kind="pdf_converted",
                source_path=cache_path,
                cache_reused=True,
                text_chars=len(text),
                pdf_pages=text.count("\n\f\n") + 1,
            )

        article_path = self._resolve_catalog_path(article.article_path)
        article_payload = _load_json(article_path)
        pdf_path = self._find_pdf_source(article_payload) if self.prefer_pdf or article.source_kind_hint == "pdf_converted" else None
        if pdf_path is not None:
            text_path, cache_reused, text_chars, pdf_pages = self._extract_pdf_text(
                pdf_path,
                cache_path,
            )
            return self._completed_or_low_quality(
                article=article,
                source_kind="pdf_converted",
                source_path=text_path,
                cache_reused=cache_reused,
                text_chars=text_chars,
                pdf_pages=pdf_pages,
            )

        source_path = self._find_text_source(article_path.parent)
        if source_path is None:
            return ParserReplaySourceOutcome(
                status=ParserReplayStatus.SKIPPED,
                reason=ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE,
                message="metadata_only_no_local_source_artifact",
                path=article_path.as_posix(),
            )

        text_chars = len(source_path.read_text(encoding="utf-8", errors="replace"))
        return self._completed_or_low_quality(
            article=article,
            source_kind="html_native",
            source_path=source_path,
            cache_reused=True,
            text_chars=text_chars,
            pdf_pages=0,
        )

    def _completed_or_low_quality(
        self,
        *,
        article: ParserReplayArticle,
        source_kind: str,
        source_path: Path,
        cache_reused: bool,
        text_chars: int,
        pdf_pages: int,
    ) -> ParserReplaySourceOutcome:
        text = source_path.read_text(encoding="utf-8", errors="replace")
        quality = assess_full_text_quality(text)
        source = ParserReplaySource(
            article_id=article.article_id,
            paper_id=article.article_ref,
            source_kind=source_kind,
            source_path=source_path.as_posix(),
            text_chars=text_chars,
            cache_reused=cache_reused,
            pdf_pages=pdf_pages,
        )
        if quality.status != "ok":
            return ParserReplaySourceOutcome(
                status=ParserReplayStatus.LOW_QUALITY,
                source=source,
                reason=ParserReplayDiagnosticCode.LOW_QUALITY_SOURCE,
                message=f"fallback_reason={quality.fallback_reason or quality.status}",
                path=source_path.as_posix(),
            )
        return ParserReplaySourceOutcome(status=ParserReplayStatus.COMPLETED, source=source)

    def _resolve_catalog_path(self, path_value: str | None) -> Path:
        if not path_value:
            raise ValueError("empty article path")
        path = Path(path_value)
        return path if path.is_absolute() else self.catalog_parent / path

    def _resolve_repo_path(self, path_value: str) -> Path:
        path = Path(path_value)
        return path if path.is_absolute() else self.repo_root / path

    def _find_pdf_source(self, article: dict[str, Any]) -> Path | None:
        variants = article.get("source_variants", [])
        if not isinstance(variants, list):
            return None
        for variant in variants:
            if not isinstance(variant, dict):
                continue
            if variant.get("source_format") != "pdf":
                continue
            path_value = variant.get("path")
            if not path_value:
                continue
            path = self._resolve_catalog_path(str(path_value))
            if path.exists():
                return path
        return None

    @staticmethod
    def _find_text_source(article_dir: Path) -> Path | None:
        for ext in ("*.md", "*.markdown", "*.html", "*.txt"):
            matches = list(article_dir.rglob(ext))
            if matches:
                for preferred in ("abs.html", "article.html"):
                    for match in matches:
                        if match.name == preferred:
                            return match
                return matches[0]
        return None

    @staticmethod
    def _extract_pdf_text(pdf_path: Path, cache_path: Path) -> tuple[Path, bool, int, int]:
        if cache_path.exists() and cache_path.stat().st_size > 0:
            text = cache_path.read_text(encoding="utf-8", errors="replace")
            return cache_path, True, len(text), text.count("\n\f\n") + 1

        cache_path.parent.mkdir(parents=True, exist_ok=True)
        page_texts: list[str] = []
        with fitz.open(pdf_path) as doc:
            page_count = doc.page_count
            for page in doc:
                page_text = page.get_text("text")
                page_texts.append(page_text if isinstance(page_text, str) else str(page_text))
        text = "\n\f\n".join(page_texts).strip()
        if not text:
            raise ValueError(f"no extractable text from PDF: {pdf_path}")
        cache_path.write_text(text, encoding="utf-8")
        return cache_path, False, len(text), page_count


class ExistingFullTextParserAdapter:
    """Adapter from parser replay source descriptors to existing parser output."""

    def parse(self, source: ParserReplaySource) -> ParserReplayParsedArticle:
        ingestion = ingest_full_text(
            FullTextSource(
                paper_id=source.paper_id or source.article_id,
                source_type="text",
                source_path=Path(source.source_path),
            )
        )
        parsed = parse_article(ingestion)
        return ParserReplayParsedArticle(
            article_id=source.article_id,
            payload=parsed,
            section_count=len(parsed.elements),
            parser_warnings=list(parsed.validation_warnings),
        )


class PageIndexChunkWriterAdapter:
    """Build page-index chunk counts and optional compact output artifacts."""

    def __init__(self, output_dir: Path | str | None = None) -> None:
        self.output_dir = Path(output_dir) if output_dir is not None else None

    def write_chunks(
        self,
        article: ParserReplayArticle,
        source: ParserReplaySource,
        parsed: ParserReplayParsedArticle,
    ) -> ParserReplayChunkWriteResult:
        if not isinstance(parsed.payload, ParsedArticle):
            raise TypeError("parsed payload must be ParsedArticle")
        page_index = build_page_index_from_parsed(parsed.payload)
        chunk_count = len(page_index.nodes) if hasattr(page_index, "nodes") else 0
        if chunk_count <= 0:
            raise ValueError(f"non-positive chunk count: {chunk_count}")
        output_paths: list[str] = []
        if self.output_dir is not None:
            output_path = self.output_dir / "page-index" / f"{article.article_id}.json"
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(
                json.dumps(
                    {
                        "article_id": article.article_id,
                        "article_ref": article.article_ref,
                        "source_path": source.source_path,
                        "chunk_count": chunk_count,
                        "nodes": [
                            {
                                "id": node.id,
                                "title": node.title,
                                "level": node.level,
                                "path": list(node.path),
                            }
                            for node in page_index.nodes
                        ],
                    },
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            output_paths.append(output_path.as_posix())
        return ParserReplayChunkWriteResult(
            chunk_count=chunk_count,
            output_paths=output_paths,
            warnings=list(page_index.validation_warnings),
        )


class ParserReplayArtifactWriter:
    """Write legacy-compatible parser replay events and summary JSON."""

    def __init__(
        self,
        *,
        output_dir: Path | str,
        events_log: Path | str,
        summary_path: Path | str,
        schema_version: str,
        repo_root: Path | str,
    ) -> None:
        self.output_dir = Path(output_dir)
        self.events_log = Path(events_log)
        self.summary_path = Path(summary_path)
        self.schema_version = schema_version
        self.repo_root = Path(repo_root)

    def write(self, result: ParserReplayResult) -> None:
        self.output_dir.mkdir(parents=True, exist_ok=True)
        events = [self._event_for(record) for record in result.records]
        with self.events_log.open("w", encoding="utf-8") as handle:
            for event in events:
                handle.write(json.dumps(event) + "\n")
        self.summary_path.write_text(
            json.dumps(self._summary_for(result), indent=2) + "\n",
            encoding="utf-8",
        )

    def _event_for(self, record: ParserReplayRecord) -> dict[str, object]:
        base = {
            "timestamp": datetime.now(UTC).isoformat(),
            "article_ref": record.article_ref,
            "article_key": record.article_id,
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
        }
        if record.status == ParserReplayStatus.COMPLETED:
            return {
                "event": "parser_chunking_complete",
                **base,
                "source_kind": record.source_kind,
                "text_source": self._display_path(record.source_path),
                "text_chars": record.text_chars,
                "pdf_pages": record.pdf_pages,
                "cache_reused": record.cache_reused,
                "chunk_count": record.chunk_count,
            }
        if record.status == ParserReplayStatus.SKIPPED:
            return {
                "event": "parser_chunking_skipped_metadata_only",
                **base,
                "source_kind": "metadata_only",
                "skip_reason": _legacy_skip_reason(record),
            }
        if record.status == ParserReplayStatus.LOW_QUALITY:
            return {
                "event": "parser_chunking_low_quality_source",
                **base,
                "source_kind": record.source_kind,
                "text_source": self._display_path(record.source_path),
                "skip_reason": record.message,
            }
        return {
            "event": "parser_chunking_error",
            **base,
            "error": record.message[:120],
        }

    def _summary_for(self, result: ParserReplayResult) -> dict[str, object]:
        chunk_counts = [record.chunk_count for record in result.records if record.chunk_count > 0]
        skip_reason_counts = Counter(
            _legacy_skip_reason(record)
            for record in result.records
            if record.status == ParserReplayStatus.SKIPPED
        )
        source_kind_counts = Counter(
            record.source_kind
            for record in result.records
            if record.status == ParserReplayStatus.COMPLETED and record.source_kind is not None
        )
        return {
            "schema_version": self.schema_version,
            "total": len(result.records),
            "ok": result.completed_count,
            "skipped": result.skipped_count,
            "low_quality": result.low_quality_count,
            "errors": result.failed_count,
            "source_kind_counts": dict(sorted(source_kind_counts.items())),
            "skip_reason_counts": dict(sorted(skip_reason_counts.items())),
            "chunk_count_min": min(chunk_counts) if chunk_counts else 0,
            "chunk_count_max": max(chunk_counts) if chunk_counts else 0,
            "chunk_count_total": sum(chunk_counts),
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
        }

    def _display_path(self, path_value: str | None) -> str | None:
        if path_value is None:
            return None
        path = Path(path_value)
        try:
            return path.relative_to(self.repo_root).as_posix()
        except ValueError:
            return path.as_posix()


def _legacy_skip_reason(record: ParserReplayRecord) -> str:
    return record.message or (record.reason.value if record.reason else "metadata_only_no_local_source_artifact")


def _load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return payload


__all__ = [
    "CatalogIndexArticleSelector",
    "ExistingFullTextParserAdapter",
    "FilesystemParserReplaySourceLoader",
    "PageIndexChunkWriterAdapter",
    "ParserReplayArtifactWriter",
    "SelectionJsonArticleSelector",
]
