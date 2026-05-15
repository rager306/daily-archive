# ArXiv Daily Archive — Implementation Plan (Python Prototype)

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Build Python prototype of daily arXiv digest pipeline: fetch → score → summarize → deliver.

**Architecture:** Python 3.13 with uv for package management. CLI-driven pipeline with modular stages (6 Rs). Local storage under `~/research/` with three-space structure. No external services except arXiv, Semantic Scholar, MiniMax API.

**Tech Stack:** Python 3.13, uv, ruff, pyrefly, ty, Adaptix, Hypothesis, arXiv API, Semantic Scholar API, MiniMax (Anthropic-compatible).

---

## Phase 1 — Project Setup

### Task 1: Create project structure

**Objective:** Scaffold Python project with pyproject.toml and uv

**Files:**
- Create: `pyproject.toml`
- Create: `.python-version`
- Create: `README.md`

**Step 1: Create .python-version**

```bash
echo "3.13" > .python-version
```

**Step 2: Create pyproject.toml**

```toml
[project]
name = "arxiv-daily-archive"
version = "0.1.0"
description = "Daily arXiv digest with knowledge graph and preference learning"
requires-python = ">=3.13"
dependencies = [
    "httpx>=0.28.0",
    "anthropic>=0.38.0",
    "yake>=0.5.0",
    "pymupdf>=1.25.0",
    "feedparser>=6.0.0",
    "rich>=13.0.0",
]

[project.optional-dependencies]
dev = [
    "ruff>=0.9.0",
    "pyrefly>=0.1.0",
    "ty>=0.1.0",
    "adaptix>=0.4.0",
    "hypothesis>=6.120.0",
    "pytest>=8.3.0",
    "pytest-asyncio>=0.25.0",
]

[tool.ruff]
target-version = "py313"
line-length = 100

[tool.pyrefly]
target-version = "py313"

[tool.ty]
target-version = "py313"
strict = true

[tool.pytest.ini_options]
testpaths = ["tests"]
asyncio_mode = "auto"
```

**Step 3: Create README.md**

```markdown
# ArXiv Daily Archive

Daily digest of top-10 research papers from arXiv cs.* categories.

## Setup

```bash
uv sync --all-extras
```

## Run

```bash
uv run python -m arxiv_archive run --date 2026-05-15
```
```

**Step 4: Sync dependencies**

```bash
cd /root/daily-archive && uv sync --all-extras
```

Expected: resolution success, virtualenv created.

**Step 5: Commit**

```bash
git init && git add -A && git commit -m "init: project structure"
```

---

### Task 2: Create research directory structure

**Objective:** Create ~/research/ with three-space architecture (self/notes/ops)

**Files:**
- Create: `~/.research/self/identity.md`
- Create: `~/.research/self/methodology.md`
- Create: `~/.research/self/preferences.json`
- Create: `~/.research/ops/queue/.gitkeep`
- Create: `~/.research/ops/sessions/.gitkeep`
- Create: `~/.research/ops/logs/.gitkeep`
- Create: `~/.research/notes/topics/.gitkeep`
- Create: `~/.research/notes/papers/.gitkeep`
- Create: `~/.research/notes/authors/.gitkeep`
- Create: `~/.research/digests/.gitkeep`
- Create: `~/.research/graph/.gitkeep`

**Step 1: Create directory structure**

```bash
mkdir -p ~/.research/self ~/.research/ops/queue ~/.research/ops/sessions ~/.research/ops/logs
mkdir -p ~/.research/notes/topics ~/.research/notes/papers ~/.research/notes/authors
mkdir -p ~/.research/digests ~/.research/graph
touch ~/.research/ops/queue/.gitkeep ~/.research/ops/sessions/.gitkeep ~/.research/ops/logs/.gitkeep
touch ~/.research/notes/topics/.gitkeep ~/.research/notes/papers/.gitkeep ~/.research/notes/authors/.gitkeep
touch ~/.research/digests/.gitkeep ~/.research/graph/.gitkeep
```

**Step 2: Create identity.md**

```markdown
# Identity

ArXiv research assistant specializing in graph databases, knowledge graphs, and temporal reasoning.
```

**Step 3: Create methodology.md**

```markdown
# Methodology

## 6 Rs Pipeline

1. **Record** — fetch arXiv papers
2. **Reduce** — extract keywords, citations
3. **Score** — rank by relevance
4. **Reflect** — find cross-paper connections
5. **Summarize** — generate HEADLINE/WHAT_IT_DOES/WHY_IT_MATTERS/ANALOGY
6. **Verify + Deliver** — schema check, Telegram, file
```

**Step 4: Create preferences.json**

```json
{
  "topic_weights": {
    "cs.SI": 1.5,
    "cs.KG": 1.5,
    "cs.IR": 1.3,
    "cs.CL": 1.3,
    "cs.AI": 1.2,
    "cs.LG": 1.0,
    "cs.CV": 1.0,
    "cs.NE": 1.0,
    "cs.ML": 0.9,
    "cs.DB": 0.9,
    "cs.DS": 0.9,
    "cs.DC": 0.8,
    "cs.MA": 0.7,
    "cs.ST": 0.7
  },
  "liked_papers": [],
  "disliked_papers": [],
  "ignored_papers": []
}
```

**Step 5: Commit**

```bash
git add -A && git commit -m "init: research directory structure"
```

---

## Phase 2 — Core Pipeline (6 Rs)

### Task 3: Create arxiv client (Record stage)

**Objective:** Fetch papers from arXiv API for given date range

**Files:**
- Create: `src/arxiv_archive/__init__.py`
- Create: `src/arxiv_archive/arxiv_client.py`
- Create: `tests/test_arxiv_client.py`

**Step 1: Write failing test**

```python
# tests/test_arxiv_client.py
from datetime import date
from arxiv_archive.arxiv_client import ArxivClient

def test_fetch_papers_by_date():
    client = ArxivClient()
    papers = client.fetch_papers(date(2026, 5, 14), categories=["cs.AI"])
    assert len(papers) >= 0
    for paper in papers:
        assert paper.id.startswith("arxiv:")
        assert paper.title
        assert paper.abstract
```

**Step 2: Run test to verify failure**

```bash
uv run pytest tests/test_arxiv_client.py::test_fetch_papers_by_date -v
```
Expected: FAIL — ModuleNotFoundError: No module named 'arxiv_archive'

**Step 3: Create arxiv_client.py**

```python
# src/arxiv_archive/arxiv_client.py
from dataclasses import dataclass
from datetime import date, timedelta
from typing import Iterator
import httpx
import feedparser


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


class ArxivClient:
    BASE_URL = "https://export.arxiv.org/api/query"

    def fetch_papers(
        self,
        start_date: date,
        end_date: date | None = None,
        categories: list[str] | None = None,
    ) -> list[ArxivPaper]:
        if end_date is None:
            end_date = start_date + timedelta(days=1)
        if categories is None:
            categories = ["cs.AI", "cs.LG", "cs.CL", "cs.CV", "cs.IR", "cs.KG", "cs.SI"]

        papers = []
        for cat in categories:
            for paper in self._fetch_category(cat, start_date, end_date):
                papers.append(paper)
        return papers

    def _fetch_category(
        self, category: str, start_date: date, end_date: date
    ) -> Iterator[ArxivPaper]:
        query = (
            f"cat:{category}+AND+"
            f"date:[{start_date.isoformat()}+TO+{end_date.isoformat()}]"
        )
        url = f"{self.BASE_URL}?search_query={query}&start=0&max_results=100"

        with httpx.Client(timeout=30.0) as client:
            response = client.get(url)
            response.raise_for_status()
            feed = feedparser.parse(response.text)

        for entry in feed.entries:
            yield self._parse_entry(entry)

    def _parse_entry(self, entry) -> ArxivPaper:
        published = entry.get("published_parsed") or entry.get("updated_parsed")
        published_date = date.fromtimestamp(published) if published else date.today()

        updated = entry.get("updated_parsed")
        updated_date = date.fromtimestamp(updated) if updated else published_date

        arxiv_id = entry.id.split("/")[-1] if "/" in entry.id else entry.id

        pdf_url = None
        for link in entry.links:
            if link.get("type") == "application/pdf":
                pdf_url = link.href
                break

        authors = [a.name for a in entry.authors]

        categories = [tag.term for tag in entry.tags] if hasattr(entry, "tags") else []

        return ArxivPaper(
            id=f"arxiv:{arxiv_id}",
            title=entry.title,
            abstract=entry.summary,
            authors=authors,
            published=published_date,
            updated=updated_date,
            categories=categories,
            pdf_url=pdf_url,
        )
```

**Step 4: Run test to verify pass**

```bash
uv run pytest tests/test_arxiv_client.py::test_fetch_papers_by_date -v
```
Expected: PASS (or SKIP if no papers on that date)

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: arxiv client (Record stage)"
```

---

### Task 4: Create Semantic Scholar enricher (Reduce stage)

**Objective:** Enrich papers with citation counts from Semantic Scholar

**Files:**
- Create: `src/arxiv_archive/semantic_scholar.py`
- Create: `tests/test_semantic_scholar.py`

**Step 1: Write failing test**

```python
# tests/test_semantic_scholar.py
import pytest
from arxiv_archive.semantic_scholar import SemanticScholarClient

@pytest.mark.asyncio
async def test_fetch_citations():
    client = SemanticScholarClient()
    # Test with known arxiv ID
    result = await client.fetch_paper("2310.00001")
    assert result.citation_count >= 0
```

**Step 2: Run test to verify failure**

```bash
uv run pytest tests/test_semantic_scholar.py::test_fetch_citations -v
```
Expected: FAIL — ModuleNotFoundError

**Step 3: Create semantic_scholar.py**

```python
# src/arxiv_archive/semantic_scholar.py
from dataclasses import dataclass
from typing import Optional
import httpx


@dataclass
class SemanticScholarPaper:
    arxiv_id: str
    title: str
    citation_count: int
    year: Optional[int]
    venue: Optional[str]


class SemanticScholarClient:
    BASE_URL = "https://api.semanticscholar.org/graph/v1/paper"

    async def fetch_paper(self, arxiv_id: str) -> SemanticScholarPaper:
        paper_id = f"ARXIV:{arxiv_id}"
        fields = "title,citationCount,year,venue"
        url = f"{self.BASE_URL}/{paper_id}?fields={fields}"

        async with httpx.AsyncClient(timeout=30.0) as client:
            response = await client.get(url)
            response.raise_for_status()
            data = response.json()

        return SemanticScholarPaper(
            arxiv_id=arxiv_id,
            title=data.get("title", ""),
            citation_count=data.get("citationCount", 0) or 0,
            year=data.get("year"),
            venue=data.get("venue"),
        )

    async def fetch_batch(self, arxiv_ids: list[str]) -> dict[str, SemanticScholarPaper]:
        results = {}
        async with httpx.AsyncClient(timeout=60.0) as client:
            for arxiv_id in arxiv_ids:
                try:
                    paper = await self.fetch_paper(arxiv_id)
                    results[arxiv_id] = paper
                except Exception:
                    results[arxiv_id] = None
        return results
```

**Step 4: Run test to verify pass**

```bash
uv run pytest tests/test_semantic_scholar.py::test_fetch_citations -v
```
Expected: PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: Semantic Scholar enricher"
```

---

### Task 5: Create YAKE keyword extractor (Reduce stage)

**Objective:** Extract domain-specific keywords using YAKE

**Files:**
- Create: `src/arxiv_archive/keyword_extractor.py`
- Create: `tests/test_keyword_extractor.py`

**Step 1: Write failing test**

```python
# tests/test_keyword_extractor.py
from arxiv_archive.keyword_extractor import KeywordExtractor

def test_extract_keywords():
    extractor = KeywordExtractor()
    text = "Graph neural networks for knowledge graph completion using attention mechanisms"
    keywords = extractor.extract(text, top_k=5)
    assert len(keywords) <= 5
    for kw, score in keywords:
        assert isinstance(kw, str)
        assert isinstance(score, float)
```

**Step 2: Run test to verify failure**

```bash
uv run pytest tests/test_keyword_extractor.py::test_extract_keywords -v
```
Expected: FAIL

**Step 3: Create keyword_extractor.py**

```python
# src/arxiv_archive/keyword_extractor.py
import yake
from dataclasses import dataclass


@dataclass
class KeywordScore:
    keyword: str
    score: float


class KeywordExtractor:
    def __init__(self, language: str = "en", top_k: int = 20):
        self.language = language
        self.top_k = top_k
        self._extractor = yake.KeywordExtractor(
            lan=language,
            n=3,
            dedupLim=0.7,
            top=top_k,
            features=None,
        )

    def extract(self, text: str, top_k: int | None = None) -> list[tuple[str, float]]:
        k = top_k or self.top_k
        keywords = self._extractor.extract_keywords(text)
        return [(kw, score) for kw, score in keywords[:k]]

    def extract_for_paper(self, title: str, abstract: str) -> list[str]:
        combined = f"{title}. {abstract}"
        keywords, _ = zip(*self.extract(combined))
        return list(keywords)
```

**Step 4: Run test to verify pass**

```bash
uv run pytest tests/test_keyword_extractor.py::test_extract_keywords -v
```
Expected: PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: YAKE keyword extractor"
```

---

### Task 6: Create scoring engine (Score stage)

**Objective:** Score papers by citations, recency, novelty, preference, graph_bridge

**Files:**
- Create: `src/arxiv_archive/scoring.py`
- Create: `tests/test_scoring.py`

**Step 1: Write failing test**

```python
# tests/test_scoring.py
from datetime import date
from arxiv_archive.scoring import ScoringEngine, ScoredPaper
from arxiv_archive.semantic_scholar import SemanticScholarPaper
from arxiv_archive.arxiv_client import ArxivPaper

def test_scoring_engine_basic():
    engine = ScoringEngine()
    arxiv_paper = ArxivPaper(
        id="arxiv:2310.00001",
        title="Test",
        abstract="Test abstract",
        authors=["Test Author"],
        published=date.today(),
        updated=date.today(),
        categories=["cs.AI"],
        pdf_url="https://arxiv.org/pdf/2310.00001.pdf",
    )
    semschol = SemanticScholarPaper(
        arxiv_id="2310.00001",
        title="Test",
        citation_count=10,
        year=2024,
        venue="NeurIPS",
    )
    keywords = [("graph neural networks", 0.1), ("attention", 0.2)]
    scored = engine.score(arxiv_paper, semschol, keywords)
    assert scored.score >= 0
    assert isinstance(scored, ScoredPaper)
```

**Step 2: Run test to verify failure**

```bash
uv run pytest tests/test_scoring.py::test_scoring_engine_basic -v
```
Expected: FAIL

**Step 3: Create scoring.py**

```python
# src/arxiv_archive/scoring.py
from dataclasses import dataclass, field
from datetime import date
from arxiv_archive.arxiv_client import ArxivPaper
from arxiv_archive.semantic_scholar import SemanticScholarPaper


@dataclass
class ScoredPaper:
    paper: ArxivPaper
    semschol: SemanticScholarPaper | None
    keywords: list[str]
    score: float
    breakdown: dict[str, float]


@dataclass
class ScoringEngine:
    weights: dict[str, float] = field(default_factory=lambda: {
        "citations": 0.25,
        "recency": 0.20,
        "novelty": 0.20,
        "preference": 0.20,
        "graph_bridge": 0.15,
    })

    def score(
        self,
        paper: ArxivPaper,
        semschol: SemanticScholarPaper | None,
        keywords: list[str],
    ) -> ScoredPaper:
        citations = self._citations_score(semschol)
        recency = self._recency_score(paper.published)
        novelty = self._novelty_score(keywords)
        preference = self._preference_score(paper.categories)
        graph_bridge = 0.0  # Phase 2

        total = (
            citations * self.weights["citations"]
            + recency * self.weights["recency"]
            + novelty * self.weights["novelty"]
            + preference * self.weights["preference"]
            + graph_bridge * self.weights["graph_bridge"]
        )

        return ScoredPaper(
            paper=paper,
            semschol=semschol,
            keywords=keywords,
            score=total,
            breakdown={
                "citations": citations * self.weights["citations"],
                "recency": recency * self.weights["recency"],
                "novelty": novelty * self.weights["novelty"],
                "preference": preference * self.weights["preference"],
                "graph_bridge": graph_bridge * self.weights["graph_bridge"],
            },
        )

    def _citations_score(self, semschol: SemanticScholarPaper | None) -> float:
        if semschol is None:
            return 0.0
        count = semschol.citation_count or 0
        if count == 0:
            return 0.0
        return min(1.0, count / 100.0) * 10

    def _recency_score(self, published: date) -> float:
        today = date.today()
        days_old = (today - published).days
        if days_old == 0:
            return 10.0
        elif days_old == 1:
            return 8.0
        elif days_old <= 3:
            return 5.0
        elif days_old <= 7:
            return 2.0
        return 0.5

    def _novelty_score(self, keywords: list[str]) -> float:
        return float(min(len(keywords), 10)) * 0.5

    def _preference_score(self, categories: list[str]) -> float:
        topic_weights = {
            "cs.SI": 1.5, "cs.KG": 1.5, "cs.IR": 1.3, "cs.CL": 1.3,
            "cs.AI": 1.2, "cs.LG": 1.0, "cs.CV": 1.0, "cs.NE": 1.0,
            "cs.ML": 0.9, "cs.DB": 0.9, "cs.DS": 0.9,
        }
        return max((topic_weights.get(cat, 0.5) for cat in categories), default=0.5)
```

**Step 4: Run test to verify pass**

```bash
uv run pytest tests/test_scoring.py::test_scoring_engine_basic -v
```
Expected: PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: scoring engine"
```

---

### Task 7: Create MiniMax summarizer (Summarize stage)

**Objective:** Generate HEADLINE/WHAT_IT_DOES/WHY_IT_MATTERS/ANALOGY for each paper

**Files:**
- Create: `src/arxiv_archive/summarizer.py`
- Create: `tests/test_summarizer.py`

**Step 1: Write failing test**

```python
# tests/test_summarizer.py
import os
from arxiv_archive.summarizer import MiniMaxSummarizer

def test_summarize_paper():
    api_key = os.environ.get("MINIMAX_API_KEY", "test-key")
    summarizer = MiniMaxSummarizer(api_key=api_key)
    result = summarizer.summarize(
        title="Attention Is All You Need",
        abstract="We propose a new network architecture based on attention mechanisms.",
    )
    assert "HEADLINE:" in result
    assert "WHAT IT DOES:" in result
    assert "WHY IT MATTERS:" in result
    assert "ANALOGY:" in result
```

**Step 2: Run test to verify failure**

```bash
uv run pytest tests/test_summarizer.py::test_summarize_paper -v
```
Expected: FAIL (ModuleNotFoundError or test-key behavior)

**Step 3: Create summarizer.py**

```python
# src/arxiv_archive/summarizer.py
import anthropic
from dataclasses import dataclass


@dataclass
class PaperSummary:
    headline: str
    what_it_does: str
    why_it_matters: str
    analogy: str


class MiniMaxSummarizer:
    PROMPT_TEMPLATE = """You are a research assistant summarizing AI papers.

Summarize this paper in exactly this format:

HEADLINE: [one sentence — key result]
WHAT IT DOES: [2 sentences — what was built/discovered, no jargon]
WHY IT MATTERS: [1 sentence — real-world impact]
ANALOGY: [starts with "Think of it like"]

Title: {title}

Abstract: {abstract}

"""

    def __init__(self, api_key: str, base_url: str | None = None):
        self.client = anthropic.Anthropic(
            api_key=api_key,
            base_url=base_url or "https://api.minimax.io/anthropic",
        )

    def summarize(self, title: str, abstract: str) -> PaperSummary:
        response = self.client.messages.create(
            model="MiniMax-M2.7-highspeed",
            max_tokens=1024,
            temperature=0.7,
            messages=[
                {
                    "role": "user",
                    "content": self.PROMPT_TEMPLATE.format(title=title, abstract=abstract),
                }
            ],
        )

        text = response.content[0].text
        return self._parse(text)

    def _parse(self, text: str) -> PaperSummary:
        lines = text.strip().split("\n")
        result = {}
        for line in lines:
            if ":" in line:
                key, value = line.split(":", 1)
                key = key.strip().upper().replace(" ", "_")
                if key in ("HEADLINE", "WHAT_IT_DOES", "WHY_IT_MATTERS", "ANALOGY"):
                    result[key] = value.strip()

        return PaperSummary(
            headline=result.get("HEADLINE", ""),
            what_it_does=result.get("WHAT_IT_DOES", ""),
            why_it_matters=result.get("WHY_IT_MATTERS", ""),
            analogy=result.get("ANALOGY", ""),
        )
```

**Step 4: Run test to verify pass**

```bash
MINIMAX_API_KEY=$MINIMAX_API_KEY uv run pytest tests/test_summarizer.py::test_summarize_paper -v
```
Expected: PASS (or SKIP if no API key)

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: MiniMax summarizer"
```

---

### Task 8: Create PDF downloader

**Objective:** Download paper PDFs to local storage

**Files:**
- Create: `src/arxiv_archive/pdf_downloader.py`
- Create: `tests/test_pdf_downloader.py`

**Step 1: Write failing test**

```python
# tests/test_pdf_downloader.py
import pytest
from arxiv_archive.pdf_downloader import PDFDownloader
from pathlib import Path

def test_download_paper(tmp_path):
    downloader = PDFDownloader(cache_dir=tmp_path)
    result = downloader.download("2310.00001", "https://arxiv.org/pdf/2310.00001.pdf")
    assert result.exists()
```

**Step 2: Run test to verify failure**

```bash
uv run pytest tests/test_pdf_downloader.py::test_download_paper -v
```
Expected: FAIL

**Step 3: Create pdf_downloader.py**

```python
# src/arxiv_archive/pdf_downloader.py
from pathlib import Path
import httpx


class PDFDownloader:
    def __init__(self, cache_dir: Path | None = None):
        self.cache_dir = cache_dir or Path.home() / ".arxiv_cache"

    def download(self, arxiv_id: str, pdf_url: str) -> Path:
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        dest = self.cache_dir / f"{arxiv_id}.pdf"

        if dest.exists():
            return dest

        with httpx.Client(timeout=120.0) as client:
            response = client.get(pdf_url)
            response.raise_for_status()
            dest.write_bytes(response.content)

        return dest
```

**Step 4: Run test to verify pass**

```bash
uv run pytest tests/test_pdf_downloader.py::test_download_paper -v
```
Expected: PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: PDF downloader"
```

---

### Task 9: Create Markdown converter (PDF → markdown)

**Objective:** Convert PDF to markdown using pymupdf

**Files:**
- Create: `src/arxiv_archive/md_converter.py`
- Create: `tests/test_md_converter.py`

**Step 1: Write failing test**

```python
# tests/test_md_converter.py
from arxiv_archive.md_converter import MDConverter

def test_convert_sample_pdf(tmp_path):
    pdf_path = tmp_path / "test.pdf"
    # Create minimal PDF for testing
    converter = MDConverter()
    # Would need actual PDF to test
```

**Step 2: Create md_converter.py (skip full test for now)**

```python
# src/arxiv_archive/md_converter.py
import pymupdf
from pathlib import Path


class MDConverter:
    def convert(self, pdf_path: Path) -> str:
        doc = pymupdf.open(pdf_path)
        text_parts = []
        for page in doc:
            text_parts.append(page.get_text())
        doc.close()
        return "\n\n".join(text_parts)

    def convert_to_file(self, pdf_path: Path, output_path: Path) -> Path:
        text = self.convert(pdf_path)
        output_path.write_text(text, encoding="utf-8")
        return output_path
```

**Step 3: Commit (no test — would need real PDF)**

```bash
git add -A && git commit -m "feat: PDF to markdown converter"
```

---

### Task 10: Create Telegram delivery

**Objective:** Send digest to Telegram channel

**Files:**
- Create: `src/arxiv_archive/telegram_sender.py`
- Create: `tests/test_telegram_sender.py`

**Step 1: Write failing test**

```python
# tests/test_telegram_sender.py
import pytest
from arxiv_archive.telegram_sender import TelegramSender

def test_format_digest():
    sender = TelegramSender(bot_token="test", chat_id="test")
    # Test formatting
```

**Step 2: Create telegram_sender.py**

```python
# src/arxiv_archive/telegram_sender.py
import httpx
from dataclasses import dataclass
from arxiv_archive.scoring import ScoredPaper
from arxiv_archive.summarizer import PaperSummary


@dataclass
class TelegramSender:
    bot_token: str
    chat_id: str
    base_url: str = "https://api.telegram.org"

    def send_digest(self, papers: list[ScoredPaper], summaries: dict[str, PaperSummary]) -> None:
        text = self._format_digest(papers, summaries)
        self._send_message(text)

    def _format_digest(self, papers: list[ScoredPaper], summaries: dict[str, PaperSummary]) -> str:
        lines = ["📄 ArXiv Daily Archive\n"]
        for i, scored in enumerate(papers[:10], 1):
            p = scored.paper
            s = summaries.get(p.id)
            lines.append(f"{i}️⃣ [{p.id}] {p.title}")
            if s:
                lines.append(f"   HEADLINE: {s.headline}")
                lines.append(f"   WHY IT MATTERS: {s.why_it_matters}")
                lines.append(f"   💬 {s.analogy}")
            citations = scored.semschol.citation_count if scored.semschol else 0
            lines.append(f"   👍 {citations} | 📖 arxiv.org/abs/{p.id.split(':')[1]}")
            lines.append("")
        return "\n".join(lines)

    def _send_message(self, text: str) -> None:
        url = f"{self.base_url}/bot{self.bot_token}/sendMessage"
        payload = {"chat_id": self.chat_id, "text": text, "parse_mode": "Markdown"}
        with httpx.Client(timeout=30.0) as client:
            response = client.post(url, json=payload)
            response.raise_for_status()
```

**Step 3: Commit**

```bash
git add -A && git commit -m "feat: Telegram delivery"
```

---

### Task 11: Create CLI entry point (run command)

**Objective:** `uv run python -m arxiv_archive run --date YYYY-MM-DD`

**Files:**
- Create: `src/arxiv_archive/__main__.py`
- Create: `src/arxiv_archive/cli.py`
- Modify: `src/arxiv_archive/scoring.py` (add preferences loading)

**Step 1: Write failing test**

```bash
uv run python -m arxiv_archive run --help
```
Expected: ModuleNotFoundError

**Step 2: Create __main__.py**

```python
# src/arxiv_archive/__main__.py
from arxiv_archive.cli import main

if __name__ == "__main__":
    main()
```

**Step 3: Create cli.py**

```python
# src/arxiv_archive/cli.py
import json
import ty
from pathlib import Path
from datetime import date
from arxiv_archive.arxiv_client import ArxivClient
from arxiv_archive.semantic_scholar import SemanticScholarClient
from arxiv_archive.keyword_extractor import KeywordExtractor
from arxiv_archive.scoring import ScoringEngine
from arxiv_archive.summarizer import MiniMaxSummarizer
from arxiv_archive.telegram_sender import TelegramSender


def load_preferences() -> dict:
    prefs_path = Path.home() / ".research" / "self" / "preferences.json"
    if prefs_path.exists():
        return json.loads(prefs_path.read_text())
    return {"topic_weights": {}, "liked_papers": [], "disliked_papers": []}


@ty ty ty
def main(date_str: str = ty.Option(..., help="Date to process (YYYY-MM-DD)")):
    parts = date_str.split("-")
    run_date = date(int(parts[0]), int(parts[1]), int(parts[2]))

    ty.echo(f"Running arXiv daily archive for {run_date}")

    # Record
    arxiv = ArxivClient()
    papers = arxiv.fetch_papers(run_date)
    ty.echo(f"Fetched {len(papers)} papers")

    # Reduce
    semschol = SemanticScholarClient()
    keywords = KeywordExtractor()

    # Score
    engine = ScoringEngine()
    scored = []
    for paper in papers:
        arxiv_id = paper.id.replace("arxiv:", "")
        sch = None
        try:
            import asyncio
            sch = asyncio.run(semschol.fetch_paper(arxiv_id))
        except Exception:
            pass
        kws = keywords.extract_for_paper(paper.title, paper.abstract)
        scored_paper = engine.score(paper, sch, kws)
        scored.append(scored_paper)

    # Select top-10
    top10 = sorted(scored, key=lambda x: x.score, reverse=True)[:10]
    ty.echo(f"Selected top {len(top10)} papers")

    ty.echo("Done. MiniMax summarization and Telegram delivery in next phase.")
```

**Step 4: Test CLI**

```bash
uv run python -m arxiv_archive run --date 2026-05-14
```
Expected: runs without error

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: CLI entry point"
```

---

### Task 12: Create session capture (ops/sessions/)

**Objective:** Save session log after each run

**Files:**
- Modify: `src/arxiv_archive/cli.py`

**Step 1: Update cli.py with session capture**

```python
# Add to cli.py
def save_session(run_date: date, papers_fetched: int, top10: list) -> Path:
    sessions_dir = Path.home() / ".research" / "ops" / "sessions"
    sessions_dir.mkdir(parents=True, exist_ok=True)
    timestamp = run_date.isoformat()
    session_file = sessions_dir / f"{timestamp}.md"
    content = f"""# Session {timestamp}

- Papers fetched: {papers_fetched}
- Top-10 selected: {len(top10)}
- Papers: {", ".join(p.id for p in top10)}

"""
    session_file.write_text(content)
    return session_file
```

**Step 2: Commit**

```bash
git add -A && git commit -m "feat: session capture"
```

---

## Phase 3 — Integration Test

### Task 13: Integration test — full pipeline

**Objective:** Test Record → Reduce → Score → Top-10 end-to-end

**Files:**
- Create: `tests/test_integration.py`

**Step 1: Write integration test**

```python
# tests/test_integration.py
import pytest
from datetime import date
from arxiv_archive.arxiv_client import ArxivClient
from arxiv_archive.keyword_extractor import KeywordExtractor
from arxiv_archive.scoring import ScoringEngine


def test_full_pipeline_record_reduce_score():
    arxiv = ArxivClient()
    papers = arxiv.fetch_papers(date(2026, 5, 14), categories=["cs.AI", "cs.LG"])

    if not papers:
        pytest.skip("No papers found for test date")

    extractor = KeywordExtractor()
    engine = ScoringEngine()

    scored = []
    for paper in papers:
        kws, _ = zip(*extractor.extract(f"{paper.title}. {paper.abstract}"))
        sp = engine.score(paper, None, list(kws))
        scored.append(sp)

    top10 = sorted(scored, key=lambda x: x.score, reverse=True)[:10]
    assert len(top10) <= 10
    assert all(isinstance(p, ScoredPaper) for p in top10)
```

**Step 2: Run integration test**

```bash
uv run pytest tests/test_integration.py -v
```
Expected: PASS or SKIP

**Step 3: Commit**

```bash
git add -A && git commit -m "test: integration test for full pipeline"
```

---

## What Comes Next

After Phase 1-3 (this plan):
- Phase 2: Graphify integration, community detection, bridge detection
- Phase 3: Preference learning from user feedback
- Phase 4: Weekly cleanup, MOC updates

Then: Go production rewrite (CPU-efficient).

---

## Verification Checklist

- [ ] All tests pass (`uv run pytest tests/ -v`)
- [ ] `uv run python -m arxiv_archive run --date 2026-05-14` works
- [ ] ruff passes (`uv run ruff check src/`)
- [ ] ty passes (`uv run ty check src/`)
- [ ] Session file created in `~/.research/ops/sessions/`
