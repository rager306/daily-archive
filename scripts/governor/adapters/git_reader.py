"""Adapter: git reader — reads git log and commit metadata."""

import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass
class GitInfo:
    """Git repository metadata for trajectory analysis."""

    total_commits: int = 0
    latest_commit_hash: str = ""
    latest_commit_date: str = ""  # ISO format


class GitReader:
    """Reads git information via subprocess."""

    def __init__(self, project_root: Path):
        self.root = project_root

    def _run_git(self, *args: str) -> str:
        try:
            result = subprocess.run(
                ["git", *args],
                cwd=self.root,
                capture_output=True,
                text=True,
                timeout=5,
            )
            return result.stdout.strip() if result.returncode == 0 else ""
        except (subprocess.TimeoutExpired, FileNotFoundError):
            return ""

    def get_git_info(self) -> GitInfo:
        info = GitInfo()
        info.latest_commit_hash = self._run_git("rev-parse", "HEAD")[:12]
        info.latest_commit_date = self._run_git("log", "-1", "--format=%ai")
        # Total commit count
        count_str = self._run_git("rev-list", "--count", "HEAD")
        try:
            info.total_commits = int(count_str)
        except ValueError:
            pass
        return info

    def commits_since_date(self, since_date: str) -> int:
        """Count commits since an ISO date string."""
        output = self._run_git("rev-list", "--count", f"--since={since_date}", "HEAD")
        try:
            return int(output)
        except ValueError:
            return 0

    def is_repo(self) -> bool:
        return bool(self._run_git("rev-parse", "--git-dir"))
