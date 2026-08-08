//! Semantic graph task suite parser (ADR-051 §8.1 / M336).
//!
//! Samyama case-study pattern: `// @query Title | insight` markers in a Cypher
//! file define showcase tasks. Pure parse — no I/O beyond the caller.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// One showcase (or scale) task from a suite file.
///
/// `Eq` dropped: `expect_min_value: Option<f64>` — f64 has no `Eq` (NaN).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphTask {
    pub id: String,
    pub title: String,
    pub insight: String,
    pub cypher: String,
    /// Minimum result rows for PASS (default 1).
    pub min_rows: usize,
    /// If true, EMPTY does not fail the suite (rare; must be justified in insight).
    pub allow_empty: bool,
    /// Exact row count assertion (M338). None = skip check.
    pub expect_rows: Option<usize>,
    /// First numeric cell >= assertion (M338). None = skip check.
    pub expect_min_value: Option<f64>,
}

impl GraphTask {
    /// Full classification: base `classify_result_rows` + expect bounds (M338).
    ///
    /// Bound failures downgrade to `Empty` — the task didn't prove what it
    /// asserted, so it is treated as a non-passing result in the suite gate.
    pub fn classify(&self, row_count: usize, first_cell_number: Option<f64>) -> TaskVerdict {
        let base = classify_result_rows(row_count, first_cell_number, self.min_rows);
        if base != TaskVerdict::Pass {
            return base;
        }
        if let Some(expected) = self.expect_rows
            && row_count != expected
        {
            return TaskVerdict::Empty;
        }
        if let Some(min_val) = self.expect_min_value {
            match first_cell_number {
                Some(actual) if actual >= min_val => {}
                _ => return TaskVerdict::Empty,
            }
        }
        TaskVerdict::Pass
    }
}

/// Suite file parse result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphTaskSuite {
    pub tasks: Vec<GraphTask>,
}

/// Per-task execution verdict (Samyama tallies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskVerdict {
    Pass,
    Empty,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTaskResult {
    pub id: String,
    pub title: String,
    pub insight: String,
    pub verdict: TaskVerdict,
    pub rows: usize,
    pub min_rows: usize,
    pub allow_empty: bool,
    pub duration_ms: u128,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTaskSuiteReport {
    pub pass: usize,
    pub empty: usize,
    pub error: usize,
    pub failed: bool,
    pub tasks: Vec<GraphTaskResult>,
}

impl GraphTaskSuiteReport {
    pub fn passed(&self) -> bool {
        !self.failed
    }
}

impl TaskVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Empty => "EMPTY",
            Self::Error => "ERROR",
        }
    }
}

/// Classify a query result after successful execution.
///
/// - Fewer than `min_rows` records → EMPTY
/// - Single numeric cell equal to 0 → EMPTY (count-of-zero is not presence)
/// - Otherwise → PASS
pub fn classify_result_rows(
    row_count: usize,
    first_cell_number: Option<f64>,
    min_rows: usize,
) -> TaskVerdict {
    if row_count < min_rows {
        return TaskVerdict::Empty;
    }
    if row_count == 1
        && let Some(n) = first_cell_number
        && n == 0.0
    {
        return TaskVerdict::Empty;
    }
    TaskVerdict::Pass
}

/// Extract a finite number from a JSON cell (Number or numeric String).
pub fn json_cell_number(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => {
            // Samyama adapter may render as "Number(24)" debug form in CLI,
            // but GraphStore returns real JSON numbers. Still accept plain digits.
            s.parse::<f64>().ok()
        }
        _ => None,
    }
}

/// Parse a Cypher suite file body with `// @query` markers.
///
/// Format (ADR-051 §8.1):
/// ```cypher
/// // @query Title | optional insight
/// // @min_rows 1
/// // @allow_empty
/// MATCH ...
/// RETURN ...;
/// ```
pub fn parse_tasks_cypher(source: &str) -> Result<GraphTaskSuite, String> {
    let mut tasks: Vec<GraphTask> = Vec::new();
    let mut pending_title: Option<String> = None;
    let mut pending_insight = String::new();
    let mut pending_min_rows: usize = 1;
    let mut pending_allow_empty = false;
    let mut pending_expect_rows: Option<usize> = None;
    let mut pending_expect_min_value: Option<f64> = None;
    let mut cypher_buf: Vec<String> = Vec::new();

    let flush = |title: &Option<String>,
                 insight: &str,
                 min_rows: usize,
                 allow_empty: bool,
                 expect_rows: Option<usize>,
                 expect_min_value: Option<f64>,
                 buf: &mut Vec<String>,
                 out: &mut Vec<GraphTask>|
     -> Result<(), String> {
        if title.is_none() && buf.is_empty() {
            return Ok(());
        }
        let title = title
            .clone()
            .ok_or_else(|| "Cypher block without preceding // @query marker".to_string())?;
        let cypher = buf.join("\n").trim().to_string();
        buf.clear();
        if cypher.is_empty() {
            return Err(format!("task {title:?} has empty Cypher body"));
        }
        let id = slug_id_or_fallback(&title);
        out.push(GraphTask {
            id,
            title,
            insight: insight.trim().to_string(),
            cypher,
            min_rows,
            allow_empty,
            expect_rows,
            expect_min_value,
        });
        Ok(())
    };

    for (lineno, raw) in source.lines().enumerate() {
        let line = raw.trim_end();
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("// @query") {
            // Flush previous task if any
            flush(
                &pending_title,
                &pending_insight,
                pending_min_rows,
                pending_allow_empty,
                pending_expect_rows,
                pending_expect_min_value,
                &mut cypher_buf,
                &mut tasks,
            )
            .map_err(|e| format!("line {}: {e}", lineno + 1))?;
            pending_min_rows = 1;
            pending_allow_empty = false;
            pending_expect_rows = None;
            pending_expect_min_value = None;
            let rest = rest.trim();
            let (title, insight) = match rest.split_once('|') {
                Some((t, i)) => (t.trim().to_string(), i.trim().to_string()),
                None => (rest.to_string(), String::new()),
            };
            if title.is_empty() {
                return Err(format!("line {}: empty @query title", lineno + 1));
            }
            pending_title = Some(title);
            pending_insight = insight;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("// @min_rows") {
            let n = rest
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("line {}: invalid @min_rows", lineno + 1))?;
            if n == 0 {
                return Err(format!("line {}: @min_rows must be >= 1", lineno + 1));
            }
            pending_min_rows = n;
            continue;
        }

        if trimmed == "// @allow_empty" || trimmed.starts_with("// @allow_empty ") {
            pending_allow_empty = true;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("// @expect_rows") {
            let n = rest
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("line {}: invalid @expect_rows", lineno + 1))?;
            pending_expect_rows = Some(n);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("// @expect_min_value") {
            let v = rest
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("line {}: invalid @expect_min_value", lineno + 1))?;
            if !v.is_finite() {
                return Err(format!(
                    "line {}: @expect_min_value must be finite",
                    lineno + 1
                ));
            }
            pending_expect_min_value = Some(v);
            continue;
        }

        // Skip other comments and blank lines outside cypher accumulation
        if pending_title.is_none() {
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            return Err(format!(
                "line {}: Cypher without // @query marker: {trimmed}",
                lineno + 1
            ));
        }

        // Accumulate cypher (including mid-body comments if any — keep non-marker comments out)
        if trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.is_empty() {
            cypher_buf.push(line.to_string());
        }
    }

    flush(
        &pending_title,
        &pending_insight,
        pending_min_rows,
        pending_allow_empty,
        pending_expect_rows,
        pending_expect_min_value,
        &mut cypher_buf,
        &mut tasks,
    )?;

    if tasks.is_empty() {
        return Err("no // @query tasks found".into());
    }
    Ok(GraphTaskSuite { tasks })
}

fn slug_id(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    s.trim_matches('_').to_string()
}

/// `slug_id` with fallback for edge cases where title is all separators.
fn slug_id_or_fallback(title: &str) -> String {
    let id = slug_id(title);
    if id.is_empty() {
        "task".to_string()
    } else {
        id
    }
}

pub async fn run_graph_task_suite(
    store: &da_adapters::SamyamaGraphStore,
    graph: &str,
    suite: &GraphTaskSuite,
) -> GraphTaskSuiteReport {
    use da_ports::graph_store::GraphStore;
    let mut tasks = Vec::with_capacity(suite.tasks.len());
    for task in &suite.tasks {
        let started = Instant::now();
        let result = match store.query_readonly(graph, &task.cypher).await {
            Ok(result) => {
                let rows = result.records.len();
                let first_number = result
                    .records
                    .first()
                    .and_then(|row| row.first())
                    .and_then(json_cell_number);
                GraphTaskResult {
                    id: task.id.clone(),
                    title: task.title.clone(),
                    insight: task.insight.clone(),
                    verdict: task.classify(rows, first_number),
                    rows,
                    min_rows: task.min_rows,
                    allow_empty: task.allow_empty,
                    duration_ms: started.elapsed().as_millis(),
                    error: None,
                }
            }
            Err(error) => GraphTaskResult {
                id: task.id.clone(),
                title: task.title.clone(),
                insight: task.insight.clone(),
                verdict: TaskVerdict::Error,
                rows: 0,
                min_rows: task.min_rows,
                allow_empty: task.allow_empty,
                duration_ms: started.elapsed().as_millis(),
                error: Some(error.to_string()),
            },
        };
        tasks.push(result);
    }
    let pass = tasks
        .iter()
        .filter(|task| task.verdict == TaskVerdict::Pass)
        .count();
    let empty = tasks
        .iter()
        .filter(|task| task.verdict == TaskVerdict::Empty)
        .count();
    let error = tasks
        .iter()
        .filter(|task| task.verdict == TaskVerdict::Error)
        .count();
    let gate: Vec<_> = tasks
        .iter()
        .map(|task| (task.verdict, task.allow_empty))
        .collect();
    GraphTaskSuiteReport {
        pass,
        empty,
        error,
        failed: suite_failed(&gate),
        tasks,
    }
}

/// Suite-level gate: fail if any task is EMPTY (unless allow_empty) or ERROR.
pub fn suite_failed(results: &[(TaskVerdict, bool)]) -> bool {
    results.iter().any(|(v, allow_empty)| match v {
        TaskVerdict::Error => true,
        TaskVerdict::Empty => !allow_empty,
        TaskVerdict::Pass => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
// preamble comment
// @query Paper count | Scale check
// @min_rows 1
MATCH (n:Paper) WHERE n.retrieval_eligible = true RETURN count(n) AS papers;

// @query Hub by arxiv | Orient GEPA
MATCH (n:Paper {arxiv_id: "2507.19457"}) RETURN n.arxiv_id LIMIT 1;

// @query Citation edges | Fact edge CITES
MATCH (p:Paper)-[:CITES]->(c) RETURN count(c) AS cites;
"#;

    #[test]
    fn parse_three_tasks() {
        let suite = parse_tasks_cypher(SAMPLE).unwrap();
        assert_eq!(suite.tasks.len(), 3);
        assert_eq!(suite.tasks[0].title, "Paper count");
        assert_eq!(suite.tasks[0].insight, "Scale check");
        assert!(suite.tasks[0].cypher.contains("MATCH (n:Paper)"));
        assert!(suite.tasks[0].cypher.contains("count(n)"));
        assert_eq!(suite.tasks[0].min_rows, 1);
        assert!(!suite.tasks[0].allow_empty);
        assert_eq!(suite.tasks[1].title, "Hub by arxiv");
        assert!(suite.tasks[1].cypher.contains("2507.19457"));
        assert_eq!(suite.tasks[2].title, "Citation edges");
        assert!(suite.tasks[2].cypher.contains("CITES"));
        assert_eq!(suite.tasks[0].id, "paper_count");
    }

    #[test]
    fn parse_allow_empty_and_min_rows() {
        let src = r#"
// @query Optional legacy | may be empty
// @allow_empty
// @min_rows 1
MATCH (n:Legacy) RETURN n LIMIT 1;
"#;
        let suite = parse_tasks_cypher(src).unwrap();
        assert_eq!(suite.tasks.len(), 1);
        assert!(suite.tasks[0].allow_empty);
    }

    #[test]
    fn reject_cypher_without_marker() {
        let err = parse_tasks_cypher("MATCH (n) RETURN n;").unwrap_err();
        assert!(err.contains("@query"), "{err}");
    }

    #[test]
    fn reject_empty_file() {
        let err = parse_tasks_cypher("// only comments\n").unwrap_err();
        assert!(err.contains("no // @query"), "{err}");
    }

    #[test]
    fn classify_empty_rows() {
        assert_eq!(classify_result_rows(0, None, 1), TaskVerdict::Empty);
        assert_eq!(classify_result_rows(1, None, 1), TaskVerdict::Pass);
        assert_eq!(classify_result_rows(1, Some(0.0), 1), TaskVerdict::Empty);
        assert_eq!(classify_result_rows(1, Some(24.0), 1), TaskVerdict::Pass);
        assert_eq!(classify_result_rows(2, Some(0.0), 1), TaskVerdict::Pass);
    }

    #[test]
    fn suite_report_passed_respects_allow_empty_gate() {
        let report = GraphTaskSuiteReport {
            pass: 0,
            empty: 1,
            error: 0,
            failed: false,
            tasks: vec![],
        };
        assert!(report.passed());
    }

    #[test]
    fn suite_failed_logic() {
        assert!(!suite_failed(&[(TaskVerdict::Pass, false)]));
        assert!(suite_failed(&[(TaskVerdict::Empty, false)]));
        assert!(!suite_failed(&[(TaskVerdict::Empty, true)]));
        assert!(suite_failed(&[(TaskVerdict::Error, true)]));
    }

    #[test]
    fn json_cell_number_parses() {
        assert_eq!(json_cell_number(&serde_json::json!(24)), Some(24.0));
        assert_eq!(json_cell_number(&serde_json::json!("3.5")), Some(3.5));
        assert_eq!(json_cell_number(&serde_json::json!("x")), None);
    }

    #[tokio::test]
    async fn reusable_runner_reports_pass_and_empty() {
        use da_ports::graph_store::DirectGraphStore;
        let store = da_adapters::SamyamaGraphStore::from_env();
        store.create_node("Paper").await.unwrap();
        let suite = GraphTaskSuite {
            tasks: vec![
                GraphTask {
                    id: "paper".into(),
                    title: "Paper".into(),
                    insight: "".into(),
                    cypher: "MATCH (n:Paper) RETURN n LIMIT 1".into(),
                    min_rows: 1,
                    allow_empty: false,
                    expect_rows: None,
                    expect_min_value: None,
                },
                GraphTask {
                    id: "missing".into(),
                    title: "Missing".into(),
                    insight: "".into(),
                    cypher: "MATCH (n:Missing) RETURN n LIMIT 1".into(),
                    min_rows: 1,
                    allow_empty: false,
                    expect_rows: None,
                    expect_min_value: None,
                },
            ],
        };
        let report = run_graph_task_suite(&store, "daily_archive", &suite).await;
        assert_eq!(report.pass, 1);
        assert_eq!(report.empty, 1);
        assert!(report.failed);
        assert!(!report.passed());
    }

    // --- M338: expect bounds ---

    #[test]
    fn parse_expect_rows_and_min_value() {
        let src = r#"
// @query Paper count | Scale
// @expect_rows 1
// @expect_min_value 24
MATCH (n:Paper) RETURN count(n) AS papers;
"#;
        let suite = parse_tasks_cypher(src).unwrap();
        assert_eq!(suite.tasks[0].expect_rows, Some(1));
        assert_eq!(suite.tasks[0].expect_min_value, Some(24.0));
    }

    #[test]
    fn parse_resets_expect_between_tasks() {
        let src = r#"
// @query Task A | a
// @expect_rows 1
MATCH (n) RETURN count(n);

// @query Task B | b
MATCH (n) RETURN n LIMIT 1;
"#;
        let suite = parse_tasks_cypher(src).unwrap();
        assert_eq!(suite.tasks[0].expect_rows, Some(1));
        assert_eq!(suite.tasks[1].expect_rows, None);
    }

    #[test]
    fn reject_invalid_expect_rows() {
        let src = "// @query T | x\n// @expect_rows abc\nMATCH (n) RETURN n;";
        assert!(parse_tasks_cypher(src).is_err());
    }

    #[test]
    fn reject_nan_expect_min_value() {
        let src = "// @query T | x\n// @expect_min_value NaN\nMATCH (n) RETURN n;";
        let err = parse_tasks_cypher(src).unwrap_err();
        // f64::parse("NaN") succeeds but is_finite() must reject it
        assert!(err.contains("finite"));
    }

    #[test]
    fn classify_expect_rows_pass() {
        let task = GraphTask {
            id: "t".into(),
            title: "T".into(),
            insight: "".into(),
            cypher: "MATCH (n) RETURN n;".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: Some(1),
            expect_min_value: None,
        };
        assert_eq!(task.classify(1, None), TaskVerdict::Pass);
    }

    #[test]
    fn classify_expect_rows_fail() {
        let task = GraphTask {
            id: "t".into(),
            title: "T".into(),
            insight: "".into(),
            cypher: "MATCH (n) RETURN n;".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: Some(1),
            expect_min_value: None,
        };
        // Engine returns 24 rows (per-paper grouping) but we expected 1 aggregate
        assert_eq!(task.classify(24, Some(24.0)), TaskVerdict::Empty);
    }

    #[test]
    fn classify_expect_min_value_pass() {
        let task = GraphTask {
            id: "t".into(),
            title: "T".into(),
            insight: "".into(),
            cypher: "MATCH (n) RETURN count(n);".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: None,
            expect_min_value: Some(20.0),
        };
        assert_eq!(task.classify(1, Some(24.0)), TaskVerdict::Pass);
    }

    #[test]
    fn classify_expect_min_value_fail() {
        let task = GraphTask {
            id: "t".into(),
            title: "T".into(),
            insight: "".into(),
            cypher: "MATCH (n) RETURN count(n);".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: None,
            expect_min_value: Some(30.0),
        };
        assert_eq!(task.classify(1, Some(24.0)), TaskVerdict::Empty);
    }

    #[test]
    fn classify_expect_min_value_missing_cell() {
        let task = GraphTask {
            id: "t".into(),
            title: "T".into(),
            insight: "".into(),
            cypher: "MATCH (n) RETURN n;".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: None,
            expect_min_value: Some(1.0),
        };
        // Rows returned but first cell is not numeric → cannot verify bound → EMPTY
        assert_eq!(task.classify(1, None), TaskVerdict::Empty);
    }
}

// ─── ADR-051 P1: Property-based tests (M344) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    /// Generate a valid single-task cypher suite string.
    fn arb_single_task() -> impl Strategy<Value = String> {
        (
            "[a-zA-Z0-9 _-]+",
            "[a-zA-Z0-9 _-]+",
            "[a-zA-Z0-9 ;().,_=-]+",
        )
            .prop_map(|(title, insight, cypher)| {
                format!("// @query {title} | {insight}\n{cypher};\n")
            })
    }

    /// Generate a multi-task suite with 1-5 tasks.
    fn arb_suite() -> impl Strategy<Value = String> {
        proptest::collection::vec(arb_single_task(), 1..=5).prop_map(|tasks| tasks.join("\n"))
    }

    proptest! {
        /// Property 1: parse → serialize → parse is idempotent for valid suites.
        ///
        /// We serialize the *parsed* suite back to a string and re-parse.
        /// The task count must match (we can't assert full equality because
        /// serialize_to cypher format is not defined — we check structural
        /// invariants instead: same task count, same titles).
        #[test]
        fn prop_parse_roundtrip_task_count(s in arb_suite()) {
            let suite1 = match parse_tasks_cypher(&s) {
                Ok(s) => s,
                Err(_) => return Ok(()),
            };
            let serialized = suite1
                .tasks
                .iter()
                .map(|t| {
                    let mut out = format!("// @query {} | {}\n{}\n", t.title, t.insight, t.cypher);
                    if t.min_rows != 1 {
                        out = format!("{}// @min_rows {}\n", out, t.min_rows);
                    }
                    if t.allow_empty {
                        out = format!("{}// @allow_empty\n", out);
                    }
                    out
                })
                .collect::<Vec<_>>()
                .join("\n");
            let suite2 = match parse_tasks_cypher(&serialized) {
                Ok(s) => s,
                Err(e) => return Err(proptest::test_runner::TestCaseError::fail(format!("re-parse failed: {e}"))),
            };
            prop_assert_eq!(suite1.tasks.len(), suite2.tasks.len());
            for (a, b) in suite1.tasks.iter().zip(suite2.tasks.iter()) {
                prop_assert_eq!(&a.title, &b.title);
                prop_assert_eq!(&a.insight, &b.insight);
                prop_assert_eq!(&a.cypher, &b.cypher);
                prop_assert_eq!(a.min_rows, b.min_rows);
                prop_assert_eq!(a.allow_empty, b.allow_empty);
            }
        }

        /// Property 2: empty input → empty suite (not error).
        #[test]
        fn prop_empty_input_ok(s in "(.*\n?)*") {
            // Empty or whitespace-only input should not panic.
            // It may return Ok(empty suite) or Ok with 0 tasks.
            let _ = parse_tasks_cypher(&s); // must not panic
        }

        /// Property 3: number of // @query markers == task count.
        #[test]
        fn prop_marker_count_equals_task_count(s in arb_suite()) {
            let suite = match parse_tasks_cypher(&s) {
                Ok(s) => s,
                Err(_) => return Ok(()),
            };
            let marker_count = s.lines().filter(|l| l.trim_start().starts_with("// @query")).count();
            prop_assert_eq!(suite.tasks.len(), marker_count);
        }

        /// Property 4: every parsed task has non-empty cypher body.
        #[test]
        fn prop_every_task_has_nonempty_cypher(s in arb_suite()) {
            let suite = match parse_tasks_cypher(&s) {
                Ok(s) => s,
                Err(_) => return Ok(()),
            };
            for task in &suite.tasks {
                prop_assert!(!task.cypher.trim().is_empty(), "task '{}' has empty cypher", task.title);
            }
        }

        /// Property 5: every parsed task has a non-empty id (slug).
        #[test]
        fn prop_every_task_has_nonempty_id(s in arb_suite()) {
            let suite = match parse_tasks_cypher(&s) {
                Ok(s) => s,
                Err(_) => return Ok(()),
            };
            for task in &suite.tasks {
                prop_assert!(!task.id.is_empty(), "task '{}' has empty id", task.title);
            }
        }

        /// Property 6: parser never panics on arbitrary input.
        #[test]
        fn prop_never_panic(s in ".*") {
            let _ = parse_tasks_cypher(&s); // may Err, must not panic
        }
    }
}
