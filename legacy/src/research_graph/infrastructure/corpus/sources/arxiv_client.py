# Formerly: src/arxiv_archive/arxiv_client.py

"""Arxiv API client with typed retry/backoff diagnostics (M199 S01).

Infrastructure adapter only: talks to export.arxiv.org via httpx, returns
typed ``ArxivPaper`` rows or raises ``ArxivFetchError``. Retry schedule and
Retry-After parsing mirror the established ``catalog_ingest`` pattern without
importing that module (keeps corpus/sources free of ingestion coupling).
"""

from __future__ import annotations

import email.utils
import time
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from datetime import date
from typing import Literal

import feedparser
import httpx
from loguru import logger

ARXIV_API_URL = "https://export.arxiv.org/api/query"
ARXIV_USER_AGENT = "daily-archive/1.0 (mailto: contact)"
ARXIV_MAX_RETRY_ATTEMPTS = 3
ARXIV_BACKOFF_SECONDS: tuple[float, ...] = (1.0, 5.0, 15.0, 60.0, 300.0)

ArxivErrorCode = Literal[
    "ARXIV_TIMEOUT",
    "ARXIV_CONNECT",
    "ARXIV_429",
    "ARXIV_5XX",
    "ARXIV_4XX",
    "ARXIV_PARSE",
]
ArxivOutcome = Literal["recovered", "exhausted"]


@dataclass
class ArxivPaper:
    id: str
    title: str
    abstract: str
    authors: list[str]
    published: date
    updated: date
    categories: list[str]
    pdf_url: str


@dataclass
class ApiMetrics:
    """Per-category counters for arxiv API traffic."""

    requests_made: int = 0
    rate_limit_429s: int = 0
    retry_delay_seconds: float = 0.0
    failures: int = 0


@dataclass
class RequestPacer:
    """Minimal single-flight bookkeeping for retry delay totals.

    S01 does not enforce inter-request min-interval pacing (catalog_ingest does);
    this type only accumulates retry delays so diagnostics stay typed.
    """

    total_delay_seconds: float = 0.0


class ArxivFetchError(RuntimeError):
    """Typed, redacted arxiv API failure with retry diagnostics."""

    def __init__(
        self,
        *,
        code: ArxivErrorCode,
        message: str,
        retry_count: int,
        outcome: ArxivOutcome,
        service: str = "arxiv_api",
        category: str | None = None,
    ) -> None:
        self.code = code
        self.service = service
        self.message = message
        self.retry_count = retry_count
        self.outcome = outcome
        self.category = category
        super().__init__(self.diagnostic)

    @property
    def diagnostic(self) -> str:
        """Compact redacted diagnostic for state.json / logs (no raw payload)."""
        base = f"{self.service}:{self.code} {self.outcome} after {self.retry_count} retries"
        if self.category:
            return f"{base} category={self.category}: {self.message}"
        return f"{base}: {self.message}"


def parse_retry_after(value: str | None) -> float | None:
    """Parse Retry-After header (delta-seconds or HTTP-date) into seconds."""
    if not value:
        return None
    try:
        return max(0.0, float(value))
    except ValueError:
        try:
            parsed = email.utils.parsedate_to_datetime(value)
        except (ValueError, TypeError):
            return None
        if parsed is None:
            return None
        return max(0.0, parsed.timestamp() - time.time())


def _classify_http_status(status_code: int) -> tuple[ArxivErrorCode, bool]:
    """Return (error_code, is_transient)."""
    if status_code == 429:
        return "ARXIV_429", True
    if 500 <= status_code <= 599:
        return "ARXIV_5XX", True
    return "ARXIV_4XX", False


def _redacted_http_message(status_code: int) -> str:
    return f"HTTP {status_code}"


class ArxivClient:
    def __init__(
        self,
        *,
        sleep: Callable[[float], None] = time.sleep,
        transport: httpx.BaseTransport | None = None,
        timeout: float = 60.0,
    ) -> None:
        self._sleep = sleep
        self._transport = transport
        self._timeout = timeout
        self.last_metrics: dict[str, ApiMetrics] = {}

    def fetch_papers(
        self,
        start_date: date,
        end_date: date | None = None,
        categories: list[str] | None = None,
    ) -> list[ArxivPaper]:
        if categories is None:
            categories = []

        seen_ids: set[str] = set()
        papers: list[ArxivPaper] = []
        for category in categories:
            for paper in self._fetch_category(category, start_date, end_date):
                if paper.id not in seen_ids:
                    seen_ids.add(paper.id)
                    papers.append(paper)
        return papers

    def _fetch_category(
        self, category: str, start_date: date, end_date: date | None = None
    ) -> Iterator[ArxivPaper]:
        end = end_date if end_date else start_date
        query_parts = [
            f"cat:{category}",
            f"submittedDate:[{start_date.strftime('%Y%m%d')} TO {end.strftime('%Y%m%d')}]",
        ]
        query = " AND ".join(query_parts)
        metrics = ApiMetrics()
        self.last_metrics[category] = metrics

        params = {"search_query": query, "start": 0, "max_results": 100}
        headers = {"User-Agent": ARXIV_USER_AGENT}
        last_code: ArxivErrorCode = "ARXIV_PARSE"
        last_message = "unknown arxiv API failure"
        recovered = False
        retry_count = 0

        if self._transport is not None:
            client = httpx.Client(
                timeout=self._timeout,
                headers=headers,
                transport=self._transport,
            )
        else:
            client = httpx.Client(timeout=self._timeout, headers=headers)
        try:
            for attempt in range(ARXIV_MAX_RETRY_ATTEMPTS + 1):
                metrics.requests_made += 1
                try:
                    response = client.get(ARXIV_API_URL, params=params)
                except httpx.TimeoutException as exc:
                    last_code = "ARXIV_TIMEOUT"
                    last_message = type(exc).__name__
                    if attempt >= ARXIV_MAX_RETRY_ATTEMPTS:
                        break
                    delay = ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
                    retry_count += 1
                    logger.warning(
                        "arxiv_api retry code={} attempt={} delay={} category={}",
                        last_code,
                        attempt + 1,
                        delay,
                        category,
                    )
                    self._sleep(delay)
                    metrics.retry_delay_seconds += delay
                    continue
                except httpx.ConnectError as exc:
                    last_code = "ARXIV_CONNECT"
                    last_message = type(exc).__name__
                    if attempt >= ARXIV_MAX_RETRY_ATTEMPTS:
                        break
                    delay = ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
                    retry_count += 1
                    logger.warning(
                        "arxiv_api retry code={} attempt={} delay={} category={}",
                        last_code,
                        attempt + 1,
                        delay,
                        category,
                    )
                    self._sleep(delay)
                    metrics.retry_delay_seconds += delay
                    continue
                except httpx.HTTPError as exc:
                    last_code = "ARXIV_CONNECT"
                    last_message = type(exc).__name__
                    if attempt >= ARXIV_MAX_RETRY_ATTEMPTS:
                        break
                    delay = ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
                    retry_count += 1
                    logger.warning(
                        "arxiv_api retry code={} attempt={} delay={} category={}",
                        last_code,
                        attempt + 1,
                        delay,
                        category,
                    )
                    self._sleep(delay)
                    metrics.retry_delay_seconds += delay
                    continue

                status = response.status_code
                if status >= 400:
                    code, transient = _classify_http_status(status)
                    last_code = code
                    last_message = _redacted_http_message(status)
                    if code == "ARXIV_429":
                        metrics.rate_limit_429s += 1
                    if not transient:
                        metrics.failures += 1
                        logger.error(
                            "arxiv_api exhausted code={} retries={} category={}",
                            code,
                            0,
                            category,
                        )
                        raise ArxivFetchError(
                            code=code,
                            message=last_message,
                            retry_count=0,
                            outcome="exhausted",
                            category=category,
                        )
                    if attempt >= ARXIV_MAX_RETRY_ATTEMPTS:
                        break
                    retry_after = parse_retry_after(response.headers.get("Retry-After"))
                    delay = (
                        retry_after
                        if retry_after is not None
                        else ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
                    )
                    retry_count += 1
                    logger.warning(
                        "arxiv_api retry code={} attempt={} delay={} category={}",
                        code,
                        attempt + 1,
                        delay,
                        category,
                    )
                    self._sleep(delay)
                    metrics.retry_delay_seconds += delay
                    continue

                try:
                    feed = feedparser.parse(response.text)
                    entries = list(feed.entries)
                except Exception as exc:  # noqa: BLE001 — parse boundary
                    last_code = "ARXIV_PARSE"
                    last_message = type(exc).__name__
                    metrics.failures += 1
                    logger.error(
                        "arxiv_api exhausted code={} retries={} category={}",
                        last_code,
                        retry_count,
                        category,
                    )
                    raise ArxivFetchError(
                        code=last_code,
                        message=last_message,
                        retry_count=retry_count,
                        outcome="exhausted",
                        category=category,
                    ) from exc

                if attempt > 0:
                    recovered = True
                if recovered:
                    # Surface recovery via metrics only; yield proceeds normally.
                    logger.warning(
                        "arxiv_api recovered code={} retries={} category={}",
                        last_code,
                        retry_count,
                        category,
                    )
                for entry in entries:
                    yield self._parse_entry(entry)
                return

            metrics.failures += 1
            logger.error(
                "arxiv_api exhausted code={} retries={} category={}",
                last_code,
                retry_count,
                category,
            )
            raise ArxivFetchError(
                code=last_code,
                message=last_message,
                retry_count=retry_count,
                outcome="exhausted",
                category=category,
            )
        finally:
            client.close()

    def _parse_entry(self, entry) -> ArxivPaper:
        paper_id = entry.id.split("/")[-1] if "/" in entry.id else entry.id
        title = entry.title
        abstract = entry.summary
        authors = [author.name for author in entry.authors]
        published = date.fromisoformat(entry.published[:10])
        updated = date.fromisoformat(entry.updated[:10]) if hasattr(entry, "updated") else published
        categories = [tag.term for tag in entry.tags] if hasattr(entry, "tags") else []
        pdf_url = ""
        for link in entry.links:
            if link.type == "application/pdf":
                pdf_url = link.href
                break
        return ArxivPaper(
            id=paper_id,
            title=title,
            abstract=abstract,
            authors=authors,
            published=published,
            updated=updated,
            categories=categories,
            pdf_url=pdf_url,
        )
