//! Compiled strategy: reads text runs and semantic tags from the `PagedDocument`. It reads
//! no source text, coordinates or layout geometry.
//!
//! Rules as implemented:
//! - A unit opens at each `par`, `heading`, `caption`, `item`, `cell`, `entry` (footnote body)
//!   and `place` tag, and collects every text run inside it, nested or not.
//! - Text outside any unit is emitted as its own unit in position order and reported on stderr
//!   as `untagged text`. It is never dropped or merged across unit boundaries.
//! - Runs under `artifact` (page furniture) and `footnote` (the in-text reference marker) are
//!   suppressed and counted on stderr.
//! - U+00AD (soft hyphen, inserted at automatic hyphenation) is removed.
//! - Whitespace inside a unit is normalized. Runs keep their own leading and trailing spaces.
//! - Not implemented: separating generated markers from prose, using `GroupItem::parent`, and
//!   reconstructing math source. The frame facts behind these rules and the open work are in
//!   docs/EXTRACTION-COMPARISON.md.

use typst::introspection::{Location, Tag};
use typst::layout::{Frame, FrameItem};
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
    loose: Option<usize>,
    suppressed: usize,
}

impl State {
    fn new_buffer(&mut self) -> usize {
        self.buffers.push(String::new());
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
                        Some(slot) => self.buffers[slot].push_str(&text.text),
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
}
