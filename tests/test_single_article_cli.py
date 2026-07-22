"""Single-article CLI + composition pipeline tests (no network)."""

from __future__ import annotations

import json
from pathlib import Path

from typer.testing import CliRunner

from research_graph.cli import app
from research_graph.infrastructure.corpus.ingestion.fetchers import (
    arxiv_html_url,
    arxiv_pdf_url,
    normalize_arxiv_ref,
)
from research_graph.workflows.composition.single_article_pipeline import (
    SingleArticleRunRequest,
    run_single_article_pipeline,
)

ROOT = Path(__file__).resolve().parents[1]
runner = CliRunner()


def test_normalize_arxiv_ref_urls_and_ids() -> None:
    assert normalize_arxiv_ref("2607.13104v1") == "2607.13104v1"
    assert normalize_arxiv_ref("https://arxiv.org/html/2607.13104v1") == "2607.13104v1"
    assert normalize_arxiv_ref("https://arxiv.org/pdf/2607.13104v1.pdf") == "2607.13104v1"
    assert normalize_arxiv_ref("arxiv:2607.13104") == "2607.13104"
    assert arxiv_html_url("2607.13104v1").endswith("/html/2607.13104v1")
    assert arxiv_pdf_url("2607.13104v1").endswith("/pdf/2607.13104v1")


def test_resolve_local_html_and_run_readiness(tmp_path: Path) -> None:
    html = tmp_path / "paper.html"
    html.write_text(
        """<!doctype html><html><body>
        <h1>Single Article CLI Paper</h1>
        <p>Local HTML is enough for readiness composition.</p>
        <h2>Method</h2>
        <p>Deterministic structure without network fetch.</p>
        <h2>Results</h2>
        <p>Fail-closed import flags remain false.</p>
        </body></html>
        """,
        encoding="utf-8",
    )
    work = tmp_path / "work"
    result = run_single_article_pipeline(
        SingleArticleRunRequest(
            source=str(html),
            work_dir=work,
            mode="local",
            also_pdf=False,
            repo_root=ROOT,
        )
    )
    assert result.paper_id == "paper"
    assert result.body_route == "html_native"
    assert result.readiness is not None
    assert result.readiness.package.import_eligible is False
    assert result.readiness.package.graph_writes_allowed is False
    assert result.package_path is not None and result.package_path.is_file()
    payload = json.loads(result.package_path.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert payload["body_route"] == "html_native"
    assert payload["hybrid_claimed_success"] is False
    assert payload["package"] is not None
    assert payload["package"]["verdict"] in {"ready_for_review", "repair", "blocked"}
    assert any(src["origin"].startswith("body_route:") for src in result.local_sources)


def test_cli_article_run_local(tmp_path: Path) -> None:
    html = tmp_path / "cli-paper.html"
    html.write_text(
        """<!doctype html><html><body>
        <h1>CLI Article</h1>
        <p>Body for structure and candidate stages.</p>
        <h2>Discussion</h2>
        <p>Readiness package must stay import-blocked.</p>
        </body></html>
        """,
        encoding="utf-8",
    )
    out = tmp_path / "out"
    completed = runner.invoke(
        app,
        [
            "article",
            "run",
            str(html),
            "--output-dir",
            str(out),
            "--mode",
            "local",
            "--no-also-pdf",
            "--json",
        ],
    )
    assert completed.exit_code == 0, completed.output
    payload = json.loads(completed.output)
    assert payload["import_eligible"] is False
    assert payload["graph_writes_allowed"] is False
    assert (out / "readiness" / "package.json").is_file()


def test_missing_local_file_raises(tmp_path: Path) -> None:
    try:
        run_single_article_pipeline(
            SingleArticleRunRequest(
                source=str(tmp_path / "missing.html"),
                work_dir=tmp_path / "w",
                mode="local",
                also_pdf=False,
                allow_network=False,
            )
        )
        raise AssertionError("expected FileNotFoundError")
    except FileNotFoundError:
        pass


def test_cli_json_includes_body_route(tmp_path: Path) -> None:
    html = tmp_path / "route-paper.html"
    html.write_text(
        """<!doctype html><html><body>
        <h1>Route Paper</h1>
        <p>Body for CLI body_route field.</p>
        <h2>Sec</h2>
        <p>Must expose html_native.</p>
        </body></html>
        """,
        encoding="utf-8",
    )
    out = tmp_path / "out"
    completed = runner.invoke(
        app,
        [
            "article",
            "run",
            str(html),
            "--output-dir",
            str(out),
            "--mode",
            "local",
            "--no-also-pdf",
            "--json",
        ],
    )
    assert completed.exit_code == 0, completed.output
    payload = json.loads(completed.output)
    assert payload["body_route"] == "html_native"
    assert payload["hybrid_claimed_success"] is False
