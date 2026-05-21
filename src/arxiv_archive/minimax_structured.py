from __future__ import annotations

from dataclasses import dataclass
from typing import Any

MINIMAX_ANTHROPIC_MESSAGES_ENDPOINT = "https://api.minimax.io/anthropic/v1/messages"
DEFAULT_MINIMAX_MODEL = "MiniMax-M2.7-highspeed"
DEFAULT_MAX_TOKENS = 1024
DEFAULT_TEMPERATURE = 0.2


@dataclass(frozen=True)
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
    model: str = DEFAULT_MINIMAX_MODEL,
    max_tokens: int = DEFAULT_MAX_TOKENS,
    temperature: float = DEFAULT_TEMPERATURE,
) -> MiniMaxStructuredRequest:
    """Build a dev-only forced-tool request for MiniMax structured helper output."""

    if payload_class not in {"synthetic", "redacted"}:
        raise ValueError("MiniMax structured helper only accepts synthetic or redacted payloads")
    if temperature <= 0 or temperature > 1:
        raise ValueError("MiniMax temperature must be in (0.0, 1.0]")
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
        endpoint=MINIMAX_ANTHROPIC_MESSAGES_ENDPOINT,
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
