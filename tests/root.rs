//! Project root resolution: `--root`, `TYPST_ROOT`, and the default (the entry file's
//! directory). Runs the binary on the tree in `tests/root-fixtures` and checks both modes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/root-fixtures")
}

fn run(mode: &str, entry: &Path, root: Option<&Path>, env_root: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_typst-prose"));
    command.arg(mode);
    if let Some(root) = root {
        command.arg("--root").arg(root);
    }
    command.arg(entry).env_remove("TYPST_ROOT");
    if let Some(env_root) = env_root {
        command.env("TYPST_ROOT", env_root);
    }
    command.output().expect("run typst-prose")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn project() -> PathBuf {
    fixtures().join("project")
}

fn main_doc() -> PathBuf {
    project().join("doc/main.typ")
}

fn escape_doc() -> PathBuf {
    project().join("doc/escape.typ")
}

fn assert_main_prose(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(output));
    let text = stdout(output);
    assert!(text.contains("RootChapterWord"), "shared include missing: {text}");
    assert!(text.contains("MainDocumentWord"), "main text missing: {text}");
}

#[test]
fn source_root_flag_resolves_root_relative_include() {
    assert_main_prose(&run("source", &main_doc(), Some(&project()), None));
}

#[test]
fn source_root_from_environment() {
    assert_main_prose(&run("source", &main_doc(), None, Some(&project())));
}

#[test]
fn source_flag_overrides_environment() {
    let wrong = project().join("doc");
    assert_main_prose(&run("source", &main_doc(), Some(&project()), Some(&wrong)));
}

#[test]
fn source_default_root_is_entry_directory() {
    // The default root is project/doc, so "/shared/chapter.typ" is not found.
    let output = run("source", &main_doc(), None, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("shared/chapter.typ"), "stderr: {}", stderr(&output));
}

#[test]
fn source_escaping_path_is_error() {
    let output = run("source", &escape_doc(), Some(&project()), None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("would escape the project root"), "stderr: {}", stderr(&output));
}

#[test]
fn source_escaping_path_is_error_with_default_root() {
    let output = run("source", &escape_doc(), None, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("would escape the project root"), "stderr: {}", stderr(&output));
}

#[test]
fn entry_outside_root_is_error() {
    let output = run("source", &main_doc(), Some(&project().join("shared")), None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("outside the project root"), "stderr: {}", stderr(&output));
}

#[test]
fn compiled_root_flag_resolves_root_relative_include() {
    assert_main_prose(&run("compiled", &main_doc(), Some(&project()), None));
}

#[test]
fn compiled_root_from_environment() {
    assert_main_prose(&run("compiled", &main_doc(), None, Some(&project())));
}

#[test]
fn compiled_default_root_is_entry_directory() {
    let output = run("compiled", &main_doc(), None, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(!stderr(&output).is_empty());
}

#[test]
fn compiled_escaping_path_is_error() {
    let output = run("compiled", &escape_doc(), Some(&project()), None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("would escape the project root"), "stderr: {}", stderr(&output));
}
