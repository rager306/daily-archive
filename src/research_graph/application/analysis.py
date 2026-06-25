"""Application-owned analysis result DTOs."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime
from typing import Any, Literal

DailyAnalysisStatus = Literal["done", "empty"]


@dataclass(frozen=True)
class DailyAnalysis:
    """Normalized in-memory analysis result for one arXiv archive day."""

    run_date: date
    status: DailyAnalysisStatus
    papers_fetched: int
    papers: list[Any]
    top_papers: list[Any]
    analysis_timestamp: datetime
