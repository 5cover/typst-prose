# Repository Guidelines

## Project Structure & Module Organization

This is a Rust 2024 command-line application. `src/main.rs` defines the CLI and connects the extraction modes; `src/source.rs` walks Typst syntax, while `src/extract.rs` reads compiled documents. Shared project-root, output, trace, and Typst-world code lives in `src/project.rs`, `src/output.rs`, `src/trace.rs`, and `src/world.rs`. Integration tests are in `tests/`. Typst examples and expected output are under `fixtures/`; the broader coverage corpus and its manifest are in `fixtures/corpus/`. Supporting scripts live in `scripts/`, and design and coverage notes are in `docs/`.

## Build, Test, and Development Commands

- `cargo build` builds the `typst-prose` executable.
- `cargo run -- source document.typ` or `cargo run -- compiled document.typ` runs an extraction mode locally; pass `--help` for its options.
- `cargo test` runs the integration tests and corpus harness.
- `bash scripts/run-fixtures.sh` checks compiled-mode regression fixtures; `bash scripts/invariants.sh` checks semantics-preserving variants.
- `bash scripts/coverage.sh` regenerates `docs/COVERAGE.md` from the corpus. Review generated report changes before committing.
- `cargo fmt` formats Rust code. No repository-specific lint configuration is defined.

## Coding Style & Naming Conventions

Follow standard Rust style and run `cargo fmt` after Rust changes. Use `snake_case` for modules, functions, and variables; use `UpperCamelCase` for types and enums. Keep the two extraction modes' contracts aligned with their CLI help text and README. Preserve fixture naming by topic, with `.typ` inputs and matching `.expected.txt` files where applicable.

## Testing Guidelines

Add integration coverage in `tests/` for CLI behavior. For extraction behavior, update or add a focused Typst fixture and its expected output; corpus cases should also be declared in `fixtures/corpus/manifest.toml`. Run the narrow relevant script or test while iterating, then `cargo test` before submitting when practical.

## Commit & Pull Request Guidelines

Recent history uses Conventional Commit prefixes such as `feat:`, `fix:`, `docs:`, and `chore:`. Use a concise imperative subject, for example `fix: preserve paragraph boundaries`. Pull requests should explain user-visible behavior changes, list relevant validation commands and results, and include representative CLI output when it clarifies extraction changes. Keep generated coverage reports in sync when their source corpus changes.

## Configuration Notes

Compiled mode reads Typst packages only from local data and cache directories; it does not download packages. `TYPST_ROOT` sets the project root unless overridden with `--root`. Avoid committing machine-specific paths or generated build artifacts.
