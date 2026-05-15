"""Telegram delivery for arxiv daily archive digests."""

from dataclasses import dataclass
from datetime import date
from typing import Any

import httpx

from arxiv_archive.scoring import ScoredPaper
from arxiv_archive.summarizer import PaperSummary

NUMBER_EMOJIS = ["1️⃣", "2️⃣", "3️⃣", "4️⃣", "5️⃣", "6️⃣", "7️⃣", "8️⃣", "9️⃣", "🔟"]


@dataclass
class TelegramSender:
    """Sends arxiv digest messages to a Telegram chat."""

    bot_token: str
    chat_id: str
    base_url: str = "https://api.telegram.org"

    def send_digest(
        self, papers: list[ScoredPaper], summaries: dict[str, PaperSummary]
    ) -> None:
        """Send a digest of papers to the Telegram chat.

        Args:
            papers: List of scored papers to include in the digest.
            summaries: Dict mapping paper IDs to their summaries.
        """
        text = self._format_digest(papers, summaries)
        self._send_message(text)

    def _format_digest(
        self, papers: list[ScoredPaper], summaries: dict[str, PaperSummary]
    ) -> str:
        """Format papers into a Telegram message.

        Args:
            papers: List of scored papers to format.
            summaries: Dict mapping paper IDs to their summaries.

        Returns:
            Formatted Telegram message string.
        """
        lines = [f"📄 ArXiv Daily Archive — {date.today().isoformat()}"]

        for i, scored_paper in enumerate(papers):
            paper = scored_paper.paper
            summary = summaries.get(paper.id)
            emoji = NUMBER_EMOJIS[i] if i < len(NUMBER_EMOJIS) else f"{i + 1}."

            citations = scored_paper.semschol.citation_count if scored_paper.semschol else 0

            lines.append("")
            lines.append(f"{emoji} [{paper.id}] {paper.title}")
            if summary:
                lines.append(f"   HEADLINE: {summary.headline}")
                lines.append(f"   WHY IT MATTERS: {summary.why_it_matters}")
                lines.append(f"   💬 ANALOGY: {summary.analogy}")
            lines.append(f"   👍 {citations} citations | 📖 https://arxiv.org/abs/{paper.id}")

        return "\n".join(lines)

    def _send_message(self, text: str) -> None:
        """Send a message via the Telegram Bot API.

        Args:
            text: The message text to send.
        """
        url = f"{self.base_url}/bot{self.bot_token}/sendMessage"
        payload: dict[str, Any] = {
            "chat_id": self.chat_id,
            "text": text,
            "parse_mode": "HTML",
        }
        with httpx.Client() as client:
            response = client.post(url, json=payload)
            response.raise_for_status()
