# Roadmap

## Purpose

`typst-prose` extracts prose from Typst 0.15.1 documents, in two strategies that answer different questions:

- `source` answers "what did the author write?" It walks the syntax tree and never evaluates the document. Its defining invariant is that it does not run the code.
- `compiled` answers "what does the reader see?" It reads the rendered `PagedDocument`, so it gets values, generated labels, navigation and bibliography text that source cannot see.

Both strategies are measured against a hand-written corpus with explicit expectations (`fixtures/corpus/manifest.toml`). Expectations are the intended output. Where an extractor differs, the entry is recorded as a limitation with the observed output. The gate fails on regressions and on unexpected passes. The corpus is not a coverage claim for arbitrary documents.

Three kinds of entry are tracked separately:

- **Known limitations** are costs that the design accepts. Each has a reason, and changing it changes a design decision, not a bug.
- **Outstanding limitations** are defects with a planned fix.
- **Infeasible** items have an attempt and an analysis recorded, so the next person can start from there.

## Current state

- Corpus gate: 62 fixtures, 71 exact-text checks, 194 sentinel checks, 13 `known` entries (source 11, compiled 2), 0 failures (`scripts/coverage.sh`).
- Fixture scripts: `run-fixtures.sh` (10 passed, 4 failing on purpose), `invariants.sh` (6 passed).
- Implemented: root resolution, installed packages (never downloaded), `datetime.today()` (fixed with `--date`, or the system clock), system fonts with `--font-path` and `--no-system-fonts`, the source content-value rule, and compiled word boundaries from the element structure.
- Smoke test: enssat-E1 (outside this repository) runs in both modes. The contents page numbers of its 73 entries match the author's `dist/rapport.pdf` (produced by Typst 0.15.1) exactly, with the system fonts. Its outputs are kept under `target/out/` and are not committed.

## Known limitations

These are accepted or decided. The corpus records each one.

Source mode:

- Tables, figures and block quotes written in a sentence join the sentence. Source cannot tell a block call from an inline call. Accepted.
- A footnote body joins its sentence. The walker does not match callee names, because a user definition can shadow `footnote`. Accepted.
- Two content blocks that touch are split, so the rendered `ab` becomes `a b`. A split word is preferred to a glued one. Accepted.
- The template text of an unused `#let` definition is reported as its own unit. Source reports markup wherever it is written. Accepted.
- Text computed from strings is invisible: numbering, supplements and displayed strings need evaluation. Part of the contract.
- Data files (`json`, `read`, `.yml`, `.bib`) are not read. Values from them stay placeholders. Decided: source must not evaluate or parse data.
- Quote attributions come before quote bodies, because source follows the argument list. This is the rule the walker implements.
- Trailing content of a call is inline, so a nested block joins the outer block's text. Same cause as the table case.

Compiled mode:

- Generated text that the document includes is extracted as rendered: filler from `lorem()`, the contents and figure list, and bibliography entries. Decided: compiled mode includes what the reader sees.
- Rendered math text is prose. Decided. Source mode excludes it.
- The reference number of a footnote is written before its note body. Decided: the number is generated, and the reader sees it.
- Only installed packages are read. Packages are never downloaded, so the tool stays offline. Decided.

## Open decision

- **Raw text.** Typst raw text is code: inline `` `code` `` and fenced blocks. Compiled mode extracts it as prose. For example, the sample `` Run `cargo build` to compile. `` and a fenced `fn main() {}` block come out as `Run cargo build to compile.` and `fn main() {}`. Source mode writes the placeholder `[...]` for each. Options: keep raw text as prose, as now, or exclude it as source does. The corpus has `RawInlineWord` and `RawBlockWord` as known entries until this is decided.

## Infeasible

### Source: `#linebreak()` between a label and its value

Symptom: in source mode, `#linebreak()` is a value expression, so the placeholder `[...]` is written between a label and its value. The boundary is a word boundary (or a newline), so the placeholder is wrong. Test: `compiled-gaps/grid-labels`, source exact check, which is a known limitation.

Attempts and why they do not work:

1. Match the callee name `linebreak`. This works for the common case, but a user definition can shadow the name, and the walker cannot resolve which binding a name refers to without evaluating the document. The footnote case was dropped for exactly this reason, so the same rule applies here.
2. Treat any value expression with no content and no arguments as a boundary. Wrong for `#term("x")`, `#h(1em)` and `#sym.nbsp`, which are not line breaks.
3. Treat the markup form `\` as the break. This is already done: the markup linebreak is a newline. It covers the markup form only, not the call form.

## Outstanding

- **Math superscripts and subscripts lose their script.** `$x^2$` is extracted as `x2`, so the exponent reads as part of the variable. The reader sees a superscript. Analysis: the frame writes the superscript as a separate text run, so the run order is right and the script is lost only in the join. The equation's Content has typed `attach` fields (`base`, `t`, `b`), so a fix can use the element type, not positions. Expected output needs a decision on the form: `x^2` or `x²`. Test: to be added as `math/scripts`, with a source expectation of the placeholder.

## Next steps

1. Decide raw text, then change the two `RawInlineWord` and `RawBlockWord` expectations in the same commit.
2. Decide the form of scripts (`x^2` or `x²`), then fix the superscript extraction and add `math/scripts`.
3. Smoke-test enssat-E1 in both modes after each change, and compare with the previous output, to confirm that a change moves only the lines it should.

## Non-goals

- Source mode does not evaluate the document, and it does not read data files.
- No coverage claim for arbitrary documents. The corpus measures specific constructs in Typst 0.15.1.
- Packages are never downloaded.
- No attempt to reproduce Typst's layout, or to reconstruct math source.

## Maintenance

- Typst is pinned to 0.15.1. A version change needs a corpus re-run, and a review of every known entry, because layout changes can fix or break them.
- Expectations are written as intended semantics before the output is checked. A mismatch becomes a known entry with the observed output. Expectations are not changed to match output.
- When a limitation is fixed, its `known` entry is removed in the same commit, because the gate fails on unexpected passes.
- Every change runs `CARGO="cargo +stable-x86_64-pc-windows-gnu" bash scripts/coverage.sh`, plus `cargo test`, `run-fixtures.sh` and `invariants.sh`.
