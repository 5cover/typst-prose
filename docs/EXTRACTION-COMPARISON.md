# Source syntax versus compiled extraction

Two strategies extract prose from a Typst 0.15.1 document:

- `typst-prose source` walks the syntax tree from `typst-syntax`. It does not evaluate
  anything. Markup is reported wherever it is written.
- `typst-prose compiled` reads the `PagedDocument` produced by compilation, as before.

Full contracts are in `typst-prose source --help` and `typst-prose compiled --help`.
The numbers below come from `docs/COVERAGE.md`, which `scripts/coverage.sh` generates from
the observed output of 31 fixtures (`fixtures/corpus`, expectations in
`fixtures/corpus/manifest.toml`). Nothing here is an arbitrary-document claim.

## Results

| Measure | source | compiled |
| --- | ---: | ---: |
| Expected prose tokens recovered | 71 of 72 | 68 of 75 |
| Non-prose tokens included (the contract excludes them) | 0 | 3 (raw x2, math string) |
| Exact-text checks passing (of 10 fixtures) | 7 | 5 |
| Documented known limitations (sentinel and exact-text) | 4 | 15 |
| Regressions or unexpected results | 0 | 0 |

Tokens recovered only by source (16): 10 are authored text that compiled mode does not render
(unexecuted branches, unused functions and variables, an unused imported helper, a running
header, the attribution of an inline quote, and text replaced by a show rule). The other 6
are rendered by compiled mode but glued to a neighbour, so they are not separable there.

Tokens recovered only by compiled (16): 13 are string-derived or generated (displayed or
argument strings, function results, a numbering pattern, reference supplements, a context
string, a string table cell). 3 are non-prose (raw text twice, a math string).

Missed by both: the footnote body word. Source joins it to the sentence with no space
("note.FootnoteBodyWord"), and compiled glues the reference number to it ("1FootnoteBodyWord").

## Which mode suits checking authored prose

Source mode. It recovers 71 of the 72 expected tokens, and the one miss is a footnote body
glued to its sentence. It includes text the reader never sees, and it does not emit strings,
numbering, reference text, or raw and math content. For a checker that should review what
the author wrote, that is the right input.

Its limits are structural, and each one is a known, tested limitation:

- Source cannot tell a block call from an inline call. A trailing body such as
  `#block[...]` is joined to the surrounding text ("OuterBlock NestedBlock").
- Out-of-flow content is joined in place. Footnote bodies follow the sentence with no space.
- Source order follows the argument list, so a block quote's attribution comes before its body.
- Text computed from strings is invisible. Numbering, supplements and displayed strings need
  evaluation.
- A code expression or equation between two words ends the unit, so a sentence that contains
  one is split: "The value is" / "and is correct.". This is deliberate, because joining across
  the value would invent a sentence.

Compiled mode suits checking what the reader sees, with the caveats in its `--help`. It is
weaker on boundaries: explicit line breaks, term lists, adjacent block quotes, and the
footnote number glue words, and it also reports rendered raw text and math glyphs as prose.

## What evaluation adds

When Typst evaluates the document, the additional prose is:

- Strings that are displayed, passed to functions, returned by functions, used as captions,
  supplements, table cells, show replacements, or produced by a context block.
- Generated numbers and prefixes, including heading numbering (`Qorvbu` appears in every
  numbered heading) and reference text ("See RefSupplementWord 1").
- Repeated output from a single authored definition. A closure that builds two cells gives
  two rendered copies, and the corpus checks the count (`GeneratedCell`: source 1, compiled 2).

Evaluation also removes text that source mode keeps: unexecuted branches, unused functions,
and replaced show-rule text. It is the reason the two modes disagree on those tokens.

## Policy the source walker implements

These are the rules `typst-prose source --help` states. They are the basis of the boundary
tests.

- A paragraph break, heading, list item, enum item or term item starts a new unit.
- A content block passed as an argument, or in a code position, is its own unit. A trailing
  content block `f(...)[body]` and a bare `[body]` stay inline.
- Any other embedded element (code, math, raw, link, reference) ends the unit. Nothing is
  removed and joined.
- Escapes and shorthands are decoded. Whitespace is collapsed. Soft hyphens are removed.
- Imports and includes with a plain string-literal path are followed by parsing, never by
  execution. Each imported module is walked once. Each include is walked at each occurrence.
  Variable paths, package paths and escaped paths are reported on stderr and not followed.

## Limits of this study

- 31 hand-written fixtures. They show the behaviour of these constructs in Typst 0.15.1. They
  do not measure coverage of arbitrary documents.
- Packages are not followed, and neither are dynamic import paths.
- The compiled extractor is unchanged. Its limitations are recorded, not fixed, as the task
  required. Each is a known limitation in the manifest with a reason.
- Expectations are the intended semantics written from Typst behaviour, then checked against
  real output. No expected value was changed to match output. Mismatches that reflect a real
  extractor defect are documented known limitations, with the observed output recorded. One
  fixture was changed: the quote attribution cases now use `block: true`, because Typst does
  not render the attribution of an inline quote, and the sentinel was meant to test a rendered
  attribution.

## Reproducing

```
CARGO="cargo +stable-x86_64-pc-windows-gnu" bash scripts/coverage.sh
```

On this machine the default toolchain cannot link. The GNU toolchain needs the
`LIBRARY_PATH` workaround in `docs/FINDINGS.md`. Without the override, `cargo test --test corpus`
runs the same harness.
