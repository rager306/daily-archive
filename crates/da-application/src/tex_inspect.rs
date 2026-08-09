//! Candidate-only TeX archive inspection use case.

use da_domain::{ContentDigest, TexAnalysisPolicy, TexSourceAnalysisReport, analyze_tex_members};
use da_ports::{TexAnalysisReportStore, TexAnalysisRequest, TexArchiveError, TexArchivePort};

pub struct TexInspectUseCase<'a> {
    archive: &'a dyn TexArchivePort,
    store: Option<&'a dyn TexAnalysisReportStore>,
    policy: TexAnalysisPolicy,
}

impl<'a> TexInspectUseCase<'a> {
    pub fn new(archive: &'a dyn TexArchivePort) -> Self {
        Self {
            archive,
            store: None,
            policy: TexAnalysisPolicy::default(),
        }
    }

    pub fn with_store(
        archive: &'a dyn TexArchivePort,
        store: &'a dyn TexAnalysisReportStore,
    ) -> Self {
        Self {
            archive,
            store: Some(store),
            policy: TexAnalysisPolicy::default(),
        }
    }

    pub fn inspect(
        &self,
        request: TexAnalysisRequest,
    ) -> Result<TexSourceAnalysisReport, anyhow::Error> {
        let inventory = self.archive.inventory()?;
        let mut unique_paths = std::collections::BTreeSet::new();
        let mut text_bytes = 0_u64;
        for member in &inventory.members {
            if !unique_paths.insert(member.path.as_str()) {
                return Err(TexArchiveError::UnsafeMember(format!(
                    "duplicate inventory path: {}",
                    member.path
                ))
                .into());
            }
            if member.path == "00README.json"
                || member.path.ends_with(".tex")
                || member.path.ends_with(".bib")
            {
                if member.byte_size > 20 << 20 {
                    return Err(TexArchiveError::UnsafeMember(format!(
                        "text member too large: {}",
                        member.path
                    ))
                    .into());
                }
                text_bytes = text_bytes
                    .checked_add(member.byte_size)
                    .ok_or_else(|| TexArchiveError::UnsafeMember("text size overflow".into()))?;
            }
        }
        if inventory.members.len() > 1000 || text_bytes > 100 << 20 {
            return Err(
                TexArchiveError::UnsafeMember("archive analysis budget exceeded".into()).into(),
            );
        }
        let paths = inventory
            .members
            .iter()
            .filter(|member| {
                member.path == "00README.json"
                    || member.path.ends_with(".tex")
                    || member.path.ends_with(".bib")
            })
            .map(|member| member.path.clone())
            .collect::<Vec<_>>();
        let members = self.archive.read_members(&paths)?;
        for path in &paths {
            let expected = inventory
                .members
                .iter()
                .find(|member| &member.path == path)
                .ok_or_else(|| TexArchiveError::MissingMember(path.clone()))?;
            let bytes = members
                .get(path)
                .ok_or_else(|| TexArchiveError::MissingMember(path.clone()))?;
            if bytes.len() as u64 != expected.byte_size
                || ContentDigest::of_bytes(bytes) != expected.sha256
            {
                return Err(TexArchiveError::ChecksumMismatch(path.clone()).into());
            }
        }
        let providers = request.providers;
        let analysis = analyze_tex_members(
            &inventory.article_id,
            inventory.archive_sha256.clone(),
            &members,
            providers.clone(),
            &self.policy,
        );
        match analysis {
            Ok(report) => {
                report.validate()?;
                if let Some(store) = self.store {
                    store.store(&report)?;
                }
                Ok(report)
            }
            Err(error) => {
                if let Some(store) = self.store {
                    let reject = TexSourceAnalysisReport::rejected(
                        &inventory.article_id,
                        inventory.archive_sha256,
                        providers,
                        format!("analysis_rejected:{error}"),
                    );
                    reject.validate()?;
                    if let Err(store_error) = store.store(&reject) {
                        return Err(anyhow::anyhow!(
                            "analysis failed: {error}; reject persistence failed: {store_error}"
                        ));
                    }
                }
                Err(error.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::{TexArchiveInventory, TexArchiveMember};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    struct FakeArchive {
        inventory: TexArchiveInventory,
        members: BTreeMap<String, Vec<u8>>,
    }

    impl TexArchivePort for FakeArchive {
        fn inventory(&self) -> Result<TexArchiveInventory, TexArchiveError> {
            Ok(self.inventory.clone())
        }

        fn read_members(
            &self,
            paths: &[String],
        ) -> Result<BTreeMap<String, Vec<u8>>, TexArchiveError> {
            Ok(paths
                .iter()
                .filter_map(|path| {
                    self.members
                        .get(path)
                        .map(|bytes| (path.clone(), bytes.clone()))
                })
                .collect())
        }
    }

    fn fake() -> FakeArchive {
        let members: BTreeMap<String, Vec<u8>> = BTreeMap::from([
            (
                "main.tex".into(),
                b"\\documentclass{x}\\bibliography{refs}".to_vec(),
            ),
            ("refs.bib".into(), b"@article{k,title={T}}".to_vec()),
        ]);
        let inventory = TexArchiveInventory {
            article_id: "paper".into(),
            archive_sha256: ContentDigest::of_text("archive"),
            members: members
                .iter()
                .map(|(path, bytes)| TexArchiveMember {
                    path: path.clone(),
                    byte_size: bytes.len() as u64,
                    sha256: ContentDigest::of_bytes(bytes),
                })
                .collect(),
        };
        FakeArchive { inventory, members }
    }

    #[test]
    fn use_case_verifies_inventory_before_analysis() {
        let archive = fake();
        let report = TexInspectUseCase::new(&archive)
            .inspect(TexAnalysisRequest { providers: vec![] })
            .unwrap();
        assert_eq!(report.bibliography_candidates.len(), 1);
        assert!(!report.import_eligible);
    }

    struct FakeStore(Mutex<Vec<TexSourceAnalysisReport>>);

    impl TexAnalysisReportStore for FakeStore {
        fn store(&self, report: &TexSourceAnalysisReport) -> Result<(), TexArchiveError> {
            self.0.lock().unwrap().push(report.clone());
            Ok(())
        }
    }

    #[test]
    fn failure_is_persisted_as_candidate_only_reject() {
        let mut archive = fake();
        archive.members.remove("main.tex");
        archive
            .inventory
            .members
            .retain(|item| item.path != "main.tex");
        let store = FakeStore(Mutex::new(vec![]));
        let error = TexInspectUseCase::with_store(&archive, &store)
            .inspect(TexAnalysisRequest { providers: vec![] })
            .unwrap_err();
        assert!(error.to_string().contains("root"));
        let reports = store.0.lock().unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].status, da_domain::TexAnalysisStatus::Reject);
        assert!(!reports[0].import_eligible);
        assert!(reports[0].validate().is_ok());
    }

    #[test]
    fn checksum_drift_fails_before_analysis() {
        let mut archive = fake();
        archive
            .members
            .insert("refs.bib".into(), b"changed".to_vec());
        let error = TexInspectUseCase::new(&archive)
            .inspect(TexAnalysisRequest { providers: vec![] })
            .unwrap_err();
        assert!(error.to_string().contains("checksum mismatch"));
    }
}
