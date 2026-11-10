use typst::introspection::{Location, Tag};
use typst::layout::{Frame, FrameItem};
use typst::syntax::Span;
use typst::WorldExt;
use typst_layout::PagedDocument;

use crate::world::FileWorld;

pub struct Tracer<'a> {
    world: &'a FileWorld,
    open: Vec<(String, Option<Location>)>,
    pub mismatches: usize,
    pub out: String,
}

impl<'a> Tracer<'a> {
    pub fn new(world: &'a FileWorld) -> Self {
        Self { world, open: Vec::new(), mismatches: 0, out: String::new() }
    }

    pub fn document(&mut self, doc: &PagedDocument) {
        for (i, page) in doc.pages().iter().enumerate() {
            self.frame(&page.frame, i + 1, &[], 0);
        }
        self.out.push_str(&format!(
            "# end: unclosed tags={} mismatched end tags={}\n",
            self.open.len(),
            self.mismatches
        ));
    }

    fn frame(&mut self, frame: &Frame, page: usize, path: &[usize], depth: usize) {
        let path_str = path.iter().map(usize::to_string).collect::<Vec<_>>().join(".");
        for (seq, (_pos, item)) in frame.items().enumerate() {
            let head = format!("p{page} path=[{path_str}] #{seq:<3}");
            match item {
                FrameItem::Tag(Tag::Start(content, flags)) => {
                    let name = content.elem().name();
                    let loc = content.location();
                    self.open.push((name.to_string(), loc));
                    self.out.push_str(&format!(
                        "{head} TAG_START elem={name} loc={loc:?} introspectable={} tagged={} open_depth={}\n",
                        flags.introspectable,
                        flags.tagged,
                        self.open.len()
                    ));
                }
                FrameItem::Tag(Tag::End(loc, _key, flags)) => {
                    let matched = self.open.last().map(|(_, l)| *l == Some(*loc)).unwrap_or(false);
                    if !matched {
                        self.mismatches += 1;
                    }
                    let name = self.open.pop().map(|(n, _)| n).unwrap_or_else(|| "?".into());
                    self.out.push_str(&format!(
                        "{head} TAG_END   elem={name} loc={loc:?} matched={matched} tagged={} open_depth={}\n",
                        flags.tagged,
                        self.open.len()
                    ));
                }
                FrameItem::Text(text) => {
                    let spans: Vec<Span> = text.glyphs.iter().map(|g| g.span.0).collect();
                    let src = self.source_range(&spans);
                    self.out.push_str(&format!(
                        "{head} TEXT lang={} glyphs={} src={} text={:?}\n",
                        text.lang.as_str(),
                        text.glyphs.len(),
                        src,
                        text.text.as_str()
                    ));
                }
                FrameItem::Group(group) => {
                    self.out.push_str(&format!(
                        "{head} GROUP parent={:?} label={:?} clip={}\n",
                        group.parent,
                        group.label,
                        group.clip.is_some()
                    ));
                    let mut child = path.to_vec();
                    child.push(seq);
                    self.frame(&group.frame, page, &child, depth + 1);
                }
                FrameItem::Shape(..) => self.out.push_str(&format!("{head} SHAPE\n")),
                FrameItem::Image(image, _, _) => {
                    self.out.push_str(&format!("{head} IMAGE alt={:?}\n", image.alt()))
                }
                FrameItem::Link(..) => self.out.push_str(&format!("{head} LINK\n")),
            }
        }
    }

    fn source_range(&self, spans: &[Span]) -> String {
        let Some(first) = spans.first() else {
            return "none".into();
        };
        let Some(id) = first.id() else {
            return "detached".into();
        };
        let ranges: Vec<_> = spans
            .iter()
            .filter_map(|s| self.world.range(*s))
            .collect();
        match (ranges.first(), ranges.last()) {
            (Some(a), Some(b)) => format!("{}:{}..{}", id.vpath().get_without_slash(), a.start, b.end),
            _ => "unresolved".into(),
        }
    }
}
