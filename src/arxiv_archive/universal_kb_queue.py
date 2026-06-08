"""Local SQLite durable queue for M035 Universal KB prototypes.

This module implements a single-node, local-first queue state machine. It is
not a distributed production queue and it never authorizes GraphDB writes,
LadybugDB writes, production import, or candidate promotion.
"""

from __future__ import annotations

import json
import re
import sqlite3
from collections.abc import Callable, Iterable
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

from arxiv_archive.universal_kb_contracts import FORBIDDEN_DIAGNOSTIC_KEYS, SafetyFlags

Clock = Callable[[], str]

SECRET_SHAPED_PATTERN = re.compile(
    r"(?i)(sk-[a-z0-9][a-z0-9._-]{8,}|bearer\s+[a-z0-9._-]{12,}|x-api-key\s*[:=]\s*[^\s]+)"
)
METADATA_REF_PATTERN = re.compile(r"^[a-z][a-z0-9_.-]*:[A-Za-z0-9][A-Za-z0-9_.:/@-]*$")
METADATA_CODE_PATTERN = re.compile(r"^[a-z][a-z0-9_.:-]*$")

STATUSES = frozenset(
    {
        "pending",
        "ready",
        "running",
        "succeeded",
        "failed_retryable",
        "failed_terminal",
        "blocked",
        "stale",
        "needs_review",
        "skipped",
    }
)


def _utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _parse_timestamp(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def _add_seconds(value: str, seconds: int) -> str:
    return (_parse_timestamp(value) + timedelta(seconds=seconds)).isoformat().replace("+00:00", "Z")


def _json_list(values: Iterable[str] | None) -> str:
    return json.dumps(list(values or ()), sort_keys=True)


def _loads_list(value: str | None) -> list[str]:
    if not value:
        return []
    loaded = json.loads(value)
    if not isinstance(loaded, list):
        raise ValueError("queue JSON list field was not a list")
    return [str(item) for item in loaded]


class UniversalKBQueue:
    """Small SQLite-backed queue for local M035 evidence prototypes."""

    def __init__(
        self,
        db_path: str | Path,
        *,
        clock: Clock | None = None,
        busy_timeout_ms: int = 5000,
    ) -> None:
        self.db_path = Path(db_path)
        self.clock = clock or _utc_now
        self.busy_timeout_ms = busy_timeout_ms
        self.connection = sqlite3.connect(self.db_path)
        self.connection.row_factory = sqlite3.Row

    def initialize(self) -> UniversalKBQueue:
        """Create queue tables and configure SQLite for local durable use."""
        self.connection.execute(f"PRAGMA busy_timeout = {int(self.busy_timeout_ms)}")
        self.connection.execute("PRAGMA foreign_keys = ON")
        self.connection.execute("PRAGMA journal_mode = WAL")
        with self.connection:
            self.connection.execute(
                """
                CREATE TABLE IF NOT EXISTS jobs (
                    job_id TEXT PRIMARY KEY,
                    stage TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN (
                        'pending', 'ready', 'running', 'succeeded',
                        'failed_retryable', 'failed_terminal', 'blocked',
                        'stale', 'needs_review', 'skipped'
                    )),
                    priority INTEGER NOT NULL DEFAULT 0,
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    max_attempts INTEGER NOT NULL DEFAULT 3,
                    retry_after TEXT,
                    lease_owner TEXT,
                    lease_until TEXT,
                    heartbeat_at TEXT,
                    input_refs TEXT NOT NULL,
                    input_hash TEXT NOT NULL,
                    tool_version TEXT NOT NULL,
                    contract_version TEXT NOT NULL,
                    output_paths TEXT NOT NULL,
                    last_error_code TEXT,
                    last_error_message TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )
                """
            )
            self.connection.execute(
                """
                CREATE TABLE IF NOT EXISTS job_dependencies (
                    job_id TEXT NOT NULL REFERENCES jobs(job_id) ON DELETE CASCADE,
                    depends_on_job_id TEXT REFERENCES jobs(job_id) ON DELETE CASCADE,
                    depends_on_artifact_ref TEXT,
                    expected_hash TEXT,
                    required_status TEXT NOT NULL DEFAULT 'succeeded',
                    UNIQUE(job_id, depends_on_job_id, depends_on_artifact_ref)
                )
                """
            )
            self.connection.execute(
                """
                CREATE TABLE IF NOT EXISTS job_events (
                    event_id TEXT PRIMARY KEY,
                    job_id TEXT NOT NULL REFERENCES jobs(job_id) ON DELETE CASCADE,
                    event_type TEXT NOT NULL,
                    old_status TEXT,
                    new_status TEXT,
                    reason TEXT,
                    worker_id TEXT,
                    error_code TEXT,
                    created_at TEXT NOT NULL
                )
                """
            )
            self.connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_jobs_claimable "
                "ON jobs(status, retry_after, priority, created_at)"
            )
            self.connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_jobs_lease_recovery ON jobs(status, lease_until)"
            )
            self.connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_jobs_stale_keys "
                "ON jobs(input_hash, tool_version, contract_version)"
            )
            self.connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_job_events_order ON job_events(job_id, created_at)"
            )
        return self

    def enqueue(
        self,
        *,
        job_id: str,
        stage: str,
        input_refs: Iterable[str],
        input_hash: str,
        tool_version: str,
        contract_version: str,
        output_paths: Iterable[str] | None = None,
        priority: int = 0,
        max_attempts: int = 3,
    ) -> dict[str, Any]:
        self._require_non_empty(job_id, "job_id")
        self._require_non_empty(stage, "stage")
        self._require_non_empty(input_hash, "input_hash")
        self._require_non_empty(tool_version, "tool_version")
        self._require_non_empty(contract_version, "contract_version")
        if max_attempts < 1:
            raise ValueError("max_attempts must be >= 1")
        input_refs_tuple = tuple(str(ref) for ref in input_refs)
        for input_ref in input_refs_tuple:
            self._require_metadata_ref(input_ref, "input_ref")

        now = self.clock()
        with self.connection:
            existing = self._fetch_job(job_id)
            if existing is not None:
                return self._row_to_job(existing)
            self.connection.execute(
                """
                INSERT INTO jobs (
                    job_id, stage, status, priority, attempt_count, max_attempts,
                    retry_after, lease_owner, lease_until, heartbeat_at, input_refs,
                    input_hash, tool_version, contract_version, output_paths,
                    last_error_code, last_error_message, created_at, updated_at
                ) VALUES (?, ?, 'pending', ?, 0, ?, NULL, NULL, NULL, NULL, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
                """,
                (
                    job_id,
                    stage,
                    int(priority),
                    int(max_attempts),
                    _json_list(input_refs_tuple),
                    input_hash,
                    tool_version,
                    contract_version,
                    _json_list(output_paths),
                    now,
                    now,
                ),
            )
            self._insert_event(job_id, "enqueue", None, "pending", "job enqueued", None, None, now)
            row = self._fetch_job(job_id)
        return self._row_to_job(row)

    def unblock_ready_jobs(self) -> list[dict[str, Any]]:
        """Move pending/retryable jobs whose gates are open to ready."""
        now = self.clock()
        changed: list[dict[str, Any]] = []
        with self.connection:
            rows = self.connection.execute(
                """
                SELECT * FROM jobs
                WHERE status IN ('pending', 'failed_retryable', 'blocked')
                  AND (retry_after IS NULL OR retry_after <= ?)
                ORDER BY priority DESC, created_at ASC
                """,
                (now,),
            ).fetchall()
            for row in rows:
                if not self._dependencies_satisfied(row["job_id"]):
                    continue
                self.connection.execute(
                    "UPDATE jobs SET status = 'ready', updated_at = ? WHERE job_id = ?",
                    (now, row["job_id"]),
                )
                self._insert_event(
                    row["job_id"], "unblock", row["status"], "ready", "gates open", None, None, now
                )
                changed.append(self._row_to_job(self._fetch_job(row["job_id"])))
        return changed

    def add_dependency(
        self,
        job_id: str,
        *,
        depends_on_job_id: str | None = None,
        depends_on_artifact_ref: str | None = None,
        expected_hash: str | None = None,
        required_status: str = "succeeded",
    ) -> None:
        """Record a dependency and block the dependent job until it is satisfied."""
        row = self._require_job(job_id)
        if depends_on_job_id is None and depends_on_artifact_ref is None:
            raise ValueError("dependency must reference a job or artifact")
        if depends_on_job_id is not None:
            self._require_job(depends_on_job_id)
        if depends_on_artifact_ref is not None:
            self._require_metadata_ref(depends_on_artifact_ref, "depends_on_artifact_ref")
        self._require_non_empty(required_status, "required_status")
        now = self.clock()
        with self.connection:
            self.connection.execute(
                """
                INSERT OR IGNORE INTO job_dependencies (
                    job_id, depends_on_job_id, depends_on_artifact_ref, expected_hash, required_status
                ) VALUES (?, ?, ?, ?, ?)
                """,
                (job_id, depends_on_job_id, depends_on_artifact_ref, expected_hash, required_status),
            )
            if row["status"] in {"pending", "ready"} and not self._dependencies_satisfied(job_id):
                self.connection.execute(
                    "UPDATE jobs SET status = 'blocked', updated_at = ? WHERE job_id = ?",
                    (now, job_id),
                )
                self._insert_event(
                    job_id,
                    "block",
                    row["status"],
                    "blocked",
                    "waiting for dependency",
                    None,
                    None,
                    now,
                )

    def claim(self, *, worker_id: str, lease_seconds: int) -> dict[str, Any] | None:
        self._require_non_empty(worker_id, "worker_id")
        if lease_seconds <= 0:
            raise ValueError("lease_seconds must be > 0")
        now = self.clock()
        lease_until = _add_seconds(now, lease_seconds)
        with self.connection:
            row = self.connection.execute(
                """
                SELECT * FROM jobs
                WHERE status = 'ready'
                   OR (status = 'failed_retryable' AND (retry_after IS NULL OR retry_after <= ?))
                ORDER BY priority DESC, created_at ASC
                LIMIT 1
                """,
                (now,),
            ).fetchone()
            if row is None:
                return None
            self.connection.execute(
                """
                UPDATE jobs
                SET status = 'running', lease_owner = ?, lease_until = ?, heartbeat_at = ?,
                    attempt_count = attempt_count + 1, updated_at = ?
                WHERE job_id = ? AND status IN ('ready', 'failed_retryable')
                """,
                (worker_id, lease_until, now, now, row["job_id"]),
            )
            self._insert_event(row["job_id"], "claim", row["status"], "running", "job claimed", worker_id, None, now)
            claimed = self._fetch_job(row["job_id"])
        return self._row_to_job(claimed)

    def heartbeat(self, job_id: str, *, worker_id: str, lease_seconds: int) -> dict[str, Any]:
        job = self._require_running_owner(job_id, worker_id)
        now = self.clock()
        lease_until = _add_seconds(now, lease_seconds)
        with self.connection:
            self.connection.execute(
                "UPDATE jobs SET lease_until = ?, heartbeat_at = ?, updated_at = ? WHERE job_id = ?",
                (lease_until, now, now, job_id),
            )
            self._insert_event(job_id, "heartbeat", job["status"], job["status"], "lease heartbeat", worker_id, None, now)
            row = self._fetch_job(job_id)
        return self._row_to_job(row)

    def complete(self, job_id: str, *, worker_id: str, output_paths: Iterable[str]) -> dict[str, Any]:
        job = self._require_running_owner(job_id, worker_id)
        now = self.clock()
        with self.connection:
            self.connection.execute(
                """
                UPDATE jobs
                SET status = 'succeeded', output_paths = ?, lease_owner = NULL,
                    lease_until = NULL, heartbeat_at = NULL, updated_at = ?
                WHERE job_id = ?
                """,
                (_json_list(output_paths), now, job_id),
            )
            self._insert_event(job_id, "complete", job["status"], "succeeded", "job completed", worker_id, None, now)
            row = self._fetch_job(job_id)
        return self._row_to_job(row)

    def fail_retryable(
        self,
        job_id: str,
        *,
        worker_id: str,
        error_code: str,
        redacted_message: str,
        retry_after: str,
    ) -> dict[str, Any]:
        job = self._require_running_owner(job_id, worker_id)
        self._require_metadata_code(error_code, "error_code")
        self._require_safe_diagnostic(redacted_message)
        now = self.clock()
        status = "failed_terminal" if int(job["attempt_count"]) >= int(job["max_attempts"]) else "failed_retryable"
        event_type = "fail_terminal" if status == "failed_terminal" else "fail_retryable"
        with self.connection:
            self.connection.execute(
                """
                UPDATE jobs
                SET status = ?, retry_after = ?, lease_owner = NULL, lease_until = NULL,
                    heartbeat_at = NULL, last_error_code = ?, last_error_message = ?, updated_at = ?
                WHERE job_id = ?
                """,
                (status, retry_after, error_code, redacted_message, now, job_id),
            )
            self._insert_event(job_id, event_type, job["status"], status, redacted_message, worker_id, error_code, now)
            row = self._fetch_job(job_id)
        return self._row_to_job(row)

    def fail_terminal(
        self,
        job_id: str,
        *,
        worker_id: str,
        error_code: str,
        redacted_message: str,
    ) -> dict[str, Any]:
        job = self._require_running_owner(job_id, worker_id)
        self._require_metadata_code(error_code, "error_code")
        self._require_safe_diagnostic(redacted_message)
        now = self.clock()
        with self.connection:
            self.connection.execute(
                """
                UPDATE jobs
                SET status = 'failed_terminal', lease_owner = NULL, lease_until = NULL,
                    heartbeat_at = NULL, last_error_code = ?, last_error_message = ?, updated_at = ?
                WHERE job_id = ?
                """,
                (error_code, redacted_message, now, job_id),
            )
            self._insert_event(
                job_id, "fail_terminal", job["status"], "failed_terminal", redacted_message, worker_id, error_code, now
            )
            row = self._fetch_job(job_id)
        return self._row_to_job(row)

    def block(self, job_id: str, *, reason: str) -> dict[str, Any]:
        row = self._require_job(job_id)
        self._require_safe_diagnostic(reason)
        now = self.clock()
        with self.connection:
            self.connection.execute(
                "UPDATE jobs SET status = 'blocked', updated_at = ? WHERE job_id = ?",
                (now, job_id),
            )
            self._insert_event(job_id, "block", row["status"], "blocked", reason, None, None, now)
            updated = self._fetch_job(job_id)
        return self._row_to_job(updated)

    def reclaim_expired_leases(self) -> list[dict[str, Any]]:
        now = self.clock()
        reclaimed: list[dict[str, Any]] = []
        with self.connection:
            rows = self.connection.execute(
                "SELECT * FROM jobs WHERE status = 'running' AND lease_until < ? ORDER BY lease_until ASC",
                (now,),
            ).fetchall()
            for row in rows:
                self._insert_event(
                    row["job_id"],
                    "lease_expired",
                    row["status"],
                    row["status"],
                    "lease expired before heartbeat",
                    row["lease_owner"],
                    "lease_expired",
                    now,
                )
                next_status = (
                    "failed_terminal"
                    if int(row["attempt_count"]) >= int(row["max_attempts"])
                    else "ready"
                )
                self.connection.execute(
                    """
                    UPDATE jobs
                    SET status = ?, lease_owner = NULL, lease_until = NULL, heartbeat_at = NULL,
                        last_error_code = ?, last_error_message = ?, updated_at = ?
                    WHERE job_id = ?
                    """,
                    (
                        next_status,
                        "lease_expired" if next_status == "failed_terminal" else row["last_error_code"],
                        "lease expired" if next_status == "failed_terminal" else row["last_error_message"],
                        now,
                        row["job_id"],
                    ),
                )
                self._insert_event(
                    row["job_id"],
                    "reclaim",
                    row["status"],
                    next_status,
                    "expired lease reclaimed",
                    row["lease_owner"],
                    "lease_expired",
                    now,
                )
                reclaimed.append(self._row_to_job(self._fetch_job(row["job_id"])))
        return reclaimed

    def mark_stale(
        self,
        job_id: str,
        *,
        input_hash: str,
        tool_version: str,
        contract_version: str,
    ) -> dict[str, Any]:
        row = self._require_job(job_id)
        now = self.clock()
        stale_events: list[tuple[str, str]] = []
        if row["input_hash"] != input_hash:
            stale_events.append(("stale_input", "input hash changed"))
        if row["tool_version"] != tool_version:
            stale_events.append(("stale_tool", "tool version changed"))
        if row["contract_version"] != contract_version:
            stale_events.append(("stale_contract", "contract version changed"))
        if not stale_events:
            return self._row_to_job(row)

        with self.connection:
            self.connection.execute(
                """
                UPDATE jobs
                SET status = 'stale', input_hash = ?, tool_version = ?, contract_version = ?, updated_at = ?
                WHERE job_id = ?
                """,
                (input_hash, tool_version, contract_version, now, job_id),
            )
            for event_type, reason in stale_events:
                self._insert_event(job_id, event_type, row["status"], "stale", reason, None, event_type, now)
            updated = self._fetch_job(job_id)
        return self._row_to_job(updated)

    def inspect(self, job_id: str) -> dict[str, Any]:
        return {"job": self._row_to_job(self._require_job(job_id)), "events": self.events(job_id)}

    def events(self, job_id: str) -> list[dict[str, Any]]:
        rows = self.connection.execute(
            "SELECT * FROM job_events WHERE job_id = ? ORDER BY created_at ASC, event_id ASC",
            (job_id,),
        ).fetchall()
        return [dict(row) for row in rows]

    def close(self) -> None:
        self.connection.close()

    def _dependencies_satisfied(self, job_id: str) -> bool:
        dependencies = self.connection.execute(
            "SELECT * FROM job_dependencies WHERE job_id = ?", (job_id,)
        ).fetchall()
        for dependency in dependencies:
            upstream_job_id = dependency["depends_on_job_id"]
            if upstream_job_id is None:
                # Artifact references require a future explicit artifact registry/hash
                # verifier. Without one, treating the dependency as satisfied would
                # silently bypass lineage checks.
                return False
            upstream = self._fetch_job(upstream_job_id)
            if upstream is None or upstream["status"] != dependency["required_status"]:
                return False
        return True

    @staticmethod
    def _require_safe_diagnostic(value: str) -> None:
        lowered = value.lower()
        if any(forbidden in lowered for forbidden in FORBIDDEN_DIAGNOSTIC_KEYS) or SECRET_SHAPED_PATTERN.search(value):
            raise ValueError("diagnostic must be redacted and metadata-only")

    @staticmethod
    def _require_metadata_ref(value: str, field_name: str) -> None:
        lowered = value.lower()
        if any(forbidden in lowered for forbidden in FORBIDDEN_DIAGNOSTIC_KEYS) or SECRET_SHAPED_PATTERN.search(value):
            raise ValueError(f"{field_name} must be redacted and metadata-only")
        if not METADATA_REF_PATTERN.fullmatch(value):
            raise ValueError(f"{field_name} must be a metadata reference")

    @staticmethod
    def _require_metadata_code(value: str, field_name: str) -> None:
        lowered = value.lower()
        if any(forbidden in lowered for forbidden in FORBIDDEN_DIAGNOSTIC_KEYS) or SECRET_SHAPED_PATTERN.search(value):
            raise ValueError(f"{field_name} must be a metadata code")
        if not METADATA_CODE_PATTERN.fullmatch(value):
            raise ValueError(f"{field_name} must be a metadata code")

    def _require_running_owner(self, job_id: str, worker_id: str) -> sqlite3.Row:
        row = self._require_job(job_id)
        if row["status"] != "running":
            raise ValueError(f"job {job_id!r} is not running")
        if row["lease_owner"] != worker_id:
            raise ValueError("lease owner mismatch")
        return row

    def _require_job(self, job_id: str) -> sqlite3.Row:
        row = self._fetch_job(job_id)
        if row is None:
            raise KeyError(job_id)
        return row

    def _fetch_job(self, job_id: str) -> sqlite3.Row | None:
        return self.connection.execute("SELECT * FROM jobs WHERE job_id = ?", (job_id,)).fetchone()

    def _insert_event(
        self,
        job_id: str,
        event_type: str,
        old_status: str | None,
        new_status: str,
        reason: str | None,
        worker_id: str | None,
        error_code: str | None,
        created_at: str,
    ) -> None:
        event_count = self.connection.execute(
            "SELECT COUNT(*) FROM job_events WHERE job_id = ?", (job_id,)
        ).fetchone()[0]
        event_id = f"{job_id}:{event_count + 1:04d}:{event_type}"
        self.connection.execute(
            """
            INSERT INTO job_events (
                event_id, job_id, event_type, old_status, new_status, reason,
                worker_id, error_code, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (event_id, job_id, event_type, old_status, new_status, reason, worker_id, error_code, created_at),
        )

    @staticmethod
    def _require_non_empty(value: str, field_name: str) -> None:
        if not value.strip():
            raise ValueError(f"{field_name} must be non-empty")

    @staticmethod
    def _row_to_job(row: sqlite3.Row | None) -> dict[str, Any]:
        if row is None:
            raise KeyError("missing job row")
        data = dict(row)
        data["input_refs"] = _loads_list(data["input_refs"])
        data["output_paths"] = _loads_list(data["output_paths"])
        data["safety_flags"] = SafetyFlags().to_dict()
        return data
