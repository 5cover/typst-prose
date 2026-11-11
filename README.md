# typst-prose

`typst-prose` is a command-line prototype that extracts natural-language prose from a [Typst](https://typst.app) document and writes it to stdout as UTF-8 plain text. It is written in Rust against the exact Typst 0.15.1 crates.

The prototype compares two extraction strategies, so that the prose each one recovers, and the non-prose each one wrongly includes, can be measured:

- `compiled`: compiles the document and reads the resulting `PagedDocument`, using semantic tags and text runs.
- `source`: parses the Typst source with `typst-syntax` and walks the syntax tree. It does not evaluate anything, so it reports markup wherever it is written, including unused code.

The output is meant as input for a later checker, such as spelling or grammar. Neither checker is part of this project.

## Scope and status

In scope:

- Extracting text from one entry `.typ` file, with the files it imports or includes.
- Preserving paragraph, heading, list, table, caption, and footnote boundaries where the strategy can recover them.
- A sentinel corpus of real Typst fixtures, an expectation manifest, and a harness that compares both strategies and reports known limitations explicitly.

Out of scope:

- Spelling, grammar, or any language model. Sentinel words are checked for presence, not validity.
- Typst packages (`@preview/...`) that are not already installed. Compiled mode reads packages from the local Typst data and cache directories and never downloads one, so a missing package is an error. The source walker reports package imports as warnings and does not follow them.
- Source rewriting, editor integration, and changes to upstream Typst.
- A claim of coverage for arbitrary documents. The corpus is 50 hand-written fixtures.

## Quick start

Build and run on Linux or macOS with the standard toolchain:

```sh
cargo build
target/debug/typst-prose compiled document.typ
target/debug/typst-prose source document.typ
```

These commands were not tested on Linux or macOS in this repository. The project was developed and tested on Windows, which needs the workaround described in [Building on Windows](#building-on-windows).

## Usage

A subcommand is required. There is no default mode.

```
typst-prose source <FILE> [--root <DIR>] [--placeholder-text <TEXT>] [--trace]
typst-prose compiled <FILE> [--root <DIR>] [--date <YYYY-MM-DD>] [--font-path <DIR>]... [--no-system-fonts] [--trace]
typst-prose <subcommand> --help
```

`--date` fixes the date that `datetime.today()` returns, so compiled output is the same on every run. Without it, or with an empty value, the system date is used.

The project root bounds everything the run may read. It is chosen in this order, as in the
Typst CLI:

1. `--root <DIR>`.
2. The `TYPST_ROOT` environment variable (the same option, read by clap).
3. The directory of the canonical entry file, or the current directory if that has none.

The entry file must lie inside the root. Import and include paths resolve against the root:
a leading `/` starts at the root, and a `..` that would leave it is an error in both modes.

In `source` mode, an element that renders no authored text is replaced by a placeholder on the same line: a reference, math, raw text, a link, or a value expression such as `#term("rse")`. The placeholder defaults to `[...]`. `--placeholder-text TEXT` changes it, and an empty `TEXT` removes it, leaving the source spaces. Placeholders separated only by whitespace collapse to one. Only block structure (paragraphs, headings, list items) starts a new unit. A content block is an inline value: where a letter or digit meets it, one space is written, so words never join.

- Prose is written to stdout. Units are separated by a blank line and the output ends with a newline.
- Diagnostics go to stderr, with `file:line:column` spans where available.
- `--trace` replaces the prose with a diagnostic trace on stdout. In `compiled` mode it dumps the frame tree. In `source` mode it prints one line per boundary, gap, and followed file.
- Exit status: `0` on success, `1` on a compile or extraction error (including syntax errors in `source` mode, which print no prose), and `2` on a usage error.

The contract for each mode, covering what is included, what is excluded, and how units are formed, is in the `--help` text of that subcommand. Read it before relying on the output.

## The two strategies

| | `compiled` | `source` |
| --- | --- | --- |
| Input | The compiled `PagedDocument` | The `typst-syntax` tree of each parsed file |
| Evaluation | Full compilation | None |
| Reports unused or unexecuted markup | No | Yes |
| Reports strings, numbering, reference text | Yes, when rendered | No |
| Reports raw text or math | No, as the placeholder | No |
| Boundary handling | Frame tags; some structures lack boundaries | Syntax-based policy (see `--help`) |

`source` mode follows `#import` and `#include` only when the path is a plain string literal. The path is resolved as Typst resolves it: relative to the importing file, with a leading `/` starting at the project root (see Usage). Each module imported with `import` is walked once. Each `include` is walked at each occurrence. Variable paths and package paths are reported as warnings and not followed. A path that leaves the project root is an error.

`docs/EXTRACTION-COMPARISON.md` gives the results and the conclusions. Short version: for checking what an author wrote, `source` is the better input. It recovers all 72 expected tokens in the corpus and includes no non-prose tokens. `compiled` recovers all 75 expected tokens and includes no non-prose tokens either. The two modes miss different things, and the comparison document explains which.

## Testing

Build the binary first. The regression scripts call `target/debug/typst-prose`.

```sh
cargo test                       # corpus harness (tests/corpus.rs) and root tests (tests/root.rs)
bash scripts/coverage.sh         # same harness, writes docs/COVERAGE.md
bash scripts/run-fixtures.sh     # compiled-mode regression fixtures; VERBOSE=1 for diffs
bash scripts/invariants.sh       # semantics-preserving variants must give identical output
```

On Windows, add the toolchain and environment from [Building on Windows](#building-on-windows) first. `scripts/coverage.sh` accepts a `CARGO` override for this:

```sh
CARGO="cargo +stable-x86_64-pc-windows-gnu" bash scripts/coverage.sh
```

Things to know about these checks:

- **The corpus harness** (`tests/corpus.rs`) runs both modes on every file under `fixtures/corpus`. It compares Unicode word tokens exactly, so `Forest` does not match `CallerForest`. Each result is classified as pass, known limitation, regression, unexpected pass, or fixture error. Only regressions, unexpected passes, fixture errors, and manifest or layout problems fail the run. A fixture that is not listed in the manifest is an error, so no fixture is skipped silently. The report is written to `target/coverage/report.md` and copied to `docs/COVERAGE.md`.
- **`run-fixtures.sh`** checks the compiled extractor against `.expected.txt` files. Four fixtures fail on purpose, because they document known gaps: `contextual/counters-and-refs`, `structure/footnote`, `structure/lists`, and `structure/math-inline`. The gaps are described in `docs/EXTRACTION-COMPARISON.md`. The script prints a summary and always exits `0`, so read its `passed=` and `failed=` lines.
- **`invariants.sh`** should report `failed=0`.

## Building on Windows

The Rust GNU target on Windows links with `-lgcc_eh` and `-lgcc`. The LLVM-MinGW toolchain that provides the linker ships no `libgcc.a` or `libgcc_eh.a`, so a plain build fails at link time with `lld: error: unable to find library -lgcc_eh`. The MSVC toolchain also cannot link on machines without Visual Studio Build Tools, which is the default toolchain on the development machine.

The workaround has three parts:

1. Use the GNU host toolchain, `stable-x86_64-pc-windows-gnu`, explicitly.
2. Put the LLVM-MinGW `bin` directory on `PATH`, so the linker `x86_64-w64-mingw32-gcc` is found.
3. Point `LIBRARY_PATH` at a directory containing `libgcc.a` and `libgcc_eh.a`. These are stub files. The `libgcc.a` stub holds one unused symbol. `libgcc_eh.a` is a copy of LLVM-MinGW's `libunwind.a`. Neither file is committed to the repository.

### One-time setup

Verified with rustc 1.99.1 and LLVM-MinGW 23.1.4-20261020 (UCRT, x86_64).

1. Install the GNU host toolchain:

   ```sh
   rustup toolchain install stable-x86_64-pc-windows-gnu
   ```

2. Install LLVM-MinGW from winget:

   ```sh
   winget install --exact --id MartinStorsjo.LLVM-MinGW.UCRT
   ```

3. Find its `bin` directory. The install location includes a winget source hash, so do not guess it. In PowerShell, after the install:

   ```powershell
   Split-Path (Get-Command x86_64-w64-mingw32-clang).Source
   ```

   The command prints the `bin` directory. `LLVM_DIR` is its parent, the directory that contains `bin` and `x86_64-w64-mingw32/lib/libunwind.a`.

4. Create the stub directory. Use a path outside the repository, such as `C:/Users/<you>/typst-prose-mingw-stubs`.

   Bash (Git Bash):

   ```sh
   LLVM_DIR="/c/path/to/llvm-mingw-20261020-ucrt-x86_64"   # set this to your install
   STUBS="C:/Users/<you>/typst-prose-mingw-stubs"
   mkdir -p "$STUBS"
   printf 'int typst_prose_stub_symbol_unused;\n' > "$STUBS/stub.c"
   "$LLVM_DIR/bin/x86_64-w64-mingw32-clang" -c "$STUBS/stub.c" -o "$STUBS/stub.o"
   "$LLVM_DIR/bin/llvm-ar" rcs "$STUBS/libgcc.a" "$STUBS/stub.o"
   cp "$LLVM_DIR/x86_64-w64-mingw32/lib/libunwind.a" "$STUBS/libgcc_eh.a"
   ```

   PowerShell:

   ```powershell
   $LlvmDir = "C:\path\to\llvm-mingw-20261020-ucrt-x86_64"   # set this to your install
   $Stubs = "C:\Users\<you>\typst-prose-mingw-stubs"
   New-Item -ItemType Directory -Force -Path $Stubs | Out-Null
   Set-Content -Path "$Stubs\stub.c" -Value 'int typst_prose_stub_symbol_unused;'
   & "$LlvmDir\bin\x86_64-w64-mingw32-clang.exe" -c "$Stubs\stub.c" -o "$Stubs\stub.o"
   & "$LlvmDir\bin\llvm-ar.exe" rcs "$Stubs\libgcc.a" "$Stubs\stub.o"
   Copy-Item "$LlvmDir\x86_64-w64-mingw32\lib\libunwind.a" "$Stubs\libgcc_eh.a"
   ```

   Do not replace `libgcc_eh.a` with the output of a different LLVM-MinGW build without rebuilding the project and rerunning the tests.

### Each shell session

Bash (Git Bash):

```sh
export PATH="$LLVM_DIR/bin:$PATH"
export LIBRARY_PATH="C:/Users/<you>/typst-prose-mingw-stubs"
cargo +stable-x86_64-pc-windows-gnu build
cargo +stable-x86_64-pc-windows-gnu test
```

PowerShell:

```powershell
$env:PATH = "$LlvmDir\bin;$env:PATH"
$env:LIBRARY_PATH = "C:\Users\<you>\typst-prose-mingw-stubs"
cargo +stable-x86_64-pc-windows-gnu build
cargo +stable-x86_64-pc-windows-gnu test
```

Use forward slashes in `LIBRARY_PATH` under Git Bash, and a Windows path in PowerShell. A single directory is enough. Without `LIBRARY_PATH`, linking fails.

### Verifying the setup

```sh
cargo +stable-x86_64-pc-windows-gnu build    # must finish with "Finished"
cargo +stable-x86_64-pc-windows-gnu test     # corpus test must report "test result: ok"
```

If the build fails with `unable to find library -lgcc_eh` or `-lgcc`, `LIBRARY_PATH` is not set in the current shell, or it does not contain both stub files. If it fails with `link.exe`, the command ran the MSVC toolchain. Add `+stable-x86_64-pc-windows-gnu`.

The MSVC toolchain is not used by this project. Installing Visual Studio Build Tools may also allow the default toolchain to link. That was not tested here.

## Reproducibility

- `Cargo.lock` is committed. Build with it, and do not run `cargo update` when reproducing a result.
- The Typst crates are pinned exactly to `=0.15.1` in `Cargo.toml`.
- Fonts come from `typst-assets`, bundled with the crate. The output does not depend on the fonts installed on the machine.
- `today()` returns a fixed date (2025-01-01), so date-dependent output does not change between runs.
- The corpus expectations and the generated report are deterministic. `docs/COVERAGE.md` is regenerated by `scripts/coverage.sh` and should match the committed copy after a clean run.
- Two inputs live outside the repository: the rustup GNU host toolchain and the LLVM-MinGW install, both on Windows. The stub files are generated locally from those tools, and they are never committed.

The repository does not pin the Rust toolchain. It requires an edition 2024 toolchain. The rustc version used for development and verification is 1.99.1. Adding a `rust-toolchain.toml` would pin the GNU host on Windows for everyone who clones the repository. That was not done.

## Repository layout

```
src/main.rs        CLI: subcommands, --root, --help contracts, --trace, exit codes
src/project.rs     project root resolution shared by both strategies
src/source.rs      source strategy: typst-syntax tree walker and boundary policy
src/extract.rs     compiled strategy: PagedDocument tag and text-run extractor
src/trace.rs       compiled-mode frame trace (--trace in compiled mode)
src/world.rs       minimal Typst World: project-root files, bundled fonts, fixed date
src/output.rs      stdout rendering shared by both strategies
tests/corpus.rs    coverage harness: runs both modes, checks the manifest, writes report
tests/root.rs      project root tests: --root, TYPST_ROOT, defaults, escapes, both modes
tests/root-fixtures/
                   small project tree used by tests/root.rs
fixtures/corpus/   50 Typst fixtures, support modules, manifest.toml
fixtures/basic/, contextual/, structure/
                   compiled-mode regression fixtures with .expected.txt files
probes/            small Typst documents used during feasibility work
scripts/           run-fixtures.sh, invariants.sh, coverage.sh
docs/              COVERAGE.md (generated report), EXTRACTION-COMPARISON.md (comparison, compiled-extractor frame facts and open work)
```

## Known limitations

The main limitations are listed in `--help` for each mode and in full in `docs/COVERAGE.md` (generated from the test run). In short:

- Compiled mode: raw text and math are placeholders, as in source mode, and the generated text that the document includes (filler, contents, bibliography) is extracted as rendered. The boundaries that the element structure marks are separated: line breaks, term items, numbered headings, list markers, footnote and outline numbers, quote attributions, and bibliography labels. Output depends on the machine's fonts unless `--no-system-fonts` is given.
- Source mode: content blocks are inline values, so tables, figures, block quotes and footnote bodies join the sentence that contains them, a footnote body is not set apart from its sentence, quote attributions come before quote bodies, and text computed from strings is not visible.

`docs/EXTRACTION-COMPARISON.md` records the frame facts behind the compiled strategy, the gaps they explain, and the open upstream work.
