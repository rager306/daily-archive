"""Level 3 domain pipeline profiles (ADR-033 Step 5).

Each profile assembles a :class:`~research_graph.pipeline.types.Pipeline` for a
specific source domain. Only the paper domain is built here (ADR-001 first
domain); textbook / code_repo / dataset / tech_doc arrive in Phase 5 (ADR-032,
EP-1 Domain Profile seam).
"""

from __future__ import annotations

from research_graph.pipeline.profiles.paper import build_paper_pipeline

__all__ = ["build_paper_pipeline"]
