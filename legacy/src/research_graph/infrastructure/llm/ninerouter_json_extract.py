"""9router chat → JSON extraction helper for Wave B pilot.

Not MiniMax forced-tool. Uses OpenAI-compatible chat with strict JSON instructions.
Fail-closed: bad parse → empty entities/relations, json_valid=false.
Never logs secrets.

Supports two modes:
  - legacy raw body window (build_extraction_user_prompt)
  - structured context pack (outline/sections/candidates + optional section follow-up)
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
from research_graph.application.corpus.wave_b_structured_extract_context import (
    build_structured_extract_context,
    parse_need_sections,
    render_structured_extract_prompt,
)
from research_graph.infrastructure.llm.ninerouter_client import NineRouterChatClient

DEFAULT_PILOT_MODEL = "agnes-ai/agnes-2.0-flash"
AGNES_25_PILOT_MODEL = "agnes-ai/agnes-2.5-flash"
AGNES_FREE_25_PILOT_MODEL = "agnes-ai-free/agnes-2.5-flash"
AGNES_FREE_20_PILOT_MODEL = "agnes-ai-free/agnes-2.0-flash"
QUALITY_PILOT_MODEL = "minimax/MiniMax-M2.7-highspeed"

_SYSTEM = (
    "You extract scientific knowledge-graph candidates from structured paper context. "
    "Return ONLY a single JSON object. No markdown fences. No prose. "
    "No thinking, no explanation, no preamble — JSON object only. "
    "Prefer grounded candidates and section evidence. "
    "You may request more sections via need_sections instead of inventing labels. "
    "Candidates only — never claim import eligibility or production facts."
)

_SYSTEM_RAW = (
    "You extract scientific knowledge-graph candidates from paper text. "
    "Return ONLY a single JSON object. No markdown fences. No prose. "
    "No thinking, no explanation, no preamble — JSON object only. "
    "Candidates only — never claim import eligibility or production facts."
)


def build_extraction_user_prompt(*, case_id: str, body_text: str) -> str:
    """Legacy raw-md prompt (kept for ablations / rollback)."""
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
    use_structured_context: bool = True
    max_followup_rounds: int = 1
    last_diagnostics: dict[str, Any] = field(default_factory=dict)

    def extract_case(
        self,
        body_text: str,
        case_id: str,
        *,
        paper_id: str = "",
    ) -> dict[str, Any]:
        """Return parse_llm_extraction_json result; never raises."""
        if self.use_structured_context:
            return self._extract_structured(
                body_text=body_text, case_id=case_id, paper_id=paper_id
            )
        return self._extract_raw(body_text=body_text, case_id=case_id)

    def _extract_raw(self, *, body_text: str, case_id: str) -> dict[str, Any]:
        prompt = build_extraction_user_prompt(case_id=case_id, body_text=body_text)
        result = self.chat_client.chat(
            model=self.model,
            messages=[
                {"role": "system", "content": _SYSTEM_RAW},
                {"role": "user", "content": prompt},
            ],
            max_tokens=self.max_tokens,
            temperature=self.temperature,
        )
        parsed = parse_llm_extraction_json(result.text if result.ok else "")
        self.last_diagnostics = {
            "provider": "ninerouter",
            "model": self.model,
            "mode": "raw_body",
            "chat_ok": result.ok,
            "json_valid": bool(parsed.get("json_valid")),
            "entity_count": len(parsed.get("entities") or []),
            "relation_count": len(parsed.get("relations") or []),
            "error": result.error,
            "credential_value_logged": False,
            "followup_rounds": 0,
        }
        if not result.ok:
            parsed = {"entities": [], "relations": [], "json_valid": False}
        return parsed

    def _extract_structured(
        self, *, body_text: str, case_id: str, paper_id: str = ""
    ) -> dict[str, Any]:
        ctx = build_structured_extract_context(
            body_text=body_text,
            paper_id=paper_id or case_id,
            case_id=case_id,
        )
        followups: list[dict[str, Any]] = []
        requested_total: list[str] = []
        rounds = 0
        parsed: dict[str, Any] = {
            "entities": [],
            "relations": [],
            "json_valid": False,
        }
        last_error: str | None = None
        chat_ok = False

        while True:
            prompt = render_structured_extract_prompt(
                ctx,
                allowed_entity_types=sorted(ALLOWED_ENTITY_TYPES),
                allowed_relation_types=sorted(ALLOWED_RELATION_TYPES),
                followup_sections=followups,
            )
            result = self.chat_client.chat(
                model=self.model,
                messages=[
                    {"role": "system", "content": _SYSTEM},
                    {"role": "user", "content": prompt},
                ],
                max_tokens=self.max_tokens,
                temperature=self.temperature,
            )
            chat_ok = bool(result.ok)
            last_error = result.error
            if not result.ok:
                parsed = {"entities": [], "relations": [], "json_valid": False}
                break
            parsed = parse_llm_extraction_json(result.text)
            # recover need_sections even if parser strips unknown keys
            need = parse_need_sections(parsed)
            if not need:
                # try raw JSON for need_sections only
                need = _extract_need_sections_from_raw(result.text)
            if not need or rounds >= max(0, int(self.max_followup_rounds)):
                break
            resolved = ctx.resolve_followup_sections(need, max_sections=4)
            if not resolved:
                # Cannot satisfy follow-up; keep current parse (may be empty) and stop.
                break
            # only keep newly requested sections not already in pack text budget
            already = {str(x.get("section_id")) for x in followups}
            new_secs = [
                s for s in resolved if str(s.get("section_id")) not in already
            ]
            if not new_secs:
                break
            followups.extend(new_secs)
            requested_total.extend(str(s.get("section_id")) for s in new_secs)
            rounds += 1
            # continue loop → second chat with FOLLOWUP SECTIONS filled

        self.last_diagnostics = {
            "provider": "ninerouter",
            "model": self.model,
            "mode": "structured_context",
            "chat_ok": chat_ok,
            "json_valid": bool(parsed.get("json_valid")),
            "entity_count": len(parsed.get("entities") or []),
            "relation_count": len(parsed.get("relations") or []),
            "error": last_error,
            "credential_value_logged": False,
            "followup_rounds": rounds,
            "requested_sections": requested_total,
            "outline_count": len(ctx.outline),
            "section_count": len(ctx.section_catalog),
            "candidate_count": len(ctx.candidates),
            "import_eligible": False,
        }
        return parsed

    def as_extract_fn(self) -> Any:
        """Bound callable for score_gold_hybrid_llm_pilot(extract_fn=...)."""

        def _fn(body_text: str, case_id: str) -> Mapping[str, Any]:
            return self.extract_case(body_text, case_id)

        return _fn


def _extract_need_sections_from_raw(raw: str) -> list[str]:
    import json
    import re

    text = raw or ""
    # fenced or bare object
    m = re.search(r"\{[\s\S]*\}", text)
    if not m:
        return []
    try:
        obj = json.loads(m.group(0))
    except json.JSONDecodeError:
        return []
    if isinstance(obj, Mapping):
        return parse_need_sections(obj)
    return []


__all__ = [
    "AGNES_25_PILOT_MODEL",
    "AGNES_FREE_20_PILOT_MODEL",
    "AGNES_FREE_25_PILOT_MODEL",
    "DEFAULT_PILOT_MODEL",
    "QUALITY_PILOT_MODEL",
    "NineRouterJsonExtractClient",
    "build_extraction_user_prompt",
]
