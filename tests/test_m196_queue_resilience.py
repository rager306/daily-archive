from __future__ import annotations

from pathlib import Path

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


def test_retryable_failure_persists_operator_diagnostics_without_payloads(tmp_path: Path) -> None:
    clock = FixedClock()
    queue = _queue(tmp_path, clock)
    queue.enqueue(
        job_id="job-retry",
        stage="parsing",
        input_refs=("artifact:source-pdf",),
        input_hash="sha256:source-pdf",
        tool_version="parser-v1",
        contract_version="pipeline-contract-v1",
    )
    queue.unblock_ready_jobs()
    claimed = queue.claim(worker_id="worker-1", lease_seconds=30)
    assert claimed is not None

    clock.set("2026-06-08T00:00:10Z")
    failed = queue.fail_retryable(
        "job-retry",
        worker_id="worker-1",
        error_code="parser_timeout",
        redacted_message="parser timed out after bounded sample",
        retry_after="2026-06-08T00:05:00Z",
    )
    inspected = queue.inspect("job-retry")

    assert failed["status"] == "failed_retryable"
    assert failed["attempt_count"] == 1
    assert failed["last_error_code"] == "parser_timeout"
    assert failed["last_error_message"] == "parser timed out after bounded sample"
    assert inspected["events"][-1]["event_type"] == "fail_retryable"
    serialized = str(inspected).lower()
    for term in ("api_key", "secret_value", "raw_prompt", "paper_text_payload"):
        assert term not in serialized


def test_artifact_dependency_resumes_only_on_exact_hash_match(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-artifact-resume",
        stage="graph_candidate",
        input_refs=("artifact:chunk-bundle",),
        input_hash="sha256:chunk-bundle",
        tool_version="candidate-builder-v1",
        contract_version="pipeline-contract-v1",
    )
    queue.add_dependency(
        job_id="job-artifact-resume",
        depends_on_artifact_ref="artifact:candidate-packet",
        expected_hash="sha256:candidate-packet",
    )

    assert queue.unblock_ready_jobs() == []
    assert queue.inspect("job-artifact-resume")["job"]["status"] == "blocked"

    queue.register_artifact(
        artifact_ref="artifact:candidate-packet",
        artifact_hash="sha256:wrong-candidate-packet",
    )
    assert queue.unblock_ready_jobs() == []

    queue.register_artifact(
        artifact_ref="artifact:candidate-packet",
        artifact_hash="sha256:candidate-packet",
    )
    ready = queue.unblock_ready_jobs()
    inspected = queue.inspect("job-artifact-resume")

    assert ready[0]["job_id"] == "job-artifact-resume"
    assert inspected["job"]["status"] == "ready"
    assert inspected["events"][-1]["event_type"] == "unblock"


def test_completed_job_keeps_false_graph_import_safety_flags(tmp_path: Path) -> None:
    queue = _queue(tmp_path)
    queue.enqueue(
        job_id="job-complete",
        stage="projection_rehearsal",
        input_refs=("artifact:candidate-packet",),
        input_hash="sha256:candidate-packet",
        tool_version="networkx-projection-v1",
        contract_version="projection-contract-v1",
    )
    queue.unblock_ready_jobs()
    claimed = queue.claim(worker_id="worker-1", lease_seconds=30)
    assert claimed is not None
    completed = queue.complete(
        "job-complete",
        worker_id="worker-1",
        output_paths=("projection_result.json", "summary.json"),
    )
    inspected = queue.inspect("job-complete")

    assert completed["status"] == "succeeded"
    assert completed["safety_flags"]["graphdb_written"] is False
    assert completed["safety_flags"]["ladybugdb_written"] is False
    assert completed["safety_flags"]["graph_import_allowed"] is False
    assert completed["safety_flags"]["import_eligible"] is False
    assert inspected["events"][-1]["event_type"] == "complete"
