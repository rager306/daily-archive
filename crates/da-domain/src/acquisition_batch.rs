//! Provider-neutral batch source acquisition evidence.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchAcquisitionStatus {
    Ok,
    Skip,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchAcquisitionAsset {
    pub kind: String,
    pub path: String,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
    pub disposition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchAcquisitionItem {
    pub ordinal: usize,
    pub id: String,
    pub status: BatchAcquisitionStatus,
    pub catalog_hit: bool,
    pub assets: Vec<BatchAcquisitionAsset>,
    pub error: Option<String>,
    pub import_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchAcquisitionReport {
    pub schema_version: String,
    pub provider: String,
    pub total: usize,
    pub ok: usize,
    pub skip: usize,
    pub fail: usize,
    pub network_authorized: bool,
    pub items: Vec<BatchAcquisitionItem>,
    pub import_eligible: bool,
    pub report_sha256: String,
}

impl BatchAcquisitionReport {
    pub fn seal(
        provider: String,
        network_authorized: bool,
        items: Vec<BatchAcquisitionItem>,
    ) -> Result<Self, String> {
        if provider.trim().is_empty() || items.is_empty() {
            return Err("batch acquisition provider/items must not be empty".into());
        }
        for (index, item) in items.iter().enumerate() {
            if item.ordinal != index + 1 || item.id.trim().is_empty() || item.import_eligible {
                return Err(format!("invalid batch acquisition item at index {index}"));
            }
            if item.status == BatchAcquisitionStatus::Fail
                && item.error.as_deref().unwrap_or("").is_empty()
            {
                return Err(format!("failed item {} requires error", item.id));
            }
            for asset in &item.assets {
                if let Some(sha) = &asset.sha256
                    && (sha.len() != 64 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()))
                {
                    return Err(format!("item {} has noncanonical asset digest", item.id));
                }
            }
        }
        let ok = items
            .iter()
            .filter(|item| item.status == BatchAcquisitionStatus::Ok)
            .count();
        let skip = items
            .iter()
            .filter(|item| item.status == BatchAcquisitionStatus::Skip)
            .count();
        let fail = items.len() - ok - skip;
        let mut report = Self {
            schema_version: "batch-acquisition-report.v1".into(),
            provider,
            total: items.len(),
            ok,
            skip,
            fail,
            network_authorized,
            items,
            import_eligible: false,
            report_sha256: String::new(),
        };
        report.report_sha256 = digest(&report);
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "batch-acquisition-report.v1" || self.import_eligible {
            return Err("batch acquisition schema/D127 invalid".into());
        }
        if self.total != self.items.len() || self.ok + self.skip + self.fail != self.total {
            return Err("batch acquisition counts mismatch".into());
        }
        if self.report_sha256 != digest(self) {
            return Err("batch acquisition digest mismatch".into());
        }
        Ok(())
    }
}

fn digest(report: &BatchAcquisitionReport) -> String {
    let mut clone = report.clone();
    clone.report_sha256.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("report serializes"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(status: BatchAcquisitionStatus) -> BatchAcquisitionItem {
        BatchAcquisitionItem {
            ordinal: 1,
            id: "2501.00001".into(),
            status,
            catalog_hit: true,
            assets: vec![],
            error: None,
            import_eligible: false,
        }
    }
    #[test]
    fn report_roundtrip_and_digest_binding() {
        let r = BatchAcquisitionReport::seal(
            "arxiv".into(),
            false,
            vec![item(BatchAcquisitionStatus::Skip)],
        )
        .unwrap();
        let mut q: BatchAcquisitionReport =
            serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert!(q.validate().is_ok());
        q.items[0].id.push('x');
        assert!(q.validate().is_err());
    }
    #[test]
    fn failed_item_requires_error() {
        assert!(
            BatchAcquisitionReport::seal(
                "arxiv".into(),
                true,
                vec![item(BatchAcquisitionStatus::Fail)]
            )
            .is_err()
        );
    }
}
