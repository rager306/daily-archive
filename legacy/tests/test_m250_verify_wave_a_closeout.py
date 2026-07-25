"""M250 S02: operator Wave A closeout script."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_a_closeout.py"

_BODY = """# Graph Neural Networks for Structured Learning

## Abstract
Graph neural networks process graph-structured data using iterative message
passing between neighboring nodes. This survey summarizes encoder designs,
training objectives, and evaluation protocols for citation networks, molecular
graphs, and knowledge graphs used in scientific literature mining.

## Introduction
Recent work demonstrates that relational inductive biases improve sample
efficiency when labels are scarce. We review spectral and spatial operators,
attention mechanisms, and hierarchical pooling strategies that preserve local
topology while scaling to large document corpora.

## Method
We evaluate citation graphs and molecular graphs for node classification and
link prediction. Baselines include logistic regression on bag-of-words, multi-
layer perceptrons on averaged embeddings, and several message-passing variants
with early stopping on validation accuracy.

## Experiments
Datasets cover computer science abstracts, materials science papers, and open
knowledge bases. Metrics report macro-F1, mean reciprocal rank, and calibration
error under distribution shift. Ablations isolate neighborhood size, depth, and
feature normalization choices.

## Results
Across five random seeds the best graph encoder outperforms text-only models on
low-resource splits while remaining competitive on dense features. Error
analysis highlights long-tail categories and missing cross-domain edges.

## Conclusion
Graph-aware preprocess pipelines remain useful for research knowledge graphs
when hybrid body extraction is available and import stays fail-closed.
"""


def _index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps({"schema_version": "article-catalog-index.v1", "articles": articles}),
        encoding="utf-8",
    )


def _bodies(root: Path, n: int) -> None:
    for i in range(1, n + 1):
        p = root / f"p{i}" / "body" / f"p{i}.hybrid.body.md"
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(_BODY, encoding="utf-8")


def test_script_closeout_pass_on_fixture(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    n = 45
    arts = [
        {
            "article_key": f"p{i}",
            "article_ref": f"arxiv/cs-cl/p{i}",
            "source_code": "arxiv",
        }
        for i in range(1, n + 1)
    ]
    _index(idx, arts)
    body_root = tmp_path / "bodies"
    _bodies(body_root, 42)

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-index",
            str(idx),
            "--catalog-root",
            str(tmp_path),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
            "--min-hybrid-found",
            "40",
            "--json",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["wave_b_gate_open"] is False
    assert report["closeout_pass"] is True
    assert report["closeout_signal"] == "wave_a_closed"
    assert report["hybrid_found"] >= 40


def test_script_blocked_low_hybrid(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    arts = [
        {
            "article_key": f"p{i}",
            "article_ref": f"arxiv/cs-cl/p{i}",
            "source_code": "arxiv",
        }
        for i in range(1, 20)
    ]
    _index(idx, arts)
    body_root = tmp_path / "bodies"
    _bodies(body_root, 5)

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-index",
            str(idx),
            "--catalog-root",
            str(tmp_path),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
            "--min-hybrid-found",
            "40",
            "--json",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["closeout_pass"] is False
    assert report["closeout_signal"] == "blocked"
    assert report["import_eligible"] is False


def test_strict_exits_one_when_blocked(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _index(
        idx,
        [
            {
                "article_key": "p1",
                "article_ref": "arxiv/cs-cl/p1",
                "source_code": "arxiv",
            }
        ],
    )
    body_root = tmp_path / "bodies"
    _bodies(body_root, 1)

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-index",
            str(idx),
            "--catalog-root",
            str(tmp_path),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
            "--strict",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 1
    assert "wave-a-closeout" in proc.stdout
    assert "import_eligible: false" in proc.stdout


def test_live_smoke_if_catalog_present() -> None:
    index = ROOT / "data" / "article_catalog" / "index.json"
    if not index.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "wave-a-closeout" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "wave_b_gate_open: false" in proc.stdout
    assert "signal:" in proc.stdout
