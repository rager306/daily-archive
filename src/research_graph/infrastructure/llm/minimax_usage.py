# Formerly: src/arxiv_archive/minimax_usage.py

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

MINIMAX_GLOBAL_USAGE_ENDPOINTS: tuple[str, ...] = (
    "https://www.minimax.io/v1/token_plan/remains",
    "https://api.minimax.io/v1/api/openplatform/coding_plan/remains",
)


MINIMAX_KEY_ENV_ORDER: tuple[str, ...] = (
    "MINIMAX_API_KEY",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
)


@dataclass(frozen=True)
class MiniMaxApiKeyResolution:
    """Resolved MiniMax key plus redacted alias diagnostics."""

    value: str | None
    source_env: str | None
    alias_envs_present: tuple[str, ...]
    distinct_value_count: int

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "source_env": self.source_env,
            "alias_envs_present": list(self.alias_envs_present),
            "key_present": self.value is not None,
            "distinct_value_count": self.distinct_value_count,
            "alias_value_mismatch": self.distinct_value_count > 1,
            "credential_value_logged": False,
        }


@dataclass(frozen=True, repr=False)
class MiniMaxUsageRequest:
    """Prepared MiniMax usage request; raw headers may contain secrets."""

    endpoint: str
    method: str
    headers: dict[str, str]

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "endpoint": self.endpoint,
            "method": self.method,
            "headers": {
                "Authorization": "Bearer <redacted>",
                "Accept": self.headers.get("Accept", ""),
                "Content-Type": self.headers.get("Content-Type", ""),
            },
            "credential_value_logged": False,
        }


@dataclass(frozen=True)
class SafeMiniMaxQuotaSummary:
    """Sanitized quota summary with percentages only, not exact account values."""

    name: str
    window: str
    used_percentage: float
    remaining_percentage: float
    reset_at_present: bool


@dataclass(frozen=True)
class MiniMaxUsageSummary:
    """Sanitized MiniMax usage/remains parse result."""

    endpoint: str
    http_status: int
    provider_status_code: int | None
    true_success: bool
    model_remains_present: bool
    model_remains_count: int
    quota_row_count: int
    count_semantics: str
    safe_quota_summaries: tuple[SafeMiniMaxQuotaSummary, ...]
    raw_response_persisted: bool = False
    exact_quota_values_persisted: bool = False
    credential_values_logged: bool = False

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "endpoint": self.endpoint,
            "http_status": self.http_status,
            "provider_status_code": self.provider_status_code,
            "true_success": self.true_success,
            "model_remains_present": self.model_remains_present,
            "model_remains_count": self.model_remains_count,
            "quota_row_count": self.quota_row_count,
            "count_semantics": self.count_semantics,
            "safe_quota_summaries": [
                {
                    "name": item.name,
                    "window": item.window,
                    "used_percentage": item.used_percentage,
                    "remaining_percentage": item.remaining_percentage,
                    "reset_at_present": item.reset_at_present,
                }
                for item in self.safe_quota_summaries
            ],
            "raw_response_persisted": self.raw_response_persisted,
            "exact_quota_values_persisted": self.exact_quota_values_persisted,
            "credential_values_logged": self.credential_values_logged,
        }


def resolve_minimax_api_key(environ: dict[str, str]) -> MiniMaxApiKeyResolution:
    """Resolve one canonical MiniMax key from canonical and SDK alias env vars."""

    present = {name: value for name in MINIMAX_KEY_ENV_ORDER if (value := environ.get(name))}
    source_env = next((name for name in MINIMAX_KEY_ENV_ORDER if name in present), None)
    value = present.get(source_env) if source_env else None
    distinct_value_count = len(set(present.values()))
    alias_envs_present = tuple(
        name for name in ("ANTHROPIC_API_KEY", "OPENAI_API_KEY") if name in present
    )
    return MiniMaxApiKeyResolution(
        value=value,
        source_env=source_env,
        alias_envs_present=alias_envs_present,
        distinct_value_count=distinct_value_count,
    )



def build_minimax_usage_requests(api_key: str) -> tuple[MiniMaxUsageRequest, ...]:
    """Build 9router-compatible global MiniMax usage/remains requests."""

    return tuple(
        MiniMaxUsageRequest(
            endpoint=endpoint,
            method="GET",
            headers={
                "Authorization": f"Bearer {api_key}",
                "Accept": "application/json",
                "Content-Type": "application/json",
            },
        )
        for endpoint in MINIMAX_GLOBAL_USAGE_ENDPOINTS
    )



def _field(mapping: dict[str, Any], snake_key: str, camel_key: str) -> Any:
    return mapping.get(snake_key, mapping.get(camel_key))


def _number(value: Any) -> float:
    if isinstance(value, int | float):
        return float(value)
    if isinstance(value, str) and value.strip():
        try:
            return float(value)
        except ValueError:
            return 0.0
    return 0.0


def _percentage(numerator: float, denominator: float) -> float:
    if denominator <= 0:
        return 0.0
    return round(max(0.0, min(100.0, (numerator / denominator) * 100.0)), 2)


def _quota_summary(
    *,
    model: dict[str, Any],
    window: str,
    total_key: tuple[str, str],
    count_key: tuple[str, str],
    remains_key: tuple[str, str],
    count_means_remaining: bool,
) -> SafeMiniMaxQuotaSummary | None:
    total = _number(_field(model, *total_key))
    if total <= 0:
        return None
    count = max(0.0, _number(_field(model, *count_key)))
    remaining = min(count, total) if count_means_remaining else max(total - count, 0.0)
    used = max(total - remaining, 0.0)
    model_name = str(_field(model, "model_name", "modelName") or "MiniMax").strip() or "MiniMax"
    reset_at_present = _field(model, *remains_key) is not None
    return SafeMiniMaxQuotaSummary(
        name=model_name,
        window=window,
        used_percentage=_percentage(used, total),
        remaining_percentage=_percentage(remaining, total),
        reset_at_present=reset_at_present,
    )


def parse_minimax_usage_response(
    payload: dict[str, Any], *, endpoint: str, http_status: int
) -> MiniMaxUsageSummary:
    """Parse MiniMax usage/remains payload into sanitized account-safe metadata."""

    base_resp = payload.get("base_resp", payload.get("baseResp"))
    provider_status_code = None
    if isinstance(base_resp, dict):
        raw_status = base_resp.get("status_code", base_resp.get("statusCode"))
        try:
            provider_status_code = int(raw_status) if raw_status is not None else None
        except (TypeError, ValueError):
            provider_status_code = None

    model_remains = payload.get("model_remains", payload.get("modelRemains"))
    models = model_remains if isinstance(model_remains, list) else []
    count_means_remaining = "/coding_plan/remains" in endpoint
    summaries: list[SafeMiniMaxQuotaSummary] = []
    for model in models:
        if not isinstance(model, dict):
            continue
        session = _quota_summary(
            model=model,
            window="5h",
            total_key=("current_interval_total_count", "currentIntervalTotalCount"),
            count_key=("current_interval_usage_count", "currentIntervalUsageCount"),
            remains_key=("remains_time", "remainsTime"),
            count_means_remaining=count_means_remaining,
        )
        weekly = _quota_summary(
            model=model,
            window="7d",
            total_key=("current_weekly_total_count", "currentWeeklyTotalCount"),
            count_key=("current_weekly_usage_count", "currentWeeklyUsageCount"),
            remains_key=("weekly_remains_time", "weeklyRemainsTime"),
            count_means_remaining=count_means_remaining,
        )
        summaries.extend(item for item in (session, weekly) if item is not None)

    http_ok = 200 <= http_status < 300
    true_success = http_ok and provider_status_code == 0 and bool(summaries)
    return MiniMaxUsageSummary(
        endpoint=endpoint,
        http_status=http_status,
        provider_status_code=provider_status_code,
        true_success=true_success,
        model_remains_present=isinstance(model_remains, list),
        model_remains_count=len(models),
        quota_row_count=len(summaries),
        count_semantics="remaining" if count_means_remaining else "used",
        safe_quota_summaries=tuple(summaries),
    )
