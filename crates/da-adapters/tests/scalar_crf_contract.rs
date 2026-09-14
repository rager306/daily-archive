mod common;

use common::scalar_crf_contract::{
    FakeCrfLoader, MALFORMED_FIXTURE, SYNTHETIC_FIXTURE, VARIANT_FIXTURE, assert_loader_contract,
    assert_malformed_model, assert_oversize_error, fixture_path,
};
use da_adapters::FilesystemCrfModelLoader;
use da_domain::scalar_crf::{CrfModel, MAX_CRF_MODEL_BYTES};
use da_ports::CrfModelLoaderPort;

fn load_fixture(name: &str) -> CrfModel {
    CrfModel::parse(&std::fs::read(fixture_path(name)).expect("fixture should exist"))
        .expect("fixture should parse")
}

#[test]
fn fake_and_filesystem_loaders_share_the_hand_computed_contract() {
    let model = load_fixture(SYNTHETIC_FIXTURE);
    assert_loader_contract(&FakeCrfLoader {
        result: Ok(model.clone()),
    });

    let filesystem = FilesystemCrfModelLoader::new(fixture_path(SYNTHETIC_FIXTURE));
    assert_loader_contract(&filesystem);
}

#[test]
fn second_hand_authored_variant_preserves_model_shape_and_hex_float_semantics() {
    let model = load_fixture(VARIANT_FIXTURE);
    assert_eq!(model.nftr, 8);
    assert_eq!(model.weights.get(&1), Some(&2.0));
    assert_eq!(model.weights.get(&6), Some(&-1.0));
    assert!(!model.weights.contains_key(&0));

    let loader = FakeCrfLoader { result: Ok(model) };
    let loaded = loader.load().expect("variant fake should load");
    assert_eq!(loaded.labels, ["A", "B"]);
}

#[test]
fn malformed_fixture_rejects_out_of_range_weight() {
    assert_malformed_model(
        &std::fs::read(fixture_path(MALFORMED_FIXTURE)).expect("malformed fixture should exist"),
    );

    let filesystem = FilesystemCrfModelLoader::new(fixture_path(MALFORMED_FIXTURE));
    assert!(matches!(
        filesystem.load(),
        Err(da_ports::CrfModelLoadError::Malformed(_))
    ));
}

#[test]
fn filesystem_loader_rejects_resource_over_limit_before_reading() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("oversized.wapiti");
    let file = std::fs::File::create(&path).expect("create oversized fixture");
    file.set_len(MAX_CRF_MODEL_BYTES as u64 + 1)
        .expect("extend oversized fixture");

    let error = FilesystemCrfModelLoader::new(path).load().unwrap_err();
    assert_oversize_error(error);
}
