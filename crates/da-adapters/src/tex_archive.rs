//! Filesystem TeX archive adapter backed by the tested safe inventory.
//!
//! Members are streamed with `tar -xO`; nothing is extracted to disk. The
//! application layer independently verifies each byte count and checksum.

use da_domain::ContentDigest;
use da_ports::{
    TexAnalysisReportStore, TexArchiveError, TexArchiveInventory, TexArchiveMember, TexArchivePort,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Deserialize)]
struct InventoryFile {
    schema_version: String,
    archive_path: String,
    article_id: String,
    sha256: String,
    byte_size: u64,
    import_eligible: bool,
    files: Vec<InventoryMember>,
}

#[derive(Debug, Deserialize)]
struct InventoryMember {
    path: String,
    byte_size: u64,
    sha256: String,
}

pub struct FilesystemTexAnalysisReportStore {
    path: PathBuf,
}

impl FilesystemTexAnalysisReportStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl TexAnalysisReportStore for FilesystemTexAnalysisReportStore {
    fn store(&self, report: &da_domain::TexSourceAnalysisReport) -> Result<(), TexArchiveError> {
        report
            .validate()
            .map_err(|error| TexArchiveError::Io(error.to_string()))?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| TexArchiveError::Io("report parent missing".into()))?;
        std::fs::create_dir_all(parent).map_err(io_error)?;
        let mut bytes = serde_json::to_vec_pretty(report).map_err(io_error)?;
        bytes.push(b'\n');
        let temp = parent.join(format!(
            ".{}.tmp-{}",
            self.path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("tex-report"),
            std::process::id()
        ));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temp)
                .map_err(io_error)?;
            file.write_all(&bytes).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
            std::fs::rename(&temp, &self.path).map_err(io_error)?;
            std::fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(io_error)
        })();
        let _ = std::fs::remove_file(temp);
        result
    }
}

pub struct FilesystemTexArchive {
    archive_path: PathBuf,
    inventory_path: PathBuf,
}

impl FilesystemTexArchive {
    pub fn new(archive_path: impl Into<PathBuf>, inventory_path: impl Into<PathBuf>) -> Self {
        Self {
            archive_path: archive_path.into(),
            inventory_path: inventory_path.into(),
        }
    }

    fn load_inventory(&self) -> Result<TexArchiveInventory, TexArchiveError> {
        let bytes = std::fs::read(&self.inventory_path).map_err(io_error)?;
        let raw: InventoryFile = serde_json::from_slice(&bytes).map_err(io_error)?;
        if raw.schema_version != "arxiv-tex-source-inventory.v1"
            || raw.article_id.trim().is_empty()
            || raw.import_eligible
        {
            return Err(TexArchiveError::Io(
                "invalid TeX inventory schema, article identity, or authority".into(),
            ));
        }
        let declared_archive = PathBuf::from(&raw.archive_path);
        let declared_archive = if declared_archive.is_absolute() {
            declared_archive
        } else {
            std::env::current_dir()
                .map_err(io_error)?
                .join(declared_archive)
        };
        if std::fs::canonicalize(&declared_archive).map_err(io_error)?
            != std::fs::canonicalize(&self.archive_path).map_err(io_error)?
        {
            return Err(TexArchiveError::ChecksumMismatch(raw.archive_path));
        }
        let archive_sha256 = ContentDigest::try_from(raw.sha256)
            .map_err(|error| TexArchiveError::Io(error.to_string()))?;
        let metadata = std::fs::metadata(&self.archive_path).map_err(io_error)?;
        if metadata.len() != raw.byte_size || digest_file(&self.archive_path)? != archive_sha256 {
            return Err(TexArchiveError::ChecksumMismatch(
                self.archive_path.display().to_string(),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        let members = raw
            .files
            .into_iter()
            .map(|member| {
                validate_member_path(&member.path)?;
                if !seen.insert(member.path.clone()) {
                    return Err(TexArchiveError::UnsafeMember(format!(
                        "duplicate inventory path: {}",
                        member.path
                    )));
                }
                Ok(TexArchiveMember {
                    path: member.path,
                    byte_size: member.byte_size,
                    sha256: ContentDigest::try_from(member.sha256)
                        .map_err(|error| TexArchiveError::Io(error.to_string()))?,
                })
            })
            .collect::<Result<Vec<_>, TexArchiveError>>()?;
        Ok(TexArchiveInventory {
            article_id: raw.article_id,
            archive_sha256,
            members,
        })
    }
}

impl TexArchivePort for FilesystemTexArchive {
    fn inventory(&self) -> Result<TexArchiveInventory, TexArchiveError> {
        self.load_inventory()
    }

    fn read_members(&self, paths: &[String]) -> Result<BTreeMap<String, Vec<u8>>, TexArchiveError> {
        let allowed = self
            .load_inventory()?
            .members
            .into_iter()
            .map(|member| (member.path, (member.byte_size, member.sha256)))
            .collect::<BTreeMap<_, _>>();
        let mut output = BTreeMap::new();
        for path in paths {
            validate_member_path(path)?;
            let (expected_size, expected_sha256) = allowed
                .get(path)
                .cloned()
                .ok_or_else(|| TexArchiveError::MissingMember(path.clone()))?;
            ensure_unique_archive_member(&self.archive_path, path)?;
            let mut child = hermetic_tar_command()
                .args(["--no-wildcards", "--no-unquote", "-xOzf"])
                .arg(&self.archive_path)
                .arg("--")
                .arg(path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(io_error)?;
            let mut bytes = Vec::with_capacity(expected_size as usize);
            child
                .stdout
                .take()
                .ok_or_else(|| TexArchiveError::Io("tar stdout missing".into()))?
                .take(expected_size + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes.len() as u64 > expected_size {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TexArchiveError::ChecksumMismatch(path.clone()));
            }
            let result = child.wait_with_output().map_err(io_error)?;
            if !result.status.success() {
                return Err(TexArchiveError::Io(format!(
                    "tar member read failed for {path}: {}",
                    String::from_utf8_lossy(&result.stderr)
                )));
            }
            if bytes.len() as u64 != expected_size
                || ContentDigest::of_bytes(&bytes) != expected_sha256
            {
                return Err(TexArchiveError::ChecksumMismatch(path.clone()));
            }
            output.insert(path.clone(), bytes);
        }
        Ok(output)
    }
}

fn validate_member_path(path: &str) -> Result<(), TexArchiveError> {
    if path.is_empty()
        || path.starts_with(['/', '\\'])
        || path.contains('\0')
        || path.contains('\\')
        || path.starts_with('-')
        || path.chars().any(char::is_control)
        || path.split('/').any(|part| part.is_empty() || part == ".")
        || Path::new(path).components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(TexArchiveError::UnsafeMember(path.into()));
    }
    Ok(())
}

fn hermetic_tar_command() -> Command {
    let mut command = Command::new("tar");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg("--quoting-style=literal");
    command
}

fn ensure_unique_archive_member(archive: &Path, path: &str) -> Result<(), TexArchiveError> {
    let output = hermetic_tar_command()
        .args(["--no-wildcards", "--no-unquote", "-tzf"])
        .arg(archive)
        .arg("--")
        .arg(path)
        .output()
        .map_err(io_error)?;
    if !output.status.success() {
        return Err(TexArchiveError::Io(format!(
            "tar member listing failed for {path}: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let matches = String::from_utf8(output.stdout)
        .map_err(|_| TexArchiveError::UnsafeMember(path.into()))?
        .lines()
        .filter(|item| *item == path)
        .count();
    if matches != 1 {
        return Err(TexArchiveError::UnsafeMember(format!(
            "archive member must be unique: {path} (matches={matches})"
        )));
    }
    Ok(())
}

fn digest_file(path: &Path) -> Result<ContentDigest, TexArchiveError> {
    let mut file = std::fs::File::open(path).map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1 << 20];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    ContentDigest::try_from(hex::encode(digest.finalize()))
        .map_err(|error| TexArchiveError::Io(error.to_string()))
}

fn io_error(error: impl std::fmt::Display) -> TexArchiveError {
    TexArchiveError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_substitution_is_rejected_before_member_read() {
        let root = std::env::temp_dir().join(format!("da-tex-adapter-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("source.tar.gz");
        let inventory = root.join("inventory.json");
        std::fs::write(&archive, b"original").unwrap();
        let digest = ContentDigest::of_bytes(b"original");
        std::fs::write(
            &inventory,
            serde_json::json!({
                "schema_version": "arxiv-tex-source-inventory.v1",
                "archive_path": archive.to_string_lossy(),
                "article_id": "paper", "sha256": digest.as_str(), "byte_size": 8,
                "import_eligible": false,
                "files": []
            })
            .to_string(),
        )
        .unwrap();
        let adapter = FilesystemTexArchive::new(&archive, &inventory);
        assert!(adapter.inventory().is_ok());
        std::fs::write(&archive, b"tampered").unwrap();
        assert!(matches!(
            adapter.inventory(),
            Err(TexArchiveError::ChecksumMismatch(_))
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn path_validation_rejects_escape_absolute_and_noncanonical_forms() {
        for path in [
            "../x.tex",
            "/x.tex",
            "\\x.tex",
            "a/../../x.tex",
            "./x.tex",
            "a//b.tex",
            "a/./b.tex",
            "a\\b.tex",
            "-option.tex",
            "line\nbreak.tex",
            "",
        ] {
            assert!(validate_member_path(path).is_err(), "{path}");
        }
        assert!(validate_member_path("sections/body.tex").is_ok());
        assert!(validate_member_path("sections/café.tex").is_ok());
    }

    #[test]
    fn filesystem_report_store_is_atomic_and_rejects_authority_drift() {
        let root = std::env::temp_dir().join(format!("da-tex-report-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("report.json");
        let store = FilesystemTexAnalysisReportStore::new(&path);
        let report = da_domain::TexSourceAnalysisReport::rejected(
            "paper",
            ContentDigest::of_text("archive"),
            vec![],
            "test_reject",
        );
        store.store(&report).unwrap();
        let stored: da_domain::TexSourceAnalysisReport =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored, report);
        let mut poisoned = report;
        poisoned.import_eligible = !poisoned.import_eligible;
        assert!(store.store(&poisoned).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inventory_requires_known_schema_and_nonempty_article_id() {
        let root = std::env::temp_dir().join(format!("da-tex-schema-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("source.tar.gz");
        let inventory = root.join("inventory.json");
        std::fs::write(&archive, b"archive").unwrap();
        std::fs::write(
            &inventory,
            serde_json::json!({
                "schema_version": "unknown",
                "archive_path": archive.to_string_lossy(),
                "article_id": "",
                "sha256": ContentDigest::of_bytes(b"archive").as_str(),
                "byte_size": 7,
                "import_eligible": false,
                "files": []
            })
            .to_string(),
        )
        .unwrap();
        let adapter = FilesystemTexArchive::new(&archive, &inventory);
        assert!(adapter.inventory().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
