"""MiniMax-based paper summarizer for AI research papers."""

from dataclasses import dataclass

import anthropic


@dataclass
class PaperSummary:
    """Structured summary of an AI research paper.

    Attributes:
        headline: One sentence title capturing the paper's main contribution.
        what_it_does: Two sentences describing the paper's approach/method.
        why_it_matters: One sentence explaining the significance/impact.
        analogy: An accessible analogy starting with "Think of it like".
    """

    headline: str
    what_it_does: str
    why_it_matters: str
    analogy: str


PROMPT_TEMPLATE = """You are a research assistant summarizing AI papers. Format as:
HEADLINE: [one sentence]
WHAT IT DOES: [2 sentences]
WHY IT MATTERS: [1 sentence]
ANALOGY: [starts with "Think of it like"]

Paper to summarize:
Title: {title}
Abstract: {abstract}"""


class MiniMaxSummarizer:
    """Summarizer using MiniMax's Anthropic-compatible API.

    Uses the MiniMax-M2.7-highspeed model to generate structured
    summaries of AI research papers.
    """

    API_URL = "https://api.minimax.io/anthropic/v1/messages"
    MODEL = "MiniMax-M2.7-highspeed"
    MAX_TOKENS = 1024
    TEMPERATURE = 0.7

    def __init__(self, api_key: str, base_url: str | None = None) -> None:
        """Initialize the MiniMax summarizer.

        Args:
            api_key: MiniMax API key for authentication.
            base_url: Optional custom base URL. Defaults to MiniMax API endpoint.
        """
        self.api_key = api_key
        self.base_url = base_url or self.API_URL
        self._client = anthropic.Anthropic(
            api_key=api_key,
            base_url=self.base_url,
        )

    def summarize(self, title: str, abstract: str) -> PaperSummary:
        """Generate a structured summary of a research paper.

        Args:
            title: The paper's title.
            abstract: The paper's abstract.

        Returns:
            PaperSummary with headline, what_it_does, why_it_matters, and analogy.
        """
        prompt = PROMPT_TEMPLATE.format(title=title, abstract=abstract)

        response = self._client.messages.create(
            model=self.MODEL,
            max_tokens=self.MAX_TOKENS,
            temperature=self.TEMPERATURE,
            messages=[{"role": "user", "content": prompt}],
        )

        # Get the text content from the response
        # Response content is a list of blocks - extract text from TextBlock
        text = ""
        for block in response.content:
            block_text = getattr(block, "text", None)
            if block_text is not None:
                text = block_text
                break

        if not text:
            raise ValueError("No text content in model response")

        return self._parse(text)

    def _parse(self, text: str) -> PaperSummary:
        """Parse the model response into a PaperSummary.

        Args:
            text: Raw response text from the model.

        Returns:
            PaperSummary with fields extracted from the text.

        Raises:
            ValueError: If required fields cannot be parsed from the text.
        """
        lines = text.split("\n")
        result: dict[str, str] = {}
        current_field: str | None = None
        current_value_parts: list[str] = []

        for line in lines:
            stripped = line.strip()

            # Check if this line starts a new field
            if stripped.startswith("HEADLINE:"):
                self._finish_field(result, current_field, current_value_parts)
                current_field = "headline"
                current_value_parts = [stripped[len("HEADLINE:") :].strip()]
            elif stripped.startswith("WHAT IT DOES:"):
                self._finish_field(result, current_field, current_value_parts)
                current_field = "what_it_does"
                current_value_parts = [stripped[len("WHAT IT DOES:") :].strip()]
            elif stripped.startswith("WHY IT MATTERS:"):
                self._finish_field(result, current_field, current_value_parts)
                current_field = "why_it_matters"
                current_value_parts = [stripped[len("WHY IT MATTERS:") :].strip()]
            elif stripped.startswith("ANALOGY:"):
                self._finish_field(result, current_field, current_value_parts)
                current_field = "analogy"
                current_value_parts = [stripped[len("ANALOGY:") :].strip()]
            elif current_field and stripped:
                # Continuation of current field
                current_value_parts.append(stripped)

        # Finish last field
        self._finish_field(result, current_field, current_value_parts)

        headline = result.get("headline", "")
        what_it_does = result.get("what_it_does", "")
        why_it_matters = result.get("why_it_matters", "")
        analogy = result.get("analogy", "")

        if not headline or not what_it_does or not why_it_matters or not analogy:
            raise ValueError(f"Could not parse all required fields from response: {text!r}")

        return PaperSummary(
            headline=headline,
            what_it_does=what_it_does,
            why_it_matters=why_it_matters,
            analogy=analogy,
        )

    def _finish_field(
        self, result: dict[str, str], field: str | None, parts: list[str]
    ) -> None:
        """Finish and store the current field if exists."""
        if field and parts:
            result[field] = " ".join(parts)
