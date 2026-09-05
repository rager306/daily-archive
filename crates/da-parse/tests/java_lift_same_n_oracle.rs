//! Java TeiLifter same-n vs independent header/refs denominators (Wave C).
//!
//! Expected counts are test-local constants copied from the independent
//! S09/S12 oracle (MEM996). TeiLifter is the SUT only. Native compose_header
//! / compose_references must not appear in this file's expected builder.
//! Header and citation populations stay separate: pooling is a hard failure.
//! Native stays shadow. Java remains production_route. No graph write.

use da_parse::TeiLifter;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

struct Fixture {
    name: &'static str,
    sha256: &'static str,
    header_raw: usize,
    bibl_struct: usize,
}

/// Independent denominators. Not derived from TeiLifter or native compose.
const FIXTURES: [Fixture; 3] = [
    Fixture {
        name: "hub_2507.19457.tei.xml",
        sha256: "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
        header_raw: 21,
        bibl_struct: 80,
    },
    Fixture {
        name: "rlm_2512.24601.tei.xml",
        sha256: "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
        header_raw: 4,
        bibl_struct: 49,
    },
    Fixture {
        name: "reca_2605.26525v1.tei.xml",
        sha256: "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
        header_raw: 11,
        bibl_struct: 78,
    },
];

/// Named native-accepted gap on hub only. Not Java truth. Not pooled.
const HUB_NATIVE_ACCEPTED_HEADER: usize = 20;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preserved")
        .join(name)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn java_lift_header_and_citations_match_independent_denominators() {
    let mut header_n = 0usize;
    let mut citation_n = 0usize;
    for fixture in FIXTURES {
        let bytes = fs::read(fixture_path(fixture.name)).unwrap();
        assert_eq!(
            digest(&bytes),
            fixture.sha256,
            "fixture checksum drift: {}",
            fixture.name
        );
        let xml = String::from_utf8(bytes).unwrap();
        let ir = TeiLifter::lift_tei(xml, fixture.name, "fixture");
        assert_eq!(
            ir.header_authors.len(),
            fixture.header_raw,
            "Java header_authors vs independent header_raw: {}",
            fixture.name
        );
        assert_eq!(
            ir.citations.len(),
            fixture.bibl_struct,
            "Java citations vs independent biblStruct: {}",
            fixture.name
        );
        header_n += ir.header_authors.len();
        citation_n += ir.citations.len();
        if fixture.name == "hub_2507.19457.tei.xml" {
            assert_ne!(
                ir.header_authors.len(),
                HUB_NATIVE_ACCEPTED_HEADER,
                "hub Java header_raw must not be replaced by native accepted=20"
            );
        }
    }
    assert_eq!(header_n, 21 + 4 + 11, "header population stays 36");
    assert_eq!(citation_n, 80 + 49 + 78, "citation population stays 207");
    assert_ne!(
        header_n, citation_n,
        "header_n and citation_n must remain distinct populations"
    );
    assert_ne!(
        header_n + citation_n,
        header_n,
        "pooling header into citations is forbidden"
    );
}

#[test]
fn expected_builder_does_not_import_native_compose() {
    let src = include_str!("java_lift_same_n_oracle.rs");
    let use_block: String = src
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect();
    for name in [
        "compose_header",
        "compose_references",
        "run_header_sequence",
        "run_reference_sequence",
    ] {
        assert!(
            !use_block.contains(name),
            "{name} must not be imported into the Java same-n expected builder"
        );
        assert!(
            !src.contains(&format!("{name}(")),
            "{name} must not be called from the Java same-n expected builder"
        );
    }
}
