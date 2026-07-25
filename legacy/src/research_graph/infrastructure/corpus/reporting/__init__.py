"""Corpus reporting infrastructure adapters."""

from research_graph.infrastructure.corpus.reporting.coverage_report import (
    CoverageReportWriteResult,
    FilesystemCoverageReportWriter,
)

__all__ = ["CoverageReportWriteResult", "FilesystemCoverageReportWriter"]
