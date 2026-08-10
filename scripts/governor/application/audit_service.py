"""Application service: audit — runs reverse ADR rules against code."""

from dataclasses import dataclass, field
from pathlib import Path

from governor.domain.checks import CheckResult, CheckRule, CheckStatus
from governor.ports.code_scanner import CodeScanner
from governor.ports.rule_repository import RuleRepository


@dataclass
class AuditReport:
    """Result of running all ADR audit rules."""

    status: str  # "clear" or "violations"
    results: list[CheckResult] = field(default_factory=list)

    @property
    def violations_count(self) -> int:
        return sum(
            len(r.violations) for r in self.results if r.status == CheckStatus.VIOLATION
        )

    @property
    def has_blocking(self) -> bool:
        return any(r.is_blocking for r in self.results)

    def to_dict(self) -> dict:
        all_violations = []
        for result in self.results:
            if result.status == CheckStatus.VIOLATION:
                for violation in result.violations:
                    all_violations.append(violation.to_dict())
        return {
            "status": self.status,
            "violations": all_violations,
            "violations_count": len(all_violations),
        }


class AuditService:
    """Runs reverse ADR audit rules against the codebase."""

    def __init__(self, scanner: CodeScanner, rule_repo: RuleRepository):
        self.scanner = scanner
        self.rule_repo = rule_repo

    def run_audit(self) -> AuditReport:
        rules_data = self.rule_repo.load_adr_rules()
        results = []
        for rule_data in rules_data:
            rule = CheckRule(
                id=rule_data["id"],
                anchor=rule_data["anchor"],
                severity=rule_data["severity"],
                scan=rule_data["scan"],
                pattern=rule_data["pattern"],
                rationale=rule_data["rationale"],
                filename_filter=rule_data.get("filename_filter"),
                path_prefix_include=rule_data.get("path_prefix_include"),
                path_prefix_exclude=rule_data.get("path_prefix_exclude"),
                path_contains_exclude=rule_data.get("path_contains_exclude"),
                section_filter=rule_data.get("section_filter"),
                suffix_allowlist=rule_data.get("suffix_allowlist"),
            )
            violations = list(self._run_rule(rule))
            status = CheckStatus.VIOLATION if violations else CheckStatus.CLEAR
            results.append(CheckResult(rule=rule, status=status, violations=violations))

        has_violations = any(r.status == CheckStatus.VIOLATION for r in results)
        return AuditReport(
            status="violations" if has_violations else "clear", results=results
        )

    def _run_rule(self, rule: CheckRule):
        from governor.domain.artifacts import ArtifactViolation

        for path, line_no, matched_line in self.scanner.scan_pattern(
            scan_path=rule.scan,
            pattern=rule.pattern,
            filename_filter=rule.filename_filter,
            path_prefix_include=rule.path_prefix_include,
            path_prefix_exclude=rule.path_prefix_exclude,
            path_contains_exclude=rule.path_contains_exclude,
            section_filter=rule.section_filter,
            suffix_allowlist=set(rule.suffix_allowlist) if rule.suffix_allowlist else None,
        ):
            yield ArtifactViolation(
                rule_id=rule.id,
                file=str(path),
                line=line_no,
                anchor=rule.anchor,
                severity=rule.severity,
                rationale=rule.rationale,
                matched_text=matched_line.strip(),
            )
