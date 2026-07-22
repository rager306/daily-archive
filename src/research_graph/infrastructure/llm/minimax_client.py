"""MiniMax LLMClientPort adapter for semantic extraction (M201 S01).

Reuses :func:`build_minimax_structured_request` and
:func:`validate_minimax_tool_response`. HTTP is injectible for tests.
Fail-closed: any transport/validation error returns ``{}`` (empty candidates),
never raw credential logs.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from typing import Any

import httpx

from research_graph.domain.ports import (
    EXTRACTION_KIND_ENTITIES,
    EXTRACTION_KIND_RELATIONS,
    LLMClientPort,
)
from research_graph.infrastructure.llm.extraction_schemas import (
    ENTITY_EXTRACTION_TOOL_DESCRIPTION,
    ENTITY_EXTRACTION_TOOL_NAME,
    RELATION_EXTRACTION_TOOL_DESCRIPTION,
    RELATION_EXTRACTION_TOOL_NAME,
    entity_extraction_input_schema,
    relation_extraction_input_schema,
)
from research_graph.infrastructure.llm.minimax_structured import (
    DEFAULT_MINIMAX_MODEL,
    MINIMAX_ANTHROPIC_MESSAGES_ENDPOINT,
    build_minimax_structured_request,
    validate_minimax_tool_response,
)
from research_graph.infrastructure.llm.provider_config import (
    PROVIDER_MINIMAX,
    load_provider_config,
)

#: ``(method, url, headers, json_body) -> response_json``
HttpPostJson = Callable[[str, str, Mapping[str, str], Mapping[str, Any]], dict[str, Any]]


def _default_http_post_json(
    method: str, url: str, headers: Mapping[str, str], body: Mapping[str, Any]
) -> dict[str, Any]:
    with httpx.Client(timeout=60.0) as client:
        response = client.request(method, url, headers=dict(headers), json=dict(body))
        response.raise_for_status()
        data = response.json()
        if not isinstance(data, dict):
            raise ValueError("minimax_response_not_object")
        return data


@dataclass
class MiniMaxLLMClient:
    """Anthropic-compatible MiniMax forced-tool client implementing LLMClientPort."""

    api_key: str | None = None
    model: str | None = None
    endpoint: str | None = None
    http_post_json: HttpPostJson = field(default=_default_http_post_json)
    last_diagnostics: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.api_key is None:
            cfg = load_provider_config(PROVIDER_MINIMAX)
            self.api_key = cfg.api_key
            if self.model is None:
                self.model = cfg.model
            if self.endpoint is None:
                # messages endpoint; provider config stores anthropic base without /messages
                base = cfg.anthropic_base_url.rstrip("/")
                self.endpoint = (
                    base if base.endswith("/messages") else f"{base}/messages"
                )
        if self.model is None:
            self.model = DEFAULT_MINIMAX_MODEL
        if self.endpoint is None:
            self.endpoint = MINIMAX_ANTHROPIC_MESSAGES_ENDPOINT

    def extract(
        self, prompt: str, kind: str, *, context: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        """Return structured extraction dict; ``{}`` on any failure (fail-closed)."""
        del context  # reserved for future statistical grounding headers
        self.last_diagnostics = {
            "provider": "minimax",
            "kind": kind,
            "valid": False,
            "diagnostic_codes": (),
            "credential_value_logged": False,
        }
        try:
            tool_name, tool_description, schema, result_key = self._tool_for_kind(kind)
        except ValueError as exc:
            self.last_diagnostics["diagnostic_codes"] = (str(exc),)
            return {}

        if not self.api_key:
            self.last_diagnostics["diagnostic_codes"] = ("missing_api_key",)
            return {}

        try:
            request = build_minimax_structured_request(
                prompt=prompt,
                tool_name=tool_name,
                tool_description=tool_description,
                input_schema=schema,
                payload_class="redacted",
                model=self.model,
                endpoint=self.endpoint,
            )
        except ValueError as exc:
            self.last_diagnostics["diagnostic_codes"] = (f"request_build:{exc}",)
            return {}

        headers = {
            request.auth_header: self.api_key,
            "Content-Type": "application/json",
            "Accept": "application/json",
        }
        try:
            response = self.http_post_json(
                "POST", request.endpoint, headers, request.body
            )
        except Exception as exc:  # noqa: BLE001 — transport boundary fail-closed
            self.last_diagnostics["diagnostic_codes"] = (
                f"transport:{type(exc).__name__}",
            )
            return {}

        content = response.get("content")
        if not isinstance(content, list):
            self.last_diagnostics["diagnostic_codes"] = ("missing_content_blocks",)
            return {}

        validation = validate_minimax_tool_response(
            content, tool_name=tool_name, input_schema=schema
        )
        self.last_diagnostics.update(
            {
                "valid": validation.valid,
                "diagnostic_codes": validation.diagnostic_codes,
                "tool_name": validation.tool_name,
                "helper_evidence_only": validation.helper_evidence_only,
            }
        )
        if not validation.valid:
            return {}

        for block in content:
            if block.get("type") == "tool_use" and block.get("name") == tool_name:
                tool_input = block.get("input")
                if isinstance(tool_input, dict) and result_key in tool_input:
                    return {result_key: tool_input[result_key]}
                if isinstance(tool_input, dict):
                    return tool_input
        self.last_diagnostics["diagnostic_codes"] = ("missing_tool_input",)
        return {}

    @staticmethod
    def _tool_for_kind(kind: str) -> tuple[str, str, dict[str, Any], str]:
        if kind == EXTRACTION_KIND_ENTITIES or kind == "entities":
            return (
                ENTITY_EXTRACTION_TOOL_NAME,
                ENTITY_EXTRACTION_TOOL_DESCRIPTION,
                entity_extraction_input_schema(),
                "entities",
            )
        if kind == EXTRACTION_KIND_RELATIONS or kind == "relations":
            return (
                RELATION_EXTRACTION_TOOL_NAME,
                RELATION_EXTRACTION_TOOL_DESCRIPTION,
                relation_extraction_input_schema(),
                "relations",
            )
        raise ValueError(f"unsupported_extraction_kind:{kind}")


# Structural Protocol satisfaction for type checkers
def _as_port(client: MiniMaxLLMClient) -> LLMClientPort:
    return client


__all__ = ["HttpPostJson", "MiniMaxLLMClient"]
