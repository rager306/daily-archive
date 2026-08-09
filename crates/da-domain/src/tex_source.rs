//! Candidate-only TeX source analysis (ADR-055).
//!
//! This module is pure: callers provide allowlisted archive member bytes. It
//! selects the root document, resolves a jailed include closure, and parses only
//! bibliography databases declared by that closure. It never writes graph truth.

use crate::ContentDigest;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const TEX_ANALYSIS_SCHEMA_VERSION: &str = "tex-source-analysis.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TexAnalysisStatus {
    NoClaim,
    Incomplete,
    Conflict,
    Reject,
    OkCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum TexAnalysisError {
    #[error("no TeX root document could be selected")]
    RootUnresolved,
    #[error("multiple TeX root documents remain: {0:?}")]
    AmbiguousRoot(Vec<String>),
    #[error("README root is inconsistent: {0}")]
    ManifestInconsistent(String),
    #[error("unsafe include path from {from}: {target}")]
    UnsafeIncludePath { from: String, target: String },
    #[error("include cycle: {0:?}")]
    IncludeCycle(Vec<String>),
    #[error("include target missing from {from}: {target}")]
    IncludeMissing { from: String, target: String },
    #[error("include closure budget exceeded")]
    IncludeBudgetExceeded,
    #[error("TeX lexical masking incomplete in {0}")]
    TexLexIncomplete(String),
    #[error("declared bibliography missing: {0}")]
    BibliographyMissing(String),
    #[error("duplicate BibTeX key: {0}")]
    DuplicateBibKey(String),
    #[error("unsupported BibTeX construct in {0}: {1}")]
    UnsupportedBibtex(String, String),
    #[error("malformed BibTeX database {0}")]
    MalformedBibtex(String),
    #[error("TeX source is not valid UTF-8: {0}")]
    InvalidUtf8(String),
    #[error("invalid TeX analysis report: {0}")]
    InvalidReport(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TexAnalysisPolicy {
    pub max_closure_files: usize,
    pub max_include_depth: usize,
}

impl Default for TexAnalysisPolicy {
    fn default() -> Self {
        Self {
            max_closure_files: 256,
            max_include_depth: 32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TexIncludeEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BibtexCandidate {
    pub candidate_id: ContentDigest,
    pub entry_key: String,
    pub entry_type: String,
    pub source_member: String,
    pub raw_sha256: ContentDigest,
    pub title: Option<String>,
    pub doi: Option<String>,
    pub eprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDenominator {
    pub provider: String,
    pub count: Option<usize>,
    pub payload_sha256: Option<ContentDigest>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TexSourceAnalysisReport {
    pub schema_version: String,
    pub article_id: String,
    pub archive_sha256: ContentDigest,
    pub root_document: Option<String>,
    pub closure_files: Vec<String>,
    pub include_edges: Vec<TexIncludeEdge>,
    pub declared_bibliographies: Vec<String>,
    pub bibliography_candidates: Vec<BibtexCandidate>,
    pub local_reference_denominator: Option<usize>,
    pub providers: Vec<ProviderDenominator>,
    pub diagnostics: Vec<String>,
    pub status: TexAnalysisStatus,
    pub import_eligible: bool,
    pub analysis_sha256: ContentDigest,
}

impl TexSourceAnalysisReport {
    pub fn rejected(
        article_id: impl Into<String>,
        archive_sha256: ContentDigest,
        providers: Vec<ProviderDenominator>,
        diagnostic: impl Into<String>,
    ) -> Self {
        let mut report = Self {
            schema_version: TEX_ANALYSIS_SCHEMA_VERSION.into(),
            article_id: article_id.into(),
            archive_sha256,
            root_document: None,
            closure_files: vec![],
            include_edges: vec![],
            declared_bibliographies: vec![],
            bibliography_candidates: vec![],
            local_reference_denominator: None,
            providers,
            diagnostics: vec![diagnostic.into()],
            status: TexAnalysisStatus::Reject,
            import_eligible: false,
            analysis_sha256: ContentDigest::of_text("pending"),
        };
        report.analysis_sha256 = report_digest(&report);
        report
    }

    pub fn validate(&self) -> Result<(), TexAnalysisError> {
        if self.schema_version != TEX_ANALYSIS_SCHEMA_VERSION {
            return Err(TexAnalysisError::InvalidReport(format!(
                "unsupported schema version: {}",
                self.schema_version
            )));
        }
        if self.article_id.trim().is_empty() {
            return Err(TexAnalysisError::InvalidReport(
                "article_id must not be empty".into(),
            ));
        }
        if self.import_eligible {
            return Err(TexAnalysisError::InvalidReport(
                "candidate TeX evidence cannot be import eligible".into(),
            ));
        }
        match self.status {
            TexAnalysisStatus::Reject | TexAnalysisStatus::NoClaim => {
                if self.root_document.is_some()
                    || !self.closure_files.is_empty()
                    || !self.include_edges.is_empty()
                    || !self.declared_bibliographies.is_empty()
                    || !self.bibliography_candidates.is_empty()
                    || self.local_reference_denominator.is_some()
                {
                    return Err(TexAnalysisError::InvalidReport(
                        "reject or no-claim report contains accepted evidence".into(),
                    ));
                }
            }
            TexAnalysisStatus::OkCandidate
            | TexAnalysisStatus::Incomplete
            | TexAnalysisStatus::Conflict => {
                if self.root_document.is_none()
                    || self.local_reference_denominator != Some(self.bibliography_candidates.len())
                {
                    return Err(TexAnalysisError::InvalidReport(
                        "candidate report denominator or root is inconsistent".into(),
                    ));
                }
            }
        }
        if self.status == TexAnalysisStatus::Conflict
            && !self
                .diagnostics
                .iter()
                .any(|item| item.starts_with("provider_denominator_conflict:"))
        {
            return Err(TexAnalysisError::InvalidReport(
                "conflict report lacks a provider denominator conflict".into(),
            ));
        }
        if self.analysis_sha256 != report_digest(self) {
            return Err(TexAnalysisError::InvalidReport(
                "analysis digest mismatch".into(),
            ));
        }
        Ok(())
    }

    pub fn reconcile_provider_denominators(&mut self) {
        let Some(local) = self.local_reference_denominator else {
            return;
        };
        for provider in &self.providers {
            if let Some(count) = provider.count
                && count != local
            {
                self.diagnostics.push(format!(
                    "provider_denominator_conflict:{}:{count}:{local}",
                    provider.provider
                ));
            }
        }
        self.diagnostics.sort();
        self.diagnostics.dedup();
        if self
            .diagnostics
            .iter()
            .any(|item| item.starts_with("provider_denominator_conflict:"))
            && matches!(
                self.status,
                TexAnalysisStatus::OkCandidate | TexAnalysisStatus::Incomplete
            )
        {
            self.status = TexAnalysisStatus::Conflict;
        }
        self.analysis_sha256 = report_digest(self);
    }
}

pub fn analyze_tex_members(
    article_id: &str,
    archive_sha256: ContentDigest,
    members: &BTreeMap<String, Vec<u8>>,
    providers: Vec<ProviderDenominator>,
    policy: &TexAnalysisPolicy,
) -> Result<TexSourceAnalysisReport, TexAnalysisError> {
    if members.is_empty() {
        return Err(TexAnalysisError::RootUnresolved);
    }
    let root = select_root(article_id, members)?;
    let (closure, edges) = resolve_closure(&root, members, policy)?;
    let mut declared = BTreeSet::new();
    for path in &closure {
        let text = member_text(members, path)?;
        let masked =
            mask_tex(&text).ok_or_else(|| TexAnalysisError::TexLexIncomplete(path.clone()))?;
        for database in command_arguments(&masked, "bibliography", path)? {
            for item in database.split(',') {
                let name = normalize_bib_path(path, item.trim())?;
                declared.insert(name);
            }
        }
        for database in command_arguments(&masked, "addbibresource", path)? {
            declared.insert(normalize_bib_path(path, database.trim())?);
        }
    }
    let mut candidates = Vec::new();
    let mut keys = BTreeSet::new();
    for path in &declared {
        let bytes = members
            .get(path)
            .ok_or_else(|| TexAnalysisError::BibliographyMissing(path.clone()))?;
        for candidate in parse_bibtex(path, bytes, &archive_sha256)? {
            if !keys.insert(candidate.entry_key.clone()) {
                return Err(TexAnalysisError::DuplicateBibKey(candidate.entry_key));
            }
            candidates.push(candidate);
        }
    }
    candidates.sort_by(|a, b| a.entry_key.cmp(&b.entry_key));
    let status = if declared.is_empty() {
        TexAnalysisStatus::Incomplete
    } else {
        TexAnalysisStatus::OkCandidate
    };
    let mut report = TexSourceAnalysisReport {
        schema_version: TEX_ANALYSIS_SCHEMA_VERSION.into(),
        article_id: article_id.into(),
        archive_sha256,
        root_document: Some(root),
        closure_files: closure,
        include_edges: edges,
        declared_bibliographies: declared.into_iter().collect(),
        local_reference_denominator: Some(candidates.len()),
        bibliography_candidates: candidates,
        providers,
        diagnostics: vec![],
        status,
        import_eligible: false,
        analysis_sha256: ContentDigest::of_text("pending"),
    };
    report.reconcile_provider_denominators();
    report.validate()?;
    Ok(report)
}

fn select_root(
    article_id: &str,
    members: &BTreeMap<String, Vec<u8>>,
) -> Result<String, TexAnalysisError> {
    if let Some(readme) = members.get("00README.json") {
        let value: serde_json::Value = serde_json::from_slice(readme)
            .map_err(|_| TexAnalysisError::ManifestInconsistent("invalid 00README.json".into()))?;
        let roots = value["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["usage"] == "toplevel")
            .filter_map(|item| item["filename"].as_str())
            .collect::<Vec<_>>();
        if roots.len() > 1 {
            return Err(TexAnalysisError::AmbiguousRoot(
                roots.into_iter().map(str::to_string).collect(),
            ));
        }
        if let Some(root) = roots.first() {
            if !members.contains_key(*root) {
                return Err(TexAnalysisError::ManifestInconsistent((*root).into()));
            }
            return Ok((*root).into());
        }
    }
    let preferred = [
        format!("{article_id}.tex"),
        "main.tex".into(),
        "paper.tex".into(),
        "ms.tex".into(),
    ];
    for path in preferred {
        if members.contains_key(&path) {
            return Ok(path);
        }
    }
    let roots = members
        .iter()
        .filter(|(path, bytes)| {
            path.ends_with(".tex") && String::from_utf8_lossy(bytes).contains("\\documentclass")
        })
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    match roots.as_slice() {
        [] => Err(TexAnalysisError::RootUnresolved),
        [root] => Ok(root.clone()),
        _ => Err(TexAnalysisError::AmbiguousRoot(roots)),
    }
}

fn resolve_closure(
    root: &str,
    members: &BTreeMap<String, Vec<u8>>,
    policy: &TexAnalysisPolicy,
) -> Result<(Vec<String>, Vec<TexIncludeEdge>), TexAnalysisError> {
    fn visit(
        path: &str,
        depth: usize,
        members: &BTreeMap<String, Vec<u8>>,
        policy: &TexAnalysisPolicy,
        visiting: &mut Vec<String>,
        visited: &mut BTreeSet<String>,
        edges: &mut Vec<TexIncludeEdge>,
    ) -> Result<(), TexAnalysisError> {
        if depth > policy.max_include_depth || visited.len() >= policy.max_closure_files {
            return Err(TexAnalysisError::IncludeBudgetExceeded);
        }
        if let Some(index) = visiting.iter().position(|item| item == path) {
            let mut cycle = visiting[index..].to_vec();
            cycle.push(path.into());
            return Err(TexAnalysisError::IncludeCycle(cycle));
        }
        if visited.contains(path) {
            return Ok(());
        }
        visiting.push(path.into());
        let text = member_text(members, path)?;
        let masked =
            mask_tex(&text).ok_or_else(|| TexAnalysisError::TexLexIncomplete(path.into()))?;
        let mut targets = command_arguments(&masked, "input", path)?;
        targets.extend(command_arguments(&masked, "include", path)?);
        for target in targets {
            let resolved = normalize_tex_path(path, target.trim())?;
            if !members.contains_key(&resolved) {
                return Err(TexAnalysisError::IncludeMissing {
                    from: path.into(),
                    target: resolved,
                });
            }
            edges.push(TexIncludeEdge {
                from: path.into(),
                to: resolved.clone(),
            });
            visit(
                &resolved,
                depth + 1,
                members,
                policy,
                visiting,
                visited,
                edges,
            )?;
        }
        visiting.pop();
        visited.insert(path.into());
        Ok(())
    }
    let mut visited = BTreeSet::new();
    let mut edges = Vec::new();
    visit(
        root,
        0,
        members,
        policy,
        &mut Vec::new(),
        &mut visited,
        &mut edges,
    )?;
    edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
    edges.dedup();
    Ok((visited.into_iter().collect(), edges))
}

fn member_text(
    members: &BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<String, TexAnalysisError> {
    members
        .get(path)
        .map(|bytes| {
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| TexAnalysisError::InvalidUtf8(path.into()))
        })
        .transpose()?
        .ok_or_else(|| TexAnalysisError::IncludeMissing {
            from: path.into(),
            target: path.into(),
        })
}

fn mask_tex(text: &str) -> Option<String> {
    let mut result = String::with_capacity(text.len());
    let mut environment: Option<&str> = None;
    for line in text.lines() {
        if let Some(name) = environment {
            if line.contains(&format!("\\end{{{name}}}")) {
                environment = None;
            }
            result.push('\n');
            continue;
        }
        if let Some(name) = ["verbatim", "lstlisting", "minted", "alltt", "comment"]
            .into_iter()
            .find(|name| line.contains(&format!("\\begin{{{name}}}")))
        {
            environment = Some(name);
            result.push('\n');
            continue;
        }
        let mut escaped = false;
        for character in line.chars() {
            if character == '%' && !escaped {
                break;
            }
            result.push(character);
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
        }
        result.push('\n');
    }
    environment.is_none().then_some(result)
}

fn command_arguments(
    text: &str,
    command: &str,
    path: &str,
) -> Result<Vec<String>, TexAnalysisError> {
    let needle = format!("\\{command}");
    let mut values = Vec::new();
    let mut offset = 0;
    while let Some(index) = text[offset..].find(&needle) {
        let start = offset + index + needle.len();
        let next = text[start..].chars().next();
        if next.is_some_and(|value| value.is_alphanumeric() || value == '@') {
            offset = start;
            continue;
        }
        let mut rest = text[start..].trim_start();
        if rest.starts_with('[') {
            let end = rest
                .find(']')
                .ok_or_else(|| TexAnalysisError::TexLexIncomplete(path.into()))?;
            rest = rest[end + 1..].trim_start();
        }
        let body = rest
            .strip_prefix('{')
            .ok_or_else(|| TexAnalysisError::TexLexIncomplete(path.into()))?;
        let mut depth = 1_i32;
        let mut end = None;
        for (index, character) in body.char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end.ok_or_else(|| TexAnalysisError::TexLexIncomplete(path.into()))?;
        values.push(body[..end].trim().into());
        offset = start + (text[start..].len() - body[end + 1..].len());
    }
    Ok(values)
}

fn normalize_tex_path(from: &str, target: &str) -> Result<String, TexAnalysisError> {
    normalize_relative(from, target, ".tex")
}

fn normalize_bib_path(from: &str, target: &str) -> Result<String, TexAnalysisError> {
    normalize_relative(from, target, ".bib")
}

fn normalize_relative(
    from: &str,
    target: &str,
    extension: &str,
) -> Result<String, TexAnalysisError> {
    if target.is_empty() || target.starts_with(['/', '\\']) || target.contains('\0') {
        return Err(TexAnalysisError::UnsafeIncludePath {
            from: from.into(),
            target: target.into(),
        });
    }
    let mut parts: Vec<&str> = from
        .rsplit_once('/')
        .map(|(parent, _)| parent.split('/').collect())
        .unwrap_or_default();
    let normalized_target = target.replace('\\', "/");
    for part in normalized_target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(TexAnalysisError::UnsafeIncludePath {
                        from: from.into(),
                        target: target.into(),
                    });
                }
            }
            value if value.contains(':') => {
                return Err(TexAnalysisError::UnsafeIncludePath {
                    from: from.into(),
                    target: target.into(),
                });
            }
            value => parts.push(value),
        }
    }
    let mut result = parts.join("/");
    if !result.ends_with(extension) {
        result.push_str(extension);
    }
    Ok(result)
}

fn parse_bibtex(
    path: &str,
    bytes: &[u8],
    archive: &ContentDigest,
) -> Result<Vec<BibtexCandidate>, TexAnalysisError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| TexAnalysisError::InvalidUtf8(path.into()))?;
    let mut entries = Vec::new();
    let raw = text.as_bytes();
    let mut cursor = 0;
    while let Some(relative) = text[cursor..].find('@') {
        let start = cursor + relative;
        let Some(open_rel) = text[start..].find('{') else {
            return Err(TexAnalysisError::MalformedBibtex(path.into()));
        };
        let open = start + open_rel;
        let entry_type = text[start + 1..open].trim().to_ascii_lowercase();
        let end = find_bibtex_entry_end(raw, open)
            .ok_or_else(|| TexAnalysisError::MalformedBibtex(path.into()))?;
        if entry_type == "comment" || entry_type == "preamble" {
            cursor = end + 1;
            continue;
        }
        if entry_type == "string" {
            return Err(TexAnalysisError::UnsupportedBibtex(
                path.into(),
                "macro_or_concat".into(),
            ));
        }
        let body = &text[open + 1..end];
        let Some((key, fields)) = body.split_once(',') else {
            return Err(TexAnalysisError::MalformedBibtex(path.into()));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(TexAnalysisError::MalformedBibtex(path.into()));
        }
        if fields.to_ascii_lowercase().contains("crossref") {
            return Err(TexAnalysisError::UnsupportedBibtex(
                path.into(),
                format!("crossref:{key}"),
            ));
        }
        let entry_raw = &text.as_bytes()[start..=end];
        if has_top_level_concat(entry_raw) {
            return Err(TexAnalysisError::UnsupportedBibtex(
                path.into(),
                format!("macro_or_concat:{key}"),
            ));
        }
        let raw_sha256 = ContentDigest::of_bytes(entry_raw);
        let candidate_id = ContentDigest::of_text(&format!(
            "{}\0{path}\0{key}\0{}",
            archive.as_str(),
            raw_sha256.as_str()
        ));
        entries.push(BibtexCandidate {
            candidate_id,
            entry_key: key.into(),
            entry_type,
            source_member: path.into(),
            raw_sha256,
            title: bib_field(fields, "title"),
            doi: bib_field(fields, "doi"),
            eprint: bib_field(fields, "eprint"),
        });
        cursor = end + 1;
    }
    if entries.is_empty() {
        return Err(TexAnalysisError::MalformedBibtex(path.into()));
    }
    Ok(entries)
}

fn find_bibtex_entry_end(raw: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0_i32;
    let mut quoted = false;
    let mut escaped = false;
    for (relative, &byte) in raw.get(open..)?.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            continue;
        }
        match byte {
            b'"' => quoted = !quoted,
            b'{' if !quoted => depth += 1,
            b'}' if !quoted => {
                depth -= 1;
                if depth == 0 {
                    return (!quoted).then_some(open + relative);
                }
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

fn has_top_level_concat(entry: &[u8]) -> bool {
    let mut depth = 0_i32;
    let mut quoted = false;
    let mut escaped = false;
    for &byte in entry {
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            continue;
        }
        match byte {
            b'"' => quoted = !quoted,
            b'{' if !quoted => depth += 1,
            b'}' if !quoted => depth -= 1,
            b'#' if !quoted && depth == 1 => return true,
            _ => {}
        }
    }
    false
}

fn bib_field(fields: &str, name: &str) -> Option<String> {
    let lower = fields.to_ascii_lowercase();
    let needle = name.to_string();
    let mut offset = 0;
    while let Some(index) = lower[offset..].find(&needle) {
        let start = offset + index;
        let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        let after = &fields[start + needle.len()..];
        if before_ok && after.trim_start().starts_with('=') {
            let value = after.trim_start().strip_prefix('=')?.trim_start();
            if let Some(body) = value.strip_prefix('{') {
                let mut depth = 1_i32;
                for (index, byte) in body.as_bytes().iter().enumerate() {
                    match byte {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(body[..index].trim().into());
                            }
                        }
                        _ => {}
                    }
                }
            } else if let Some(body) = value.strip_prefix('"')
                && let Some(end) = body.find('"')
            {
                return Some(body[..end].trim().into());
            }
        }
        offset = start + needle.len();
    }
    None
}

fn report_digest(report: &TexSourceAnalysisReport) -> ContentDigest {
    let mut clone = report.clone();
    clone.analysis_sha256 = ContentDigest::of_text("normalized");
    ContentDigest::of_bytes(
        &serde_json::to_vec(&clone).expect("report serialization is infallible"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members(main: &str) -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("main.tex".into(), main.as_bytes().to_vec()),
            ("body.tex".into(), b"\\section{Body}".to_vec()),
            (
                "refs.bib".into(),
                b"@article{k, title={Title}, doi={10/x}}".to_vec(),
            ),
        ])
    }

    #[test]
    fn declared_closure_and_bibliography_are_candidate_only() {
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &members(
                "\\documentclass{article}\n\\input{body}\n% \\input{fake}\n\\bibliography{refs}",
            ),
            vec![],
            &TexAnalysisPolicy::default(),
        )
        .unwrap();
        assert_eq!(report.root_document.as_deref(), Some("main.tex"));
        assert_eq!(report.closure_files, vec!["body.tex", "main.tex"]);
        assert_eq!(report.declared_bibliographies, vec!["refs.bib"]);
        assert_eq!(report.bibliography_candidates.len(), 1);
        assert_eq!(report.status, TexAnalysisStatus::OkCandidate);
        assert!(!report.import_eligible);
    }

    #[test]
    fn ambiguous_roots_and_unsafe_or_cyclic_includes_fail_closed() {
        let roots = BTreeMap::from([
            ("a.tex".into(), b"\\documentclass{x}".to_vec()),
            ("b.tex".into(), b"\\documentclass{x}".to_vec()),
        ]);
        assert!(matches!(
            analyze_tex_members(
                "x",
                ContentDigest::of_text("a"),
                &roots,
                vec![],
                &Default::default()
            ),
            Err(TexAnalysisError::AmbiguousRoot(_))
        ));
        assert!(matches!(
            analyze_tex_members(
                "x",
                ContentDigest::of_text("a"),
                &members("\\documentclass{x}\\input{../x}"),
                vec![],
                &Default::default()
            ),
            Err(TexAnalysisError::UnsafeIncludePath { .. })
        ));
        let cyclic = BTreeMap::from([
            ("main.tex".into(), b"\\documentclass{x}\\input{a}".to_vec()),
            ("a.tex".into(), b"\\input{main}".to_vec()),
        ]);
        assert!(matches!(
            analyze_tex_members(
                "x",
                ContentDigest::of_text("a"),
                &cyclic,
                vec![],
                &Default::default()
            ),
            Err(TexAnalysisError::IncludeCycle(_))
        ));
    }

    #[test]
    fn provider_mismatch_is_conflict_not_truth() {
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &members("\\documentclass{article}\\bibliography{refs}"),
            vec![ProviderDenominator {
                provider: "openalex".into(),
                count: Some(0),
                payload_sha256: Some(ContentDigest::of_text("payload")),
                status: "fetched".into(),
            }],
            &Default::default(),
        )
        .unwrap();
        assert_eq!(report.local_reference_denominator, Some(1));
        assert_eq!(report.status, TexAnalysisStatus::Conflict);
        assert!(report.diagnostics[0].contains("openalex"));
    }

    #[test]
    fn duplicate_keys_and_macros_fail_closed() {
        let mut duplicate = members("\\documentclass{x}\\bibliography{refs,more}");
        duplicate.insert("more.bib".into(), b"@article{k,title={Other}}".to_vec());
        assert!(matches!(
            analyze_tex_members(
                "x",
                ContentDigest::of_text("a"),
                &duplicate,
                vec![],
                &Default::default()
            ),
            Err(TexAnalysisError::DuplicateBibKey(_))
        ));
        let mut macros = members("\\documentclass{x}\\bibliography{refs}");
        macros.insert(
            "refs.bib".into(),
            b"@string{x={y}}\n@article{k,title=x # {z}}".to_vec(),
        );
        assert!(matches!(
            analyze_tex_members(
                "x",
                ContentDigest::of_text("a"),
                &macros,
                vec![],
                &Default::default()
            ),
            Err(TexAnalysisError::UnsupportedBibtex(_, _))
        ));
    }

    #[test]
    fn bibtex_url_fragment_is_not_misclassified_as_concat() {
        let mut fixture = members("\\documentclass{x}\\bibliography{refs}");
        fixture.insert(
            "refs.bib".into(),
            br"@article{k,title={T},url={https://example.test/a#section}}".to_vec(),
        );
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &fixture,
            vec![],
            &Default::default(),
        )
        .unwrap();
        assert_eq!(report.bibliography_candidates.len(), 1);
    }

    #[test]
    fn bibtex_non_utf8_is_rejected_without_lossy_identity() {
        let mut fixture = members("\\documentclass{x}\\bibliography{refs}");
        fixture.insert("refs.bib".into(), b"@article{k,title={T\xff}}".to_vec());
        assert!(
            analyze_tex_members(
                "paper",
                ContentDigest::of_text("archive"),
                &fixture,
                vec![],
                &Default::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn bibtex_digest_is_over_exact_entry_bytes() {
        let fixture = members("\\documentclass{x}\\bibliography{refs}");
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &fixture,
            vec![],
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            report.bibliography_candidates[0].raw_sha256,
            ContentDigest::of_bytes(b"@article{k, title={Title}, doi={10/x}}")
        );
    }

    #[test]
    fn incomplete_include_is_rejected_and_duplicate_edges_are_canonical() {
        assert!(
            analyze_tex_members(
                "paper",
                ContentDigest::of_text("archive"),
                &members("\\documentclass{x}\\input{body"),
                vec![],
                &Default::default(),
            )
            .is_err()
        );
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &members("\\documentclass{x}\\input{body}\\input{body}"),
            vec![],
            &Default::default(),
        )
        .unwrap();
        assert_eq!(report.include_edges.len(), 1);
    }

    #[test]
    fn quoted_closing_brace_does_not_split_or_inject_entries() {
        let mut fixture = members("\\documentclass{x}\\bibliography{refs}");
        let bib = br#"@article{good,title="} @article{evil,title={x}}",year={2024}}"#;
        fixture.insert("refs.bib".into(), bib.to_vec());
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &fixture,
            vec![],
            &Default::default(),
        )
        .unwrap();
        assert_eq!(report.bibliography_candidates.len(), 1);
        assert_eq!(report.bibliography_candidates[0].entry_key, "good");
        assert_eq!(
            report.bibliography_candidates[0].raw_sha256,
            ContentDigest::of_bytes(bib)
        );
    }

    #[test]
    fn comment_and_preamble_bodies_cannot_inject_candidates() {
        let mut fixture = members("\\documentclass{x}\\bibliography{refs}");
        fixture.insert(
            "refs.bib".into(),
            br"@comment{note @article{injected,title={X}} more}
@preamble{"
                .to_vec(),
        );
        assert!(
            analyze_tex_members(
                "paper",
                ContentDigest::of_text("archive"),
                &fixture,
                vec![],
                &Default::default(),
            )
            .is_err()
        );

        fixture.insert(
            "refs.bib".into(),
            br"@comment{note @article{injected,title={X}} more}
@preamble{{prefix}}
@article{real,title={Y}}"
                .to_vec(),
        );
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &fixture,
            vec![],
            &Default::default(),
        )
        .unwrap();
        assert_eq!(report.bibliography_candidates.len(), 1);
        assert_eq!(report.bibliography_candidates[0].entry_key, "real");
    }

    #[test]
    fn report_validation_rejects_schema_or_authority_drift() {
        let mut report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &members("\\documentclass{x}\\bibliography{refs}"),
            vec![],
            &Default::default(),
        )
        .unwrap();
        assert!(report.validate().is_ok());
        report.import_eligible = !report.import_eligible;
        assert!(report.validate().is_err());
        report.import_eligible = false;
        report.schema_version = "future".into();
        assert!(report.validate().is_err());
        report.schema_version = TEX_ANALYSIS_SCHEMA_VERSION.into();
        report.local_reference_denominator = Some(999);
        report.analysis_sha256 = report_digest(&report);
        assert!(report.validate().is_err());
    }

    #[test]
    fn report_serde_roundtrip() {
        let report = analyze_tex_members(
            "paper",
            ContentDigest::of_text("archive"),
            &members("\\documentclass{x}\\bibliography{refs}"),
            vec![],
            &Default::default(),
        )
        .unwrap();
        crate::roundtrip_json(&report);
    }
}
