//! Host Java-CPU accounting counter reader (M513-r7dg4z/S04/T01).
//!
//! The H05/H19 measurement harness needs the GROBID sidecar's CPU total. The
//! seam already existed inside the example, but it only understood one file
//! shape: a bare integer. On a cgroup v2 host the sidecar's counter lives in
//! `cpu.stat` as `usage_usec <n>` among other key/value lines, where a bare
//! `parse::<u64>()` fails — so the live measurement was physically impossible.
//! This module owns the host/cgroup convention.
//!
//! It lives in `da-cli` (infrastructure), not `da-parse`: probing a cgroup file
//! is host plumbing, not parsing, and `da-parse` must not grow a host
//! dependency (onion lock). Nothing here writes a graph, opens a network
//! connection, or claims a production route; `import_eligible` stays false.
//!
//! Fail-closed contract: a missing, unreadable, non-numeric, or `usage_usec`-less
//! path yields `None` — a typed absence, never a fabricated `0`. A genuine
//! counter reading of zero is returned as `Some(0)`; the decision that "a zero
//! value or a zero delta is not a measurement" belongs to the harness layer
//! (`zero_is_not_a_measurement`, `JAVA_CPU_NO_DELTA_REASON`).

use std::path::{Path, PathBuf};

/// Microseconds-to-nanoseconds scale for the cgroup v2 `usage_usec` counter.
const NANOS_PER_MICRO: u64 = 1_000;

/// Read a Java CPU accounting counter and return nanoseconds, or `None` when
/// the path carries no usable counter.
///
/// Accepted shapes:
/// * a directory — `<dir>/cpu.stat` is read (cgroup v2 convention);
/// * a file whose trimmed content is a bare unsigned integer — already
///   nanoseconds, preserving the original seam contract;
/// * a cgroup v2 `cpu.stat` body — the first `usage_usec <n>` line, scaled to
///   nanoseconds, robust to line order, indentation and tabs.
///
/// A directory without `cpu.stat`, a missing path, an unreadable or non-UTF-8
/// file, non-numeric content, and content without a `usage_usec` line all
/// return `None`.
pub fn read_java_cpu_ns(path: &Path) -> Option<u64> {
    let counter_file = resolve_counter_file(path)?;
    let text = std::fs::read_to_string(counter_file).ok()?;
    parse_java_cpu_ns(&text)
}

/// Resolve the declared path to the file that actually carries the counter.
///
/// A directory means the cgroup v2 convention, so `<dir>/cpu.stat` is used; a
/// directory without `cpu.stat` is a typed absence rather than an error.
fn resolve_counter_file(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        let cpu_stat = path.join("cpu.stat");
        return cpu_stat.is_file().then_some(cpu_stat);
    }
    path.is_file().then(|| path.to_path_buf())
}

/// Parse a counter body into nanoseconds, or `None` when it is not a counter.
///
/// Order matters: an all-integer body is read as nanoseconds first, so a file
/// that happens to be a bare number keeps the original contract; anything else
/// is treated as cgroup v2 key/value text. Only the first `usage_usec` line is
/// consulted, and an unparseable or overflowing `usage_usec` value is a typed
/// absence rather than a silently skipped line.
fn parse_java_cpu_ns(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(nanos) = trimmed.parse::<u64>() {
        return Some(nanos);
    }
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() != Some("usage_usec") {
            continue;
        }
        let micros: u64 = fields.next()?.parse().ok()?;
        return micros.checked_mul(NANOS_PER_MICRO);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Unique scratch directory per test, torn down when the guard drops.
    fn scratch() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("da-java-cpu-counter-")
            .tempdir()
            .expect("scratch dir")
    }

    #[test]
    fn reads_bare_integer_as_nanoseconds() {
        let dir = scratch();
        let path = dir.path().join("counter");
        fs::write(&path, "1234567890\n").expect("write counter");
        assert_eq!(read_java_cpu_ns(&path), Some(1_234_567_890));
    }

    #[test]
    fn reads_usage_usec_among_other_cpu_stat_lines() {
        let dir = scratch();
        let path = dir.path().join("cpu.stat");
        fs::write(
            &path,
            "nr_periods 0\nusage_usec 4200000\nuser_usec 3000000\nsystem_usec 1200000\n",
        )
        .expect("write cpu.stat");
        assert_eq!(read_java_cpu_ns(&path), Some(4_200_000_000));
    }

    #[test]
    fn reads_cpu_stat_through_a_directory_argument() {
        let dir = scratch();
        let cgroup = dir.path().join("system.slice/docker-deadbeef.scope");
        fs::create_dir_all(&cgroup).expect("create cgroup dir");
        fs::write(cgroup.join("cpu.stat"), "usage_usec 17\n").expect("write cpu.stat");
        assert_eq!(read_java_cpu_ns(&cgroup), Some(17_000));
    }

    #[test]
    fn tolerates_leading_whitespace_and_tabs() {
        let dir = scratch();
        let path = dir.path().join("cpu.stat");
        fs::write(&path, "\tnr_periods\t0\n   usage_usec \t  250  \n").expect("write cpu.stat");
        assert_eq!(read_java_cpu_ns(&path), Some(250_000));
    }

    #[test]
    fn missing_path_is_a_typed_absence() {
        let dir = scratch();
        let path = dir.path().join("does-not-exist");
        assert_eq!(read_java_cpu_ns(&path), None);
    }

    #[test]
    fn directory_without_cpu_stat_is_a_typed_absence() {
        let dir = scratch();
        let nested = dir.path().join("empty-cgroup");
        fs::create_dir_all(&nested).expect("create dir");
        assert_eq!(read_java_cpu_ns(&nested), None);
    }

    #[test]
    fn non_numeric_text_without_usage_usec_is_a_typed_absence() {
        let dir = scratch();
        let path = dir.path().join("cpu.stat");
        fs::write(&path, "nr_periods 0\nuser_usec 5\n").expect("write cpu.stat");
        assert_eq!(read_java_cpu_ns(&path), None);
    }

    #[test]
    fn usage_usec_zero_is_a_real_reading_not_an_absence() {
        let dir = scratch();
        let path = dir.path().join("cpu.stat");
        fs::write(&path, "nr_periods 0\nusage_usec 0\n").expect("write cpu.stat");
        assert_eq!(read_java_cpu_ns(&path), Some(0));
    }

    #[test]
    fn value_beyond_32_bits_survives() {
        let dir = scratch();
        let bare = dir.path().join("bare");
        fs::write(&bare, "4294967296\n").expect("write bare");
        assert_eq!(read_java_cpu_ns(&bare), Some(u64::from(u32::MAX) + 1));

        let stat = dir.path().join("cpu.stat");
        fs::write(&stat, "usage_usec 4294967296\n").expect("write cpu.stat");
        assert_eq!(
            read_java_cpu_ns(&stat),
            Some((u64::from(u32::MAX) + 1) * NANOS_PER_MICRO)
        );
    }

    #[test]
    fn overflowing_usage_usec_is_a_typed_absence() {
        let dir = scratch();
        let path = dir.path().join("cpu.stat");
        fs::write(&path, format!("usage_usec {}\n", u64::MAX)).expect("write cpu.stat");
        assert_eq!(read_java_cpu_ns(&path), None);
    }
}
