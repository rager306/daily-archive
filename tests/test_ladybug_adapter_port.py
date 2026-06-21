"""Port substitutability tests for the M104 S01 hexagonal seams (D086).

Pins the contract that any object implementing
:class:`~research_graph.domain.ports.GraphDBPort` is interchangeable at the
call site. ``LadybugAdapter`` (production, delegates to ladybug_client) and
``FakeGraphDB`` (in-memory test double) both satisfy the Port structurally, so
the application layer can swap them — and Phase 3 can add a FalkorDB adapter
implementing the same Port without touching callers.

Also verifies the existing ``FailingConn`` fake pattern (from
``test_ladybug_scientific_kg.py``) flows through the adapter unchanged.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any
from unittest.mock import patch

import pytest

from research_graph.domain.ports import GraphDBPort
from research_graph.graph import ladybug_client
from research_graph.infrastructure.graph import LadybugAdapter


class FakeGraphDB:
    """In-memory test double implementing :class:`GraphDBPort` (D086).

    Records every call so tests assert behaviour without a real graph driver.
    """

    def __init__(self) -> None:
        self.schema_initialized = False
        self.upserts: list[tuple[Any, Any, Any, Any]] = []

    def init_schema(self) -> None:
        self.schema_initialized = True

    def upsert_scientific_kg(self, document, chunks, evidence_paths, patch) -> None:
        self.upserts.append((document, chunks, evidence_paths, patch))


class FailingInitGraphDB:
    """Port-compliant double whose ``init_schema`` raises (fail-closed path)."""

    def __init__(self) -> None:
        self.calls: list[str] = []

    def init_schema(self) -> None:
        self.calls.append("init_schema")
        raise RuntimeError("simulated graph init failure")

    def upsert_scientific_kg(self, document, chunks, evidence_paths, patch) -> None:
        self.calls.append("upsert")


# ── Port satisfaction ────────────────────────────────────────────────────────


class TestPortSatisfaction:
    def test_ladybug_adapter_satisfies_graph_port(self, tmp_path: Path) -> None:
        conn = ladybug_client.init_db(tmp_path / "kg")
        adapter = LadybugAdapter(conn=conn, init_schema_on_construct=False)
        assert isinstance(adapter, GraphDBPort)

    def test_fake_graphdb_satisfies_graph_port(self) -> None:
        assert isinstance(FakeGraphDB(), GraphDBPort)

    def test_failing_init_double_satisfies_graph_port(self) -> None:
        # Even a failing double satisfies the Port structurally — the contract
        # is about the method shape, not the behaviour.
        assert isinstance(FailingInitGraphDB(), GraphDBPort)


# ── Substitutability: both adapters usable through the Port type ─────────────


def _consume(graph: GraphDBPort, payload: tuple[Any, Any, Any, Any]) -> None:
    """Application-layer helper that depends on the Port, not the adapter."""
    graph.init_schema()
    graph.upsert_scientific_kg(*payload)


class TestSubstitutability:
    def test_fake_graphdb_records_calls_through_port(self) -> None:
        fake = FakeGraphDB()
        payload = ("doc", ["chunk"], ["path"], "patch")
        _consume(fake, payload)
        assert fake.schema_initialized is True
        assert fake.upserts == [payload]

    def test_ladybug_adapter_delegates_through_port(self, tmp_path: Path) -> None:
        conn = ladybug_client.init_db(tmp_path / "kg")
        adapter = LadybugAdapter(conn=conn, init_schema_on_construct=False)
        payload = ("doc", ["chunk"], ["path"], "patch")
        with (
            patch.object(ladybug_client, "upsert_scientific_kg") as mock_upsert,
            patch.object(ladybug_client, "init_base_schema"),
            patch.object(ladybug_client, "init_scientific_kg_schema"),
        ):
            _consume(adapter, payload)
            mock_upsert.assert_called_once_with(conn, *payload)

    def test_failing_init_propagates_through_port(self) -> None:
        failing = FailingInitGraphDB()
        payload = ("doc", [], [], "patch")
        with pytest.raises(RuntimeError, match="simulated graph init failure"):
            _consume(failing, payload)
        # upsert never reached because init_schema raised first
        assert failing.calls == ["init_schema"]


# ── Adapter construction edge cases ──────────────────────────────────────────


class TestLadybugAdapterConstruction:
    def test_requires_conn_or_db_path(self) -> None:
        with pytest.raises(ValueError, match="requires conn= or db_path="):
            LadybugAdapter()

    def test_db_path_initializes_connection_and_schema(self, tmp_path: Path) -> None:
        adapter = LadybugAdapter(db_path=tmp_path / "via_path")
        assert adapter.conn is not None
        assert isinstance(adapter, GraphDBPort)

    def test_injected_external_connection_skips_auto_init(self, tmp_path: Path) -> None:
        conn = ladybug_client.init_db(tmp_path / "ext")
        # init_schema_on_construct=False with an external conn must NOT re-init
        adapter = LadybugAdapter(conn=conn, init_schema_on_construct=False)
        with (
            patch.object(ladybug_client, "init_base_schema") as mb,
            patch.object(ladybug_client, "init_scientific_kg_schema") as ms,
        ):
            adapter.init_schema()
            mb.assert_called_once_with(conn)
            ms.assert_called_once_with(conn)
