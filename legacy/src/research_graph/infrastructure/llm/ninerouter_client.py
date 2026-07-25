"""Thin local 9router OpenAI-compatible chat client (M254 S02).

Primary LLM surface for paper-summary and general chat via
``NINEROUTER_URL`` (default ``http://127.0.0.1:20128``) and optional
``NINEROUTER_KEY``.

Not an extraction ``LLMClientPort`` adapter (that stays MiniMax/GLM forced-tool).
This client is chat-only: ``POST /v1/chat/completions``.

Safety:
  * never logs credential values
  * HTTP injectible for tests
  * empty/error results are fail-closed (ok=False) unless raise_on_error
"""

from __future__ import annotations

import os
import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

import httpx

DEFAULT_NINEROUTER_URL = "http://127.0.0.1:20128"
DEFAULT_CHAT_PATH = "/v1/chat/completions"

#: ``(method, url, headers, json_body) -> response_json``
HttpPostJson = Callable[[str, str, Mapping[str, str], Mapping[str, Any]], dict[str, Any]]

_SECRET_PATTERN = re.compile(
    r"(?i)(sk-[a-z0-9][a-z0-9._\-]{8,}|bearer\s+[a-z0-9._\-]{8,}"
    r"|api[_-]?key\s*[:=]\s*\S+|super-secret[^\s]*)"
)


class NineRouterChatError(RuntimeError):
    """Transport or parse failure for 9router chat (no secrets in message)."""


def _redact(text: str) -> str:
    return _SECRET_PATTERN.sub("[REDACTED]", text)[:400]


def message_text_from_choice(message: Mapping[str, Any] | None) -> str:
    """Prefer ``content``; fall back to ``reasoning_content`` (GLM quirk)."""
    if not isinstance(message, Mapping):
        return ""
    content = str(message.get("content") or "").strip()
    if content:
        return content
    return str(message.get("reasoning_content") or "").strip()


def _normalize_base_url(url: str) -> str:
    base = url.rstrip("/")
    if base.endswith("/v1"):
        base = base[: -len("/v1")]
    return base.rstrip("/")


def _default_http_post_json(
    method: str, url: str, headers: Mapping[str, str], body: Mapping[str, Any]
) -> dict[str, Any]:
    with httpx.Client(timeout=120.0) as client:
        response = client.request(method, url, headers=dict(headers), json=dict(body))
        response.raise_for_status()
        data = response.json()
        if not isinstance(data, dict):
            raise ValueError("ninerouter_response_not_object")
        return data


@dataclass(frozen=True)
class NineRouterChatResult:
    ok: bool
    text: str
    content: str
    reasoning_content: str
    model: str
    usage: dict[str, Any] | None
    error: str | None
    raw_choice_count: int = 0

    def to_dict(self) -> dict[str, Any]:
        return {
            "ok": self.ok,
            "text": self.text,
            "content": self.content,
            "reasoning_content": self.reasoning_content,
            "model": self.model,
            "usage": self.usage,
            "error": self.error,
            "raw_choice_count": self.raw_choice_count,
        }


@dataclass
class NineRouterChatClient:
    """OpenAI-compatible chat client for local 9router."""

    base_url: str | None = None
    api_key: str | None = None
    http_post_json: HttpPostJson = field(default=_default_http_post_json)
    raise_on_error: bool = False
    last_diagnostics: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.base_url is None:
            self.base_url = os.environ.get("NINEROUTER_URL") or DEFAULT_NINEROUTER_URL
        if self.api_key is None:
            self.api_key = os.environ.get("NINEROUTER_KEY") or ""
        self.base_url = _normalize_base_url(str(self.base_url))

    def __repr__(self) -> str:
        return (
            f"NineRouterChatClient(base_url={self.base_url!r}, "
            f"api_key_present={bool(self.api_key)}, "
            f"raise_on_error={self.raise_on_error})"
        )

    def chat(
        self,
        *,
        model: str,
        messages: Sequence[Mapping[str, str]],
        max_tokens: int = 700,
        temperature: float = 0.2,
        extra_body: Mapping[str, Any] | None = None,
    ) -> NineRouterChatResult:
        """POST /v1/chat/completions; fail-closed result unless raise_on_error."""
        self.last_diagnostics = {
            "provider": "ninerouter",
            "model": model,
            "ok": False,
            "credential_value_logged": False,
            "api_key_present": bool(self.api_key),
        }
        url = f"{self.base_url}{DEFAULT_CHAT_PATH}"
        headers: dict[str, str] = {"Content-Type": "application/json"}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        body: dict[str, Any] = {
            "model": model,
            "messages": [dict(m) for m in messages],
            "max_tokens": int(max_tokens),
            "temperature": float(temperature),
            "stream": False,
        }
        if extra_body:
            body.update(dict(extra_body))

        try:
            data = self.http_post_json("POST", url, headers, body)
            choices = data.get("choices") or []
            if not isinstance(choices, list):
                choices = []
            msg = {}
            if choices and isinstance(choices[0], Mapping):
                raw_msg = choices[0].get("message") or {}
                if isinstance(raw_msg, Mapping):
                    msg = raw_msg
            content = str(msg.get("content") or "").strip()
            reasoning = str(msg.get("reasoning_content") or "").strip()
            text = message_text_from_choice(msg)
            usage = data.get("usage") if isinstance(data.get("usage"), dict) else None
            result = NineRouterChatResult(
                ok=True,
                text=text,
                content=content,
                reasoning_content=reasoning,
                model=model,
                usage=usage,
                error=None,
                raw_choice_count=len(choices),
            )
            self.last_diagnostics.update(
                {
                    "ok": True,
                    "raw_choice_count": len(choices),
                    "used_reasoning_content": bool(not content and reasoning),
                }
            )
            return result
        except Exception as exc:  # noqa: BLE001 - fail-closed chat boundary
            err = _redact(f"{type(exc).__name__}: {exc}")
            self.last_diagnostics.update(
                {"ok": False, "diagnostic_codes": (err,), "error": err}
            )
            if self.raise_on_error:
                raise NineRouterChatError(err) from None
            return NineRouterChatResult(
                ok=False,
                text="",
                content="",
                reasoning_content="",
                model=model,
                usage=None,
                error=err,
                raw_choice_count=0,
            )


__all__ = [
    "DEFAULT_NINEROUTER_URL",
    "NineRouterChatClient",
    "NineRouterChatError",
    "NineRouterChatResult",
    "message_text_from_choice",
]
