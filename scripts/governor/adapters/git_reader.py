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

    def added_files_since(self, since_date: str, pathspec: str) -> list[str]:
        """Repo-relative files ADDED since a date, under ``pathspec``.

        Used by the coverage-delta improvement: only files that arrived after the
        last milestone close are in scope, so a long-lived untested file is not
        re-reported on every run. `--format=` suppresses commit headers so the
        output is only file names; a rename shows as an add of the new path.
        """
        output = self._run_git(
            "log", "--diff-filter=A", "--name-only", "--format=",
            f"--since={since_date}", "HEAD", "--", pathspec,
        )
        return [line.strip() for line in output.splitlines() if line.strip()]

    def untracked_files(self, pathspec: str) -> list[str]:
        """Repo-relative untracked or newly staged files under ``pathspec``.

        A file added but not yet committed is exactly the case a coverage delta
        must see, so it is unioned with the committed adds rather than left to the
        next commit. Porcelain v1 is two status chars, a space, then the path.
        """
        output = self._run_git("status", "--porcelain", "--", pathspec)
        paths: list[str] = []
        for line in output.splitlines():
            if len(line) < 4:
                continue
            status, path = line[:2], line[3:].strip()
            # New means untracked (`??`) or staged as added (`A ` / `AM`). A
            # modified tracked file (` M`) is not a new source and must not be
            # reported as one, or every edit would look like an untested arrival.
            if status in ("??", "A ", "AM") and path:
                paths.append(path.strip('"'))
        return paths

    def is_repo(self) -> bool:
        return bool(self._run_git("rev-parse", "--git-dir"))
