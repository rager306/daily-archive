//! Unsafe product captures refuse before touching a source or journal.

#[test]
fn retired_pair_actions_refuse_without_running_a_parser() {
    for action in ["pair", "gold-pair"] {
        let dir = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_da"))
            .current_dir(dir.path())
            .env("LOG_DIR", dir.path().join("logs"))
            .args([
                "policy",
                action,
                "--pdf",
                "missing.pdf",
                "--paper-id",
                "untrusted",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!dir.path().join("data").exists());
        assert!(!dir.path().join("artifacts").exists());
    }
}
