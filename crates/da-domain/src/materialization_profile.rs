//! Comparable, self-digested materialization scaling evidence.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterializationProfileCase {
    pub name: String,
    pub papers: u64,
    pub nodes: u64,
    pub edges: u64,
    pub mutations_attempted: u64,
    pub wall_seconds: f64,
    pub user_seconds: f64,
    pub system_seconds: f64,
    pub cpu_percent: f64,
    pub max_rss_kb: u64,
    pub seconds_per_1000_mutations: f64,
    pub semantic_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterializationScalingProfile {
    pub schema_version: String,
    pub source_revision: String,
    pub binary_sha256: String,
    pub cpu_count: u32,
    pub cpu_model: String,
    pub memory_total_kb: u64,
    pub cases: Vec<MaterializationProfileCase>,
    pub scaling_exponent: f64,
    pub projected_200_seconds: f64,
    pub import_eligible: bool,
    pub report_sha256: String,
}

impl MaterializationScalingProfile {
    pub fn seal(
        source_revision: String,
        binary_sha256: String,
        cpu_count: u32,
        cpu_model: String,
        memory_total_kb: u64,
        cases: Vec<MaterializationProfileCase>,
    ) -> Result<Self, String> {
        validate_cases(&cases)?;
        let first = cases.first().expect("validated nonempty cases");
        let last = cases.last().expect("validated nonempty cases");
        let scaling_exponent = (last.wall_seconds / first.wall_seconds).ln()
            / (last.papers as f64 / first.papers as f64).ln();
        let projected_200_seconds =
            last.wall_seconds * (200.0 / last.papers as f64).powf(scaling_exponent);
        let mut report = Self {
            schema_version: "materialization-scaling-profile.v1".into(),
            source_revision,
            binary_sha256,
            cpu_count,
            cpu_model,
            memory_total_kb,
            cases,
            scaling_exponent,
            projected_200_seconds,
            import_eligible: false,
            report_sha256: String::new(),
        };
        report.report_sha256 = report_digest(&report);
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "materialization-scaling-profile.v1" {
            return Err("materialization profile schema mismatch".into());
        }
        if self.import_eligible || self.cpu_count == 0 || self.memory_total_kb == 0 {
            return Err("materialization profile environment/D127 invalid".into());
        }
        if self.source_revision.trim().is_empty()
            || self.cpu_model.trim().is_empty()
            || self.binary_sha256.len() != 64
        {
            return Err("materialization profile provenance invalid".into());
        }
        validate_cases(&self.cases)?;
        if self.report_sha256 != report_digest(self) {
            return Err("materialization profile digest mismatch".into());
        }
        Ok(())
    }
}

fn validate_cases(cases: &[MaterializationProfileCase]) -> Result<(), String> {
    if cases.len() < 2 {
        return Err("materialization profile requires at least two scaling cases".into());
    }
    let mut previous = 0;
    for case in cases {
        if case.papers <= previous
            || case.nodes == 0
            || case.mutations_attempted == 0
            || case.wall_seconds <= 0.0
            || case.semantic_sha256.len() != 64
        {
            return Err(format!(
                "invalid or non-monotonic materialization case {}",
                case.name
            ));
        }
        previous = case.papers;
    }
    Ok(())
}

fn report_digest(report: &MaterializationScalingProfile) -> String {
    let mut clone = report.clone();
    clone.report_sha256.clear();
    let json = serde_json::to_vec(&clone).expect("profile must serialize");
    hex::encode(Sha256::digest(json))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(name: &str, papers: u64, wall: f64) -> MaterializationProfileCase {
        MaterializationProfileCase {
            name: name.into(),
            papers,
            nodes: papers,
            edges: papers,
            mutations_attempted: papers * 100,
            wall_seconds: wall,
            user_seconds: wall,
            system_seconds: 0.1,
            cpu_percent: 100.0,
            max_rss_kb: 1000,
            seconds_per_1000_mutations: wall / (papers as f64 * 100.0) * 1000.0,
            semantic_sha256: "a".repeat(64),
        }
    }

    #[test]
    fn profile_roundtrips_and_digest_binds_cases() {
        let report = MaterializationScalingProfile::seal(
            "rev".into(),
            "b".repeat(64),
            12,
            "cpu".into(),
            1024,
            vec![case("one", 1, 1.0), case("ten", 10, 20.0)],
        )
        .unwrap();
        let bytes = serde_json::to_vec(&report).unwrap();
        let mut roundtrip: MaterializationScalingProfile = serde_json::from_slice(&bytes).unwrap();
        assert!(roundtrip.validate().is_ok());
        roundtrip.cases[1].wall_seconds += 1.0;
        assert!(roundtrip.validate().is_err());
    }

    #[test]
    fn profile_rejects_non_monotonic_cases() {
        assert!(
            MaterializationScalingProfile::seal(
                "rev".into(),
                "b".repeat(64),
                12,
                "cpu".into(),
                1024,
                vec![case("ten", 10, 1.0), case("one", 1, 2.0)]
            )
            .is_err()
        );
    }
}
