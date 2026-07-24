"""9router chat → JSON extraction helper for Wave B pilot.

Not MiniMax forced-tool. Uses OpenAI-compatible chat with strict JSON instructions.
Fail-closed: bad parse → empty entities/relations, json_valid=false.
Never logs secrets.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any

from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    ALLOWED_ENTITY_TYPES,
    ALLOWED_RELATION_TYPES,
    parse_llm_extraction_json,
)
from research_graph.infrastructure.llm.ninerouter_client import NineRouterChatClient

DEFAULT_PILOT_MODEL = "agnes-ai/agnes-2.0-flash"
QUALITY_PILOT_MODEL = "minimax/MiniMax-M2.7-highspeed"

_SYSTEM = (
    "You extract scientific knowledge-graph candidates from paper text. "
    "Return ONLY a single JSON object. No markdown fences. No prose. "
    "Candidates only — never claim import eligibility or production facts."
)


def build_extraction_user_prompt(*, case_id: str, body_text: str) -> str:
    entity_types = ", ".join(sorted(ALLOWED_ENTITY_TYPES))
    rel_types = ", ".join(sorted(ALLOWED_RELATION_TYPES))
    return (
        f"case_id={case_id}\n"
        f"Allowed entity types: {entity_types}\n"
        f"Allowed relation types: {rel_types}\n"
        "Extract ONLY the 2-4 most central paper contributions (core method/task/field/dataset).\n"
        "Labels must be multi-word technical phrases copied from the text when possible\n"
        "(title-case preferred, e.g. 'Neural Machine Translation', 'Zero-shot Learning').\n"
        "Do NOT invent generic labels like 'deep learning' or 'neural network' unless central.\n"
        "Prefer fewer precise entities over many weak ones.\n"
        "JSON schema only (no markdown, no thinking, no prose):\n"
        "{\n"
        '  "entities": [{"type": "Method|Task|Field|Dataset|Model|Metric", "label": "..."}],\n'
        '  "relations": [{"type": "APPLIED_TO|USES_COMPONENT|EVALUATED_ON|OUTPERFORMS",'
        ' "source_label": "...", "target_label": "..."}]\n'
        "}\n"
        "source_label/target_label must match entity labels exactly.\n"
        "--- PAPER TEXT ---\n"
        f"{body_text}\n"
        "--- END ---\n"
        "JSON:"
    )


@dataclass
class NineRouterJsonExtractClient:
    """Chat-based JSON extract for pilot metrics (not production KG write path)."""

    chat_client: NineRouterChatClient = field(default_factory=NineRouterChatClient)
    model: str = DEFAULT_PILOT_MODEL
    max_tokens: int = 900
    temperature: float = 0.0
    last_diagnostics: dict[str, Any] = field(default_factory=dict)

    def extract_case(self, body_text: str, case_id: str) -> dict[str, Any]:
        """Return parse_llm_extraction_json result; never raises."""
        prompt = build_extraction_user_prompt(case_id=case_id, body_text=body_text)
        result = self.chat_client.chat(
            model=self.model,
            messages=[
                {"role": "system", "content": _SYSTEM},
                {"role": "user", "content": prompt},
            ],
            max_tokens=self.max_tokens,
            temperature=self.temperature,
        )
        parsed = parse_llm_extraction_json(result.text if result.ok else "")
        self.last_diagnostics = {
            "provider": "ninerouter",
            "model": self.model,
            "chat_ok": result.ok,
            "json_valid": bool(parsed.get("json_valid")),
            "entity_count": len(parsed.get("entities") or []),
            "relation_count": len(parsed.get("relations") or []),
            "error": result.error,
            "credential_value_logged": False,
        }
        if not result.ok:
            parsed = {"entities": [], "relations": [], "json_valid": False}
        return parsed

    def as_extract_fn(self) -> Any:
        """Bound callable for score_gold_hybrid_llm_pilot(extract_fn=...)."""

        def _fn(body_text: str, case_id: str) -> Mapping[str, Any]:
            return self.extract_case(body_text, case_id)

        return _fn


__all__ = [
    "DEFAULT_PILOT_MODEL",
    "QUALITY_PILOT_MODEL",
    "NineRouterJsonExtractClient",
    "build_extraction_user_prompt",
]
