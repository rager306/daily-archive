"""Public CLI help contract tests for Hermes/cron agents."""

import subprocess


def run_cli_help(*args: str) -> subprocess.CompletedProcess[str]:
    """Run the public module entrypoint exactly as agents and cron jobs do."""
    return subprocess.run(
        ["uv", "run", "python", "-m", "research_graph", *args],
        check=False,
        capture_output=True,
        text=True,
    )


def combined_output(result: subprocess.CompletedProcess[str]) -> str:
    return f"{result.stdout}\n{result.stderr}".lower()


def assert_help_contract(output: str) -> None:
    """Assert help teaches an agent the durable M001 operating contract."""
    assert "arxiv" in output
    assert "archive" in output
    assert any(term in output for term in ["daily", "day"])
    assert any(term in output for term in ["analy", "score", "research"])

    assert "hermes" in output
    assert "cron" in output

    assert "~/research/ops/sessions/yyyy-mm-dd.json" in output
    assert "~/research/analysis/yyyy-mm-dd/overview.json" in output
    assert "~/research/papers/" in output

    assert "--date" in output
    assert "yyyy-mm-dd" in output
    assert "--json" in output

    assert "exit" in output
    assert "0" in output
    assert "1" in output
    assert "2" in output

    assert any(term in output for term in ["status", "state"])
    for status in ["running", "done", "empty", "failed"]:
        assert status in output

    assert any(term in output for term in ["example", "examples"])
    assert "uv run python -m research_graph --date" in output

    assert any(term in output for term in ["non-goal", "non goal", "out of scope"])
    for non_goal in ["telegram", "graphify", "surprise me", "preference learning", "pdf", "llm"]:
        assert non_goal in output


def test_top_level_help_is_agent_contract() -> None:
    result = run_cli_help("--help")

    assert result.returncode == 0
    assert_help_contract(combined_output(result))


def test_run_help_is_agent_contract() -> None:
    result = run_cli_help("run", "--help")

    assert result.returncode == 0
    output = combined_output(result)
    assert "run" in output
    assert_help_contract(output)
