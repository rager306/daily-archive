//! Deterministic gold-corpus discovery and bounded batch planning.
//!
//! Gold fixtures are evaluation evidence, not graph truth. This module only
//! resolves canonical inputs and inventories already-produced enrichment layers.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const GOLD_CORPUS_MANIFEST_VERSION: &str = "gold-corpus-manifest-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalSourceKind {
    Pdf,
    Html,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalSource {
    pub kind: CanonicalSourceKind,
    pub path: Option<String>,
    pub sha256: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerAvailability {
    pub article_metadata: bool,
    pub loader_events: bool,
    pub figure_manifest: bool,
    pub figure_contexts: usize,
    pub figure_analyses: usize,
    pub pipeline_cache_manifests: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldCorpusRecord {
    pub paper_id: String,
    pub title: String,
    pub fixture_path: String,
    pub gold_entity_count: usize,
    pub source: CanonicalSource,
    pub layers: LayerAvailability,
    pub batch: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldCorpusManifest {
    pub version: String,
    pub batch_size: usize,
    pub fixture_count: usize,
    pub source_ready_count: usize,
    pub unavailable_count: usize,
    pub batch_count: usize,
    pub records: Vec<GoldCorpusRecord>,
}

#[derive(Debug, Deserialize)]
struct GoldFixtureHeader {
    paper_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    entities: Vec<serde_json::Value>,
}

pub fn build_gold_corpus_manifest(
    fixture_root: &Path,
    catalog_root: &Path,
    batch_size: usize,
) -> Result<GoldCorpusManifest, String> {
    if batch_size == 0 {
        return Err("batch_size must be greater than zero".into());
    }
    let fixture_paths = sorted_files_with_suffix(fixture_root, ".gold.json")?;
    if fixture_paths.is_empty() {
        return Err(format!(
            "no *.gold.json fixtures found under {}",
            fixture_root.display()
        ));
    }
    let catalog_files = sorted_files(catalog_root)?;
    let mut seen = BTreeSet::new();
    let mut records = Vec::with_capacity(fixture_paths.len());

    for fixture_path in fixture_paths {
        let fixture: GoldFixtureHeader = serde_json::from_slice(
            &std::fs::read(&fixture_path)
                .map_err(|error| format!("read {}: {error}", fixture_path.display()))?,
        )
        .map_err(|error| format!("parse {}: {error}", fixture_path.display()))?;
        let file_id = fixture_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".gold.json"))
            .ok_or_else(|| format!("invalid fixture filename {}", fixture_path.display()))?;
        if fixture.paper_id != file_id {
            return Err(format!(
                "fixture identity mismatch: filename '{file_id}', paper_id '{}'",
                fixture.paper_id
            ));
        }
        if !seen.insert(fixture.paper_id.clone()) {
            return Err(format!("duplicate gold paper_id '{}'", fixture.paper_id));
        }

        let source_path = resolve_source(&fixture.paper_id, &catalog_files)?;
        let source = match source_path {
            Some((kind, path)) => CanonicalSource {
                kind,
                sha256: Some(file_sha256(&path)?),
                path: Some(path.to_string_lossy().to_string()),
                unavailable_reason: None,
            },
            None => CanonicalSource {
                kind: CanonicalSourceKind::Unavailable,
                path: None,
                sha256: None,
                unavailable_reason: Some("no canonical PDF or chapter HTML".into()),
            },
        };
        let article_root = source
            .path
            .as_deref()
            .and_then(|path| infer_article_root(Path::new(path)));
        let layers = article_root
            .as_deref()
            .map(|root| inventory_layers(root, &catalog_files))
            .unwrap_or_default();
        let batch = records.len() / batch_size + 1;
        records.push(GoldCorpusRecord {
            paper_id: fixture.paper_id,
            title: fixture.title,
            fixture_path: fixture_path.to_string_lossy().to_string(),
            gold_entity_count: fixture.entities.len(),
            source,
            layers,
            batch,
        });
    }

    let source_ready_count = records
        .iter()
        .filter(|record| record.source.kind != CanonicalSourceKind::Unavailable)
        .count();
    let fixture_count = records.len();
    Ok(GoldCorpusManifest {
        version: GOLD_CORPUS_MANIFEST_VERSION.into(),
        batch_size,
        fixture_count,
        source_ready_count,
        unavailable_count: fixture_count - source_ready_count,
        batch_count: fixture_count.div_ceil(batch_size),
        records,
    })
}

pub fn write_manifest(path: &Path, manifest: &GoldCorpusManifest) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("serialize manifest: {error}"))?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|error| {
        format!(
            "commit manifest {} -> {}: {error}",
            temporary.display(),
            path.display()
        )
    })
}

fn resolve_source(
    paper_id: &str,
    catalog_files: &[PathBuf],
) -> Result<Option<(CanonicalSourceKind, PathBuf)>, String> {
    let exact_pdf_name = format!("{paper_id}.pdf");
    let exact_pdfs = catalog_files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some(&exact_pdf_name))
        .collect::<Vec<_>>();
    if exact_pdfs.len() == 1 {
        return Ok(Some((CanonicalSourceKind::Pdf, exact_pdfs[0].clone())));
    }
    if exact_pdfs.len() > 1 {
        let hashes = exact_pdfs
            .iter()
            .map(|path| file_sha256(path))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let canonical = exact_pdfs
            .iter()
            .filter(|path| category_component_is_canonical(path))
            .collect::<Vec<_>>();
        if hashes.len() == 1 && canonical.len() == 1 {
            return Ok(Some((CanonicalSourceKind::Pdf, (*canonical[0]).clone())));
        }
        return Err(format!(
            "ambiguous exact PDF sources for '{paper_id}': {}",
            exact_pdfs
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let versioned_pdfs = catalog_files
        .iter()
        .filter(|path| {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                return false;
            };
            path.extension().and_then(|extension| extension.to_str()) == Some("pdf")
                && versioned_identity_matches(stem, paper_id)
                && path_components_match_paper(path, paper_id)
        })
        .collect::<Vec<_>>();
    match versioned_pdfs.as_slice() {
        [path] => return Ok(Some((CanonicalSourceKind::Pdf, (*path).clone()))),
        [] => {}
        paths => {
            return Err(format!(
                "ambiguous PDF sources for '{paper_id}': {}",
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    Ok(catalog_files
        .iter()
        .find(|path| {
            matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("chapter.html" | "article.html")
            ) && path_components_match_paper(path, paper_id)
        })
        .cloned()
        .map(|path| (CanonicalSourceKind::Html, path)))
}

fn category_component_is_canonical(path: &Path) -> bool {
    path.parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .is_some_and(|category| category == category.to_ascii_lowercase())
}

fn versioned_identity_matches(value: &str, paper_id: &str) -> bool {
    value.strip_prefix(paper_id).is_some_and(|suffix| {
        suffix.starts_with('v')
            && suffix.len() > 1
            && suffix[1..]
                .chars()
                .all(|character| character.is_ascii_digit())
    })
}

fn path_components_match_paper(path: &Path, paper_id: &str) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        value == paper_id || versioned_identity_matches(&value, paper_id)
    })
}

fn infer_article_root(source: &Path) -> Option<PathBuf> {
    let parent = source.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) == Some("source") {
        parent.parent().map(Path::to_path_buf)
    } else {
        Some(parent.to_path_buf())
    }
}

fn inventory_layers(article_root: &Path, catalog_files: &[PathBuf]) -> LayerAvailability {
    let files: Vec<&PathBuf> = catalog_files
        .iter()
        .filter(|path| path.starts_with(article_root))
        .collect();
    let count_suffix = |suffix: &str| {
        files
            .iter()
            .filter(|path| path.to_string_lossy().ends_with(suffix))
            .count()
    };
    LayerAvailability {
        article_metadata: files
            .iter()
            .any(|path| path.file_name().and_then(|name| name.to_str()) == Some("article.json")),
        loader_events: files
            .iter()
            .any(|path| path.to_string_lossy().ends_with("loader/events.jsonl")),
        figure_manifest: files.iter().any(|path| {
            path.file_name().and_then(|name| name.to_str()) == Some("pdffigures2.manifest.json")
        }),
        figure_contexts: count_suffix(".context.json"),
        figure_analyses: count_suffix(".analysis.json"),
        pipeline_cache_manifests: files
            .iter()
            .filter(|path| {
                path.file_name().and_then(|name| name.to_str()) == Some("manifest.json")
                    && path
                        .components()
                        .any(|part| part.as_os_str() == "pipeline_cache")
            })
            .count(),
    }
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn sorted_files_with_suffix(root: &Path, suffix: &str) -> Result<Vec<PathBuf>, String> {
    Ok(sorted_files(root)?
        .into_iter()
        .filter(|path| path.to_string_lossy().ends_with(suffix))
        .collect())
}

fn sorted_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.is_dir() {
        return Err(format!("directory not found: {}", root.display()));
    }
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| format!("read directory {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("read directory entry: {error}"))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub fn layer_totals(manifest: &GoldCorpusManifest) -> BTreeMap<&'static str, usize> {
    let mut totals = BTreeMap::new();
    totals.insert(
        "article_metadata",
        manifest
            .records
            .iter()
            .filter(|record| record.layers.article_metadata)
            .count(),
    );
    totals.insert(
        "figure_manifests",
        manifest
            .records
            .iter()
            .filter(|record| record.layers.figure_manifest)
            .count(),
    );
    totals.insert(
        "figure_contexts",
        manifest
            .records
            .iter()
            .map(|record| record.layers.figure_contexts)
            .sum(),
    );
    totals.insert(
        "figure_analyses",
        manifest
            .records
            .iter()
            .map(|record| record.layers.figure_analyses)
            .sum(),
    );
    totals.insert(
        "pipeline_cache_manifests",
        manifest
            .records
            .iter()
            .map(|record| record.layers.pipeline_cache_manifests)
            .sum(),
    );
    totals
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("da-gold-corpus-{name}-{nonce}"));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn fixture(path: &Path, paper_id: &str, entities: usize) {
        let values = (0..entities)
            .map(|index| serde_json::json!({"label": format!("e{index}"), "entity_type": "Method"}))
            .collect::<Vec<_>>();
        std::fs::write(
            path,
            serde_json::to_vec(&serde_json::json!({
                "paper_id": paper_id,
                "title": format!("Paper {paper_id}"),
                "entities": values
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn deterministic_manifest_batches_sources_and_richer_layers() {
        let root = temp_root("manifest");
        let fixtures = root.join("gold");
        let catalog = root.join("catalog");
        std::fs::create_dir_all(&fixtures).unwrap();
        for id in ["p1", "p2", "p3"] {
            fixture(&fixtures.join(format!("{id}.gold.json")), id, 2);
        }
        let p1 = catalog.join("arxiv/cs-ai/p1");
        std::fs::create_dir_all(p1.join("source")).unwrap();
        std::fs::create_dir_all(p1.join("figures")).unwrap();
        std::fs::create_dir_all(p1.join("pipeline_cache/p1/parse/key")).unwrap();
        std::fs::write(p1.join("source/p1.pdf"), b"pdf").unwrap();
        std::fs::write(p1.join("article.json"), b"{}").unwrap();
        std::fs::write(p1.join("figures/Figure1.context.json"), b"{}").unwrap();
        std::fs::write(p1.join("pipeline_cache/p1/parse/key/manifest.json"), b"{}").unwrap();
        let p2 = catalog.join("books/p2");
        std::fs::create_dir_all(&p2).unwrap();
        std::fs::write(p2.join("chapter.html"), b"html").unwrap();

        let first = build_gold_corpus_manifest(&fixtures, &catalog, 2).unwrap();
        let second = build_gold_corpus_manifest(&fixtures, &catalog, 2).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.fixture_count, 3);
        assert_eq!(first.source_ready_count, 2);
        assert_eq!(first.unavailable_count, 1);
        assert_eq!(first.batch_count, 2);
        assert_eq!(
            first.records.iter().map(|r| r.batch).collect::<Vec<_>>(),
            [1, 1, 2]
        );
        assert_eq!(first.records[0].source.kind, CanonicalSourceKind::Pdf);
        assert!(first.records[0].layers.article_metadata);
        assert_eq!(first.records[0].layers.figure_contexts, 1);
        assert_eq!(first.records[0].layers.pipeline_cache_manifests, 1);
        assert_eq!(first.records[1].source.kind, CanonicalSourceKind::Html);
        assert_eq!(
            first.records[2].source.kind,
            CanonicalSourceKind::Unavailable
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn versioned_pdf_and_article_html_are_valid_canonical_sources() {
        let root = temp_root("versioned-sources");
        let fixtures = root.join("gold");
        let catalog = root.join("catalog");
        std::fs::create_dir_all(&fixtures).unwrap();
        fixture(&fixtures.join("2605.26525.gold.json"), "2605.26525", 1);
        fixture(&fixtures.join("2606.11190.gold.json"), "2606.11190", 1);

        let pdf_root = catalog.join("arxiv/cs-cv/2605.26525v1/source");
        std::fs::create_dir_all(&pdf_root).unwrap();
        std::fs::write(pdf_root.join("2605.26525v1.pdf"), b"pdf").unwrap();
        let html_root = catalog.join("arxiv/cs-lg/2606.11190v2");
        std::fs::create_dir_all(&html_root).unwrap();
        std::fs::write(html_root.join("article.html"), b"html").unwrap();

        let manifest = build_gold_corpus_manifest(&fixtures, &catalog, 10).unwrap();
        assert_eq!(manifest.source_ready_count, 2);
        assert_eq!(manifest.unavailable_count, 0);
        assert_eq!(manifest.records[0].source.kind, CanonicalSourceKind::Pdf);
        assert_eq!(manifest.records[1].source.kind, CanonicalSourceKind::Html);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_exact_pdf_prefers_unique_lowercase_category_when_content_matches() {
        let root = temp_root("duplicate-category-case");
        let fixtures = root.join("gold");
        let catalog = root.join("catalog");
        std::fs::create_dir_all(&fixtures).unwrap();
        fixture(&fixtures.join("2605.26525.gold.json"), "2605.26525", 1);
        for category in ["cs-CV", "cs-cv"] {
            let source = catalog.join(format!("arxiv/{category}/2605.26525/source"));
            std::fs::create_dir_all(&source).unwrap();
            std::fs::write(source.join("2605.26525.pdf"), b"same").unwrap();
        }
        std::fs::write(catalog.join("arxiv/cs-cv/2605.26525/article.json"), b"{}").unwrap();
        let manifest = build_gold_corpus_manifest(&fixtures, &catalog, 10).unwrap();
        assert!(
            manifest.records[0]
                .source
                .path
                .as_ref()
                .unwrap()
                .contains("cs-cv")
        );
        assert!(manifest.records[0].layers.article_metadata);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn conflicting_versioned_pdfs_fail_closed() {
        let root = temp_root("conflicting-versions");
        let fixtures = root.join("gold");
        let catalog = root.join("catalog");
        std::fs::create_dir_all(&fixtures).unwrap();
        fixture(&fixtures.join("2605.26525.gold.json"), "2605.26525", 1);
        for (version, bytes) in [("v1", b"one"), ("v2", b"two")] {
            let source = catalog.join(format!("arxiv/cs-cv/2605.26525{version}/source"));
            std::fs::create_dir_all(&source).unwrap();
            std::fs::write(source.join(format!("2605.26525{version}.pdf")), bytes).unwrap();
        }
        let error = build_gold_corpus_manifest(&fixtures, &catalog, 10).unwrap_err();
        assert!(error.contains("ambiguous PDF sources"), "{error}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fixture_filename_identity_mismatch_fails_closed() {
        let root = temp_root("identity");
        let fixtures = root.join("gold");
        let catalog = root.join("catalog");
        std::fs::create_dir_all(&fixtures).unwrap();
        std::fs::create_dir_all(&catalog).unwrap();
        fixture(&fixtures.join("expected.gold.json"), "other", 1);
        let error = build_gold_corpus_manifest(&fixtures, &catalog, 10).unwrap_err();
        assert!(error.contains("identity mismatch"), "{error}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zero_batch_size_is_rejected() {
        let error =
            build_gold_corpus_manifest(Path::new("missing"), Path::new("missing"), 0).unwrap_err();
        assert!(error.contains("batch_size"));
    }
}
