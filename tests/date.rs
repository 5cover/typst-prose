//! `compiled --date`: a fixed date is what `datetime.today()` returns, an empty value means
//! the system date, and a malformed date is rejected before compilation.

use std::path::PathBuf;
use std::process::{Command, Output};

fn today_doc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/corpus/compiled-gaps/today.typ")
}

fn run(date_args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_typst-prose"))
        .arg("compiled")
        .args(date_args)
        .arg(today_doc())
        .output()
        .expect("run typst-prose")
}

#[test]
fn fixed_date_is_the_date_datetime_today_returns() {
    let output = run(&["--date", "1999-12-31"]);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("1999-12-31"), "fixed date missing: {text}");
}

#[test]
fn empty_date_matches_the_system_date() {
    let missing = run(&[]);
    let empty = run(&["--date", ""]);
    assert!(empty.status.success(), "stderr: {}", String::from_utf8_lossy(&empty.stderr));
    assert_eq!(missing.stdout, empty.stdout);
}

#[test]
fn malformed_date_is_rejected() {
    for bad in ["2026-02-30", "26-3-4"] {
        let output = run(&["--date", bad]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "accepted {bad}");
        assert!(stderr.contains("YYYY-MM-DD"), "no format hint for {bad}: {stderr}");
    }
}
