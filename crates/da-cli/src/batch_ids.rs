//! Pure helpers for `da batch-ingest` id resolution (M322).
//!
//! Closes process gap: research scripts and gepa_hub_research use `--ids-file`,
//! but CLI only accepted comma-separated `--ids`.

use std::path::Path;

/// Errors from resolving batch paper ids (no I/O details leak secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchIdsError {
    /// Neither `--ids` nor `--ids-file` provided a non-empty list.
    Empty,
    /// `--ids-file` path could not be read.
    IdsFileRead { path: String, message: String },
}

impl std::fmt::Display for BatchIdsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(
                f,
                "no paper ids: pass --ids a,b,c and/or --ids-file PATH (one id per line)"
            ),
            Self::IdsFileRead { path, message } => {
                write!(f, "failed to read --ids-file {path}: {message}")
            }
        }
    }
}

impl std::error::Error for BatchIdsError {}

/// Parse comma-separated `--ids` string into trimmed non-empty tokens.
pub fn parse_comma_ids(ids: &str) -> Vec<String> {
    ids.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parse ids-file text: one id per line; `#` comments and blanks skipped.
pub fn parse_ids_file_text(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Merge CLI sources: `--ids` then `--ids-file` (file appends after comma list).
/// Dedupes preserving first-seen order.
pub fn merge_id_lists(from_ids: Vec<String>, from_file: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for id in from_ids.into_iter().chain(from_file) {
        if seen.insert(id.clone()) {
            out.push(id);
        }
    }
    out
}

/// Resolve paper ids from optional comma list and optional file path.
pub fn resolve_batch_ids(
    ids: Option<&str>,
    ids_file: Option<&Path>,
) -> Result<Vec<String>, BatchIdsError> {
    let mut from_ids = Vec::new();
    if let Some(s) = ids {
        from_ids = parse_comma_ids(s);
    }
    let mut from_file = Vec::new();
    if let Some(path) = ids_file {
        let text = std::fs::read_to_string(path).map_err(|e| BatchIdsError::IdsFileRead {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        from_file = parse_ids_file_text(&text);
    }
    let merged = merge_id_lists(from_ids, from_file);
    if merged.is_empty() {
        return Err(BatchIdsError::Empty);
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_comma_ids_trims_and_skips_empty() {
        assert_eq!(
            parse_comma_ids("  a, b,,c , "),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert!(parse_comma_ids("").is_empty());
        assert!(parse_comma_ids(" , , ").is_empty());
    }

    #[test]
    fn parse_ids_file_skips_comments_and_blanks() {
        let text = "# hub\n2507.19457\n\n# skip\n2212.14024\n";
        assert_eq!(
            parse_ids_file_text(text),
            vec!["2507.19457".to_string(), "2212.14024".to_string()]
        );
    }

    #[test]
    fn merge_dedupes_preserving_order() {
        let a = vec!["a".into(), "b".into()];
        let b = vec!["b".into(), "c".into()];
        assert_eq!(merge_id_lists(a, b), vec!["a", "b", "c"]);
    }

    #[test]
    fn resolve_empty_errors() {
        assert_eq!(resolve_batch_ids(None, None), Err(BatchIdsError::Empty));
        assert_eq!(
            resolve_batch_ids(Some(" , "), None),
            Err(BatchIdsError::Empty)
        );
    }

    #[test]
    fn resolve_ids_only() {
        let v = resolve_batch_ids(Some("x,y"), None).unwrap();
        assert_eq!(v, vec!["x", "y"]);
    }

    #[test]
    fn resolve_ids_file_only() {
        let dir = tempfile_dir();
        let path = dir.join("ids.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# comment").unwrap();
        writeln!(f, "p1").unwrap();
        writeln!(f, "p2").unwrap();
        let v = resolve_batch_ids(None, Some(&path)).unwrap();
        assert_eq!(v, vec!["p1", "p2"]);
    }

    #[test]
    fn resolve_combines_ids_and_file() {
        let dir = tempfile_dir();
        let path = dir.join("ids.txt");
        std::fs::write(&path, "b\nc\n").unwrap();
        let v = resolve_batch_ids(Some("a,b"), Some(&path)).unwrap();
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    #[test]
    fn resolve_missing_file_errors() {
        let err =
            resolve_batch_ids(None, Some(Path::new("/no/such/ids-file-m322.txt"))).unwrap_err();
        match err {
            BatchIdsError::IdsFileRead { path, .. } => {
                assert!(path.contains("ids-file-m322"));
            }
            other => panic!("expected IdsFileRead, got {other:?}"),
        }
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "da-cli-batch-ids-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
