//! Strict loader for diagnostic context conservation policy DATA.

use std::path::Path;

use da_domain::context_conservation::ContextConservationPolicy;

pub fn load_context_conservation_policy(path: &Path) -> Result<ContextConservationPolicy, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("stat context policy {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > 256 * 1024 {
        return Err(format!(
            "context policy {} is not a bounded file",
            path.display()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read context policy {}: {error}", path.display()))?;
    let policy: ContextConservationPolicy = serde_yaml::from_slice(&bytes)
        .map_err(|error| format!("parse context policy {}: {error}", path.display()))?;
    policy.validate()?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_policy_is_strict_and_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/ontology/context_conservation_policy.yaml");
        let policy = load_context_conservation_policy(&path).unwrap();
        assert_eq!(policy.minimum_section_grounding_bps, 9_500);
    }

    #[test]
    fn policy_bytes_have_stable_change_sensitive_digest() {
        use da_domain::assurance::ContentDigest;
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/ontology/context_conservation_policy.yaml");
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(
            ContentDigest::of_bytes(&bytes),
            ContentDigest::of_bytes(&bytes)
        );
        let mut changed = bytes.clone();
        changed.push(b'\n');
        assert_ne!(
            ContentDigest::of_bytes(&bytes),
            ContentDigest::of_bytes(&changed)
        );
    }

    #[test]
    fn rejects_unknown_and_out_of_range_policy() {
        let path = std::env::temp_dir().join(format!("context-policy-{}.yaml", std::process::id()));
        std::fs::write(&path, "schema_version: context-conservation-policy.v1\npolicy_id: test\nminimum_text_retention_bps: {a: 10001}\nminimum_section_grounding_bps: 0\nmaximum_navigation_noise_bps: 0\nnavigation_noise_titles: [references]\nunknown: true\n").unwrap();
        assert!(
            load_context_conservation_policy(&path)
                .unwrap_err()
                .contains("unknown field")
        );
        std::fs::write(&path, "schema_version: context-conservation-policy.v1\npolicy_id: test\nminimum_text_retention_bps: {a: 10001}\nminimum_section_grounding_bps: 0\nmaximum_navigation_noise_bps: 0\nnavigation_noise_titles: [references]\n").unwrap();
        assert!(load_context_conservation_policy(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
