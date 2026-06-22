# Formerly: src/arxiv_archive/minimax_structured.py

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from research_graph.infrastructure.llm.models_registry import (
    ModelsRegistry,
    get_model,
    get_model_for_binding,
    load_models_registry,
)

# Module-level constants are kept as fallback for callers that explicitly
# want to bypass the registry (e.g., synthetic test fixtures). Production
# helpers should rely on the registry via _resolve_default_model().
MINIMAX_ANTHROPIC_MESSAGES_ENDPOINT = "https://api.minimax.io/anthropic/v1/messages"
DEFAULT_MINIMAX_MODEL = "MiniMax-M3-512k"
DEFAULT_MAX_TOKENS = 1024
DEFAULT_TEMPERATURE = 0.2
DEFAULT_MODEL_ID = "minimax-m3-512k-anthropic"
DEFAULT_BINDING_ID = "article-artifact-classify"
RAW_CORPUS_MARKERS: tuple[str, ...] = (
    "RAW PAPER TEXT",
    "RAW CHUNK TEXT",
    "FULL ARTICLE BODY",
    "BEGIN PDF",
    "BASE64",
)


def _resolve_default_model(registry: ModelsRegistry | None = None) -> tuple[str, str]:
    """Resolve (model_name, endpoint) from registry. Falls back to module constants.

    Returns:
        (model_name, endpoint) tuple
    """
    if registry is None:
        try:
            registry = load_models_registry()
        except (FileNotFoundError, KeyError):
            return DEFAULT_MINIMAX_MODEL, MINIMAX_ANTHROPIC_MESSAGES_ENDPOINT
    try:
        model = get_model_for_binding(registry, DEFAULT_BINDING_ID)
    except KeyError:
        try:
            model = get_model(registry, DEFAULT_MODEL_ID)
        except KeyError:
            return DEFAULT_MINIMAX_MODEL, MINIMAX_ANTHROPIC_MESSAGES_ENDPOINT
    return model.model_name, model.endpoint


def _looks_like_raw_corpus_payload(prompt: str) -> bool:
    normalized = prompt.upper()
    return any(marker in normalized for marker in RAW_CORPUS_MARKERS)


@dataclass(frozen=True, repr=False)
class MiniMaxStructuredRequest:
    """Prepared Anthropic-compatible forced-tool request metadata."""

    endpoint: str
    auth_header: str
    body: dict[str, Any]
    raw_corpus_payload_allowed: bool = False
    minimax_source_of_truth: bool = False

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "endpoint": self.endpoint,
            "auth_header": self.auth_header,
            "model": self.body.get("model"),
            "tool_choice": self.body.get("tool_choice"),
            "tool_count": len(self.body.get("tools", [])),
            "temperature": self.body.get("temperature"),
            "raw_corpus_payload_allowed": self.raw_corpus_payload_allowed,
            "minimax_source_of_truth": self.minimax_source_of_truth,
            "raw_prompt_persisted": False,
            "credential_value_logged": False,
        }


@dataclass(frozen=True)
class MiniMaxToolValidationResult:
    """Local validation result for a MiniMax tool-use response."""

    valid: bool
    tool_name: str
    diagnostic_codes: tuple[str, ...]
    helper_evidence_only: bool = True
    minimax_source_of_truth: bool = False
    raw_model_content_persisted: bool = False

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "valid": self.valid,
            "tool_name": self.tool_name,
            "diagnostic_codes": list(self.diagnostic_codes),
            "helper_evidence_only": self.helper_evidence_only,
            "minimax_source_of_truth": self.minimax_source_of_truth,
            "raw_model_content_persisted": self.raw_model_content_persisted,
        }


def build_minimax_structured_request(
    *,
    prompt: str,
    tool_name: str,
    tool_description: str,
    input_schema: dict[str, Any],
    payload_class: str,
    model: str | None = None,
    endpoint: str | None = None,
    max_tokens: int = DEFAULT_MAX_TOKENS,
    temperature: float = DEFAULT_TEMPERATURE,
    registry: ModelsRegistry | None = None,
) -> MiniMaxStructuredRequest:
    """Build a dev-only forced-tool request for MiniMax structured helper output.

    If `model` is None, resolved from registry (DEFAULT_BINDING_ID = article-artifact-classify,
    fallback DEFAULT_MODEL_ID = minimax-m3-512k-anthropic). Same for `endpoint`.
    Pass explicit `model`/`endpoint` to bypass registry (e.g., synthetic tests).
    """

    if payload_class not in {"synthetic", "redacted"}:
        raise ValueError("MiniMax structured helper only accepts synthetic or redacted payloads")
    if _looks_like_raw_corpus_payload(prompt):
        raise ValueError("MiniMax structured helper refuses raw corpus payload markers")
    if temperature <= 0 or temperature > 1:
        raise ValueError("MiniMax temperature must be in (0.0, 1.0]")

    if model is None or endpoint is None:
        default_model, default_endpoint = _resolve_default_model(registry)
        if model is None:
            model = default_model
        if endpoint is None:
            endpoint = default_endpoint

    body = {
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "messages": [{"role": "user", "content": prompt}],
        "tools": [
            {
                "name": tool_name,
                "description": tool_description,
                "input_schema": input_schema,
            }
        ],
        "tool_choice": {"type": "tool", "name": tool_name},
    }
    return MiniMaxStructuredRequest(
        endpoint=endpoint,
        auth_header="X-Api-Key",
        body=body,
    )


def _type_matches(value: Any, expected_type: str) -> bool:
    if expected_type == "string":
        return isinstance(value, str)
    if expected_type == "number":
        return isinstance(value, int | float) and not isinstance(value, bool)
    if expected_type == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected_type == "boolean":
        return isinstance(value, bool)
    if expected_type == "array":
        return isinstance(value, list)
    if expected_type == "object":
        return isinstance(value, dict)
    return True


def _validate_schema(value: Any, schema: dict[str, Any], path: str = "$") -> list[str]:
    diagnostics: list[str] = []
    expected_type = schema.get("type")
    if isinstance(expected_type, str) and not _type_matches(value, expected_type):
        return [f"schema_type_mismatch:{path}:{expected_type}"]

    if "enum" in schema and value not in schema["enum"]:
        diagnostics.append(f"schema_enum_mismatch:{path}")

    if expected_type == "object" and isinstance(value, dict):
        required = schema.get("required", [])
        for key in required:
            if key not in value:
                diagnostics.append(f"schema_missing_required:{path}.{key}")
        properties = schema.get("properties", {})
        if isinstance(properties, dict):
            for key, child_schema in properties.items():
                if key in value and isinstance(child_schema, dict):
                    diagnostics.extend(_validate_schema(value[key], child_schema, f"{path}.{key}"))

    if expected_type == "array" and isinstance(value, list):
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(value):
                diagnostics.extend(_validate_schema(item, item_schema, f"{path}[{index}]"))

    return diagnostics


def validate_minimax_tool_response(
    content_blocks: list[dict[str, Any]], *, tool_name: str, input_schema: dict[str, Any]
) -> MiniMaxToolValidationResult:
    """Validate MiniMax tool_use input locally and return sanitized diagnostics."""

    for block in content_blocks:
        if block.get("type") != "tool_use":
            continue
        if block.get("name") != tool_name:
            return MiniMaxToolValidationResult(
                valid=False,
                tool_name=tool_name,
                diagnostic_codes=("unexpected_tool_name",),
            )
        tool_input = block.get("input")
        diagnostics = _validate_schema(tool_input, input_schema)
        return MiniMaxToolValidationResult(
            valid=not diagnostics,
            tool_name=tool_name,
            diagnostic_codes=tuple(diagnostics),
        )

    return MiniMaxToolValidationResult(
        valid=False,
        tool_name=tool_name,
        diagnostic_codes=("missing_tool_use",),
    )
