//! Coverage corpus harness.
//!
//! Runs `typst-prose source` and `typst-prose compiled` on every fixture under
//! `fixtures/corpus`, then checks the expectations in `fixtures/corpus/manifest.toml`.
//! Tokens are Unicode word boundaries (UAX 29) compared exactly. Writes a markdown
//! report to `target/coverage/report.md`.
//!
//! Outcomes:
//! - pass: the expectation holds.
//! - known: the expectation does not hold, and the manifest documents a limitation.
//! - regression: the expectation does not hold, with no documented limitation.
//! - unexpected-pass: a documented limitation whose expectation now holds. Remove the
//!   limitation from the manifest.
//! - fixture-error: the fixture is missing, does not compile, or the binary failed.
//! - invalid: the manifest or corpus layout is inconsistent (reported separately from outcomes).
//!
//! Only regression, unexpected-pass, fixture-error and manifest or layout problems fail the test.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use unicode_segmentation::UnicodeSegmentation;

const CORPUS_DIR: &str = "fixtures/corpus";
const REPORT_PATH: &str = "target/coverage/report.md";

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    fixture: Vec<FixtureEntry>,
    #[serde(default)]
    support: Vec<FixtureEntry>,
    #[serde(default)]
    sentinel: Vec<Sentinel>,
    #[serde(default)]
    exact: Vec<Exact>,
}

#[derive(Deserialize)]
struct FixtureEntry {
    path: String,
    #[allow(dead_code)]
    description: String,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Markup,
    String,
    Generated,
    NonProse,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Markup => "markup",
            Kind::String => "string",
            Kind::Generated => "generated",
            Kind::NonProse => "non-prose",
        }
    }
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Source,
    Compiled,
}

impl Mode {
    const ALL: [Mode; 2] = [Mode::Source, Mode::Compiled];

    fn arg(self) -> &'static str {
        match self {
            Mode::Source => "source",
            Mode::Compiled => "compiled",
        }
    }
}

#[derive(Deserialize)]
struct Sentinel {
    fixture: String,
    token: String,
    category: String,
    kind: Kind,
    context: String,
    rendered: bool,
    source: bool,
    source_count: Option<usize>,
    compiled: bool,
    compiled_count: Option<usize>,
    reason: String,
    known_source: Option<String>,
    known_compiled: Option<String>,
}

impl Sentinel {
    fn expected(&self, mode: Mode) -> (bool, Option<usize>) {
        match mode {
            Mode::Source => (self.source, self.source_count),
            Mode::Compiled => (self.compiled, self.compiled_count),
        }
    }

    fn known(&self, mode: Mode) -> Option<&str> {
        match mode {
            Mode::Source => self.known_source.as_deref(),
            Mode::Compiled => self.known_compiled.as_deref(),
        }
    }
}

#[derive(Deserialize)]
struct Exact {
    fixture: String,
    mode: Mode,
    #[serde(default)]
    args: Vec<String>,
    expected: String,
    reason: String,
    known: Option<String>,
}

impl Exact {
    /// The mode and its arguments, as shown in the report.
    fn label(&self) -> String {
        if self.args.is_empty() {
            self.mode.arg().to_string()
        } else {
            format!("{} {}", self.mode.arg(), self.args.join(" "))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Pass,
    Known,
    Regression,
    UnexpectedPass,
    FixtureError,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Known => "known",
            Outcome::Regression => "regression",
            Outcome::UnexpectedPass => "unexpected-pass",
            Outcome::FixtureError => "fixture-error",
        }
    }

    fn fails(self) -> bool {
        matches!(self, Outcome::Regression | Outcome::UnexpectedPass | Outcome::FixtureError)
    }
}

/// Result of one sentinel check in one mode.
struct SentinelCheck {
    fixture: String,
    mode: Mode,
    token: String,
    category: String,
    kind: Kind,
    context: String,
    expected_present: bool,
    expected_count: Option<usize>,
    observed_count: usize,
    outcome: Outcome,
    detail: String,
    reason: String,
    known: Option<String>,
}

/// Result of one exact-text check in one mode.
struct ExactCheck {
    fixture: String,
    mode: Mode,
    label: String,
    outcome: Outcome,
    expected: String,
    observed: String,
    detail: String,
    reason: String,
    known: Option<String>,
}

/// Output of one fixture run in one mode.
type RunResult = Result<String, String>;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS_DIR)
}

fn fixture_id(path: &str) -> String {
    path.strip_suffix(".typ").unwrap_or(path).to_string()
}

/// Unicode word boundaries, compared exactly. "CallerForest" is one token, not "Forest".
fn tokens(text: &str) -> Vec<&str> {
    text.unicode_words().collect()
}

fn count_token(text: &str, token: &str) -> usize {
    tokens(text).iter().filter(|t| **t == token).count()
}

fn run_binary(mode: Mode, path: &Path, args: &[String]) -> RunResult {
    let output = Command::new(env!("CARGO_BIN_EXE_typst-prose"))
        .arg(mode.arg())
        .args(args)
        .arg(path)
        .output()
        .map_err(|e| format!("could not start binary: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim_end().to_string();
    if !output.status.success() {
        return Err(format!("exit status {}; stderr:\n{stderr}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|_| "stdout is not UTF-8".to_string())
}

/// The entry file plus the `support` files beside it, which fixtures may import.
fn fixture_sources(root: &Path, path: &str) -> String {
    let entry = root.join(path);
    let mut text = fs::read_to_string(&entry).unwrap_or_default();
    let mut support = Vec::new();
    collect_typ_files(&entry.parent().unwrap().join("support"), &mut support);
    for file in support {
        text.push_str(&fs::read_to_string(file).unwrap_or_default());
    }
    text
}

fn collect_typ_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_typ_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "typ") {
            out.push(path);
        }
    }
}

/// Checks the manifest and the corpus layout. Returns messages for each inconsistency.
fn validate(manifest: &Manifest, root: &Path, sources: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    let fixture_ids: BTreeSet<String> = manifest.fixture.iter().map(|f| fixture_id(&f.path)).collect();

    let mut listed: BTreeSet<String> = BTreeSet::new();
    for entry in manifest.fixture.iter().chain(manifest.support.iter()) {
        if !listed.insert(entry.path.clone()) {
            problems.push(format!("{} is listed more than once", entry.path));
        }
        if !root.join(&entry.path).is_file() {
            problems.push(format!("{} is listed but does not exist", entry.path));
        }
    }

    let mut on_disk = Vec::new();
    collect_typ_files(root, &mut on_disk);
    for file in on_disk {
        let rel = file.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        if !listed.contains(&rel) {
            problems.push(format!("{rel} is not listed as a fixture or support file (silent skip)"));
        }
    }

    for s in &manifest.sentinel {
        let prefix = format!("sentinel {} in {}", s.token, s.fixture);
        if !fixture_ids.contains(&s.fixture) {
            problems.push(format!("{prefix}: unknown fixture"));
            continue;
        }
        let source = sources.get(&s.fixture).map(String::as_str).unwrap_or("");
        if !source.contains(&s.token) {
            problems.push(format!("{prefix}: token does not appear in the fixture source"));
        }
        if s.compiled && !s.rendered {
            problems.push(format!("{prefix}: compiled is true but rendered is false"));
        }
        if s.kind == Kind::Markup && !s.source {
            problems.push(format!("{prefix}: markup is always extracted in source mode"));
        }
        if s.kind != Kind::Markup && s.source {
            problems.push(format!("{prefix}: {} is not extracted in source mode", s.kind.label()));
        }
        if s.kind == Kind::Generated && !s.rendered {
            problems.push(format!("{prefix}: generated text must be rendered"));
        }
        for (present, count, name) in [(s.source, s.source_count, "source"), (s.compiled, s.compiled_count, "compiled")] {
            match (present, count) {
                (false, Some(_)) => problems.push(format!("{prefix}: {name}_count set but {name} is false")),
                (true, Some(0)) => problems.push(format!("{prefix}: {name}_count must be at least 1")),
                _ => {}
            }
        }
    }

    let mut per_fixture: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for s in &manifest.sentinel {
        if !per_fixture.entry(&s.fixture).or_default().insert(&s.token) {
            problems.push(format!("token {} is listed twice in {}", s.token, s.fixture));
        }
    }

    let mut exact_keys: BTreeSet<(String, Mode, Vec<String>)> = BTreeSet::new();
    for e in &manifest.exact {
        if !fixture_ids.contains(&e.fixture) {
            problems.push(format!("exact entry for unknown fixture {}", e.fixture));
        }
        if !exact_keys.insert((e.fixture.clone(), e.mode, e.args.clone())) {
            problems.push(format!("exact entry for {} {} is listed twice", e.fixture, e.label()));
        }
    }

    for id in &fixture_ids {
        let has_sentinels = manifest.sentinel.iter().any(|s| &s.fixture == id);
        let has_exact = manifest.exact.iter().any(|e| &e.fixture == id);
        if !has_sentinels && !has_exact {
            problems.push(format!("fixture {id} has no expectations"));
        }
    }
    problems
}

/// One sentinel check against one mode's output.
fn check_sentinel(s: &Sentinel, mode: Mode, run: &RunResult) -> SentinelCheck {
    let (expected_present, expected_count) = s.expected(mode);
    let known = s.known(mode).map(str::to_string);
    let (observed_count, outcome, detail) = match run {
        Err(e) => (0, Outcome::FixtureError, format!("fixture did not run: {e}")),
        Ok(output) => {
            let observed = count_token(output, &s.token);
            let presence_ok = (observed > 0) == expected_present;
            let count_ok = expected_count.is_none_or(|c| c == observed);
            let matches = presence_ok && count_ok;
            let expectation = match (expected_present, expected_count) {
                (false, _) => "absent".to_string(),
                (true, None) => "present".to_string(),
                (true, Some(c)) => format!("present, {c} occurrence(s)"),
            };
            let detail = format!("expected {expectation}, observed {observed} occurrence(s)");
            let outcome = match (matches, known.is_some()) {
                (true, false) => Outcome::Pass,
                (true, true) => Outcome::UnexpectedPass,
                (false, true) => Outcome::Known,
                (false, false) => Outcome::Regression,
            };
            (observed, outcome, if matches { String::new() } else { detail })
        }
    };
    SentinelCheck {
        fixture: s.fixture.clone(),
        mode,
        token: s.token.clone(),
        category: s.category.clone(),
        kind: s.kind,
        context: s.context.clone(),
        expected_present,
        expected_count,
        observed_count,
        outcome,
        detail,
        reason: s.reason.clone(),
        known,
    }
}

fn check_exact(e: &Exact, run: &RunResult) -> ExactCheck {
    let known = e.known.clone();
    let (outcome, observed, detail) = match run {
        Err(err) => (Outcome::FixtureError, String::new(), format!("fixture did not run: {err}")),
        Ok(output) => {
            let matches = *output == e.expected;
            let outcome = match (matches, known.is_some()) {
                (true, false) => Outcome::Pass,
                (true, true) => Outcome::UnexpectedPass,
                (false, true) => Outcome::Known,
                (false, false) => Outcome::Regression,
            };
            let detail = if matches { String::new() } else { "output differs from the expected text".to_string() };
            (outcome, output.clone(), detail)
        }
    };
    ExactCheck {
        fixture: e.fixture.clone(),
        mode: e.mode,
        label: e.label(),
        outcome,
        expected: e.expected.clone(),
        observed,
        detail,
        reason: e.reason.clone(),
        known,
    }
}

#[derive(Default)]
struct CategoryStats {
    source_expected: usize,
    source_recovered: usize,
    compiled_expected: usize,
    compiled_recovered: usize,
}

/// Markdown report generated only from the observed results.
fn render_report(
    fixtures: &[String],
    runs: &BTreeMap<(String, Mode), RunResult>,
    sentinel_checks: &[SentinelCheck],
    exact_checks: &[ExactCheck],
    problems: &[String],
) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# Extraction coverage report\n");
    let _ = writeln!(
        md,
        "Generated by `cargo test --test corpus` (via `scripts/coverage.sh`) from the observed output of each run. Do not edit by hand.\n"
    );

    let _ = writeln!(md, "## Summary\n");
    let _ = writeln!(md, "| Mode | Pass | Known limitation | Regression | Unexpected pass | Fixture error |");
    let _ = writeln!(md, "| --- | ---: | ---: | ---: | ---: | ---: |");
    for mode in Mode::ALL {
        let mut counts = BTreeMap::new();
        for outcome in sentinel_checks
            .iter()
            .filter(|c| c.mode == mode)
            .map(|c| c.outcome)
            .chain(exact_checks.iter().filter(|c| c.mode == mode).map(|c| c.outcome))
        {
            *counts.entry(outcome.label()).or_insert(0usize) += 1;
        }
        let get = |k: &str| counts.get(k).copied().unwrap_or(0);
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} |",
            mode.arg(),
            get("pass"),
            get("known"),
            get("regression"),
            get("unexpected-pass"),
            get("fixture-error")
        );
    }
    let _ = writeln!(md);
    let _ = writeln!(
        md,
        "Sentinel checks and exact-text checks are counted together per mode. Manifest and corpus layout problems are not mode-specific: {} found.\n",
        problems.len()
    );

    let _ = writeln!(md, "## Fixture runs\n");
    let _ = writeln!(md, "| Fixture | source | compiled |");
    let _ = writeln!(md, "| --- | --- | --- |");
    for f in fixtures {
        let cell = |mode: Mode| match runs.get(&(f.clone(), mode)) {
            Some(Ok(_)) => "ok".to_string(),
            Some(Err(_)) => "error".to_string(),
            None => "not run".to_string(),
        };
        let _ = writeln!(md, "| {f} | {} | {} |", cell(Mode::Source), cell(Mode::Compiled));
    }
    let _ = writeln!(md);

    let mut stats: Vec<(String, CategoryStats)> = Vec::new();
    for c in sentinel_checks {
        let idx = match stats.iter().position(|(name, _)| name == &c.category) {
            Some(i) => i,
            None => {
                stats.push((c.category.clone(), CategoryStats::default()));
                stats.len() - 1
            }
        };
        let s = &mut stats[idx].1;
        let recovered = c.observed_count > 0;
        match c.mode {
            Mode::Source if c.expected_present => {
                s.source_expected += 1;
                s.source_recovered += usize::from(recovered);
            }
            Mode::Compiled if c.expected_present => {
                s.compiled_expected += 1;
                s.compiled_recovered += usize::from(recovered);
            }
            _ => {}
        }
    }
    let _ = writeln!(md, "## Coverage by category\n");
    let _ = writeln!(md, "Recovered counts tokens that are expected present and observed present. Expected counts all tokens expected present in that mode.\n");
    let _ = writeln!(md, "| Category | Source recovered | Source expected | Compiled recovered | Compiled expected |");
    let _ = writeln!(md, "| --- | ---: | ---: | ---: | ---: |");
    let mut totals = CategoryStats::default();
    for (name, s) in &stats {
        let _ = writeln!(
            md,
            "| {name} | {} | {} | {} | {} |",
            s.source_recovered, s.source_expected, s.compiled_recovered, s.compiled_expected
        );
        totals.source_expected += s.source_expected;
        totals.source_recovered += s.source_recovered;
        totals.compiled_expected += s.compiled_expected;
        totals.compiled_recovered += s.compiled_recovered;
    }
    let _ = writeln!(
        md,
        "| **Total** | {} | {} | {} | {} |\n",
        totals.source_recovered, totals.source_expected, totals.compiled_recovered, totals.compiled_expected
    );

    let row = |c: &SentinelCheck| {
        format!("| {} | `{}` | {} | {} | {} |", c.mode.arg(), c.token, c.fixture, c.context, c.kind.label())
    };
    let sentinel_list = |title: &str, pred: &dyn Fn(&SentinelCheck) -> bool, out: &mut String| {
        let rows: Vec<&SentinelCheck> = sentinel_checks.iter().filter(|c| pred(c)).collect();
        let _ = writeln!(out, "<details><summary>{title} ({})</summary>\n", rows.len());
        if rows.is_empty() {
            let _ = writeln!(out, "None.\n");
        } else {
            let _ = writeln!(out, "| Mode | Token | Fixture | Context | Kind |");
            let _ = writeln!(out, "| --- | --- | --- | --- | --- |");
            for c in rows {
                let _ = writeln!(out, "{}", row(c));
            }
            let _ = writeln!(out);
        }
        let _ = writeln!(out, "</details>\n");
    };

    let _ = writeln!(md, "## Intended prose\n");
    sentinel_list(
        "Intended prose recovered",
        &|c| c.expected_present && c.observed_count > 0,
        &mut md,
    );
    sentinel_list(
        "Intended prose missed",
        &|c| c.expected_present && c.observed_count == 0,
        &mut md,
    );

    let _ = writeln!(md, "## Non-prose and duplicate output\n");
    sentinel_list(
        "Tokens included that the contract excludes",
        &|c| !c.expected_present && c.observed_count > 0,
        &mut md,
    );
    // A duplicate is output that is present but occurs a different number of times than
    // declared, or more than once when no count is declared. Absent tokens are missed, not duplicated.
    let dup_rows: Vec<&SentinelCheck> = sentinel_checks
        .iter()
        .filter(|c| c.observed_count > 0)
        .filter(|c| match c.expected_count {
            Some(expected) => expected != c.observed_count,
            None => c.observed_count > 1,
        })
        .collect();
    let _ = writeln!(md, "<details><summary>Duplicate occurrences ({})</summary>\n", dup_rows.len());
    if dup_rows.is_empty() {
        let _ = writeln!(md, "None.\n");
    } else {
        let _ = writeln!(md, "| Mode | Token | Fixture | Expected count | Observed count |");
        let _ = writeln!(md, "| --- | --- | --- | ---: | ---: |");
        for c in dup_rows {
            let expected = c.expected_count.map_or("any".to_string(), |n| n.to_string());
            let _ = writeln!(md, "| {} | `{}` | {} | {} | {} |", c.mode.arg(), c.token, c.fixture, expected, c.observed_count);
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "</details>\n");

    let _ = writeln!(md, "## Boundary and spacing (exact text)\n");
    let _ = writeln!(md, "<details><summary>Exact-text checks ({})</summary>\n", exact_checks.len());
    let _ = writeln!(md, "| Mode | Fixture | Outcome |");
    let _ = writeln!(md, "| --- | --- | --- |");
    for c in exact_checks {
        let _ = writeln!(md, "| {} | {} | {} |", c.label, c.fixture, c.outcome.label());
    }
    let _ = writeln!(md, "\n</details>\n");
    for c in exact_checks.iter().filter(|c| c.outcome != Outcome::Pass) {
        let _ = writeln!(md, "### {} ({}): {}\n", c.fixture, c.label, c.outcome.label());
        let _ = writeln!(md, "Reason: {}\n", c.reason);
        if let Some(k) = &c.known {
            let _ = writeln!(md, "Known limitation: {k}\n");
        }
        let _ = writeln!(md, "Expected:\n\n```text\n{}```\n", c.expected);
        if c.outcome != Outcome::FixtureError {
            let _ = writeln!(md, "Observed:\n\n```text\n{}```\n", c.observed);
        }
    }

    let _ = writeln!(md, "## Source-only, compiled-only and missed by both\n");
    let observed = |c: &SentinelCheck| c.observed_count > 0;
    let mut by_fixture: BTreeMap<(String, String), (bool, bool, bool)> = BTreeMap::new();
    for c in sentinel_checks {
        let entry = by_fixture
            .entry((c.fixture.clone(), c.token.clone()))
            .or_insert((false, false, false));
        match c.mode {
            Mode::Source => {
                entry.0 = observed(c);
                entry.2 |= c.expected_present;
            }
            Mode::Compiled => {
                entry.1 = observed(c);
                entry.2 |= c.expected_present;
            }
        }
    }
    let mut source_only = Vec::new();
    let mut compiled_only = Vec::new();
    let mut missed = Vec::new();
    for ((fixture, token), (src, comp, expected_somewhere)) in &by_fixture {
        match (src, comp) {
            (true, false) => source_only.push((fixture, token)),
            (false, true) => compiled_only.push((fixture, token)),
            (false, false) if *expected_somewhere => missed.push((fixture, token)),
            _ => {}
        }
    }
    for (title, rows) in [
        ("Recovered only by source", &source_only),
        ("Recovered only by compiled", &compiled_only),
        ("Missed by both modes", &missed),
    ] {
        let _ = writeln!(md, "<details><summary>{title} ({})</summary>\n", rows.len());
        if rows.is_empty() {
            let _ = writeln!(md, "None.\n");
        } else {
            let _ = writeln!(md, "| Token | Fixture |");
            let _ = writeln!(md, "| --- | --- |");
            for (fixture, token) in rows.iter().map(|(f, t)| (*f, *t)) {
                let _ = writeln!(md, "| `{token}` | {fixture} |");
            }
            let _ = writeln!(md);
        }
        let _ = writeln!(md, "</details>\n");
    }

    let _ = writeln!(md, "## Known limitations\n");
    let mut known: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in sentinel_checks.iter().filter(|c| c.outcome == Outcome::Known) {
        known
            .entry(c.known.clone().unwrap_or_default())
            .or_default()
            .push(format!("{} `{}` ({})", c.mode.arg(), c.token, c.fixture));
    }
    for c in exact_checks.iter().filter(|c| c.outcome == Outcome::Known) {
        known
            .entry(c.known.clone().unwrap_or_default())
            .or_default()
            .push(format!("{} exact text ({})", c.label, c.fixture));
    }
    if known.is_empty() {
        let _ = writeln!(md, "None.\n");
    }
    for (reason, items) in &known {
        let _ = writeln!(md, "- {reason}");
        for item in items {
            let _ = writeln!(md, "  - {item}");
        }
    }
    let _ = writeln!(md);

    let _ = writeln!(md, "## Failures\n");
    let mut failures: Vec<String> = problems.to_vec();
    for c in sentinel_checks.iter().filter(|c| c.outcome.fails()) {
        failures.push(format!(
            "{} {} `{}` in {}: {} ({})",
            c.outcome.label(),
            c.mode.arg(),
            c.token,
            c.fixture,
            c.detail,
            c.reason
        ));
    }
    for c in exact_checks.iter().filter(|c| c.outcome.fails()) {
        failures.push(format!("{} {} exact text in {}: {}", c.outcome.label(), c.label, c.fixture, c.detail));
    }
    if failures.is_empty() {
        let _ = writeln!(md, "None.");
    }
    for f in &failures {
        let _ = writeln!(md, "- {f}");
    }
    md
}

#[test]
fn token_matching_is_exact() {
    let text = "Their CallerForest stands.";
    assert_eq!(count_token(text, "CallerForest"), 1);
    assert_eq!(count_token(text, "Forest"), 0);
    assert_eq!(count_token("OrchardBeacon, OrchardBeacon.", "OrchardBeacon"), 2);
}

#[test]
fn corpus_expectations_hold() {
    let root = corpus_root();
    let manifest_text = fs::read_to_string(root.join("manifest.toml")).expect("read manifest.toml");
    let manifest: Manifest = toml::from_str(&manifest_text).expect("parse manifest.toml");

    let mut sources = BTreeMap::new();
    for f in &manifest.fixture {
        sources.insert(fixture_id(&f.path), fixture_sources(&root, &f.path));
    }
    let problems = validate(&manifest, &root, &sources);

    let fixtures: Vec<String> = manifest.fixture.iter().map(|f| fixture_id(&f.path)).collect();
    let mut runs: BTreeMap<(String, Mode), RunResult> = BTreeMap::new();
    for f in &manifest.fixture {
        for mode in Mode::ALL {
            let run = run_binary(mode, &root.join(&f.path), &[]);
            runs.insert((fixture_id(&f.path), mode), run);
        }
    }

    let mut sentinel_checks = Vec::new();
    for s in &manifest.sentinel {
        for mode in Mode::ALL {
            let run = runs
                .get(&(s.fixture.clone(), mode))
                .cloned()
                .unwrap_or_else(|| Err("fixture is not in the manifest".into()));
            sentinel_checks.push(check_sentinel(s, mode, &run));
        }
    }

    let mut exact_checks = Vec::new();
    for e in &manifest.exact {
        let run = match manifest.fixture.iter().find(|f| fixture_id(&f.path) == e.fixture) {
            Some(f) => run_binary(e.mode, &root.join(&f.path), &e.args),
            None => Err("fixture is not in the manifest".into()),
        };
        exact_checks.push(check_exact(e, &run));
    }

    let report = render_report(&fixtures, &runs, &sentinel_checks, &exact_checks, &problems);
    let report_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(REPORT_PATH);
    fs::create_dir_all(report_path.parent().unwrap()).expect("create target/coverage");
    fs::write(&report_path, &report).expect("write report");

    let mut failures: Vec<String> = problems.iter().map(|p| format!("invalid: {p}")).collect();
    // Every fixture must compile, including fixtures that no check reads in that mode.
    for ((id, mode), run) in &runs {
        if let Err(err) = run {
            failures.push(format!("fixture-error {} {id}: {err}", mode.arg()));
        }
    }
    for c in sentinel_checks.iter().filter(|c| c.outcome.fails()) {
        let output = match runs.get(&(c.fixture.clone(), c.mode)) {
            Some(Ok(out)) => format!("actual output:\n{out}"),
            Some(Err(err)) => format!("stderr:\n{err}"),
            None => String::new(),
        };
        let context: Vec<&str> = match runs.get(&(c.fixture.clone(), c.mode)) {
            Some(Ok(out)) => out.lines().filter(|l| l.contains(&c.token)).collect(),
            _ => Vec::new(),
        };
        failures.push(format!(
            "{} {} `{}` in {}: {}\n  reason: {}\n  near matches: {:?}\n  {}",
            c.outcome.label(),
            c.mode.arg(),
            c.token,
            c.fixture,
            c.detail,
            c.reason,
            context,
            output
        ));
    }
    for c in exact_checks.iter().filter(|c| c.outcome.fails()) {
        failures.push(format!(
            "{} {} exact text in {}: {}\n  expected:\n{}\n  observed:\n{}",
            c.outcome.label(),
            c.label,
            c.fixture,
            c.detail,
            c.expected,
            c.observed
        ));
    }

    let known = sentinel_checks.iter().filter(|c| c.outcome == Outcome::Known).count()
        + exact_checks.iter().filter(|c| c.outcome == Outcome::Known).count();
    eprintln!(
        "corpus: {} fixtures, {} sentinel checks, {} exact checks, {} known limitations, {} failures. Report: {}",
        fixtures.len(),
        sentinel_checks.len(),
        exact_checks.len(),
        known,
        failures.len(),
        report_path.display()
    );
    for c in sentinel_checks.iter().filter(|c| c.outcome == Outcome::Known) {
        eprintln!("known: {} `{}` in {}: {}", c.mode.arg(), c.token, c.fixture, c.known.as_deref().unwrap_or(""));
    }

    assert!(failures.is_empty(), "{} unexpected result(s):\n\n{}", failures.len(), failures.join("\n\n"));
}
