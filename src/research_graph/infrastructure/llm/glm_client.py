"""GLM/Z.ai LLMClientPort adapter for semantic extraction (M201 S02).

Same application contract as :class:`MiniMaxLLMClient`: forced-tool Anthropic-
compatible messages, shared extraction schemas, fail-closed empty dict.
Auth uses ``Authorization: Bearer`` (Z.ai Anthropic-compatible convention).
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
    validate_minimax_tool_response,
)
from research_graph.infrastructure.llm.provider_config import (
    GLM_DEFAULT_ANTHROPIC_BASE_URL,
    GLM_DEFAULT_MODEL,
    PROVIDER_GLM_ZAI,
    load_provider_config,
)

HttpPostJson = Callable[[str, str, Mapping[str, str], Mapping[str, Any]], dict[str, Any]]


def _default_http_post_json(
    method: str, url: str, headers: Mapping[str, str], body: Mapping[str, Any]
) -> dict[str, Any]:
    with httpx.Client(timeout=60.0) as client:
        response = client.request(method, url, headers=dict(headers), json=dict(body))
        response.raise_for_status()
        data = response.json()
        if not isinstance(data, dict):
            raise ValueError("glm_response_not_object")
        return data


def _messages_endpoint(base_url: str) -> str:
    base = base_url.rstrip("/")
    if base.endswith("/messages"):
        return base
    if base.endswith("/v1"):
        return f"{base}/messages"
    return f"{base}/v1/messages"


@dataclass
class GLMLLMClient:
    """Anthropic-compatible GLM/Z.ai forced-tool client implementing LLMClientPort."""

    api_key: str | None = None
    model: str | None = None
    endpoint: str | None = None
    http_post_json: HttpPostJson = field(default=_default_http_post_json)
    last_diagnostics: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.api_key is None or self.model is None or self.endpoint is None:
            cfg = load_provider_config(PROVIDER_GLM_ZAI)
            if self.api_key is None:
                self.api_key = cfg.api_key
            if self.model is None:
                self.model = cfg.model or GLM_DEFAULT_MODEL
            if self.endpoint is None:
                self.endpoint = _messages_endpoint(
                    cfg.anthropic_base_url or GLM_DEFAULT_ANTHROPIC_BASE_URL
                )
        if self.model is None:
            self.model = GLM_DEFAULT_MODEL
        if self.endpoint is None:
            self.endpoint = _messages_endpoint(GLM_DEFAULT_ANTHROPIC_BASE_URL)

    def extract(
        self, prompt: str, kind: str, *, context: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        del context
        self.last_diagnostics = {
            "provider": "glm",
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

        body: dict[str, Any] = {
            "model": self.model,
            "max_tokens": 1024,
            "temperature": 0.2,
            "messages": [{"role": "user", "content": prompt}],
            "tools": [
                {
                    "name": tool_name,
                    "description": tool_description,
                    "input_schema": schema,
                }
            ],
            "tool_choice": {"type": "tool", "name": tool_name},
        }
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
            "Accept": "application/json",
        }
        try:
            response = self.http_post_json("POST", self.endpoint or "", headers, body)
        except Exception as exc:  # noqa: BLE001
            self.last_diagnostics["diagnostic_codes"] = (f"transport:{type(exc).__name__}",)
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
        if kind in (EXTRACTION_KIND_ENTITIES, "entities"):
            return (
                ENTITY_EXTRACTION_TOOL_NAME,
                ENTITY_EXTRACTION_TOOL_DESCRIPTION,
                entity_extraction_input_schema(),
                "entities",
            )
        if kind in (EXTRACTION_KIND_RELATIONS, "relations"):
            return (
                RELATION_EXTRACTION_TOOL_NAME,
                RELATION_EXTRACTION_TOOL_DESCRIPTION,
                relation_extraction_input_schema(),
                "relations",
            )
        raise ValueError(f"unsupported_extraction_kind:{kind}")


def _as_port(client: GLMLLMClient) -> LLMClientPort:
    return client


__all__ = ["GLMLLMClient", "HttpPostJson"]
