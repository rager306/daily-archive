# Formerly: src/research_graph/retrieval/summarizer.py

# Formerly: src/arxiv_archive/summarizer.py

"""MiniMax-based paper summarizer for AI research papers."""

import os
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

MODEL = "MiniMax-M3-512k"
MAX_TOKENS = 1024
TEMPERATURE = 0.7


class MiniMaxSummarizer:
    """Summarizer using MiniMax's Anthropic-compatible API.

    Uses the MiniMax-M3-512k Anthropic-compatible model path to generate
    structured summaries of AI research papers.

    Requires environment variables:
        ANTHROPIC_API_KEY: MiniMax API key
        ANTHROPIC_BASE_URL: https://api.minimax.io/anthropic
    """

    def __init__(self, api_key: str | None = None) -> None:
        """Initialize the MiniMax summarizer.

        Args:
            api_key: Optional API key. If omitted, reads ANTHROPIC_API_KEY from
                the current process environment without mutating it.
        """
        api_key = api_key or os.environ.get("ANTHROPIC_API_KEY")
        # MiniMax uses X-Api-Key header (not Bearer). Pass a dummy Anthropic
        # api_key to stop the SDK from falling back to ANTHROPIC_* env vars;
        # auth_token=None prevents Authorization: Bearer injection while keeping
        # the process environment unchanged for other threads/tasks.
        self._client = anthropic.Anthropic(
            api_key="unused-minimax-compat-key",
            auth_token=None,
            default_headers={"X-Api-Key": api_key or ""},
            base_url="https://api.minimax.io/anthropic",
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
            model=MODEL,
            max_tokens=MAX_TOKENS,
            temperature=TEMPERATURE,
            messages=[{"role": "user", "content": prompt}],
        )

        # Get the text content from the response
        text = ""
        for block in response.content:
            if getattr(block, "type", None) == "text":
                text = str(getattr(block, "text", ""))
                break

        if not text:
            raise ValueError("No text content in model response")

        return self._parse(text)

    def _parse(self, text: str) -> PaperSummary:
        """Parse the model response into a PaperSummary."""
        lines = text.split("\n")
        result: dict[str, str] = {}
        current_field: str | None = None
        current_value_parts: list[str] = []

        def clean_field_value(value: str) -> str:
            value = value.strip()
            while True:
                old = value
                for marker in ["**", "*", "_"]:
                    if value.startswith(marker):
                        value = value[len(marker) :].strip()
                    if value.endswith(marker):
                        value = value[: -len(marker)].strip()
                if value == old:
                    return value.strip()

        for line in lines:
            stripped = line.strip()

            # Strip all markdown bold/italic markers repeatedly until clean
            # Strip adjacent whitespace after each removal (model often outputs "** Think" with space)
            while True:
                old = stripped
                for marker in ["**", "*", "_"]:
                    if stripped.startswith(marker):
                        stripped = stripped[len(marker) :].strip()
                    if stripped.endswith(marker):
                        stripped = stripped[: -len(marker)].strip()
                if stripped == old:
                    break
            stripped = stripped.strip()

            if stripped.startswith("HEADLINE:"):
                if current_field and current_value_parts:
                    result[current_field] = " ".join(current_value_parts).strip()
                current_field = "headline"
                current_value_parts = [clean_field_value(stripped[len("HEADLINE:") :])]
            elif stripped.startswith("WHAT IT DOES:"):
                if current_field and current_value_parts:
                    result[current_field] = " ".join(current_value_parts).strip()
                current_field = "what_it_does"
                current_value_parts = [clean_field_value(stripped[len("WHAT IT DOES:") :])]
            elif stripped.startswith("WHY IT MATTERS:"):
                if current_field and current_value_parts:
                    result[current_field] = " ".join(current_value_parts).strip()
                current_field = "why_it_matters"
                current_value_parts = [clean_field_value(stripped[len("WHY IT MATTERS:") :])]
            elif stripped.startswith("ANALOGY:"):
                if current_field and current_value_parts:
                    result[current_field] = " ".join(current_value_parts).strip()
                current_field = "analogy"
                current_value_parts = [clean_field_value(stripped[len("ANALOGY:") :])]
            elif current_field and stripped:
                # Strip all markdown markers from continuation lines
                while True:
                    old = stripped
                    for marker in ["**", "*", "_"]:
                        if stripped.startswith(marker):
                            stripped = stripped[len(marker) :]
                        if stripped.endswith(marker):
                            stripped = stripped[: -len(marker)]
                    if stripped == old:
                        break
                current_value_parts.append(stripped.strip() or "")

        if current_field and current_value_parts:
            result[current_field] = " ".join(current_value_parts).strip()

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
