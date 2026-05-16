"""Tests for the MiniMax summarizer module."""

import os
from pathlib import Path

import pytest
from dotenv import load_dotenv

from src.arxiv_archive.summarizer import MiniMaxSummarizer, PaperSummary


def test_paper_summary_dataclass() -> None:
    """Test PaperSummary dataclass creation and field access."""
    summary = PaperSummary(
        headline="A new method for neural network pruning",
        what_it_does="The paper introduces a novel pruning technique that removes redundant weights while maintaining accuracy.",
        why_it_matters="It enables more efficient deployment of large models on resource-constrained devices.",
        analogy="Think of it like trimming unnecessary branches from a tree to let light and nutrients reach the important parts.",
    )

    assert summary.headline == "A new method for neural network pruning"
    assert summary.what_it_does == (
        "The paper introduces a novel pruning technique that removes redundant weights while maintaining accuracy."
    )
    assert summary.why_it_matters == (
        "It enables more efficient deployment of large models on resource-constrained devices."
    )
    assert summary.analogy.startswith("Think of it like")


def test_summarize_parse() -> None:
    """Test _parse method extracts fields correctly from formatted text."""
    summarizer = MiniMaxSummarizer(api_key="test-key")

    response_text = """HEADLINE: Scaling laws for neural language models
WHAT IT DOES: The paper studies empirical scaling laws for language model performance on the cross-entropy loss.
The study shows power-law scaling with model size, dataset size, and compute budget.
WHY IT MATTERS: These findings help predict model performance and allocate compute resources efficiently.
ANALOGY: Think of it like how a larger engine typically provides more power, but with diminishing returns at some point."""

    summary = summarizer._parse(response_text)

    assert summary.headline == "Scaling laws for neural language models"
    assert summary.what_it_does == (
        "The paper studies empirical scaling laws for language model performance on the cross-entropy loss. "
        "The study shows power-law scaling with model size, dataset size, and compute budget."
    )
    assert summary.why_it_matters == (
        "These findings help predict model performance and allocate compute resources efficiently."
    )
    assert summary.analogy == "Think of it like how a larger engine typically provides more power, but with diminishing returns at some point."


def test_summarize_parse_single_line_what_it_does() -> None:
    """Test _parse handles what_it_does that might be on one line."""
    summarizer = MiniMaxSummarizer(api_key="test-key")

    # When model puts everything on one line (malformed)
    response_text = """HEADLINE: Test Paper
WHAT IT DOES: Single sentence description.
WHY IT MATTERS: Important contribution.
ANALOGY: Think of it like a test."""

    summary = summarizer._parse(response_text)

    assert summary.headline == "Test Paper"
    assert summary.what_it_does == "Single sentence description."
    assert summary.why_it_matters == "Important contribution."
    assert summary.analogy == "Think of it like a test."


def test_summarize_api_call() -> None:
    """Test actual API call to MiniMax (skip if no real API key)."""
    # Load .env so api_key is available outside of shell context
    load_dotenv()

    api_key = os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("ANTHROPIC_AUTH_TOKEN")

    # Fallback: read from .env file in project root
    if not api_key or api_key == "your_key_here":
        env_path = Path(__file__).parent.parent / ".env"
        if env_path.exists():
            for line in open(env_path):
                if line.startswith("ANTHROPIC_API_KEY=") or line.startswith("ANTHROPIC_AUTH_TOKEN="):
                    api_key = line.split("=", 1)[1].strip()
                    break

    # Skip if no real key
    if not api_key or api_key in ("", "your_key_here"):
        pytest.skip("ANTHROPIC_AUTH_TOKEN not set (use real key in .env or env)")

    summarizer = MiniMaxSummarizer(api_key=api_key)

    title = "Attention Is All You Need"
    abstract = (
        "We propose a new simple network architecture, the Transformer, "
        "based solely on attention mechanisms, dispensing with recurrence and convolutions entirely."
    )

    summary = summarizer.summarize(title, abstract)

    assert isinstance(summary, PaperSummary)
    assert summary.headline
    assert summary.what_it_does
    assert summary.why_it_matters
    assert summary.analogy.startswith("Think of it like")
