//! The GROBID release identity must be the same everywhere it is declared.
//!
//! WHY THIS TEST EXISTS: the release was updated to 0.9.1 in M505 (declared in
//! `data/parse/acceptance_manifest.yaml#declared_release` and pinned in
//! `.docker/grobid-release.override.yml`), but the bring-up defaults in `.env`,
//! `.env.example` and `.docker/docker-compose.yml` kept naming the previous release.
//! A stack started without the override layer therefore ran the rollback release
//! while the manifest declared 0.9.1 — and nothing detected it, because the existing
//! guard `admit_release_pin` in `external_holdout_capture.rs` checks only the pin
//! file. This test closes that surface.
//!
//! Pinned BY DIGEST, deliberately: a tag can be re-pointed upstream, and the pin file
//! already documents that a locally cached image can vanish under prune. Comparing
//! digests against the manifest is an offline string equality, so the check needs no
//! network, no tag resolution and no Docker.

use std::path::{Path, PathBuf};

/// The declared release taken from the manifest, which is the single source of truth.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclaredRelease {
    version: String,
    repository: String,
    digest: String,
}

fn repo_root() -> PathBuf {
    // crates/da-cli -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

fn declared_release() -> DeclaredRelease {
    let path = repo_root().join("data/parse/acceptance_manifest.yaml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("manifest {} unreadable: {error}", path.display()));
    let parsed: serde_yaml::Value = serde_yaml::from_str(&text).expect("manifest is YAML");
    let declared = parsed
        .get("external_holdout_contract")
        .and_then(|v| v.get("release_probe"))
        .and_then(|v| v.get("declared_release"))
        .expect("external_holdout_contract.release_probe.declared_release must exist");
    let field = |name: &str| -> String {
        declared
            .get(name)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("declared_release.{name} must be a string"))
            .to_string()
    };
    DeclaredRelease {
        version: field("version"),
        repository: field("image_repository"),
        digest: field("image_digest"),
    }
}

/// `GROBID_IMAGE=<value>` out of a dotenv-style file, ignoring comments.
fn grobid_image_from_env_file(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            line.strip_prefix("GROBID_IMAGE=")
                .map(|value| value.trim().to_string())
        })
}

/// The default of `image: ${GROBID_IMAGE:-<default>}` in the compose file.
fn compose_default_image(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines().map(str::trim) {
        if !line.starts_with("image:") {
            continue;
        }
        let rest = line.trim_start_matches("image:").trim();
        let start = rest.find(":-")? + 2;
        let default = rest[start..].trim_end_matches('}').trim();
        return Some(default.to_string());
    }
    None
}

/// The single validator. Every surface is checked through it so a real file and a
/// deliberately wrong value cannot be judged by different rules.
fn check_pinned_to_declared(label: &str, value: &str, declared: &DeclaredRelease) {
    let expected = format!("{}@{}", declared.repository, declared.digest);
    assert_eq!(
        value, expected,
        "{label} must pin the declared production release by digest; \
         a tag can be re-pointed upstream and the rollback release is a different digest"
    );
}

#[test]
fn env_example_pins_the_declared_release() {
    let declared = declared_release();
    let path = repo_root().join(".env.example");
    let value = grobid_image_from_env_file(&path)
        .unwrap_or_else(|| panic!("{} must define GROBID_IMAGE", path.display()));
    check_pinned_to_declared(".env.example GROBID_IMAGE", &value, &declared);
}

#[test]
fn compose_default_pins_the_declared_release() {
    let declared = declared_release();
    let path = repo_root().join(".docker/docker-compose.yml");
    let value = compose_default_image(&path)
        .unwrap_or_else(|| panic!("{} must default GROBID_IMAGE", path.display()));
    check_pinned_to_declared("compose default image", &value, &declared);
}

#[test]
fn local_env_pins_the_declared_release_when_present() {
    // `.env` is gitignored, so a fresh checkout has none. When it does exist it is
    // what the local stack actually runs, so it must not lag the declared release.
    let declared = declared_release();
    let path = repo_root().join(".env");
    match grobid_image_from_env_file(&path) {
        Some(value) => check_pinned_to_declared(".env GROBID_IMAGE", &value, &declared),
        None => eprintln!(
            "skipping: {} absent or defines no GROBID_IMAGE",
            path.display()
        ),
    }
}

#[test]
fn rollback_release_is_a_different_digest_from_production() {
    // Guards the failure this test was written for: production was silently left on the
    // rollback release. If the two ever became equal, rollback would be a no-op and the
    // bring-up check above would pass on the wrong image.
    let declared = declared_release();
    let override_text =
        std::fs::read_to_string(repo_root().join(".docker/grobid-release.override.yml"))
            .expect("release override readable");
    let rollback = override_text
        .lines()
        .find_map(|line| line.split("GROBID_IMAGE=grobid/grobid@").nth(1))
        .map(|rest| format!("sha256:{}", rest.split_whitespace().next().unwrap_or("")))
        .expect("override documents the rollback digest");
    assert_ne!(
        rollback, declared.digest,
        "the rollback release must not be the production release"
    );
    assert!(
        declared.version.starts_with("0.9."),
        "declared version should be a 0.9.x release, got {}",
        declared.version
    );
}

#[test]
fn the_validator_rejects_a_wrong_digest_and_the_rollback_digest() {
    // Polarity guard: the validator must actually fail on wrong input, otherwise every
    // assertion above would be vacuous.
    let declared = declared_release();
    let good = format!("{}@{}", declared.repository, declared.digest);

    // Sanity: the good value passes.
    check_pinned_to_declared("sanity", &good, &declared);

    // A wrong digest fails.
    let wrong = format!("{}@sha256:{}", declared.repository, "0".repeat(64));
    let outcome = std::panic::catch_unwind(|| {
        let declared = declared_release();
        check_pinned_to_declared("wrong digest", &wrong, &declared);
    });
    assert!(outcome.is_err(), "a wrong digest must be rejected");

    // The old tag form relative to a digest is also wrong: pinning to a tag must fail,
    // because a tag can be re-pointed upstream.
    let tag_form = format!("{}:{}", declared.repository, "0.9.0-crf");
    let outcome = std::panic::catch_unwind(|| {
        let declared = declared_release();
        check_pinned_to_declared("tag form", &tag_form, &declared);
    });
    assert!(
        outcome.is_err(),
        "a tag pin must be rejected in favour of a digest"
    );
}
