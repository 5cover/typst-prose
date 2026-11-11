//! Compiled strategy: reads text runs and semantic tags from the `PagedDocument`. It reads
//! no source text, coordinates or layout geometry. Element structure comes from the `Content`
//! carried by block tags, and only for the fields that hold inline content.
//!
//! Rules as implemented:
//! - A unit opens at each `par`, `heading`, `caption`, `item`, `cell`, `entry` (footnote body)
//!   and `place` tag, and collects every text run inside it, nested or not.
//! - Inside a block, a word boundary is written between two runs when the element structure
//!   places one there: an explicit line break, the term and description of a term item, or the
//!   number and title of a numbered heading.
//! - Text outside any unit is emitted as its own unit in position order and reported on stderr
//!   as `untagged text`. It is never dropped or merged across unit boundaries.
//! - Runs under `artifact` (page furniture) and `footnote` (the in-text reference marker) are
//!   suppressed and counted on stderr.
//! - U+00AD (soft hyphen, inserted at automatic hyphenation) is removed.
//! - Whitespace inside a unit is normalized. Runs keep their own leading and trailing spaces.
//! - Not implemented: the boundary before a quote attribution (its run is detached, so it has
//!   no span to match), separating generated markers from prose elsewhere, and reconstructing
//!   math source. The frame facts behind these rules and the open work are in
//!   docs/EXTRACTION-COMPARISON.md.

use std::collections::HashSet;

use typst::foundations::{Content, Value};
use typst::introspection::{Location, Tag};
use typst::layout::{Frame, FrameItem};
use typst::syntax::Span;
use typst::text::TextItem;
use typst_layout::PagedDocument;

const BLOCK_ELEMS: &[&str] = &["par", "heading", "caption", "item", "cell", "entry", "place"];
const SUPPRESSED_ELEMS: &[&str] = &["artifact", "footnote"];

struct Open {
    loc: Option<Location>,
    block: Option<usize>,
    suppressed: bool,
}

#[derive(Default)]
pub struct Extraction {
    pub blocks: Vec<String>,
    pub untagged: Vec<String>,
    pub dropped_artifacts: usize,
    pub unmatched_tags: usize,
}

impl Extraction {
    pub fn text(&self) -> String {
        crate::output::render(&self.blocks)
    }
}

pub fn extract(doc: &PagedDocument) -> Extraction {
    let mut state = State::default();
    for page in doc.pages() {
        state.frame(&page.frame);
    }
    state.out.unmatched_tags = state.open.len();
    state.out.blocks = state
        .buffers
        .iter()
        .map(|b| crate::output::collapse_ascii_spaces(&b.replace('\u{ad}', "")))
        .collect();
    state.out
}

#[derive(Default)]
struct State {
    open: Vec<Open>,
    out: Extraction,
    buffers: Vec<String>,
    /// The span of the last run written to each block buffer.
    last: Vec<Option<Span>>,
    /// Pairs of consecutive spans that the element structure separates by a word boundary.
    separators: HashSet<(Span, Span)>,
    loose: Option<usize>,
    suppressed: usize,
}

impl State {
    fn new_buffer(&mut self) -> usize {
        self.buffers.push(String::new());
        self.last.push(None);
        self.buffers.len() - 1
    }

    fn frame(&mut self, frame: &Frame) {
        for (_, item) in frame.items() {
            match item {
                FrameItem::Tag(Tag::Start(content, _)) => {
                    let name = content.elem().name();
                    let suppressed = SUPPRESSED_ELEMS.contains(&name);
                    if suppressed {
                        self.suppressed += 1;
                    }
                    let block = BLOCK_ELEMS.contains(&name).then(|| {
                        self.loose = None;
                        self.new_buffer()
                    });
                    if block.is_some() {
                        self.record_boundaries(content);
                    }
                    self.open.push(Open { loc: content.location(), block, suppressed });
                }
                FrameItem::Tag(Tag::End(loc, _, _)) => {
                    let matched = self.open.last().is_some_and(|o| o.loc == Some(*loc));
                    if !matched {
                        self.out.unmatched_tags += 1;
                    }
                    if let Some(closed) = self.open.pop() {
                        if closed.suppressed {
                            self.suppressed -= 1;
                        }
                        if closed.block.is_some() {
                            self.loose = None;
                        }
                    }
                }
                FrameItem::Text(text) => {
                    if self.suppressed > 0 {
                        self.out.dropped_artifacts += 1;
                        continue;
                    }
                    match self.open.iter().rev().find_map(|o| o.block) {
                        Some(slot) => self.push_run(slot, text),
                        None => {
                            let slot = match self.loose {
                                Some(slot) => slot,
                                None => {
                                    let slot = self.new_buffer();
                                    self.loose = Some(slot);
                                    slot
                                }
                            };
                            self.buffers[slot].push_str(&text.text);
                            self.out.untagged.push(text.text.to_string());
                        }
                    }
                }
                FrameItem::Group(group) => self.frame(&group.frame),
                FrameItem::Shape(..) | FrameItem::Image(..) | FrameItem::Link(..) => {}
            }
        }
    }

    /// Records the word boundaries that a block's element structure places between its inline
    /// leaves. A numbered heading also separates its number from its title.
    fn record_boundaries(&mut self, content: &Content) {
        let mut leaves = Vec::new();
        collect_leaves(content, &mut leaves);
        let mut previous = None;
        let mut broken = false;
        for leaf in &leaves {
            match *leaf {
                Leaf::Break => broken = true,
                Leaf::Text(span) => {
                    if let Some(previous) = previous.filter(|_| broken) {
                        self.separators.insert((previous, span));
                    }
                    previous = Some(span);
                    broken = false;
                }
            }
        }
        let numbered = content.field_by_name("numbering").is_ok_and(|value| !matches!(value, Value::None));
        if let (true, Some(first)) = (numbered, first_text(&leaves)) {
            self.separators.insert((content.span(), first));
        }
    }

    /// Writes a run into a block buffer. A word boundary recorded for the pair of spans is written
    /// as one space before the run.
    fn push_run(&mut self, slot: usize, text: &TextItem) {
        let span = text.glyphs.first().map(|glyph| glyph.span.0);
        if let (Some(last), Some(span)) = (self.last[slot], span) {
            if self.separators.contains(&(last, span)) {
                self.buffers[slot].push(' ');
            }
        }
        self.buffers[slot].push_str(&text.text);
        if span.is_some() {
            self.last[slot] = span;
        }
    }
}

/// An inline leaf of a block: a text element, or a break between two of them.
#[derive(Clone, Copy)]
enum Leaf {
    Text(Span),
    Break,
}

fn first_text(leaves: &[Leaf]) -> Option<Span> {
    leaves.iter().find_map(|leaf| match leaf {
        Leaf::Text(span) => Some(*span),
        Leaf::Break => None,
    })
}

/// The inline leaves of an element in source order. A text element is a leaf and a line break
/// is a break. Otherwise only the fields that hold inline content are read: the term of a term
/// item (a break follows it), then the body, the children and the description.
fn collect_leaves(content: &Content, out: &mut Vec<Leaf>) {
    match content.elem().name() {
        "text" => out.push(Leaf::Text(content.span())),
        "linebreak" => out.push(Leaf::Break),
        _ => {
            if has_field(content, "term") {
                field_leaves(content, "term", out);
                out.push(Leaf::Break);
            }
            field_leaves(content, "body", out);
            field_leaves(content, "children", out);
            field_leaves(content, "description", out);
        }
    }
}

fn has_field(content: &Content, name: &str) -> bool {
    content.field_by_name(name).is_ok()
}

fn field_leaves(content: &Content, name: &str, out: &mut Vec<Leaf>) {
    match content.field_by_name(name) {
        Ok(Value::Content(child)) => collect_leaves(&child, out),
        Ok(Value::Array(items)) => {
            for item in items.iter() {
                if let Value::Content(child) = item {
                    collect_leaves(child, out);
                }
            }
        }
        _ => {}
    }
}
