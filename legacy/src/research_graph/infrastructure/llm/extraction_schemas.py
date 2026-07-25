"""JSON Schemas for LLMClientPort forced-tool extraction (M201).

Schemas mirror application Adaptix boundary models
(:class:`~research_graph.application.primitives.LLMEntityOutput` /
:class:`~research_graph.application.primitives.LLMRelationOutput`) so tool
input validates before Adaptix load. Artifact classification schemas stay in
``papers.artifacts`` — do not reuse those for semantic extraction (M201 S06).
"""

from __future__ import annotations

from typing import Any

ENTITY_EXTRACTION_TOOL_NAME = "extract_entities"
RELATION_EXTRACTION_TOOL_NAME = "extract_relations"

ENTITY_EXTRACTION_TOOL_DESCRIPTION = (
    "Extract core technical entities from a redacted paper chunk with "
    "statistical keyword grounding. Candidates only — never assert import "
    "eligibility or production facts."
)

RELATION_EXTRACTION_TOOL_DESCRIPTION = (
    "Extract typed binary relations between entities from a redacted paper "
    "chunk. Candidates only — never assert import eligibility or production facts."
)


def entity_extraction_input_schema() -> dict[str, Any]:
    """JSON Schema for tool input ``{"entities": [...]}``."""
    return {
        "type": "object",
        "properties": {
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "entity_type": {"type": "string"},
                        "canonical_name": {"type": "string"},
                        "confidence": {"type": "number"},
                        "evidence_hint": {"type": "string"},
                    },
                    "required": ["entity_type", "canonical_name", "confidence"],
                },
            }
        },
        "required": ["entities"],
    }


def relation_extraction_input_schema() -> dict[str, Any]:
    """JSON Schema for tool input ``{"relations": [...]}``."""
    return {
        "type": "object",
        "properties": {
            "relations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "relation_type": {"type": "string"},
                        "from_name": {"type": "string"},
                        "to_name": {"type": "string"},
                        "confidence": {"type": "number"},
                    },
                    "required": ["relation_type", "from_name", "to_name", "confidence"],
                },
            }
        },
        "required": ["relations"],
    }


__all__ = [
    "ENTITY_EXTRACTION_TOOL_DESCRIPTION",
    "ENTITY_EXTRACTION_TOOL_NAME",
    "RELATION_EXTRACTION_TOOL_DESCRIPTION",
    "RELATION_EXTRACTION_TOOL_NAME",
    "entity_extraction_input_schema",
    "relation_extraction_input_schema",
]
