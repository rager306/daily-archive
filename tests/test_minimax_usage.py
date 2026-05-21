from __future__ import annotations

import json

from arxiv_archive.minimax_usage import (
    build_minimax_usage_requests,
    parse_minimax_usage_response,
    resolve_minimax_api_key,
)


def test_parse_coding_plan_counts_as_remaining_without_exact_values() -> None:
    payload = {
        "base_resp": {"status_code": 0, "status_msg": ""},
        "model_remains": [
            {
                "model_name": "MiniMax-M2.7",
                "current_interval_total_count": 4000,
                "current_interval_usage_count": 3990,
                "current_weekly_total_count": 10000,
                "current_weekly_usage_count": 9750,
                "remains_time": 1000,
                "weekly_remains_time": 2000,
            }
        ],
    }

    summary = parse_minimax_usage_response(
        payload,
        endpoint="https://api.minimax.io/v1/api/openplatform/coding_plan/remains",
        http_status=200,
    )

    dumped = json.dumps(summary.to_sanitized_dict())
    assert summary.true_success is True
    assert summary.quota_row_count == 2
    assert summary.count_semantics == "remaining"
    assert summary.safe_quota_summaries[0].remaining_percentage == 99.75
    assert summary.safe_quota_summaries[0].used_percentage == 0.25
    assert "3990" not in dumped
    assert "4000" not in dumped
    assert "9750" not in dumped
    assert summary.raw_response_persisted is False
    assert summary.exact_quota_values_persisted is False


def test_parse_token_plan_counts_as_used_and_rejects_provider_error() -> None:
    payload = {
        "baseResp": {"statusCode": 0, "statusMsg": "success"},
        "modelRemains": [
            {
                "modelName": "MiniMax-M2.7-highspeed",
                "currentIntervalTotalCount": "100",
                "currentIntervalUsageCount": "25",
                "currentWeeklyTotalCount": "1000",
                "currentWeeklyUsageCount": "100",
            }
        ],
    }

    summary = parse_minimax_usage_response(
        payload,
        endpoint="https://www.minimax.io/v1/token_plan/remains",
        http_status=200,
    )

    assert summary.true_success is True
    assert summary.count_semantics == "used"
    assert summary.safe_quota_summaries[0].used_percentage == 25.0
    assert summary.safe_quota_summaries[0].remaining_percentage == 75.0
    assert summary.safe_quota_summaries[1].used_percentage == 10.0
    assert summary.safe_quota_summaries[1].remaining_percentage == 90.0

    provider_error = parse_minimax_usage_response(
        {"base_resp": {"status_code": 1004, "status_msg": "auth failed"}},
        endpoint="https://api.minimax.io/v1/api/openplatform/coding_plan/remains",
        http_status=200,
    )

    assert provider_error.true_success is False
    assert provider_error.provider_status_code == 1004
    assert provider_error.quota_row_count == 0


def test_build_usage_requests_uses_canonical_key_without_leaking_secret() -> None:
    requests = build_minimax_usage_requests("sk-test-secret")

    assert [request.endpoint for request in requests] == [
        "https://www.minimax.io/v1/token_plan/remains",
        "https://api.minimax.io/v1/api/openplatform/coding_plan/remains",
    ]
    assert all(request.method == "GET" for request in requests)
    assert all(request.headers["Authorization"] == "Bearer sk-test-secret" for request in requests)
    assert all(request.headers["Accept"] == "application/json" for request in requests)
    dumped = json.dumps([request.to_sanitized_dict() for request in requests])
    assert "sk-test-secret" not in dumped
    assert "Bearer" in dumped
    assert "X-Api-Key" not in dumped
    assert "sk-test-secret" not in repr(requests[0])


def test_resolve_minimax_api_key_prefers_canonical_and_reports_aliases_safely() -> None:
    resolved = resolve_minimax_api_key(
        {
            "OPENAI_API_KEY": "sk-same-value",
            "ANTHROPIC_API_KEY": "sk-same-value",
            "MINIMAX_API_KEY": "sk-same-value",
        }
    )

    assert resolved.value == "sk-same-value"
    assert resolved.source_env == "MINIMAX_API_KEY"
    assert resolved.alias_envs_present == ("ANTHROPIC_API_KEY", "OPENAI_API_KEY")
    dumped = json.dumps(resolved.to_sanitized_dict())
    assert "sk-same-value" not in dumped
    assert resolved.to_sanitized_dict()["distinct_value_count"] == 1


def test_resolve_minimax_api_key_reports_distinct_alias_values_without_logging_them() -> None:
    resolved = resolve_minimax_api_key(
        {
            "OPENAI_API_KEY": "sk-openai-value",
            "MINIMAX_API_KEY": "sk-minimax-value",
        }
    )

    assert resolved.value == "sk-minimax-value"
    safe = resolved.to_sanitized_dict()
    dumped = json.dumps(safe)
    assert safe["distinct_value_count"] == 2
    assert safe["alias_value_mismatch"] is True
    assert "sk-openai-value" not in dumped
    assert "sk-minimax-value" not in dumped
