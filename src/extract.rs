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

use std::collections::{HashMap, HashSet};

use typst::foundations::{Content, Value};
use unicode_normalization::UnicodeNormalization;
use typst::introspection::{Location, Tag};
use typst::layout::{Frame, FrameItem};
use typst::syntax::Span;
use typst::text::TextItem;
use typst_layout::PagedDocument;

const BLOCK_ELEMS: &[&str] = &["par", "heading", "caption", "item", "cell", "entry", "place"];
const SUPPRESSED_ELEMS: &[&str] = &["artifact", "footnote"];
/// Raw text and math: they are not erasable markup, so each is written as the placeholder.
const OPAQUE_ELEMS: &[&str] = &["raw", "equation"];

struct Open {
    loc: Option<Location>,
    name: &'static str,
    block: Option<usize>,
    suppressed: bool,
    /// A table cell that repeats a cell already written, such as a header the layout repeats.
    repeated: bool,
}

#[derive(Default)]
pub struct Extraction {
    pub blocks: Vec<String>,
    pub untagged: Vec<String>,
    pub dropped_artifacts: usize,
    pub clipped_runs: usize,
    pub repeated_runs: usize,
    pub unmatched_tags: usize,
}

/// A clip box in points, on the page.
#[derive(Clone, Copy)]
struct Clip {
    min: (f64, f64),
    max: (f64, f64),
}

impl Clip {
    fn contains(&self, (x, y): (f64, f64)) -> bool {
        x >= self.min.0 && x <= self.max.0 && y >= self.min.1 && y <= self.max.1
    }
}

impl Extraction {
    pub fn text(&self) -> String {
        crate::output::render(&self.blocks)
    }
}

pub fn extract(doc: &PagedDocument, placeholder: &str) -> Extraction {
    let mut state = State { placeholder: placeholder.to_string(), ..State::default() };
    for page in doc.pages() {
        state.frame(&page.frame, (0.0, 0.0), None);
    }
    state.out.unmatched_tags = state.open.len();
    state.out.blocks = state
        .buffers
        .iter()
        .map(|b| block_text(&plain_math(&b.replace('\u{ad}', ""))))
        .filter(|block| !is_placeholder_only(block, placeholder))
        .collect();
    state.out
}

/// A unit made only of placeholders has no authored text, so it is not emitted. This matches
/// source mode.
fn is_placeholder_only(block: &str, placeholder: &str) -> bool {
    !placeholder.is_empty() && block.split(placeholder).all(|part| part.trim().is_empty())
}

/// Collapses the ASCII whitespace of each line of a block, and keeps the line breaks between
/// the lines. Lines that are empty after collapsing are dropped. This is the normalization of the
/// source strategy, so an explicit line break is a newline in both modes.
fn block_text(text: &str) -> String {
    text.lines()
        .map(crate::output::collapse_ascii_spaces)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Replaces the mathematical alphanumerics (U+1D400 to U+1D7FF) by their compatibility
/// decomposition, which is the plain letter or digit. Rendered math uses these for its variables.
/// Only that range is decomposed, so other characters, such as U+00A0, keep their form.
fn plain_math(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if ('\u{1D400}'..='\u{1D7FF}').contains(&c) {
            out.extend(c.to_string().nfkc());
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Default)]
struct State {
    open: Vec<Open>,
    out: Extraction,
    buffers: Vec<String>,
    /// The span of the last run written to each block buffer.
    last: Vec<Option<Span>>,
    /// Pairs of consecutive spans that the element structure separates by a word boundary.
    separators: HashMap<(Span, Span), Boundary>,
    loose: Option<usize>,
    suppressed: usize,
    /// Cells already written, keyed by the table they belong to and their grid position.
    cells: HashSet<(Location, i64, i64)>,
    /// Text written in place of an opaque element: raw text and math, which are not erasable markup.
    placeholder: String,
    /// Depth of opaque elements that are open. Runs inside them are not written.
    opaque: usize,
    /// The bullet glyphs of the lists seen so far. A detached run with one of these texts is a
    /// marker, and it is not written.
    bullets: HashSet<String>,
    /// The bibliography, row and buffer of the last bibliography cell written.
    bib_row: Option<(Location, i64, usize)>,
    /// Number of open repeated cells. Runs inside them are not written.
    repeating: usize,
}

impl State {
    /// A cell is a repeat when the same table has already written a cell at its grid position.
    /// Tables are keyed by location, and a repeated header is laid out again with the same
    /// positions but new cell locations.
    fn is_repeated_cell(&mut self, content: &Content) -> bool {
        let table = self.open.iter().rev().find(|o| o.name == "table").and_then(|o| o.loc);
        let (Some(table), Ok(Value::Int(x)), Ok(Value::Int(y))) =
            (table, content.field_by_name("x"), content.field_by_name("y"))
        else {
            return false;
        };
        !self.cells.insert((table, x, y))
    }

    fn new_buffer(&mut self) -> usize {
        self.buffers.push(String::new());
        self.last.push(None);
        self.buffers.len() - 1
    }

    /// Walks a frame. `origin` is the position of the frame's origin on the page, in points, and
    /// `clip` is the innermost clip box that applies to it.
    fn frame(&mut self, frame: &Frame, origin: (f64, f64), clip: Option<Clip>) {
        for (pos, item) in frame.items() {
            match item {
                FrameItem::Tag(Tag::Start(content, _)) => {
                    let name = content.elem().name();
                    let suppressed = SUPPRESSED_ELEMS.contains(&name);
                    if suppressed {
                        self.suppressed += 1;
                    }
                    // Raw text and math are not erasable markup. The placeholder stands for the
                    // whole element, once, and the runs inside it are not written.
                    if OPAQUE_ELEMS.contains(&name) {
                        if self.opaque == 0 {
                            self.write_placeholder();
                        }
                        self.opaque += 1;
                    }
                    let block = (BLOCK_ELEMS.contains(&name) || is_block_quote(content)).then(|| {
                        self.loose = None;
                        self.block_buffer(content, name)
                    });
                    if block.is_some() {
                        self.record_boundaries(content);
                    }
                    if name == "list" {
                        self.bullets.extend(list_markers(content));
                    }
                    if matches!(name, "list" | "enum") {
                        // A list is block-level, so it starts a unit, and its first marker is in it.
                        self.loose = None;
                        if let Some(slot) = self.open.iter().rev().find_map(|o| o.block) {
                            self.start_unit_at(slot);
                        }
                        self.record_item_boundaries(content);
                    }
                    let repeated = name == "cell" && self.is_repeated_cell(content);
                    if repeated {
                        self.repeating += 1;
                    }
                    self.open.push(Open { loc: content.location(), name, block, suppressed, repeated });
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
                        if OPAQUE_ELEMS.contains(&closed.name) {
                            self.opaque -= 1;
                        }
                        if closed.repeated {
                            self.repeating -= 1;
                        }
                        // What follows a list is after the list's units, so the enclosing block
                        // continues in a new buffer.
                        if matches!(closed.name, "item" | "list" | "enum") {
                            self.loose = None;
                            if let Some(slot) = self.open.iter().rev().find_map(|o| o.block) {
                                self.start_unit_at(slot);
                            }
                        }
                        if closed.block.is_some() {
                            self.loose = None;
                        }
                    }
                }
                FrameItem::Text(text) => {
                    if self.opaque > 0 {
                        continue;
                    }
                    if self.suppressed > 0 {
                        self.out.dropped_artifacts += 1;
                        continue;
                    }
                    if self.repeating > 0 {
                        self.out.repeated_runs += 1;
                        continue;
                    }
                    let start = (origin.0 + pos.x.to_pt(), origin.1 + pos.y.to_pt());
                    if clip.is_some_and(|clip| !clip.contains(start)) {
                        self.out.clipped_runs += 1;
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
                            if text.glyphs.first().is_some_and(|glyph| glyph.span.0 == Span::detached())
                                && self.bullets.contains(text.text.as_str())
                            {
                                continue;
                            }
                            self.push_generated_gap(slot, text);
                            self.buffers[slot].push_str(&text.text);
                            self.out.untagged.push(text.text.to_string());
                        }
                    }
                }
                FrameItem::Group(group) => {
                    let origin = (
                        origin.0 + pos.x.to_pt() + group.transform.tx.to_pt(),
                        origin.1 + pos.y.to_pt() + group.transform.ty.to_pt(),
                    );
                    let clip = group
                        .clip
                        .as_ref()
                        .map(|curve| {
                            let bbox = curve.bbox(None);
                            Clip {
                                min: (origin.0 + bbox.min.x.to_pt(), origin.1 + bbox.min.y.to_pt()),
                                max: (origin.0 + bbox.max.x.to_pt(), origin.1 + bbox.max.y.to_pt()),
                            }
                        })
                        .or(clip);
                    self.frame(&group.frame, origin, clip);
                }
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
        let mut broken = None;
        for leaf in &leaves {
            match *leaf {
                Leaf::Break => broken = Some(Boundary::Newline),
                Leaf::Unit => broken = Some(Boundary::Paragraph),
                Leaf::Text(span) => {
                    if let (Some(previous), Some(boundary)) = (previous, broken) {
                        self.separators.insert((previous, span), boundary);
                    }
                    previous = Some(span);
                    broken = None;
                }
            }
        }
        let numbered = content.field_by_name("numbering").is_ok_and(|value| !matches!(value, Value::None));
        if let (true, Some(first)) = (numbered, first_text(&leaves)) {
            self.separators.insert((content.span(), first), Boundary::Space);
        }
        // The attribution of a block quote is generated, so its run is detached. It starts a unit
        // of its own after the last run of the body.
        let attributed = content.field_by_name("attribution").is_ok_and(|value| !matches!(value, Value::None));
        if let (true, Some(last)) = (attributed, last_text(&leaves)) {
            self.separators.insert((last, Span::detached()), Boundary::Paragraph);
        }
        match content.elem().name() {
            // A list item's marker is generated, so its run is detached, and it precedes the body.
            "item" => {
                if let Some(first) = first_text(&leaves) {
                    self.separators.insert((Span::detached(), first), Boundary::Space);
                }
            }
            // An outline entry's number is generated and detached, and it precedes the title.
            // A footnote entry's number is generated from the footnote, and it precedes the note.
            "entry" => {
                if let Some(element) = field_content(content, "element") {
                    let numbered = element.field_by_name("numbering").is_ok_and(|v| !matches!(v, Value::None));
                    if let (true, Some(first)) = (numbered, first_leaf_text(&element)) {
                        self.separators.insert((Span::detached(), first), Boundary::Space);
                    }
                } else if let Some(note) = field_content(content, "note") {
                    let numbered = note.field_by_name("numbering").is_ok_and(|v| !matches!(v, Value::None));
                    if let (true, Some(first)) = (numbered, first_leaf_text(&note)) {
                        self.separators.insert((Span::detached(), first), Boundary::Space);
                    }
                }
            }
            _ => {}
        }
    }

    /// Records the boundaries of the items of a list or an enum. Each item's marker is generated,
    /// so it is a detached run. A marker starts a unit, and it is separated from its item's text
    /// by a space. The items are not tagged, so the list element is the only place they appear.
    fn record_item_boundaries(&mut self, content: &Content) {
        let Ok(Value::Array(items)) = content.field_by_name("children") else {
            return;
        };
        let mut previous = None;
        for item in items.iter() {
            let Value::Content(item) = item else { continue };
            let mut leaves = Vec::new();
            collect_leaves(item, &mut leaves);
            if let Some(last) = previous {
                self.separators.insert((last, Span::detached()), Boundary::Paragraph);
            }
            if let Some(first) = first_text(&leaves) {
                self.separators.insert((Span::detached(), first), Boundary::Space);
            }
            previous = last_text(&leaves);
        }
    }

    /// The buffer a block element writes into. A cell of a bibliography that is in the same row
    /// as the previous cell joins that cell's buffer with a space, so a reference label and its
    /// entry form one unit. Other blocks start a unit of their own.
    fn block_buffer(&mut self, content: &Content, name: &'static str) -> usize {
        // A paragraph that begins right after a generated marker is the body of the item the
        // marker belongs to, so it continues the buffer of that item.
        if name == "par" {
            if let Some(slot) = self.open.iter().rev().find_map(|o| o.block) {
                if self.last[slot] == Some(Span::detached()) && slot + 1 == self.buffers.len() {
                    return slot;
                }
            }
        }
        if name == "cell" {
            let bibliography = self.open.iter().rev().find(|o| o.name == "bibliography").and_then(|o| o.loc);
            if let (Some(bibliography), Ok(Value::Int(y))) = (bibliography, content.field_by_name("y")) {
                if let Some((last_bibliography, last_y, slot)) = self.bib_row {
                    if last_bibliography == bibliography && last_y == y {
                        self.buffers[slot].push(' ');
                        return slot;
                    }
                }
                let slot = self.new_buffer();
                self.bib_row = Some((bibliography, y, slot));
                return slot;
            }
        }
        self.new_buffer()
    }

    /// Writes a run into a block buffer, and returns the buffer it was written to. A boundary
    /// recorded for the pair of spans is written as one space before the run, or starts a new unit.
    fn push_run(&mut self, slot: usize, text: &TextItem) {
        // Typst merges neighbouring text leaves into one run, so a run starts at the span of its
        // first glyph and ends at the span of its last glyph. Boundaries are matched on those.
        let first = text.glyphs.first().map(|glyph| glyph.span.0);
        let last = text.glyphs.last().map(|glyph| glyph.span.0);
        let mut slot = slot;
        if let (Some(previous), Some(first)) = (self.last[slot], first) {
            match self.separators.get(&(previous, first)) {
                // A word or a line boundary is not written before a generated run. Generated runs
                // are set apart by the rule below, and a dot leader is a run of such runs.
                Some(Boundary::Space) if first != Span::detached() => self.buffers[slot].push(' '),
                Some(Boundary::Newline) if first != Span::detached() => self.buffers[slot].push('\n'),
                Some(Boundary::Paragraph) => slot = self.start_unit(),
                Some(_) => {}
                // A generated run, such as a marker or a number, is set apart from the text after it.
                None if previous == Span::detached() && first != Span::detached() && text.text.starts_with(char::is_alphanumeric) => {
                    self.buffers[slot].push(' ')
                }
                None => {}
            }
        }
        // A list marker is a bullet glyph, which is generated and is not prose. Its boundary was
        // applied above, so only the text is dropped.
        if first == Some(Span::detached()) && self.bullets.contains(text.text.as_str()) {
            self.last[slot] = Some(Span::detached());
            return;
        }
        self.buffers[slot].push_str(&text.text);
        if last.is_some() {
            self.last[slot] = last;
        }
    }

    /// Writes the placeholder for an opaque element into the innermost block, or into the untagged
    /// text. A placeholder that already ends the text is not written again.
    fn write_placeholder(&mut self) {
        if self.placeholder.is_empty() {
            return;
        }
        let slot = match self.open.iter().rev().find_map(|o| o.block) {
            Some(slot) => slot,
            None => match self.loose {
                Some(slot) => slot,
                None => {
                    let slot = self.new_buffer();
                    self.loose = Some(slot);
                    slot
                }
            },
        };
        if !self.buffers[slot].trim_end_matches([' ', '\t', '\n']).ends_with(self.placeholder.as_str()) {
            self.buffers[slot].push_str(&self.placeholder);
        }
        self.last[slot] = None;
    }

    /// A generated run, such as a marker or a number, is set apart from the text after it. This
    /// applies to untagged text as well as to block text.
    fn push_generated_gap(&mut self, slot: usize, text: &TextItem) {
        let detached = Span::detached();
        let first = text.glyphs.first().map(|glyph| glyph.span.0);
        if let (Some(previous), Some(first)) = (self.last[slot], first) {
            if previous == detached && first != detached && text.text.starts_with(char::is_alphanumeric) {
                self.buffers[slot].push(' ');
            }
        }
        if let Some(last) = text.glyphs.last().map(|glyph| glyph.span.0) {
            self.last[slot] = Some(last);
        }
    }

    /// Starts a new unit in the innermost open block, and returns its buffer.
    fn start_unit(&mut self) -> usize {
        let slot = self.new_buffer();
        if let Some(open) = self.open.iter_mut().rev().find(|o| o.block.is_some()) {
            open.block = Some(slot);
        }
        slot
    }

    /// Starts a new unit in the open block that uses the buffer `slot`.
    fn start_unit_at(&mut self, slot: usize) {
        let new = self.new_buffer();
        if let Some(open) = self.open.iter_mut().rev().find(|o| o.block == Some(slot)) {
            open.block = Some(new);
        }
    }
}

/// What a recorded pair of spans separates: a word, a line, or a unit.
#[derive(Clone, Copy)]
enum Boundary {
    Space,
    Newline,
    Paragraph,
}

/// The marker strings of a list, which are bullet glyphs and not prose.
fn list_markers(content: &Content) -> Vec<String> {
    let mut markers = Vec::new();
    let mut take = |value: &Value| {
        if let Value::Content(marker) = value {
            if let Ok(Value::Str(text)) = marker.field_by_name("text") {
                markers.push(text.as_str().to_string());
            }
        }
    };
    match content.field_by_name("marker") {
        Ok(Value::Array(items)) => items.iter().for_each(|item| take(item)),
        Ok(value) => take(&value),
        Err(_) => {}
    }
    markers
}

fn field_content(content: &Content, name: &str) -> Option<Content> {
    match content.field_by_name(name) {
        Ok(Value::Content(child)) => Some(child),
        _ => None,
    }
}

fn first_leaf_text(content: &Content) -> Option<Span> {
    let mut leaves = Vec::new();
    collect_leaves(content, &mut leaves);
    first_text(&leaves)
}

fn last_text(leaves: &[Leaf]) -> Option<Span> {
    leaves.iter().rev().find_map(|leaf| match leaf {
        Leaf::Text(span) => Some(*span),
        Leaf::Break | Leaf::Unit => None,
    })
}

fn is_block_quote(content: &Content) -> bool {
    content.elem().name() == "quote" && content.field_by_name("block").is_ok_and(|value| matches!(value, Value::Bool(true)))
}

/// An inline leaf of a block: a text element, or a break between two of them.
#[derive(Clone, Copy)]
enum Leaf {
    Text(Span),
    /// A line break.
    Break,
    /// The end of a term, which separates it from its description as a unit.
    Unit,
}

fn first_text(leaves: &[Leaf]) -> Option<Span> {
    leaves.iter().find_map(|leaf| match leaf {
        Leaf::Text(span) => Some(*span),
        Leaf::Break | Leaf::Unit => None,
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
                out.push(Leaf::Unit);
            }
            field_leaves(content, "body", out);
            field_leaves(content, "child", out);
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
