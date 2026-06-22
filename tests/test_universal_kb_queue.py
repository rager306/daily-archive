from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.workflows.universal_kb.queue import UniversalKBQueue


class FixedClock:
    def __init__(self, value: str = "2026-06-08T00:00:00Z") -> None:
        self.value = value

    def __call__(self) -> str:
        return self.value

    def set(self, value: str) -> None:
        self.value = value


def _queue(tmp_path: Path, clock: FixedClock | None = None) -> UniversalKBQueue:
    return UniversalKBQueue(tmp_path / "queue.sqlite", clock=clock or FixedClock()).initialize()


def test_rejects_raw_or_secret_shaped_input_refs(tmp_path: Path) -> None:
    queue = _queue(tmp_path)

    with pytest.raises(ValueError, match="input_ref must"):
        queue.enqueue(
            job_id="job-raw-ref",
            stage="review",
            input_refs=("this is raw source text, not a metadata ref",),
            input_hash="sha256:input",
            tool_version="tool-v1",
            contract_version="contract-v1",
        )

    with pytest.raises(ValueError, match="input_ref must be redacted"):
        queue.enqueue(
            job_id="job-secret-ref",
            stage="review",
            input_refs=("artifact:sk-live-abc1234567890",),
            input_hash="sha256:input",
            tool_version="tool-v1",
            contract_version="contract-v1",
        )


def test_rejects_secret_shaped_persisted_diagnostics(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-secret",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-1", lease_seconds=30)

    with pytest.raises(ValueError, match="diagnostic must be redacted"):
        queue.fail_retryable(
            "job-secret",
            worker_id="worker-1",
            error_code="provider_error",
            redacted_message="sk-live-abc1234567890",
            retry_after="2026-06-08T00:01:00Z",
        )
    with pytest.raises(ValueError, match="error_code must be a metadata code"):
        queue.fail_retryable(
            "job-secret",
            worker_id="worker-1",
            error_code="sk-live-abc1234567890",
            redacted_message="provider_error",
            retry_after="2026-06-08T00:01:00Z",
        )

    inspected = queue.inspect("job-secret")
    serialized = str(inspected)
    assert "sk-live" not in serialized


def test_enqueue_adds_safe_payload_metadata_defaults(tmp_path: Path) -> None:
    queue = _queue(tmp_path)

    job = queue.enqueue(
        job_id="job-default-payload-metadata",
        stage="extract",
        input_refs=("artifact:paper-manifest",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )

    assert job["payload_metadata"] == {
        "schema_version": None,
        "stable_id_version": None,
        "metric_bundle_id": None,
        "extractor_version": None,
        "prompt_program_hash": None,
        "source_artifact_refs": [],
        "evidence_path_refs": [],
        "cost_estimate": None,
        "latency_ms": None,
        "retry_count": 0,
        "diagnostics": {},
        "write_eligibility": False,
        "promotion_eligibility": False,
    }
    assert job["safety_flags"]["graphdb_written"] is False
    assert job["safety_flags"]["import_eligible"] is False


def test_enqueue_roundtrips_m069_payload_metadata(tmp_path: Path) -> None:
    queue = _queue(tmp_path)

    job = queue.enqueue(
        job_id="job-m069-payload-metadata",
        stage="extract",
        input_refs=("artifact:paper-manifest",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
        payload_metadata={
            "schema_version": "schema:m069_v1",
            "stable_id_version": "stable_id:m069_v1",
            "metric_bundle_id": "metric_bundle:m069_v1",
            "extractor_version": "extractor:minimax_v1",
            "prompt_program_hash": "hash:abc123",
            "source_artifact_refs": ["artifact:paper-manifest"],
            "evidence_path_refs": ["evidence:path-001"],
            "cost_estimate": 0.12,
            "latency_ms": 1530,
            "retry_count": 1,
            "diagnostics": {"json_valid": True, "schema_status": "valid"},
            "write_eligibility": False,
            "promotion_eligibility": False,
        },
    )

    assert job["payload_metadata"]["schema_version"] == "schema:m069_v1"
    assert job["payload_metadata"]["metric_bundle_id"] == "metric_bundle:m069_v1"
    assert job["payload_metadata"]["source_artifact_refs"] == ["artifact:paper-manifest"]
    assert job["payload_metadata"]["evidence_path_refs"] == ["evidence:path-001"]
    assert job["payload_metadata"]["cost_estimate"] == 0.12
    assert job["payload_metadata"]["latency_ms"] == 1530
    assert job["payload_metadata"]["diagnostics"] == {"json_valid": True, "schema_status": "valid"}
    assert job["payload_metadata"]["write_eligibility"] is False
    assert job["payload_metadata"]["promotion_eligibility"] is False


def test_enqueue_rejects_unsafe_payload_metadata(tmp_path: Path) -> None:
    queue = _queue(tmp_path)

    with pytest.raises(ValueError, match="source_artifact_refs must"):
        queue.enqueue(
            job_id="job-raw-payload-ref",
            stage="extract",
            input_refs=("artifact:paper-manifest",),
            input_hash="sha256:input",
            tool_version="tool-v1",
            contract_version="contract-v1",
            payload_metadata={"source_artifact_refs": ["raw source text is not a metadata ref"]},
        )

    with pytest.raises(ValueError, match="write_eligibility must remain false"):
        queue.enqueue(
            job_id="job-write-eligible",
            stage="extract",
            input_refs=("artifact:paper-manifest",),
            input_hash="sha256:input",
            tool_version="tool-v1",
            contract_version="contract-v1",
            payload_metadata={"write_eligibility": True},
        )

    with pytest.raises(ValueError, match="diagnostics key must be a metadata code"):
        queue.enqueue(
            job_id="job-secret-diagnostic",
            stage="extract",
            input_refs=("artifact:paper-manifest",),
            input_hash="sha256:input",
            tool_version="tool-v1",
            contract_version="contract-v1",
            payload_metadata={"diagnostics": {"secret": "sk-secret-token"}},
        )


def test_update_payload_diagnostics_preserves_status_and_disabled_eligibility(
    tmp_path: Path,
) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-diagnostics-update",
        stage="extract",
        input_refs=("artifact:paper-manifest",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
        payload_metadata={
            "schema_version": "schema:m070_v1",
            "metric_bundle_id": "metric_bundle:m070_v1",
            "source_artifact_refs": ["artifact:paper-manifest"],
        },
    )
    queue.unblock_ready_jobs()
    running = queue.claim(worker_id="worker-1", lease_seconds=30)
    assert running is not None

    updated = queue.update_payload_diagnostics(
        "job-diagnostics-update",
        diagnostics={
            "json_valid": True,
            "schema_valid": True,
            "evidence_status": "valid",
            "low_quality_output": False,
        },
        cost_estimate=0.42,
        latency_ms=2400,
        retry_count=2,
        evidence_path_refs=("evidence:path-001",),
    )

    assert updated["status"] == "running"
    assert updated["lease_owner"] == "worker-1"
    assert updated["payload_metadata"]["schema_version"] == "schema:m070_v1"
    assert updated["payload_metadata"]["metric_bundle_id"] == "metric_bundle:m070_v1"
    assert updated["payload_metadata"]["diagnostics"] == {
        "json_valid": True,
        "schema_valid": True,
        "evidence_status": "valid",
        "low_quality_output": False,
    }
    assert updated["payload_metadata"]["cost_estimate"] == 0.42
    assert updated["payload_metadata"]["latency_ms"] == 2400
    assert updated["payload_metadata"]["retry_count"] == 2
    assert updated["payload_metadata"]["evidence_path_refs"] == ["evidence:path-001"]
    assert updated["payload_metadata"]["write_eligibility"] is False
    assert updated["payload_metadata"]["promotion_eligibility"] is False


def test_update_payload_diagnostics_rejects_secret_values_and_raw_refs(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-diagnostics-secret",
        stage="extract",
        input_refs=("artifact:paper-manifest",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )

    with pytest.raises(ValueError, match="diagnostics key must be a metadata code"):
        queue.update_payload_diagnostics(
            "job-diagnostics-secret",
            diagnostics={"api_key": "sk-secret-token"},
        )

    with pytest.raises(ValueError, match="evidence_path_refs must"):
        queue.update_payload_diagnostics(
            "job-diagnostics-secret",
            evidence_path_refs=("raw evidence path text",),
        )

    with pytest.raises(ValueError, match="latency_ms must"):
        queue.update_payload_diagnostics("job-diagnostics-secret", latency_ms=-1)


def test_update_payload_diagnostics_records_event_without_status_change(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-diagnostics-event",
        stage="extract",
        input_refs=("artifact:paper-manifest",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    before = queue.inspect("job-diagnostics-event")
    updated = queue.update_payload_diagnostics(
        "job-diagnostics-event",
        diagnostics={"json_valid": True},
    )
    after = queue.inspect("job-diagnostics-event")

    assert before["job"]["status"] == "pending"
    assert updated["status"] == "pending"
    assert after["job"]["status"] == "pending"
    assert after["events"][-1]["event_type"] == "payload_diagnostics_update"


def test_rejects_secret_shaped_artifact_dependency_refs(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-dep-secret",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )

    with pytest.raises(ValueError, match="depends_on_artifact_ref must be redacted"):
        queue.add_dependency(
            job_id="job-dep-secret", depends_on_artifact_ref="artifact:sk-live-abc1234567890"
        )


def test_artifact_dependencies_without_hash_do_not_unblock_job(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-artifact",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.add_dependency(job_id="job-artifact", depends_on_artifact_ref="artifact:missing")

    assert queue.unblock_ready_jobs() == []
    assert queue.inspect("job-artifact")["job"]["status"] == "blocked"


def test_initialize_sets_sqlite_pragmas_and_schema(tmp_path: Path) -> None:
    queue = _queue(tmp_path)

    journal_mode = queue.connection.execute("PRAGMA journal_mode").fetchone()[0]
    foreign_keys = queue.connection.execute("PRAGMA foreign_keys").fetchone()[0]
    busy_timeout = queue.connection.execute("PRAGMA busy_timeout").fetchone()[0]
    tables = {
        row[0]
        for row in queue.connection.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table'"
        ).fetchall()
    }

    assert journal_mode == "wal"
    assert foreign_keys == 1
    assert busy_timeout > 0
    assert {"jobs", "job_dependencies", "job_events"}.issubset(tables)


def test_enqueue_is_idempotent_and_records_event(tmp_path: Path) -> None:
    queue = _queue(tmp_path)

    first = queue.enqueue(
        job_id="job-1",
        stage="sidecar_candidate",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    second = queue.enqueue(
        job_id="job-1",
        stage="sidecar_candidate",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )

    assert first["job_id"] == second["job_id"] == "job-1"
    assert second["status"] == "pending"
    assert [event["event_type"] for event in queue.events("job-1")] == ["enqueue"]


def test_claim_is_exclusive_and_sets_lease_fields(tmp_path: Path) -> None:
    clock = FixedClock()
    queue = _queue(tmp_path, clock)
    queue.enqueue(
        job_id="job-1",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.unblock_ready_jobs()

    claimed = queue.claim(worker_id="worker-a", lease_seconds=60)
    second_claim = queue.claim(worker_id="worker-b", lease_seconds=60)

    assert claimed is not None
    assert claimed["status"] == "running"
    assert claimed["lease_owner"] == "worker-a"
    assert claimed["heartbeat_at"] == "2026-06-08T00:00:00Z"
    assert claimed["lease_until"] == "2026-06-08T00:01:00Z"
    assert second_claim is None


def test_heartbeat_extends_matching_lease_and_rejects_wrong_owner(tmp_path: Path) -> None:
    clock = FixedClock("2026-06-08T00:00:00Z")
    queue = _queue(tmp_path, clock)
    queue.enqueue(
        job_id="job-1",
        stage="sidecar_candidate",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-a", lease_seconds=60)
    clock.set("2026-06-08T00:00:30Z")

    updated = queue.heartbeat("job-1", worker_id="worker-a", lease_seconds=90)

    assert updated["heartbeat_at"] == "2026-06-08T00:00:30Z"
    assert updated["lease_until"] == "2026-06-08T00:02:00Z"
    with pytest.raises(ValueError, match="lease owner mismatch"):
        queue.heartbeat("job-1", worker_id="worker-b", lease_seconds=90)


def test_complete_persists_safe_outputs_and_clears_lease(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-1",
        stage="sidecar_candidate",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-a", lease_seconds=60)

    completed = queue.complete(
        "job-1", worker_id="worker-a", output_paths=("artifacts/job-1.json",)
    )

    assert completed["status"] == "succeeded"
    assert completed["output_paths"] == ["artifacts/job-1.json"]
    assert completed["lease_owner"] is None
    assert completed["safety_flags"]["graphdb_written"] is False


def test_retryable_failure_respects_retry_after_before_claim(tmp_path: Path) -> None:
    clock = FixedClock("2026-06-08T00:00:00Z")
    queue = _queue(tmp_path, clock)
    queue.enqueue(
        job_id="job-1",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-a", lease_seconds=60)

    failed = queue.fail_retryable(
        "job-1",
        worker_id="worker-a",
        error_code="temporary_tool_error",
        redacted_message="tool unavailable",
        retry_after="2026-06-08T00:10:00Z",
    )

    assert failed["status"] == "failed_retryable"
    assert queue.claim(worker_id="worker-b", lease_seconds=60) is None
    clock.set("2026-06-08T00:10:00Z")
    # pyrefly: ignore [unsupported-operation]
    assert queue.claim(worker_id="worker-b", lease_seconds=60)["job_id"] == "job-1"  # ty:ignore[not-subscriptable]


def test_expired_lease_reclaims_to_ready_until_attempts_exhausted(tmp_path: Path) -> None:
    clock = FixedClock("2026-06-08T00:00:00Z")
    queue = _queue(tmp_path, clock)
    queue.enqueue(
        job_id="job-1",
        stage="sidecar_candidate",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
        max_attempts=2,
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-a", lease_seconds=60)
    clock.set("2026-06-08T00:02:00Z")

    reclaimed = queue.reclaim_expired_leases()

    assert reclaimed[0]["status"] == "ready"
    assert [event["event_type"] for event in queue.events("job-1")][-2:] == [
        "lease_expired",
        "reclaim",
    ]
    queue.claim(worker_id="worker-b", lease_seconds=60)
    clock.set("2026-06-08T00:04:00Z")
    exhausted = queue.reclaim_expired_leases()[0]
    assert exhausted["status"] == "failed_terminal"


def test_mark_stale_detects_input_tool_and_contract_drift(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-1",
        stage="sidecar_candidate",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-a", lease_seconds=60)
    queue.complete("job-1", worker_id="worker-a", output_paths=("artifacts/job-1.json",))

    stale = queue.mark_stale(
        "job-1",
        input_hash="sha256:input-v2",
        tool_version="tool-v2",
        contract_version="contract-v2",
    )

    assert stale["status"] == "stale"
    assert [event["event_type"] for event in queue.events("job-1")][-3:] == [
        "stale_input",
        "stale_tool",
        "stale_contract",
    ]


def test_inspect_returns_job_events_and_no_write_safety_flags(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-1",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )

    inspected = queue.inspect("job-1")

    assert inspected["job"]["safety_flags"] == {
        "graph_import_allowed": False,
        "graphdb_written": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    assert inspected["events"][0]["event_type"] == "enqueue"
    assert "raw_text" not in str(inspected)


def test_dependency_blocks_claim_until_upstream_succeeds(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="upstream",
        stage="sidecar_candidate",
        input_refs=("artifact:a",),
        input_hash="sha256:upstream",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.enqueue(
        job_id="dependent",
        stage="review",
        input_refs=("artifact:dependent",),
        input_hash="sha256:dependent",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.add_dependency("dependent", depends_on_job_id="upstream")

    queue.unblock_ready_jobs()
    first_claim = queue.claim(worker_id="worker-a", lease_seconds=60)
    # pyrefly: ignore [unsupported-operation]
    assert first_claim["job_id"] == "upstream"  # ty:ignore[not-subscriptable]
    assert queue.claim(worker_id="worker-b", lease_seconds=60) is None

    queue.complete("upstream", worker_id="worker-a", output_paths=("artifacts/upstream.json",))
    queue.unblock_ready_jobs()
    dependent = queue.claim(worker_id="worker-b", lease_seconds=60)

    # pyrefly: ignore [unsupported-operation]
    assert dependent["job_id"] == "dependent"  # ty:ignore[not-subscriptable]
    assert [event["event_type"] for event in queue.events("dependent")][-1] == "claim"


def test_retryable_failure_becomes_terminal_when_max_attempts_reached(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-1",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
        max_attempts=1,
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-a", lease_seconds=60)

    failed = queue.fail_retryable(
        "job-1",
        worker_id="worker-a",
        error_code="tool_error",
        redacted_message="tool unavailable",
        retry_after="2026-06-08T00:10:00Z",
    )

    assert failed["status"] == "failed_terminal"
    assert queue.events("job-1")[-1]["event_type"] == "fail_terminal"


def test_failure_and_block_reject_raw_payload_diagnostics(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-1",
        stage="review",
        input_refs=("artifact:a",),
        input_hash="sha256:input-v1",
        tool_version="tool-v1",
        contract_version="contract-v1",
    )
    queue.unblock_ready_jobs()
    queue.claim(worker_id="worker-a", lease_seconds=60)

    with pytest.raises(ValueError, match="diagnostic must be redacted"):
        queue.fail_retryable(
            "job-1",
            worker_id="worker-a",
            error_code="unsafe_payload",
            redacted_message="raw_text: secret payload",
            retry_after="2026-06-08T00:10:00Z",
        )
    with pytest.raises(ValueError, match="diagnostic must be redacted"):
        queue.block("job-1", reason="embedding vector leaked")
