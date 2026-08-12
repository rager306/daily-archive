//! Provider-neutral sequential batch acquisition orchestration.

use std::sync::Arc;

use da_domain::acquisition_batch::{
    BatchAcquisitionAsset, BatchAcquisitionItem, BatchAcquisitionReport, BatchAcquisitionStatus,
};
use da_domain::{AcquisitionPolicy, ArticleLocator, AssetDisposition};
use da_ports::SourceAcquisitionPort;

pub struct BatchAcquisitionUseCase {
    acquisition: Arc<dyn SourceAcquisitionPort>,
}

impl BatchAcquisitionUseCase {
    pub fn new(acquisition: Arc<dyn SourceAcquisitionPort>) -> Self {
        Self { acquisition }
    }

    pub fn execute(
        &self,
        ids: &[String],
        allow_network: bool,
        mut progress: impl FnMut(&BatchAcquisitionItem),
    ) -> Result<BatchAcquisitionReport, String> {
        if ids.is_empty() {
            return Err("batch acquisition requires ids".into());
        }
        let mut policy = AcquisitionPolicy::ensure_only();
        policy.allow_network = allow_network;
        policy.include_html = true;
        policy.enable_import = false;
        let mut items = Vec::with_capacity(ids.len());
        for (index, id) in ids.iter().enumerate() {
            let item = match ArticleLocator::parse(id)
                .map_err(|error| error.to_string())
                .and_then(|locator| {
                    self.acquisition
                        .resolve(&locator)
                        .map_err(|error| error.to_string())
                })
                .and_then(|resolved| {
                    self.acquisition
                        .ensure_assets(&resolved, &policy)
                        .map_err(|error| error.to_string())
                }) {
                Ok(ensured) => BatchAcquisitionItem {
                    ordinal: index + 1,
                    id: id.clone(),
                    status: if ensured.catalog_hit {
                        BatchAcquisitionStatus::Skip
                    } else {
                        BatchAcquisitionStatus::Ok
                    },
                    catalog_hit: ensured.catalog_hit,
                    assets: ensured
                        .assets
                        .into_iter()
                        .map(|asset| BatchAcquisitionAsset {
                            kind: asset.kind,
                            path: asset.path,
                            sha256: asset.sha256,
                            bytes: asset.bytes,
                            disposition: disposition(asset.disposition).into(),
                        })
                        .collect(),
                    error: None,
                    import_eligible: false,
                },
                Err(error) => BatchAcquisitionItem {
                    ordinal: index + 1,
                    id: id.clone(),
                    status: BatchAcquisitionStatus::Fail,
                    catalog_hit: false,
                    assets: vec![],
                    error: Some(error),
                    import_eligible: false,
                },
            };
            progress(&item);
            items.push(item);
        }
        BatchAcquisitionReport::seal(
            self.acquisition.provider_code().into(),
            allow_network,
            items,
        )
    }
}

fn disposition(value: AssetDisposition) -> &'static str {
    match value {
        AssetDisposition::Ok => "ok",
        AssetDisposition::Unchanged => "unchanged",
        AssetDisposition::CatalogHit => "catalog_hit",
        AssetDisposition::Fetched => "fetched",
        AssetDisposition::Skip => "skip",
        AssetDisposition::Fail => "fail",
        AssetDisposition::NotRequested => "not_requested",
        AssetDisposition::NotLicensed => "not_licensed",
    }
}
