//! Source strategy: walks the Typst syntax tree of the entry file, and of imports and
//! includes whose path is a string literal, without evaluating anything.
//!
//! Boundary policy (also documented in `typst-prose source --help`):
//! - Paragraph breaks, headings, list, enum and term items each end the current unit.
//! - A content block passed as a non-trailing argument, or any content block in a
//!   code position that is not a trailing argument, is its own unit.
//! - A trailing content block (`f(..)[body]`) and a bare `[body]` stay inline, except a
//!   footnote call, `#footnote[body]`, whose body is its own unit.
//! - Any other embedded element (code expression, math, raw, link, reference) ends
//!   the current unit, so the prose on either side is emitted as separate units.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use typst_syntax::ast::{AstNode, Escape, Shorthand, Str};
use typst_syntax::{LinkedNode, Lines, PathError, SyntaxKind, VirtualPath, parse};

use crate::project::Project;

pub struct Extraction {
    pub units: Vec<String>,
    pub trace: String,
    pub warnings: Vec<String>,
}

pub fn extract(project: &Project, trace: bool) -> Result<Extraction, Vec<String>> {
    let mut walker = Walker {
        root: project.root.clone(),
        stack: Vec::new(),
        imported: HashSet::new(),
        units: Vec::new(),
        current: String::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
        trace: trace.then(String::new),
    };
    walker.file(&project.main);
    if !walker.errors.is_empty() {
        return Err(walker.errors);
    }
    walker.break_unit();
    if let Some(trace) = &mut walker.trace {
        let _ = writeln!(trace, "# end: units={} warnings={}", walker.units.len(), walker.warnings.len());
    }
    Ok(Extraction {
        units: walker.units,
        trace: walker.trace.unwrap_or_default(),
        warnings: walker.warnings,
    })
}

struct Doc<'a> {
    /// The file's path relative to the project root, as shown in diagnostics.
    path: &'a str,
    lines: Lines<&'a str>,
}

impl Doc<'_> {
    fn at(&self, node: &LinkedNode) -> String {
        let (line, column) = self.lines.byte_to_line_column(node.offset()).unwrap_or_default();
        format!("{}:{}:{}", self.path, line + 1, column + 1)
    }
}

struct Walker {
    /// Canonical project root. Virtual paths are realized against it.
    root: PathBuf,
    /// Virtual paths of the files currently being walked. The last is the current file.
    stack: Vec<VirtualPath>,
    /// Virtual paths of modules already walked through `import`.
    imported: HashSet<VirtualPath>,
    units: Vec<String>,
    current: String,
    errors: Vec<String>,
    warnings: Vec<String>,
    trace: Option<String>,
}

impl Walker {
    fn file(&mut self, vpath: &VirtualPath) {
        let display = vpath.get_without_slash();
        let real = match vpath.realize(&self.root) {
            Ok(real) => real,
            Err(e) => {
                self.errors.push(format!("error: {display}: {e:?}"));
                return;
            }
        };
        let text = match fs::read_to_string(&real) {
            Ok(text) => text,
            Err(e) => {
                self.errors.push(format!("error: {display}: {e}"));
                return;
            }
        };
        let syntax = parse(&text);
        let doc = Doc { path: display, lines: Lines::new(text.as_str()) };
        let root = LinkedNode::new(&syntax);

        let before = self.errors.len();
        self.syntax_errors(&doc, &root);
        if self.errors.len() > before {
            return;
        }

        self.stack.push(vpath.clone());
        self.markup_node(&doc, &root);
        self.stack.pop();
    }

    fn syntax_errors(&mut self, doc: &Doc, node: &LinkedNode) {
        if !node.diagnosis().errors {
            return;
        }
        if node.kind() == SyntaxKind::Error {
            for error in node.errors_and_warnings().0 {
                self.errors.push(format!("error: {}: {}", doc.at(node), error.message));
            }
            return;
        }
        for child in node.children() {
            self.syntax_errors(doc, &child);
        }
    }

    fn markup(&mut self, doc: &Doc, node: &LinkedNode) {
        let children: Vec<LinkedNode> = node.children().collect();
        let mut i = 0;
        while i < children.len() {
            if children[i].kind() == SyntaxKind::Hash && i + 1 < children.len() {
                self.embedded(doc, &children[i + 1]);
                i += 2;
            } else {
                self.markup_node(doc, &children[i]);
                i += 1;
            }
        }
    }

    fn children(&mut self, doc: &Doc, node: &LinkedNode) {
        for child in node.children() {
            self.markup_node(doc, &child);
        }
    }

    fn markup_node(&mut self, doc: &Doc, node: &LinkedNode) {
        match node.kind() {
            SyntaxKind::Markup => self.markup(doc, node),
            SyntaxKind::Text | SyntaxKind::SmartQuote => self.current.push_str(node.leaf_text()),
            SyntaxKind::Escape => {
                if let Some(escape) = Escape::from_untyped(node.get()) {
                    self.current.push(escape.get());
                }
            }
            SyntaxKind::Shorthand => {
                if let Some(shorthand) = Shorthand::from_untyped(node.get()) {
                    self.current.push(shorthand.get());
                }
            }
            SyntaxKind::Space => self.current.push(' '),
            SyntaxKind::Linebreak => self.current.push('\n'),
            SyntaxKind::Parbreak => self.gap(doc, node, "paragraph break"),
            SyntaxKind::Colon => self.gap(doc, node, "term separator"),
            SyntaxKind::Heading | SyntaxKind::ListItem | SyntaxKind::EnumItem | SyntaxKind::TermItem => {
                self.gap(doc, node, "block item");
                self.children(doc, node);
                self.break_unit();
            }
            SyntaxKind::ContentBlock | SyntaxKind::Ref => {
                self.gap(doc, node, "content block or reference");
                self.children(doc, node);
                self.break_unit();
            }
            SyntaxKind::Raw | SyntaxKind::Link | SyntaxKind::Equation => {
                self.gap(doc, node, "skipped non-prose element");
            }
            SyntaxKind::Label | SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::Shebang => {}
            _ => self.children(doc, node),
        }
    }

    /// An expression after `#` in markup.
    fn embedded(&mut self, doc: &Doc, expr: &LinkedNode) {
        match expr.kind() {
            SyntaxKind::ContentBlock => self.children(doc, expr),
            _ if is_inline_call(expr) => self.code(doc, expr),
            _ => {
                self.gap(doc, expr, "embedded code expression");
                self.code(doc, expr);
                self.break_unit();
            }
        }
    }

    /// A code expression. Content blocks in it are markup, and imports are followed.
    fn code(&mut self, doc: &Doc, node: &LinkedNode) {
        match node.kind() {
            SyntaxKind::ContentBlock if is_trailing_argument(node) => self.children(doc, node),
            SyntaxKind::ContentBlock => {
                self.gap(doc, node, "content block argument or definition");
                self.children(doc, node);
                self.break_unit();
            }
            SyntaxKind::ModuleImport | SyntaxKind::ModuleInclude => {
                self.gap(doc, node, "import or include");
                self.follow(doc, node);
                self.break_unit();
            }
            _ => {
                for child in node.children() {
                    self.code(doc, &child);
                }
            }
        }
    }

    /// Resolves an import or include path the way Typst does: relative to the directory of
    /// the importing file, with a leading `/` starting at the project root. A `..` that would
    /// leave the root is an error.
    fn follow(&mut self, doc: &Doc, node: &LinkedNode) {
        let Some(path) = node.children().find(|c| c.kind() == SyntaxKind::Str) else {
            self.warn(doc, node, "import or include path is not a string literal; not followed");
            return;
        };
        let Some(literal) = Str::from_untyped(path.get()).map(|s| s.get()) else {
            return;
        };
        if literal.starts_with('@') {
            self.warn(doc, node, "package import; not followed");
            return;
        }

        let Some(base) = self.stack.last() else { return };
        let joined = match base.parent() {
            Some(parent) => parent.join(&literal),
            None => base.join(&literal),
        };
        let target = match joined {
            Ok(target) => target,
            Err(PathError::Escapes) => {
                self.errors.push(format!(
                    "error: {}: path `{literal}` would escape the project root (set --root or TYPST_ROOT)",
                    doc.at(node)
                ));
                return;
            }
            Err(e) => {
                self.errors.push(format!("error: {}: path `{literal}`: {e}", doc.at(node)));
                return;
            }
        };
        let shown = target.get_without_slash().to_string();
        if self.stack.contains(&target) {
            self.errors.push(format!("error: {}: import cycle through {shown}", doc.at(node)));
            return;
        }
        // A module imported twice contributes its markup once. An include renders
        // at each occurrence, so it is walked each time.
        if node.kind() == SyntaxKind::ModuleImport && !self.imported.insert(target.clone()) {
            self.note(doc, node, &format!("already imported {shown}"));
            return;
        }
        self.note(doc, node, &format!("follow {shown}"));
        self.file(&target);
    }

    fn gap(&mut self, doc: &Doc, node: &LinkedNode, why: &str) {
        self.note(doc, node, why);
        self.break_unit();
    }

    fn warn(&mut self, doc: &Doc, node: &LinkedNode, message: &str) {
        self.warnings.push(format!("warning: {}: {message}", doc.at(node)));
    }

    fn note(&mut self, doc: &Doc, node: &LinkedNode, what: &str) {
        if let Some(trace) = &mut self.trace {
            let _ = writeln!(trace, "{} {:?} {what}", doc.at(node), node.kind());
        }
    }

    fn break_unit(&mut self) {
        let unit = normalize(&std::mem::take(&mut self.current));
        if unit.is_empty() {
            return;
        }
        if let Some(trace) = &mut self.trace {
            let _ = writeln!(trace, "UNIT {}: {unit:?}", self.units.len() + 1);
        }
        self.units.push(unit);
    }
}

/// `#f(..)[body]` and similar: a call whose last argument is a trailing content block. A
/// footnote call is excluded: its body is set at the foot of the page, out of the text flow.
fn is_inline_call(expr: &LinkedNode) -> bool {
    expr.kind() == SyntaxKind::FuncCall
        && !is_footnote_call(expr)
        && expr.children().last().is_some_and(|args| {
            args.kind() == SyntaxKind::Args
                && args.children().last().is_some_and(|c| c.kind() == SyntaxKind::ContentBlock)
        })
}

/// A call whose callee is the identifier `footnote`. This is a name check, so a user function
/// with that name is treated the same way.
fn is_footnote_call(expr: &LinkedNode) -> bool {
    expr.children()
        .next()
        .is_some_and(|callee| callee.kind() == SyntaxKind::Ident && callee.leaf_text().as_str() == "footnote")
}

fn is_trailing_argument(node: &LinkedNode) -> bool {
    node.parent_kind() == Some(SyntaxKind::Args) && node.next_sibling().is_none()
}

/// Removes soft hyphens, collapses whitespace inside each line, and drops empty lines.
/// Explicit line breaks survive as newlines.
fn normalize(raw: &str) -> String {
    raw.replace('\u{ad}', "")
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
