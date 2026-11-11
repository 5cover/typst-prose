//! Source strategy: walks the Typst syntax tree of the entry file, and of imports and
//! includes whose path is a string literal, without evaluating anything.
//!
//! Boundary policy (also documented in `typst-prose source --help`):
//! - Paragraph breaks, headings, list, enum and term items each end the current unit.
//! - A content block is an inline value wherever it is written: in markup, as a trailing
//!   argument, or as any other argument. It is set off from its neighbours by one space, so
//!   words never join. No blank line is added, because the source does not say whether the
//!   call places the block in a paragraph.
//! - Inline elements that render no authored text (a reference, math, raw text, a link, or an
//!   expression that yields a value) are replaced by the placeholder on the same line. They
//!   never start a unit, so a sentence containing one stays on one line.
//! - Statements that produce no text (`#let`, `#set`, `#show`, imports) end the current unit.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use typst_syntax::ast::{AstNode, Escape, Shorthand, Str};
use typst_syntax::{LinkedNode, Lines, PathError, SyntaxKind, VirtualPath, parse};

use crate::output::collapse_ascii_spaces;
use crate::project::Project;

pub struct Extraction {
    pub units: Vec<String>,
    pub trace: String,
    pub warnings: Vec<String>,
}

pub fn extract(project: &Project, trace: bool, placeholder: &str) -> Result<Extraction, Vec<String>> {
    let mut walker = Walker {
        root: project.root.clone(),
        stack: Vec::new(),
        imported: HashSet::new(),
        units: Vec::new(),
        current: String::new(),
        placeholder: placeholder.to_string(),
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
    /// Text written in place of an inline element that renders no authored text.
    placeholder: String,
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
            SyntaxKind::ContentBlock => {
                self.gap(doc, node, "content block");
                self.children(doc, node);
                self.break_unit();
            }
            SyntaxKind::Ref => {
                self.inline_gap(doc, node, "reference");
                self.children(doc, node);
            }
            SyntaxKind::Raw | SyntaxKind::Link | SyntaxKind::Equation => {
                self.inline_gap(doc, node, "non-prose element");
            }
            SyntaxKind::Label | SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::Shebang => {}
            _ => self.children(doc, node),
        }
    }

    /// An expression after `#` in markup.
    fn embedded(&mut self, doc: &Doc, expr: &LinkedNode) {
        match expr.kind() {
            SyntaxKind::ContentBlock => self.value(doc, expr),
            _ if is_value_expr(expr) && has_content_block(expr) => self.code(doc, expr),
            _ if is_value_expr(expr) => self.inline_gap(doc, expr, "value expression"),
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
            SyntaxKind::ContentBlock => self.value(doc, node),
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

    /// A content block. The space on each side keeps its words apart from the text around it.
    /// Normalization collapses the extra spaces.
    fn value(&mut self, doc: &Doc, node: &LinkedNode) {
        self.note(doc, node, "content value");
        self.current.push(' ');
        self.children(doc, node);
        self.current.push(' ');
    }

    /// An element inside a sentence. It writes the placeholder without ending the unit. A
    /// placeholder that follows another one, separated only by ASCII whitespace, adds nothing.
    fn inline_gap(&mut self, doc: &Doc, node: &LinkedNode, why: &str) {
        self.note(doc, node, why);
        if !self.ends_with_placeholder() {
            self.current.push_str(&self.placeholder);
        }
    }

    fn ends_with_placeholder(&self) -> bool {
        let before = self.current.trim_end_matches([' ', '\t', '\n']);
        !self.placeholder.is_empty() && before.ends_with(self.placeholder.as_str())
    }

    /// A unit made only of placeholders has no authored text, so it is not emitted.
    fn is_placeholder_only(&self, unit: &str) -> bool {
        !self.placeholder.is_empty() && unit.split(self.placeholder.as_str()).all(|part| part.trim().is_empty())
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
        if unit.is_empty() || self.is_placeholder_only(&unit) {
            return;
        }
        if let Some(trace) = &mut self.trace {
            let _ = writeln!(trace, "UNIT {}: {unit:?}", self.units.len() + 1);
        }
        self.units.push(unit);
    }
}

/// An expression that yields a value. Its text is not written, so it is replaced by the
/// placeholder inline, unless it contains a content block. A statement such as `#let` is not a
/// value.
fn is_value_expr(expr: &LinkedNode) -> bool {
    matches!(
        expr.kind(),
        SyntaxKind::FuncCall
            | SyntaxKind::FieldAccess
            | SyntaxKind::Ident
            | SyntaxKind::Parenthesized
            | SyntaxKind::Unary
            | SyntaxKind::Binary
            | SyntaxKind::Str
            | SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Numeric
            | SyntaxKind::Bool
            | SyntaxKind::None
            | SyntaxKind::Auto
            | SyntaxKind::Array
            | SyntaxKind::Dict
    )
}

fn has_content_block(node: &LinkedNode) -> bool {
    node.kind() == SyntaxKind::ContentBlock || node.children().any(|child| has_content_block(&child))
}

/// Removes soft hyphens, collapses ASCII whitespace inside each line, and drops empty lines.
/// Explicit line breaks survive as newlines. Non-breaking spaces are kept.
fn normalize(raw: &str) -> String {
    raw.replace('\u{ad}', "")
        .lines()
        .map(collapse_ascii_spaces)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
