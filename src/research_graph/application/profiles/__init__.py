"""Level 3 domain pipeline profiles (ADR-033 Step 5).

Each profile assembles a :class:`~research_graph.application.types.Pipeline` for a
specific source domain. Paper pipeline is fully wired (ADR-001 first domain).
Textbook constants for GNN HTML path land in M222 (ADR-032 first non-paper);
code_repo / dataset / tech_doc remain Phase 5.
"""

from __future__ import annotations

from research_graph.application.profiles.paper import build_paper_pipeline
from research_graph.application.profiles.textbook import (
    DOMAIN_PROFILE as TEXTBOOK_DOMAIN_PROFILE,
)
from research_graph.application.profiles.textbook import (
    textbook_profile_dict,
)

__all__ = ["TEXTBOOK_DOMAIN_PROFILE", "build_paper_pipeline", "textbook_profile_dict"]
