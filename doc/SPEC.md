# ArXiv Daily Archive — Specification

> **HISTORICAL / SUPERSEDED as binding product spec (M274 E0).**
>
> This document describes the early **Telegram arXiv digest** product surface
> (Graphify, daily top-10, Hermes cron). It is **not** the binding description of
> the current Universal Knowledge Base / `research_graph` ETL.
>
> **Binding surfaces now:**
> - `README.md` — architecture, current state, residual problems
> - `artifacts/etl/ETL-READINESS-MATRIX-ROADMAP.md` — live residual matrix
> - `artifacts/etl/EVIDENCE-TRACE-AND-VERIFICATION-ROADMAP.md` — evidence-trace next wave
> - `doc/adr/ADR-INDEX.md` — binding ADRs
> - `doc/REPO-HYGIENE.md` — truth paths / garbage policy
>
> Keep this file for archaeology of the digest product; do not use it to drive
> agents or import/graph decisions.

**Inspired by:** Ars Contexta (agenticnotetaking/arscontexta) — 3.3k stars, Claude Code plugin for knowledge system generation. We adapt their Three-Space Architecture (self/notes/ops) and 6 Rs pipeline for arXiv research digest.

---

## Goal (historical digest product)

Daily digest of top-10 research papers from arXiv cs.* categories, delivered to Telegram, with local knowledge base and preference learning from user feedback.

---

## Stack

| Component | Service | Cost |
|-----------|---------|------|
| Paper source | arXiv API (`export.arxiv.org/api/query`) | Free, no key |
| Citations | Semantic Scholar API | Free, no key |
| Summary generation | MiniMax-M2.7-highspeed (Anthropic-compatible) | User subscription |
| Embeddings | TEI (local) + YAKE (statistical keyword extraction) | Free |
| Storage | Local (`~/research/`) | Free |
| Graph analysis | Graphify (local) | Free |
| Vector search | TEI (local) | Free |
| Delivery | Telegram Bot API | Free |
| Scheduling | Hermes cron | Free |
| Language | Python 3.13 | Free |
| Package manager | uv | Free |
| Linter | ruff | Free |
| Refactoring | pyrefly (Astral) | Free |
| Type checker | ty (Astral) | Free |
| Testing | Adaptix + Hypothesis | Free |

**No paid external dependencies. Graphify is local-only.**

---

## Pipeline (daily at 08:00 UTC)

Inspired by Ars Contexta 6 Rs, adapted for arXiv digest:

```
┌─────────────────────────────────────────────────────────────────────┐
│  DAILY ARCHIVE PIPELINE — 6 Rs                                      │
│                                                                     │
│  1. RECORD                                                          │
│     arXiv API → all cs.* papers from last 24-48h                   │
│     Status: ops/queue/YYYY-MM-DD.json → {"stage": "record"}        │
│                                                                     │
│  2. REDUCE                                                          │
│     YAKE keyword extraction → domain-specific terms                 │
│     TF-IDF + C-value → multi-word term novelty                     │
│     Semantic Scholar → citation count, authors, venue               │
│     Status: ops/queue/YYYY-MM-DD.json → {"stage": "reduce"}        │
│                                                                     │
│  3. SCORE                                                           │
│     score = citations*0.25 + recency*0.20                          │
│           + novelty_nlp*0.20 + preference*0.20                      │
│           + graph_bridge*0.15                                       │
│     Select top-10                                                    │
│                                                                     │
│  4. REFLECT                                                         │
│     Find cross-paper connections via shared entities                 │
│     Update concept notes with new paper links                       │
│     Reweave: update old concept notes with fresh context            │
│     Status: ops/queue/YYYY-MM-DD.json → {"stage": "reflect"}       │
│                                                                     │
│  5. SUMMARIZE (MiniMax-M2.7-highspeed)                             │
│     HEADLINE / WHAT IT DOES / WHY IT MATTERS / ANALOGY              │
│     For each top-10 paper                                           │
│                                                                     │
│  6. VERIFY + DELIVER                                               │
│     Schema check: all _meta.yaml fields present                      │
│     Telegram: top-10 with summaries + links                        │
│     File: ~/research/ops/sessions/YYYY-MM-DD-HHMM.md               │
│     Full digest: ~/research/digests/YYYY-MM-DD.md                  │
│     Status: ops/queue/YYYY-MM-DD.json → {"stage": "done"}          │
└─────────────────────────────────────────────────────────────────────┘
```

**Session capture:** After each run, ops/sessions/YYYY-MM-DD-HHMM.md captures what happened, errors, papers processed.

---

## Scoring

```
score = (citations * 0.25) + (recency * 0.20)
      + (novelty_nlp * 0.20) + (preference * 0.20)
      + (graph_bridge * 0.15)
```

- **citations** — from Semantic Scholar API
- **recency** — step decay: today=30, yesterday=20, 3days=10, week=5
- **novelty_nlp** — YAKE keyword extraction + TF-IDF: uncommon terms, domain-specific phrases, C-value for multi-word terms
- **preference** — learned from user feedback over time
- **graph_bridge** — bridge centrality: paper connects different topic clusters

---

## Summary Format (MiniMax)

For each top-10 paper:

```
HEADLINE: [one sentence — key result]
WHAT IT DOES: [2 sentences — what was built/discovered, no jargon]
WHY IT MATTERS: [1 sentence — real-world impact]
ANALOGY: [starts with "Think of it like"]
```

MiniMax API:
- Endpoint: `https://api.minimax.io/anthropic/v1/messages`
- Auth: `X-Api-Key: ${API_KEY}`
- Model: `MiniMax-M2.7-highspeed`
- Max output: 1024 tokens
- Temperature: 0.7 (range 0.0–1.0, cannot be 0)

---

## Surprise Me (Graphify Module)

```
┌─────────────────────────────────────────────────────────────┐
│  GRAPHIFY — Surprise Me                                    │
│                                                             │
│  1. EXTRACT                                                │
│     YAKE → entities from abstract                          │
│     Co-occurrence → relationships                         │
│                                                             │
│  2. BUILD GRAPH                                            │
│     Papers as nodes, shared entities as edges             │
│     Weight by: same author, same category, shared keywords │
│                                                             │
│  3. COMMUNITY DETECTION                                    │
│     Leiden algorithm → topic clusters                      │
│                                                             │
│  4. BRIDGE DETECTION                                       │
│     Betweenness centrality → papers connecting clusters    │
│     Unexpected neighbors → papers close to diverse topics  │
│                                                             │
│  5. RANK                                                    │
│     bridge_score = betweenness * unexpected_neighbor_ratio │
│                                                             │
│  6. SELECT TOP-3                                           │
│     Papers with highest bridge_score                       │
│     → shown in "Surprise me" block in digest              │
└─────────────────────────────────────────────────────────────┘
```

---

## Storage Structure — Three-Space Architecture

Inspired by Ars Contexta: every knowledge system needs three distinct spaces.

```
~/research/
├── self/                          # Agent persistent mind
│   ├── identity.md                # Who I am (researcher identity)
│   ├── methodology.md             # How I work (preferences, goals)
│   └── preferences.json           # Topic weights, learned preferences
│
├── ops/                           # Operational coordination
│   ├── queue/                     # Pipeline state per digest run
│   │   └── YYYY-MM-DD.json       # Fetch→Reduce→Reflect→... status
│   ├── sessions/                  # Session captures
│   │   └── YYYY-MM-DD-HHMM.md    # What happened during digest run
│   └── logs/                      # Error logs, failed fetches
│
├── notes/                         # Knowledge graph (the reason system exists)
│   ├── _index.md                  # Map of Content (MOC) — hub entry
│   ├── topics/
│   │   ├── graph-databases/
│   │   │   ├── _meta.yaml         # Topic schema: keywords, related topics
│   │   │   ├── concept-graph-neural-networks.md
│   │   │   └── concept-knowledge-graphs.md
│   │   └── ...
│   ├── papers/
│   │   └── {arxiv-id}/
│   │       ├── _meta.yaml         # Paper schema: title, authors, citations
│   │       ├── paper.pdf
│   │       ├── paper.md           # Full text
│   │       └── _linked-concepts.md  # Wiki-links to concept notes
│   └── authors/
│       └── {author-slug}/
│           └── _meta.yaml
│
├── digests/
│   └── YYYY-MM-DD.md              # Daily digest: top-10 with summaries
│
└── graph/
    ├── graph.json                 # Knowledge graph (Graphify format)
    ├── graph.html                 # Interactive visualization
    └── GRAPH_REPORT.md            # Highlights, surprising connections
```

**Wiki-links:** Concept notes link to each other via `[[concept-name]]` syntax. Traversable without database.

**MOC hierarchy:**
- `_index.md` → hub entry, lists all topics
- `topics/_meta.yaml` → domain-level (e.g., "graph-databases")
- `{topic}/_meta.yaml` → topic-level with keywords, related topics

**Schema blocks:** Each note type has `_meta.yaml` as single source of truth (title, authors, citations, keywords, links).

Topic slugs derived from arXiv category names (e.g., `cs.CV` → `computer-vision`).

---

## Preference Learning

**Signals:**
- `/like` — user liked a paper
- `/dislike` — user disliked a paper
- Opened arXiv link / didn't open
- Read time (if trackable)
- `/digest` — manual trigger indicates active interest

**Mechanics:**
1. High-score + liked → boost similar papers (same authors, categories, keywords)
2. High-score + ignored → decrease author/category weight
3. `preference` score = weighted sum of topic relevances, updated after each signal

---

## Telegram Delivery

Daily (08:00 UTC):

```
📄 ArXiv Daily Archive — Jan 15, 2026

🏆 Top 10 Papers

1️⃣ [2501.12345] Attention Is All You Need — Revisited
   HEADLINE: Transformer architecture outperforms state-of-the-art...
   WHY IT MATTERS: 40% faster training with same accuracy
   💬 Think of it like...
   👍 127 | 📖 arxiv.org/abs/2501.12345

[... 9 more ...]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🎲 SURPRISE ME — Unexpected Connections

📊 Paper A: "Graph Neural Networks for Knowledge Graphs"
    ↕ shares entity cluster with
🧬 Paper B: "Temporal Reasoning in Neural Networks"
    Both published: Jan 14, 2026
    Bridge score: 0.87
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📁 Full digest: ~/research/digests/2026-01-15.md
🔗 Open all: https://arxiv.org/list/cs.LG/recent
```

---

## Weekly Cleanup (Cron)

1. Find all papers older than 7 days
2. Check: liked? opened? in KB?
3. Not viewed + not liked → delete PDF + markdown
4. Keep `meta.json` (history/scoring context)
5. Update graph.json (remove nodes, re-run community detection)

---

## Graphify Integration

### What is Graphify

Python CLI tool + skill for AI coding assistants (Claude Code, Codex, Hermes, etc.)

**Key features:**
- Extract entities, relationships from code/docs/papers
- Build knowledge graph with Leiden community detection
- Surprising connections detection
- Interactive HTML visualization
- MCP server mode for Hermes integration

**Installation:**
```bash
uv tool install graphifyy
graphify install --platform hermes
```

**Key commands:**
```bash
graphify add https://arxiv.org/abs/2501.12345  # add paper to graph
graphify query "what connects graph neural networks to knowledge graphs?"
graphify serve graphify-out/graph.json  # MCP server mode
```

### Graphify for Daily Archive

Graphify is the local knowledge graph tool. No external services.

```
┌─────────────────────────────────────────────────────────────────────┐
│  GRAPHIFY WORKFLOW                                                  │
│                                                                     │
│  1. DAILY INGESTION                                                │
│     graphify add https://arxiv.org/abs/{id}                         │
│     For each top-10 paper → adds to graph.json                     │
│                                                                     │
│  2. GRAPH CONSTRUCTION                                             │
│     paper-id → node with metadata                                   │
│     shared entities → edges (weighted)                             │
│     same-category → stronger edge                                  │
│     same-author → strongest edge                                   │
│                                                                     │
│  3. COMMUNITY DETECTION                                            │
│     Leiden algorithm → clusters                                    │
│     Louvain as fallback                                            │
│                                                                     │
│  4. SURPRISE ME                                                    │
│     graph_bridge_score = betweenness_centrality                    │
│                          * diversity_of_neighbors                   │
│     Select top-3 by bridge_score                                   │
│                                                                     │
│  5. MCP SERVER (optional)                                          │
│     python -m graphify.serve graph.json                            │
│     Hermes can query:                                              │
│     - get_surprising_connections()                                 │
│     - get_papers_in_cluster(topic)                                 │
│     - get_related_papers(paper_id)                                 │
└─────────────────────────────────────────────────────────────────────┘
```

---

## arXiv API Details

```
https://export.arxiv.org/api/query
```

- No API key required
- Filter: `cat:cs.*` with date range
- Returns: Atom XML with title, authors, abstract, published date, links
- Rate limit: ~1 request per 3 seconds (gentle)
- PDF URL: `https://arxiv.org/pdf/{arxiv-id}.pdf`

**Categories to track (cs.*):**
cs.AI, cs.CL, cs.CV, cs.CR, cs.CY, cs.DB, cs.DC, cs.DL, cs.DS, cs.ET,
cs.GL, cs.GR, cs.GT, cs.HC, cs.IR, cs.IT, cs.LG, cs.LO, cs.MA, cs.ML,
cs.MM, cs.MS, cs.NA, cs.NE, cs.NI, cs.OH, cs.OS, cs.PF, cs.PL, cs.RO,
cs.SC, cs.SD, cs.SE, cs.SI, cs.SY

**Boosted categories (user interests):**
- Graph-related: cs.SI, cs.KG, cs.IR, cs.CL, cs.AI
- Core ML: cs.LG, cs.CV, cs.NE, cs.ML
- Math/stats: cs.MA, cs.ST (for matanalysis LLM optimization)
- Data systems: cs.DB, cs.DS, cs.DC

---

## Semantic Scholar API

```
https://api.semanticscholar.org/graph/v1/paper/{arxiv-id}
```

- Free, no key
- Returns: title, authors, abstract, citation count, year, venue
- Fields: `title,authors,abstract,citationCount,year,venue`

---

## Open Questions — RESOLVED

| # | Question | Decision |
|---|----------|----------|
| 1 | Embeddings | TEI (local) + YAKE + TF-IDF + C-value |
| 2 | Categories | All cs.*, boosted by graph interests |
| 3 | Summary length | 4 sections |
| 4 | Fallback | Send what's available |
| 5 | Manual trigger | Yes (`/digest`) |
| 6 | History retention | Keep (no expiration) |
| 7 | Surprise me | Graphify + bridge detection |

---

## MiniMax API Reference

```python
import anthropic

client = anthropic.Anthropic(
    base_url="https://api.minimax.io/anthropic",
    api_key="your-key"
)

response = client.messages.create(
    model="MiniMax-M2.7-highspeed",
    max_tokens=1024,
    system="You are a research assistant summarizing AI papers.",
    messages=[{
        "role": "user",
        "content": [{"type": "text", "text": "Summarize: ..."}]
    }]
)
```

Supported: text, thinking, tool_use, tool_result.
Not supported: image input, document input.

Temperature range: (0.0, 1.0] — cannot be 0.

---

## Graphify CLI Reference

```bash
# Install
uv tool install graphifyy
graphify install --platform hermes

# Add arxiv paper
graphify add https://arxiv.org/abs/2501.12345

# Build graph
graphify .

# Query graph
graphify query "show connections between graph and neural networks"

# MCP server
python -m graphify.serve graphify-out/graph.json
```

Graphify output:
- `graphify-out/graph.json` — full graph (query anytime)
- `graphify-out/graph.html` — interactive visualization
- `graphify-out/GRAPH_REPORT.md` — highlights, surprising connections

---

## Implementation Phases

### Phase 1 — Core Pipeline
- [ ] arXiv API fetcher
- [ ] Semantic Scholar enricher
- [ ] Scoring engine (citations + recency + YAKE novelty)
- [ ] Top-10 selector
- [ ] MiniMax summarizer
- [ ] PDF downloader
- [ ] Markdown converter
- [ ] Telegram delivery

### Phase 2 — Knowledge Graph
- [ ] Graphify integration
- [ ] YAKE keyword extraction
- [ ] Entity/relationship extraction
- [ ] Community detection
- [ ] Bridge detection
- [ ] "Surprise me" block

### Phase 3 — Preferences
- [ ] Preference learning signals
- [ ] User feedback handlers
- [ ] Weighted scoring

### Phase 4 — Polish
- [ ] Weekly cleanup cron
- [ ] Graph maintenance
- [ ] MOC updates (topic navigation)
