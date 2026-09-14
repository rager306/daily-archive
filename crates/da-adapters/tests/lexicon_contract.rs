mod common;

use common::lexicon_contract::{FakeLexiconLoader, assert_loader_contract, valid_bundle};
use da_adapters::FilesystemLexiconLoader;
use da_ports::{LexiconLoadError, LexiconLoaderPort};

fn write_valid_yaml(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("grobid-lexicon.yaml");
    std::fs::write(
        &path,
        "schema_version: grobid-lexicon.v1\nresource_version: 0.9.0-crf\nchannels:\n  - key: chemical\n    entries: [acid, water]\nfeature_templates:\n  - key: layout-basic\n    channel_keys: [chemical]\n    feature_inputs: [token.text]\n",
    )
    .unwrap();
    path
}

#[test]
fn contract_accepts_fake_and_filesystem_loaders() {
    let fake = FakeLexiconLoader {
        result: Ok(valid_bundle()),
    };
    assert_loader_contract(&fake);

    let dir = tempfile::tempdir().unwrap();
    let filesystem = FilesystemLexiconLoader::new(write_valid_yaml(dir.path()));
    assert_loader_contract(&filesystem);
}

#[test]
fn contract_rejects_missing_malformed_and_incompatible_resources() {
    let missing = FilesystemLexiconLoader::new("missing/grobid-lexicon.yaml").load();
    assert!(matches!(missing, Err(LexiconLoadError::Unavailable(_))));

    let dir = tempfile::tempdir().unwrap();
    let malformed_path = dir.path().join("malformed.yaml");
    std::fs::write(&malformed_path, "schema_version: [broken").unwrap();
    let malformed = FilesystemLexiconLoader::new(malformed_path).load();
    assert!(matches!(malformed, Err(LexiconLoadError::Malformed(_))));

    let incompatible_path = dir.path().join("incompatible.yaml");
    std::fs::write(
        &incompatible_path,
        "schema_version: grobid-lexicon.v1\nresource_version: 0.8.0-crf\nchannels:\n  - key: chemical\n    entries: [acid]\nfeature_templates:\n  - key: layout-basic\n    channel_keys: [chemical]\n    feature_inputs: [token.text]\n",
    )
    .unwrap();
    let incompatible = FilesystemLexiconLoader::new(incompatible_path).load();
    assert!(matches!(incompatible, Err(LexiconLoadError::Invalid(_))));
}

#[test]
fn serde_rejects_unknown_fields_in_resource() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unknown.yaml");
    let mut yaml = String::from(
        "schema_version: grobid-lexicon.v1\nresource_version: 0.9.0-crf\nchannels:\n  - key: chemical\n    entries: [acid]\nfeature_templates:\n  - key: layout-basic\n    channel_keys: [chemical]\n    feature_inputs: [token.text]\n",
    );
    yaml.push_str("unknown_field: true\n");
    std::fs::write(&path, yaml).unwrap();
    let result = FilesystemLexiconLoader::new(path).load();
    assert!(matches!(result, Err(LexiconLoadError::Malformed(_))));
}
