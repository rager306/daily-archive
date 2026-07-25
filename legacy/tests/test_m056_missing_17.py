from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BFS_DIR = ROOT / "artifacts" / "m056-bfs-graph"
MISSING_MANIFEST = BFS_DIR / "missing-17-manifest.json"
GROBID_DIR = BFS_DIR / "missing-17-grobid"
OPENDATALOADER_DIR = BFS_DIR / "missing-17-opendataloader"
CANDIDATE_EDGES = BFS_DIR / "candidate-edges.json"
CUMULATIVE_CORPUS = BFS_DIR / "cumulative-corpus.json"
REPORT = BFS_DIR / "REPORT.md"
TRAJECTORY_REPORT = ROOT / "artifacts" / "project-trajectory" / "trajectory-report.json"
M044_GUARDRAIL = ROOT / "scripts" / "verify_m044_sidecar_architecture_guardrail.py"
ANCHOR_ID = "2605.18747"
DUPLICATE_ID = "2507.19457"
MISSING_IDS = {
    "2305.04032",
    "2307.12856",
    "2310.03731",
    "2312.13010",
    "2402.16117",
    "2403.00839",
    "2404.14662",
    "2409.10737",
    "2504.06939",
    "2506.10948",
    "2507.19457",
    "2510.02387",
    "2510.04618",
    "2512.15813",
    "2601.16206",
    "2601.16443",
    "2602.09856",
}
SAFETY_DEFAULTS = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "import_eligible": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
}


def _read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def test_17_acquired() -> None:
    manifest = _read_json(MISSING_MANIFEST)
    pdfs = manifest["pdfs"]
    ids = {pdf["arxiv_id"] for pdf in pdfs}

    assert manifest["schema_version"] == "m056-bfs-graph.missing-17.v1"
    assert ids == MISSING_IDS
    assert len(pdfs) == 17
    assert sum(1 for pdf in pdfs if pdf["arxiv_id"] == DUPLICATE_ID) == 1
    for pdf in pdfs:
        path = ROOT / pdf["path"]
        assert path.exists(), pdf["arxiv_id"]
        assert path.stat().st_size > 0


def test_grobid_17_packets() -> None:
    summary = _read_json(GROBID_DIR / "summary.json")
    packet_paths = sorted((GROBID_DIR / "per-pdf").glob("*.json"))

    assert summary["aggregate_counts"] == {"blocked": 0, "low_quality_source": 0, "success": 17}
    assert len(packet_paths) == 17
    assert summary["safety_defaults"] == SAFETY_DEFAULTS

    packet_ids: set[str] = set()
    for packet_path in packet_paths:
        packet = _read_json(packet_path)
        packet_ids.add(packet["arxiv_id"])
        assert packet["status"] == "success"
        assert packet["http_status"] == 200
        assert packet["endpoint"].startswith("http://127.0.0.1:")
        assert packet["parse_error"] is None
        assert packet["ref_count"] > 0
        assert packet["bibl_count"] > 0
        assert packet["safety_defaults"] == SAFETY_DEFAULTS
        assert (ROOT / packet["pdf_path"]).exists()
        assert (ROOT / packet["tei_path"]).exists()

    assert packet_ids == MISSING_IDS


def test_opendataloader_17_packets() -> None:
    summary = _read_json(OPENDATALOADER_DIR / "summary.json")
    statuses = summary["per_pdf_statuses"]

    assert summary["aggregate_counts"] == {
        "low_quality_source": 0,
        "opendataloader_unavailable": 0,
        "success": 17,
    }
    assert statuses == dict.fromkeys(MISSING_IDS, "success")
    assert summary["safety_defaults"] == SAFETY_DEFAULTS

    for arxiv_id in MISSING_IDS:
        layout_packet = OPENDATALOADER_DIR / "layout" / f"{arxiv_id}.json"
        assert layout_packet.exists()
        assert "layout" in _read_json(layout_packet)


def test_cumulative_corpus_165_unique() -> None:
    cumulative = _read_json(CUMULATIVE_CORPUS)
    pdfs = cumulative["pdfs"]
    ids = {pdf["arxiv_id"] for pdf in pdfs}

    assert cumulative["schema_version"] == "m056-bfs-graph.cumulative-corpus.v1"
    assert cumulative["pdf_count"] == 166
    assert cumulative["unique_1hop_pdf_count"] == 165
    assert len(pdfs) == 166
    assert len(ids) == 166
    assert ANCHOR_ID in ids
    assert MISSING_IDS <= ids
    assert cumulative["missing_17_new_count"] == 16
    assert cumulative["duplicate_pdf_count_in_missing_17"] == 1
    assert cumulative["safety_defaults"] == SAFETY_DEFAULTS

    by_id = {pdf["arxiv_id"]: pdf for pdf in pdfs}
    assert by_id[ANCHOR_ID]["source_milestone"] == "anchor"
    assert by_id[DUPLICATE_ID]["source_milestone"] == "pre-existing"
    assert by_id[DUPLICATE_ID]["acquisition_note"] == "duplicate pre-existing corpus PDF"

    for pdf in pdfs:
        assert {
            "arxiv_id",
            "path",
            "size_bytes",
            "sha256",
            "source_milestone",
            "pages_estimate",
        } <= set(pdf)
        path = ROOT / pdf["path"]
        assert path.exists(), pdf["arxiv_id"]
        assert path.stat().st_size == pdf["size_bytes"]
        assert _sha256(path) == pdf["sha256"]
        assert pdf["pages_estimate"] >= 1


def test_candidate_edges_updated() -> None:
    cumulative = _read_json(CUMULATIVE_CORPUS)
    candidate = _read_json(CANDIDATE_EDGES)
    corpus_ids = {pdf["arxiv_id"] for pdf in cumulative["pdfs"]}
    nodes = {node["arxiv_id"]: node for node in candidate["nodes"]}
    edges = candidate["edges"]
    edge_keys = {(edge["paper_a"], edge["paper_b"]) for edge in edges}

    assert candidate["summary"]["corpus_unique_pdf_count"] == 166
    assert candidate["summary"]["edge_count"] == 4454
    assert candidate["summary"]["missing_17_added_edge_count"] == 471
    assert candidate["summary"]["missing_17_new_pdf_count"] == 16
    assert candidate["summary"]["missing_17_duplicate_pdf_count"] == 1
    assert candidate["summary"]["node_count"] == len(candidate["nodes"])
    assert candidate["summary"]["edge_count"] == len(edges)
    assert candidate["safety_defaults"] == SAFETY_DEFAULTS

    for arxiv_id in MISSING_IDS:
        assert nodes[arxiv_id]["in_corpus"] is True
        assert nodes[arxiv_id]["source_milestone"] == "M056-lchpnp/missing-17"
        assert any(edge["paper_a"] == arxiv_id for edge in edges)

    assert len(edge_keys) == len(edges)
    assert ("2305.04032", "2107.03374") in edge_keys
    assert ("2602.09856", "2501.12326") in edge_keys
    for edge in edges:
        assert edge["paper_a_in_corpus"] is (edge["paper_a"] in corpus_ids)
        assert edge["paper_b_in_corpus"] is (edge["paper_b"] in corpus_ids)


def test_5_safety_defaults() -> None:
    candidate = _read_json(CANDIDATE_EDGES)
    cumulative = _read_json(CUMULATIVE_CORPUS)
    grobid = _read_json(GROBID_DIR / "summary.json")
    opendataloader = _read_json(OPENDATALOADER_DIR / "summary.json")
    report = REPORT.read_text(encoding="utf-8")

    for payload in (candidate, cumulative, grobid, opendataloader):
        assert payload["safety_defaults"] == SAFETY_DEFAULTS
    assert candidate["graph_writes_authorized"] is False
    assert cumulative["graph_writes_authorized"] is False
    assert "Graph import is not authorized" in report
    for key in SAFETY_DEFAULTS:
        assert f"`{key}`: `false`" in report


def test_m050_m056_regression() -> None:
    trajectory = _read_json(TRAJECTORY_REPORT)
    assert trajectory["verdict"] == "on_track"
    assert trajectory["graph_write_allowed"] is False
    assert trajectory["promotion_allowed"] is False
    assert trajectory["production_import_attempted"] is False
    assert trajectory["import_eligible"] is False

    result = subprocess.run(
        [sys.executable, str(M044_GUARDRAIL)],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
