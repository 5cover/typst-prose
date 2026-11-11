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
        .map(|b| {
            b.replace('\u{ad}', "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
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
