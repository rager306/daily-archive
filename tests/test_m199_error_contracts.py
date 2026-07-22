"""M199 S04: cross-service error-contract smoke (import/assert, no network)."""

from datetime import date

from research_graph.infrastructure.corpus.sources.arxiv_client import (
    ARXIV_BACKOFF_SECONDS,
    ARXIV_MAX_RETRY_ATTEMPTS,
    ArxivFetchError,
)
from research_graph.infrastructure.corpus.sources.markdown_converter import (
    ARXIV2MD_BACKOFF_SECONDS,
    ARXIV2MD_MAX_RETRY_ATTEMPTS,
)
from research_graph.infrastructure.evaluation.scoring import (
    DEFAULT_WEIGHTS,
    SEMANTIC_SCHOLAR_INTEGRATION,
    ScoringEngine,
)
from research_graph.infrastructure.retrieval.embedder import (
    DegradedEmbeddingSignal,
    FdAuthError,
    FdDegradedEmbeddingsError,
    is_zero_embedding_batch,
    is_zero_vector,
    validate_fd_api_key,
)


def test_arxiv_fetch_error_diagnostic_shape():
    err = ArxivFetchError(
        code="ARXIV_5XX",
        message="HTTP 503",
        retry_count=3,
        outcome="exhausted",
        category="cs.AI",
    )
    assert err.service == "arxiv_api"
    assert "ARXIV_5XX" in err.diagnostic
    assert "exhausted" in err.diagnostic
    assert ARXIV_MAX_RETRY_ATTEMPTS == 3
    assert ARXIV_BACKOFF_SECONDS[0] == 1.0


def test_embedder_auth_and_degrade_contracts():
    with __import__("pytest").raises(FdAuthError) as ei:
        validate_fd_api_key(None)
    assert ei.value.code == "FD_AUTH_MISSING"
    assert is_zero_vector([0.0, 0.0])
    assert is_zero_embedding_batch([[0.0], [0.0]])
    assert not is_zero_embedding_batch([])
    signal = DegradedEmbeddingSignal(reason="circuit_open", batch_size=2, dimensions=4)
    assert "FD_DEGRADED_ZERO_VECTORS" in signal.diagnostic
    deg = FdDegradedEmbeddingsError(
        message="refuse",
        reason="circuit_open",
        batch_size=2,
        dimensions=4,
    )
    assert deg.code == "FD_DEGRADED_ZERO_VECTORS"
    assert "refuse" in deg.diagnostic


def test_md_converter_retry_constants():
    assert ARXIV2MD_MAX_RETRY_ATTEMPTS == 3
    assert ARXIV2MD_BACKOFF_SECONDS == (1.0, 5.0, 15.0)


def test_scoring_recency_and_semantic_scholar_status():
    assert SEMANTIC_SCHOLAR_INTEGRATION == "disabled_not_wired_in_cli"
    assert DEFAULT_WEIGHTS["citations"] == 0.0
    assert abs(sum(DEFAULT_WEIGHTS.values()) - 1.0) < 1e-9
    engine = ScoringEngine()
    from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivPaper

    paper = ArxivPaper(
        id="2501.1",
        title="t",
        abstract="a",
        authors=["a"],
        published=date(2026, 5, 14),
        updated=date(2026, 5, 14),
        categories=["cs.AI"],
        pdf_url="https://arxiv.org/pdf/2501.1.pdf",
    )
    scored = engine.score(paper, None, ["kw"], run_date=date(2026, 5, 14))
    assert scored.breakdown["recency"] == 10.0


def test_operator_error_contract_doc_exists():
    from pathlib import Path

    doc = Path("artifacts/m199-pipeline-service-error-handling/error-contracts.md")
    assert doc.is_file(), f"missing operator doc: {doc}"
    text = doc.read_text(encoding="utf-8")
    for needle in (
        "ArxivFetchError",
        "FdAuthError",
        "ARXIV2MD",
        "SEMANTIC_SCHOLAR",
        "run_date",
        "FD_DEGRADED_ZERO_VECTORS",
    ):
        assert needle in text, f"doc missing {needle}"
