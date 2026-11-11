# Source syntax versus compiled extraction

Two strategies extract prose from a Typst 0.15.1 document:

- `typst-prose source` walks the syntax tree from `typst-syntax`. It does not evaluate anything. Markup is reported wherever it is written.
- `typst-prose compiled` reads the `PagedDocument` produced by compilation, as before.

Full contracts are in `typst-prose source --help` and `typst-prose compiled --help`. The numbers below come from `docs/COVERAGE.md`, which `scripts/coverage.sh` generates from the observed output of 31 fixtures (`fixtures/corpus`, expectations in `fixtures/corpus/manifest.toml`). Nothing here is an arbitrary-document claim.

## Results

| Measure | source | compiled |
| --- | ---: | ---: |
| Expected prose tokens recovered | 72 of 72 | 68 of 75 |
| Non-prose tokens included (the contract excludes them) | 0 | 3 (raw x2, math string) |
| Exact-text checks passing (of 10 fixtures) | 8 | 5 |
| Documented known limitations (sentinel and exact-text) | 2 | 15 |
| Regressions or unexpected results | 0 | 0 |

Tokens recovered only by source (17): 10 are authored text that compiled mode does not render (unexecuted branches, unused functions and variables, an unused imported helper, a running header, the attribution of an inline quote, and text replaced by a show rule). The other 6 are rendered by compiled mode but glued to a neighbour, so they are not separable there. The last is the footnote body word, which compiled glues to its reference number.

Tokens recovered only by compiled (16): 13 are string-derived or generated (displayed or argument strings, function results, a numbering pattern, reference supplements, a context string, a string table cell). 3 are non-prose (raw text twice, a math string).

Missed by both: none in the corpus. Source emits the footnote body as its own unit. Compiled mode recovers the body as text but glues the reference number to its first word ("1FootnoteBodyWord").

## Which mode suits checking authored prose

Source mode. It recovers all 72 expected tokens. It includes text the reader never sees, and it does not emit strings, numbering, reference text, or raw and math content. For a checker that should review what the author wrote, that is the right input.

Its limits are structural, and each one is a known, tested limitation:

- Source cannot tell a block call from an inline call. A trailing body such as `#block[...]` is joined to the surrounding text ("OuterBlock NestedBlock").
- Out-of-flow content is joined in place, with one exception: footnote bodies are their own units. That exception matches the callee name `footnote`, so a user function with the same name gets the same treatment.
- Source order follows the argument list, so a block quote's attribution comes before its body.
- Text computed from strings is invisible. Numbering, supplements and displayed strings need evaluation.
- A code expression or equation between two words ends the unit, so a sentence that contains one is split: "The value is" / "and is correct.". This is deliberate, because joining across the value would invent a sentence.

Compiled mode suits checking what the reader sees, with the caveats in its `--help`. It is weaker on boundaries: explicit line breaks, term lists, adjacent block quotes, and the footnote number glue words, and it also reports rendered raw text and math glyphs as prose.

## What evaluation adds

When Typst evaluates the document, the additional prose is:

- Strings that are displayed, passed to functions, returned by functions, used as captions, supplements, table cells, show replacements, or produced by a context block.
- Generated numbers and prefixes, including heading numbering (`Qorvbu` appears in every numbered heading) and reference text ("See RefSupplementWord 1").
- Repeated output from a single authored definition. A closure that builds two cells gives two rendered copies, and the corpus checks the count (`GeneratedCell`: source 1, compiled 2).

Evaluation also removes text that source mode keeps: unexecuted branches, unused functions, and replaced show-rule text. It is the reason the two modes disagree on those tokens.

## Policy the source walker implements

These are the rules `typst-prose source --help` states. They are the basis of the boundary tests.

- A paragraph break, heading, list item, enum item or term item starts a new unit.
- A content block passed as an argument, or in a code position, is its own unit. A trailing content block `f(...)[body]` and a bare `[body]` stay inline.
- A footnote call, `#footnote[...]`, is its own unit, because its body is set out of the text flow. The walker matches the callee name `footnote`.
- Any other embedded element (code, math, raw, link, reference) ends the unit. Nothing is removed and joined.
- Escapes and shorthands are decoded. Whitespace is collapsed. Soft hyphens are removed.
- Imports and includes with a plain string-literal path are followed by parsing, never by execution. Each imported module is walked once. Each include is walked at each occurrence. Variable paths, package paths and escaped paths are reported on stderr and not followed.

## Limits of this study

- 31 hand-written fixtures. They show the behaviour of these constructs in Typst 0.15.1. They do not measure coverage of arbitrary documents.
- Packages are not followed, and neither are dynamic import paths.
- The compiled extractor is unchanged. Its limitations are recorded, not fixed, as the task required. Each is a known limitation in the manifest with a reason.
- Expectations are the intended semantics written from Typst behaviour, then checked against real output. No expected value was changed to match output. Mismatches that reflect a real extractor defect are documented known limitations, with the observed output recorded. One fixture was changed: the quote attribution cases now use `block: true`, because Typst does not render the attribution of an inline quote, and the sentinel was meant to test a rendered attribution.

## Compiled extractor: frame facts, gaps and open work

These facts about the `PagedDocument` in Typst 0.15.1 explain the compiled extractor's behaviour and limits. They come from the feasibility study that preceded the source strategy. The evidence paths point to fixtures and probes that are still in the repository. The measured status of each gap is in `docs/COVERAGE.md`.

Frame facts:

- Tags present in the frame: `par`, `heading`, `figure`, `caption`, `table`, `cell`, `enum`, `item` (partial), `footnote` (reference), `entry` (footnote body), `ref`, `link-marker`, `equation`, `strong`, `emph`, `place`, `quote`, `artifact` and `pdf-marker-tag`. The bodies of `block` and `quote` are not wrapped in a `par`. Evidence: traces of all fixtures.
- A paragraph that crosses a page is one `par` start on the first page and one end on the second, with no page-separator item between them. Evidence: `fixtures/basic/page-split-paragraph.typ`.
- Runs keep their own leading and trailing spaces. Layout line breaks and explicit `\` produce no text, so the space between words can be lost. The corpus shows that explicit line breaks glue the words on either side. The earlier finding that joining runs gives the right word was wrong for these cases. Evidence: `probes/reorder-and-quote.typ`, `fixtures/corpus/tables/tables.typ`, `fixtures/corpus/boundaries/paragraphs.typ`.
- Automatic hyphenation inserts U+00AD at the break. Removing it rejoins the word. Only one fixture was tested. Evidence: `fixtures/basic/hyphenation.typ`.
- Inline math is emitted as separate Unicode math glyph runs with no spacing, so the source form is lost. Evidence: `fixtures/structure/math-inline.typ`.
- Bullets and numbers are text runs inside `pdf-marker-tag`. That tag also wraps tight list item bodies, so markers cannot be separated by tag alone. `detached` spans mark some generated runs (quote attribution) but not others: heading numbers have source spans, and a caption's authored words are detached too. No single signal is reliable. Evidence: `fixtures/structure/lists.typ`, `fixtures/structure/figure-caption.typ`.
- Header and footer text is inside `artifact` and is suppressed. Image alt text is reachable from `FrameItem::Image` through `Image::alt()`, but it is not body prose. Evidence: `fixtures/structure/page-header.typ`, `fixtures/structure/image-alt/figure.typ`.
- The footnote body is an `entry` unit, found through `GroupItem::parent`, which points to the footnote's location. The in-text marker sits in a `footnote` tag and is suppressed. The marker inside the body is not removed. Evidence: `fixtures/structure/footnote.typ`.
- A show rule that replaces text puts the replacement in the frame, with the rule's span. Text produced by a function call carries the span of the call, not the generated words. `src=detached` and `src=none` mean the provenance is unresolved. Evidence: `fixtures/contextual/show-rules.typ`, `probes/paragraph-across-pages.typ`.
- A caption is a `caption` tag that is independent of its figure's shape. Table cells come in source order. Two-column content came out in source order in the one case tested. A placed element at the bottom of a page was emitted first, which matched source order in the one probe. Evidence: `fixtures/structure/figure-caption.typ`, `fixtures/structure/table-cells.typ`, `fixtures/structure/columns.typ`, `fixtures/structure/placed-float.typ`.
- Heading numbers and reference text are generated. Their presence in the frame is correct, but the space between the number and the title is missing. Evidence: `fixtures/contextual/counters-and-refs.typ`.

Upstream work and open questions:

- The feasibility study recommended two small upstream changes that would close most of the compiled gaps: a paragraph-like tag around `block` and `quote` bodies, and an item-level tag for each list and enum item.
- Open: whether the element `Content` carried by `Tag::Start` has enough source structure to recover math source and generated markers. The compiled extractor does not use it. This needs a spike before choosing between an external extractor and a compiler integration.

Not yet tested:

- Clipped or overlaid text (`GroupItem::clip`).
- Smart quotes and case transforms. The corpus tests apostrophes and guillemets in source and output, but not transforms.
- Tables that span pages, and repeated headers over more than the two pages of the corpus layout fixture.
- Typst versions other than 0.15.1.

## Reproducing

```
CARGO="cargo +stable-x86_64-pc-windows-gnu" bash scripts/coverage.sh
```

On this machine the default toolchain cannot link. The GNU toolchain needs the `LIBRARY_PATH` workaround described in the README under "Building on Windows". Without the override, `cargo test --test corpus` runs the same harness.
