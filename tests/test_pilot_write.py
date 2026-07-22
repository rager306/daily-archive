"""M205: controlled FalkorDB write pilot — disposable path tests."""

from __future__ import annotations

import ast
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from research_graph.application.graph.pilot_write_authorization import (
    issue_pilot_write_authorization,
)
from research_graph.application.graph.pilot_write_use_case import (
    HumanBatchApproval,
    execute_authorized_pilot_write,
    execute_with_injected_failure,
    export_restore_activation_verdict,
    replay_authorized_pilot_write,
    require_fresh_human_batch_approval,
    run_controlled_pilot_batch,
    verify_read_back,
)
from research_graph.domain.ports import GraphDBPort
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.infrastructure.graph.pilot_write import (
    DisposablePilotGraphStore,
    FalkorPilotGraphDBAdapter,
    UnauthorizedPilotWriteError,
)
from tests.test_ladybug_scientific_kg import build_fixture_payload

ROOT = Path(__file__).resolve().parents[1]
PILOT_WRITE_DIR = ROOT / "src/research_graph/infrastructure/graph/pilot_write"
NO_WRITE_MODULES = [
    ROOT / "src/research_graph/infrastructure/graph/projection_backends.py",
    ROOT / "src/research_graph/infrastructure/staging/import_boundary.py",
    ROOT / "src/research_graph/application/graph/promotion_boundary.py",
    ROOT / "src/research_graph/application/graph/falkor_capability.py",
]


def _auth(
    *,
    candidate_id: str = "candidate-pilot-1",
    paper_scope: str = "m205_controlled_falkor_write_pilot",
    expired: bool = False,
    token: str = "human-approve-test-token",
    prereqs: tuple[str, ...] = (
        "graph_writes_allowed_explicit_true_in_future_milestone",
        "falkordb_write_driver_available",
        "GraphDBPort_adapter_path_m205_only",
    ),
    required: tuple[str, ...] = (
        "graph_writes_allowed_explicit_true_in_future_milestone",
        "falkordb_write_driver_available",
    ),
):
    now = datetime(2026, 7, 22, 12, 0, 0, tzinfo=UTC)
    expiry = now - timedelta(hours=1) if expired else now + timedelta(hours=24)
    return issue_pilot_write_authorization(
        auth_id="auth-1",
        candidate_id=candidate_id,
        packet_hash="abc123packet",
        operation_plan_fingerprint="planfinger01",
        environment_prerequisites=prereqs,
        rollback_plan=("abort_before_commit_if_validation_fails", "reset_graph_writes_allowed_false"),
        expiry_utc=expiry.strftime("%Y-%m-%dT%H:%M:%SZ"),
        human_approval_token=token,
        scope=paper_scope,
        required_prerequisites=required,
        now=now,
    )


def _payload():
    return build_fixture_payload()


# ── S01 driver ──────────────────────────────────────────────────────────────


def test_driver_health_and_cleanup() -> None:
    store = DisposablePilotGraphStore(store_id="pilot-test-1")
    health = store.health()
    assert health["healthy"] is True
    assert health["sdk_imported"] is False
    assert health["driver"] == "disposable_in_memory_falkor_compatible"
    cleaned = store.cleanup()
    assert cleaned["cleaned"] is True
    assert store.health()["healthy"] is False


def test_driver_transaction_rollback() -> None:
    from research_graph.infrastructure.graph.pilot_write.driver import PilotNode

    store = DisposablePilotGraphStore()
    store.begin()
    store.upsert_node(PilotNode(node_id="n1", labels=("X",), props={}))
    store.rollback()
    assert store.get_node("n1") is None


def test_driver_connect_probe_returns_status() -> None:
    store = DisposablePilotGraphStore()
    result = store.connect_probe("127.0.0.1", 1, timeout_s=0.05)
    assert result["status"].startswith("unreachable") or result["status"] == "reachable"


def test_s01_isolation_no_write_modules_do_not_import_pilot_write() -> None:
    for path in NO_WRITE_MODULES:
        text = path.read_text(encoding="utf-8")
        assert "pilot_write" not in text, f"{path} must not import pilot_write"
        assert "FalkorPilotGraphDBAdapter" not in text


# ── S02 authorization ───────────────────────────────────────────────────────


def test_s02_authorization_authorized_and_flags_closed() -> None:
    auth = _auth()
    assert auth.authorized is True
    assert auth.status == "authorized"
    auth.assert_production_flags_closed()
    assert auth.production_safety_flags == SafetyFlags()
    assert auth.production_activation is False


def test_s02_authorization_expired_denied() -> None:
    auth = _auth(expired=True)
    assert auth.authorized is False
    assert auth.status == "expired"


def test_s02_authorization_missing_prereq_denied() -> None:
    auth = _auth(prereqs=(), required=("falkordb_write_driver_available",))
    assert auth.authorized is False
    assert auth.status == "denied"


# ── S03 adapter ─────────────────────────────────────────────────────────────


def test_s03_adapter_requires_auth_and_writes() -> None:
    store = DisposablePilotGraphStore()
    adapter = FalkorPilotGraphDBAdapter(store)
    assert isinstance(adapter, GraphDBPort)
    doc, chunks, eps, patch = _payload()
    with pytest.raises(UnauthorizedPilotWriteError):
        adapter.upsert_scientific_kg(doc, chunks, eps, patch)
    auth = _auth()
    adapter.set_authorization(auth)
    receipt = execute_authorized_pilot_write(
        adapter, auth, document=doc, chunks=chunks, evidence_paths=eps, patch=patch
    )
    assert receipt.status == "success"
    assert receipt.production_activation is False
    assert store.get_node(f"paper:{doc.paper_id}") is not None


def test_s03_denied_auth_yields_failed_receipt() -> None:
    store = DisposablePilotGraphStore()
    adapter = FalkorPilotGraphDBAdapter(store)
    auth = _auth(token="")
    adapter.set_authorization(auth)
    doc, chunks, eps, patch = _payload()
    receipt = execute_authorized_pilot_write(
        adapter, auth, document=doc, chunks=chunks, evidence_paths=eps, patch=patch
    )
    assert receipt.status == "failed"
    assert receipt.classification == "authorization_denied"


# ── S04 read-back ───────────────────────────────────────────────────────────


def test_s04_read_back_evidence_and_packet_hash() -> None:
    store = DisposablePilotGraphStore()
    auth = _auth()
    adapter = FalkorPilotGraphDBAdapter(store, authorization=auth)
    doc, chunks, eps, patch = _payload()
    execute_authorized_pilot_write(
        adapter, auth, document=doc, chunks=chunks, evidence_paths=eps, patch=patch
    )
    expected_eps = [
        str(getattr(ep, "evidence_path_id", "") or getattr(ep, "id", "")) for ep in eps
    ]
    expected_eps = [e for e in expected_eps if e]
    result = verify_read_back(
        adapter,
        paper_id=str(doc.paper_id),
        expected_packet_hash=auth.packet_hash,
        expected_evidence_ids=expected_eps,
    )
    assert result["ok"] is True
    assert result["import_eligible"] is False
    assert result["packet_hash_match"] is True


# ── S05 idempotent replay ───────────────────────────────────────────────────


def test_s05_idempotent_replay_no_duplicates() -> None:
    store = DisposablePilotGraphStore()
    auth = _auth()
    adapter = FalkorPilotGraphDBAdapter(store, authorization=auth)
    doc, chunks, eps, patch = _payload()
    first = execute_authorized_pilot_write(
        adapter, auth, document=doc, chunks=chunks, evidence_paths=eps, patch=patch
    )
    assert first.status == "success"
    nodes_after_first = len(store.list_nodes())
    second = replay_authorized_pilot_write(
        adapter,
        auth,
        document=doc,
        chunks=chunks,
        evidence_paths=eps,
        patch=patch,
        prior_receipt=first,
    )
    assert second.status == "replay_noop"
    assert second.classification == "idempotent_replay"
    assert len(store.list_nodes()) == nodes_after_first


# ── S06 rollback ────────────────────────────────────────────────────────────


def test_s06_injected_mid_write_failure_no_partial() -> None:
    store = DisposablePilotGraphStore()
    auth = _auth()
    adapter = FalkorPilotGraphDBAdapter(store, authorization=auth)
    doc, chunks, eps, patch = _payload()
    receipt = execute_with_injected_failure(
        adapter,
        auth,
        document=doc,
        chunks=chunks,
        evidence_paths=eps,
        patch=patch,
        fail_after_begin=lambda: None,
    )
    assert receipt.status == "rolled_back"
    assert store.get_node(f"paper:{doc.paper_id}") is None


# ── S07 human gate ──────────────────────────────────────────────────────────


def test_s07_fresh_human_batch_approval_required() -> None:
    with pytest.raises(PermissionError):
        require_fresh_human_batch_approval(
            HumanBatchApproval(
                approval_token="",
                paper_ids=("2605.12345",),
                environment="disposable-pilot",
                rollback_plan=("abort",),
                approved=False,
            ),
            expected_environment="disposable-pilot",
        )
    ok = require_fresh_human_batch_approval(
        HumanBatchApproval(
            approval_token="fresh-token-1",
            paper_ids=("2605.12345",),
            environment="disposable-pilot",
            rollback_plan=("abort",),
            approved=True,
        ),
        expected_environment="disposable-pilot",
    )
    assert ok.approved is True


# ── S08 five-paper batch ────────────────────────────────────────────────────


def test_s08_controlled_batch_max_five() -> None:
    store = DisposablePilotGraphStore()
    doc, chunks, eps, patch = _payload()
    paper_id = str(doc.paper_id)

    def auth_factory(_pid: str):
        a = _auth(candidate_id=f"cand-{_pid}")
        return a

    adapter = FalkorPilotGraphDBAdapter(store)
    # set auth per write via factory — adapter needs auth each time
    def factory(pid: str):
        a = auth_factory(pid)
        adapter.set_authorization(a)
        return a

    approval = HumanBatchApproval(
        approval_token="batch-token",
        paper_ids=(paper_id,),
        environment="disposable-pilot",
        rollback_plan=("abort_before_commit_if_validation_fails",),
        approved=True,
        max_papers=5,
    )
    report = run_controlled_pilot_batch(
        adapter,
        factory,
        [(doc, chunks, eps, patch)],
        approval,
        expected_environment="disposable-pilot",
    )
    assert report.paper_count == 1
    assert report.verdict == "proceed"
    assert report.production_activation is False
    assert report.receipts[0].status == "success"


def test_s08_batch_rejects_more_than_max() -> None:
    with pytest.raises(ValueError):
        HumanBatchApproval(
            approval_token="t",
            paper_ids=tuple(f"p{i}" for i in range(6)),
            environment="disposable-pilot",
            rollback_plan=("abort",),
            approved=True,
            max_papers=5,
        )


# ── S09 export restore ──────────────────────────────────────────────────────


def test_s09_export_restore_and_no_production_activation() -> None:
    store = DisposablePilotGraphStore()
    auth = _auth()
    adapter = FalkorPilotGraphDBAdapter(store, authorization=auth)
    doc, chunks, eps, patch = _payload()
    execute_authorized_pilot_write(
        adapter, auth, document=doc, chunks=chunks, evidence_paths=eps, patch=patch
    )
    snap = store.export_snapshot()
    fresh = DisposablePilotGraphStore(store_id="fresh-restore")
    verdict = export_restore_activation_verdict(
        export_snapshot=snap,
        restore_snapshot_fn=fresh.restore_snapshot,
        read_export_fn=fresh.export_snapshot,
        batch_verdict="proceed",
    )
    assert verdict.restore_ok is True
    assert verdict.verdict == "proceed"
    assert verdict.production_activation is False
    assert verdict.export_hash
    assert fresh.get_node(f"paper:{doc.paper_id}") is not None


# ── SafetyFlags unchanged ───────────────────────────────────────────────────


def test_production_safety_flags_remain_default_false() -> None:
    flags = SafetyFlags()
    flags.assert_no_write()
    auth = _auth()
    assert auth.production_safety_flags.to_dict() == flags.to_dict()


def test_application_use_case_does_not_import_infrastructure() -> None:
    path = ROOT / "src/research_graph/application/graph/pilot_write_use_case.py"
    tree = ast.parse(path.read_text(encoding="utf-8"))
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom) and node.module:
            assert not node.module.startswith("research_graph.infrastructure")
        if isinstance(node, ast.Import):
            for alias in node.names:
                assert not alias.name.startswith("research_graph.infrastructure")
