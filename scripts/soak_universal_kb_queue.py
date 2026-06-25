#!/usr/bin/env python3
"""Run a bounded multiprocess UniversalKBQueue soak and emit JSON diagnostics."""

from __future__ import annotations

import argparse
import json
import tempfile
import time
from multiprocessing import Process, Queue
from pathlib import Path
from queue import Empty
from typing import Any

from research_graph.workflows.universal_kb.queue import UniversalKBQueue

SCHEMA_VERSION = "m170-universal-kb-queue-soak.v1"


def _worker(db_path: str, worker_id: str, result_queue, lease_seconds: int) -> None:
    queue = UniversalKBQueue(Path(db_path)).initialize()
    try:
        while True:
            claimed = queue.claim(worker_id=worker_id, lease_seconds=lease_seconds)
            if claimed is None:
                result_queue.put({"type": "done", "worker_id": worker_id})
                return
            job_id = str(claimed["job_id"])
            completed = queue.complete(
                job_id,
                worker_id=worker_id,
                output_paths=(f"artifacts/{job_id}.json",),
            )
            result_queue.put(
                {
                    "type": "complete",
                    "worker_id": worker_id,
                    "job_id": job_id,
                    "status": completed["status"],
                }
            )
    except Exception as exc:  # pragma: no cover - parent process reports this
        result_queue.put({"type": "error", "worker_id": worker_id, "error": repr(exc)})
    finally:
        queue.close()


def _drain_results(result_queue, max_items: int) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for _ in range(max_items):
        try:
            results.append(result_queue.get(timeout=0.1))
        except Empty:
            break
    return results


def _prepare_round(db_path: Path, round_index: int, jobs_per_round: int) -> None:
    queue = UniversalKBQueue(db_path).initialize()
    try:
        for index in range(jobs_per_round):
            job_id = f"round-{round_index:03d}-job-{index:04d}"
            queue.enqueue(
                job_id=job_id,
                stage="review",
                input_refs=(f"artifact:{job_id}",),
                input_hash=f"sha256:{job_id}",
                tool_version="soak-v1",
                contract_version="m170-soak-v1",
            )
        ready = queue.unblock_ready_jobs()
        if len(ready) != jobs_per_round:
            raise RuntimeError(f"expected {jobs_per_round} ready jobs, got {len(ready)}")
    finally:
        queue.close()


def _inspect_round(db_path: Path, round_index: int, jobs_per_round: int) -> dict[str, Any]:
    queue = UniversalKBQueue(db_path).initialize()
    try:
        succeeded = 0
        claim_event_count = 0
        complete_event_count = 0
        bad_jobs: list[dict[str, Any]] = []
        for index in range(jobs_per_round):
            job_id = f"round-{round_index:03d}-job-{index:04d}"
            inspected = queue.inspect(job_id)
            status = inspected["job"]["status"]
            event_types = [event["event_type"] for event in inspected["events"]]
            claim_count = event_types.count("claim")
            complete_count = event_types.count("complete")
            if status == "succeeded":
                succeeded += 1
            if status != "succeeded" or claim_count != 1 or complete_count != 1:
                bad_jobs.append(
                    {
                        "job_id": job_id,
                        "status": status,
                        "claim_count": claim_count,
                        "complete_count": complete_count,
                    }
                )
            claim_event_count += claim_count
            complete_event_count += complete_count
        return {
            "succeeded": succeeded,
            "claim_event_count": claim_event_count,
            "complete_event_count": complete_event_count,
            "bad_jobs": bad_jobs,
        }
    finally:
        queue.close()


def run_round(
    *,
    work_dir: Path,
    round_index: int,
    jobs_per_round: int,
    processes: int,
    lease_seconds: int,
    join_timeout_seconds: int,
) -> dict[str, Any]:
    db_path = work_dir / f"queue-round-{round_index:03d}.sqlite"
    _prepare_round(db_path, round_index, jobs_per_round)

    result_queue = Queue()
    workers = [
        Process(
            target=_worker,
            args=(str(db_path), f"round-{round_index:03d}-worker-{index:02d}", result_queue, lease_seconds),
        )
        for index in range(processes)
    ]
    round_start = time.monotonic()
    for process in workers:
        process.start()
    for process in workers:
        process.join(timeout=join_timeout_seconds)

    stuck_workers = [process.name for process in workers if process.is_alive()]
    for process in workers:
        if process.is_alive():
            process.terminate()
            process.join(timeout=5)

    max_items = jobs_per_round + processes + len(stuck_workers) + 10
    results = _drain_results(result_queue, max_items)
    result_queue.close()
    result_queue.join_thread()

    completed_job_ids = [str(item["job_id"]) for item in results if item.get("type") == "complete"]
    worker_errors = [item for item in results if item.get("type") == "error"]
    done_workers = [str(item["worker_id"]) for item in results if item.get("type") == "done"]
    inspection = _inspect_round(db_path, round_index, jobs_per_round)

    unique_completed = len(set(completed_job_ids))
    return {
        "round": round_index,
        "jobs": jobs_per_round,
        "processes": processes,
        "completed": len(completed_job_ids),
        "unique_completed": unique_completed,
        "worker_done_count": len(done_workers),
        "worker_error_count": len(worker_errors),
        "worker_errors": worker_errors,
        "stuck_worker_count": len(stuck_workers),
        "stuck_workers": stuck_workers,
        "claim_event_count": inspection["claim_event_count"],
        "complete_event_count": inspection["complete_event_count"],
        "succeeded": inspection["succeeded"],
        "bad_jobs": inspection["bad_jobs"],
        "duration_seconds": round(time.monotonic() - round_start, 3),
    }


def run_soak(args: argparse.Namespace) -> dict[str, Any]:
    started = time.monotonic()
    worker_errors: list[dict[str, Any]] = []
    stuck_workers: list[str] = []
    round_summaries: list[dict[str, Any]] = []

    with tempfile.TemporaryDirectory(dir=args.work_dir) as temp_dir:
        work_dir = Path(temp_dir)
        for round_index in range(args.rounds):
            if time.monotonic() - started > args.max_total_seconds:
                break
            round_summary = run_round(
                work_dir=work_dir,
                round_index=round_index,
                jobs_per_round=args.jobs_per_round,
                processes=args.processes,
                lease_seconds=args.lease_seconds,
                join_timeout_seconds=args.join_timeout_seconds,
            )
            round_summaries.append(round_summary)
            worker_errors.extend(round_summary["worker_errors"])
            stuck_workers.extend(round_summary["stuck_workers"])

    total_jobs = args.jobs_per_round * args.rounds
    total_completed = sum(round_summary["completed"] for round_summary in round_summaries)
    unique_completed = sum(round_summary["unique_completed"] for round_summary in round_summaries)
    duration_seconds = round(time.monotonic() - started, 3)
    timeout_exceeded = duration_seconds > args.max_total_seconds or len(round_summaries) != args.rounds
    all_jobs_succeeded = (
        len(round_summaries) == args.rounds
        and all(round_summary["succeeded"] == round_summary["jobs"] for round_summary in round_summaries)
    )
    all_jobs_completed_once = (
        len(round_summaries) == args.rounds
        and all(
            round_summary["completed"] == round_summary["jobs"]
            and round_summary["unique_completed"] == round_summary["jobs"]
            and round_summary["claim_event_count"] == round_summary["jobs"]
            and round_summary["complete_event_count"] == round_summary["jobs"]
            and not round_summary["bad_jobs"]
            for round_summary in round_summaries
        )
    )
    passed = (
        not timeout_exceeded
        and not worker_errors
        and not stuck_workers
        and all_jobs_succeeded
        and all_jobs_completed_once
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "rounds": args.rounds,
        "jobs_per_round": args.jobs_per_round,
        "processes": args.processes,
        "lease_seconds": args.lease_seconds,
        "join_timeout_seconds": args.join_timeout_seconds,
        "max_total_seconds": args.max_total_seconds,
        "total_jobs": total_jobs,
        "total_completed": total_completed,
        "unique_completed": unique_completed,
        "worker_errors": worker_errors,
        "stuck_workers": stuck_workers,
        "round_summaries": round_summaries,
        "all_jobs_succeeded": all_jobs_succeeded,
        "all_jobs_completed_once": all_jobs_completed_once,
        "timeout_exceeded": timeout_exceeded,
        "duration_seconds": duration_seconds,
        "passed": passed,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs-per-round", type=int, default=64)
    parser.add_argument("--processes", type=int, default=8)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--lease-seconds", type=int, default=30)
    parser.add_argument("--join-timeout-seconds", type=int, default=30)
    parser.add_argument("--max-total-seconds", type=int, default=120)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--work-dir", type=Path)
    args = parser.parse_args(argv)
    for name in ("jobs_per_round", "processes", "rounds", "lease_seconds", "join_timeout_seconds"):
        if getattr(args, name) <= 0:
            parser.error(f"--{name.replace('_', '-')} must be positive")
    if args.max_total_seconds <= 0:
        parser.error("--max-total-seconds must be positive")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    summary = run_soak(args)
    text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if args.json_out is not None:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
