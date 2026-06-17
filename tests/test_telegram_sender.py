"""Tests for Telegram sender."""

from datetime import date

from research_graph.corpus.sources.arxiv_client import ArxivPaper
from arxiv_archive.scoring import ScoredPaper
from research_graph.corpus.sources.semantic_scholar import SemanticScholarPaper
from research_graph.retrieval.summarizer import PaperSummary
from research_graph.ops.notifications.telegram_sender import TelegramSender


class TestTelegramSender:
    """Tests for TelegramSender class."""

    def test_telegram_sender_init(self):
        """Test creating a TelegramSender with test token and chat."""
        sender = TelegramSender(
            bot_token="test_token_12345",
            chat_id="test_chat_67890",
        )
        assert sender.bot_token == "test_token_12345"
        assert sender.chat_id == "test_chat_67890"
        assert sender.base_url == "https://api.telegram.org"

    def test_telegram_sender_custom_base_url(self):
        """Test creating a TelegramSender with custom base URL."""
        sender = TelegramSender(
            bot_token="test_token",
            chat_id="test_chat",
            base_url="https://custom.api.example.com",
        )
        assert sender.base_url == "https://custom.api.example.com"

    def test_format_digest(self):
        """Test _format_digest returns string with expected content."""
        # Create mock ArxivPaper
        paper = ArxivPaper(
            id="2501.12345",
            title="Test Paper Title",
            abstract="Test abstract",
            authors=["Author One", "Author Two"],
            published=date(2025, 1, 15),
            updated=date(2025, 1, 15),
            categories=["cs.AI", "cs.LG"],
            pdf_url="https://arxiv.org/pdf/2501.12345",
        )

        # Create mock SemanticScholarPaper
        semschol = SemanticScholarPaper(
            arxiv_id="2501.12345",
            title="Test Paper Title",
            citation_count=42,
            year=2025,
            venue="arXiv",
        )

        # Create mock ScoredPaper
        scored_paper = ScoredPaper(
            paper=paper,
            semschol=semschol,
            keywords=["machine learning", "neural networks"],
            score=7.5,
            breakdown={"citations": 4.2, "recency": 8.0, "novelty": 3.0, "preference": 1.2, "graph_bridge": 0.0},
        )

        # Create mock PaperSummary
        summary = PaperSummary(
            headline="A breakthrough in AI learning",
            what_it_does="Uses new architecture to learn faster.",
            why_it_matters="It could revolutionize how models train.",
            analogy="Think of it like teaching a dog new tricks in minutes instead of hours.",
        )

        # Create sender
        sender = TelegramSender(
            bot_token="test_token",
            chat_id="test_chat",
        )

        # Format digest
        result = sender._format_digest([scored_paper], {"2501.12345": summary})

        # Verify content
        assert "📄 ArXiv Daily Archive" in result
        assert "2501.12345" in result
        assert "Test Paper Title" in result
        assert "HEADLINE:" in result
        assert "WHY IT MATTERS:" in result
        assert "💬 ANALOGY:" in result
        assert "42 citations" in result
        assert "arxiv.org/abs/2501.12345" in result
