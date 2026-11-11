//! Package imports in compiled mode. Packages are read from the local data and cache
//! directories only, so a package that is not installed is an error. The test does not
//! depend on which packages the machine has installed.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn missing_package_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/package-fixtures/missing-package.typ")
}

fn run(mode: &str, entry: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_typst-prose"))
        .arg(mode)
        .arg(entry)
        .output()
        .expect("run typst-prose")
}

#[test]
fn compiled_missing_package_is_an_error_naming_the_package() {
    let output = run("compiled", &missing_package_doc());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected failure, stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert!(stderr.contains("no-such-package-xyz"), "package not named: {stderr}");
}
