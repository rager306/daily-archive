//! DATA-governed DOI prefix lookup.
//!
//! The configured path comes from [`crate::paths::doi_prefix_map`]. A bundled
//! copy is used only when that configured file is physically absent; malformed
//! or incomplete present data is an honest lookup miss.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;

const BUNDLED_DOI_PREFIX_MAP: &str = include_str!("../../../data/ontology/doi_prefix_map.yaml");

#[derive(Debug, Deserialize)]
struct DoiPrefixMap {
    schema_version: u32,
    doi_prefixes: BTreeMap<String, String>,
}

fn parse_map(text: &str) -> Option<BTreeMap<String, String>> {
    let map = serde_yaml::from_str::<DoiPrefixMap>(text).ok()?;
    (map.schema_version == 1).then_some(map.doi_prefixes)
}

/// Look up the DOI prefix for a source code.
///
/// The source code is used only as a map key. Unknown sources, malformed YAML,
/// unsupported schema versions, and maps without the requested key return
/// `None` rather than constructing a guessed DOI.
pub fn doi_prefix_for(source_code: &str) -> Option<String> {
    let path = crate::paths::doi_prefix_map();
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => BUNDLED_DOI_PREFIX_MAP.to_owned(),
        Err(_) => return None,
    };
    parse_map(&text)?.get(source_code).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn configured_data_path_and_failure_modes_are_deterministic() {
        let _guard = env_lock().lock().unwrap();
        let dir = tempdir().unwrap();
        let path = dir.path().join("doi_prefix_map.yaml");
        unsafe { std::env::set_var("DA_PATH_DOI_PREFIX_MAP", &path) };

        fs::write(
            &path,
            "schema_version: 1\ndoi_prefixes:\n  custom: '10.9/custom.'\n",
        )
        .unwrap();
        assert_eq!(doi_prefix_for("custom"), Some("10.9/custom.".to_string()));
        assert_eq!(doi_prefix_for("unknown"), None);

        fs::write(&path, "doi_prefixes: [not: yaml").unwrap();
        assert_eq!(doi_prefix_for("arxiv"), None);

        fs::write(
            &path,
            "schema_version: 1\ndoi_prefixes:\n  custom: '10.9/custom.'\n",
        )
        .unwrap();
        assert_eq!(doi_prefix_for("arxiv"), None);

        fs::remove_file(&path).unwrap();
        let bundled_arxiv =
            parse_map(BUNDLED_DOI_PREFIX_MAP).and_then(|map| map.get("arxiv").cloned());
        assert_eq!(doi_prefix_for("arxiv"), bundled_arxiv);
        unsafe { std::env::remove_var("DA_PATH_DOI_PREFIX_MAP") };
    }

    #[test]
    fn unknown_source_from_bundled_data_is_a_miss() {
        assert_eq!(doi_prefix_for("not-a-source"), None);
    }
}
