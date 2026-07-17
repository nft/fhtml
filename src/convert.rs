//! HTML → fhtml converter (feature `convert`).
//!
//! Pipeline: HTML text ──html5ever──▶ DOM ──this module──▶ fhtml AST
//! (`parser::Node`) ──fmt.rs──▶ canonical .fhtml. The converter never emits
//! strings directly, so output is canonical by construction.
//!
//! Fidelity contract (plan §4): converting and recompiling preserves the
//! *normalized* DOM — element tree, attributes as sets, text compared after
//! whitespace collapsing (exact inside `pre`-class subtrees). `check()` is
//! the machine verifier for that contract.

use html5ever::serialize::{serialize, SerializeOpts, TraversalScope};
use html5ever::tendril::TendrilSink;
use html5ever::{ns, parse_document, parse_fragment, Attribute, LocalName, QualName};
use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};

use crate::fmt::format_nodes;
use crate::parser::{lit_parts, AttrValue, ClassItem, Element, Node, TextPart, RESERVED};

pub struct Options {
    /// Convert `<svg>`/`<math>` subtrees to fhtml elements instead of raw
    /// passthrough.
    pub convert_svg: bool,
    /// Synthesize `>` chains for single-child wrappers.
    pub chains: bool,
    /// Parse as a fragment with this context element (e.g. `table` for a bare
    /// `<tr>`) instead of as a document.
    pub fragment: Option<String>,
    /// Contract class tokens through the shorthand codebook and emit the
    /// `#!shorthand` directive.
    pub shorthand: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            convert_svg: false,
            chains: true,
            fragment: None,
            shorthand: false,
        }
    }
}

pub struct Converted {
    pub fhtml: String,
    pub warnings: Vec<String>,
}

/// Converts HTML text to canonical fhtml. Infallible by design: real-world
/// HTML always parses (spec error recovery), and every DOM shape has a
/// representation (worst case: raw passthrough). Problems surface as warnings.
pub fn convert(html: &str, opts: &Options) -> Converted {
    let mut conv = Conv {
        opts,
        warnings: Vec::new(),
    };
    let roots = parse_roots(html, opts);
    let mut nodes = Vec::new();
    for item in items_of(&roots.roots) {
        conv.convert_item(&item, &mut nodes);
    }
    let body = format_nodes(&nodes);
    Converted {
        // The directive must precede all content so the compiler decodes every
        // element's classes (parser sets the flag on first sight).
        fhtml: if opts.shorthand {
            format!("#!shorthand\n{body}")
        } else {
            body
        },
        warnings: conv.warnings,
    }
}

/// Round-trip verifier: convert, recompile (Min), compare normalized DOMs.
/// `Ok(fhtml)` on match; `Err` describes the first difference.
pub fn check(html: &str, opts: &Options) -> Result<String, String> {
    let conv = convert(html, opts);
    let compiled = crate::compile(&conv.fhtml, crate::Mode::Min)
        .map_err(|e| format!("converted fhtml failed to recompile: {e}"))?;
    // Both sides go through the same parse+unwrap path (same fragment
    // context too — a bare `<tr>` compiled output needs it as much as the
    // source did).
    let a = normalize_roots(html, opts);
    let b = normalize_roots(&compiled, opts);
    compare(&a, &b, "root")?;
    Ok(conv.fhtml)
}

/// Compares two HTML texts for normalized-DOM equivalence — the same
/// normalization `check` uses (comments dropped, inter-element whitespace
/// non-contractual, attrs sorted, boolean forms unified). `Err` describes
/// the first difference. This is the benchmark harness's grader: "did the
/// generated markup render the same DOM as the reference?"
pub fn compare_html(a: &str, b: &str, opts: &Options) -> Result<(), String> {
    let na = normalize_roots(a, opts);
    let nb = normalize_roots(b, opts);
    compare(&na, &nb, "root")
}

// ── DOM parsing ─────────────────────────────────────────────────────────────

fn parse_dom(html: &str) -> RcDom {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .expect("reading from a byte slice cannot fail")
}

/// The DOM plus the root nodes the converter (and comparator) operate on.
/// The `RcDom` must stay alive while the handles are in use — rcdom's
/// iterative `Drop` empties every descendant's child list when the document
/// goes away, gutting any handle still held.
struct Roots {
    #[allow(dead_code)] // held for ownership, see above
    dom: RcDom,
    roots: Vec<Handle>,
}

/// Documents are unwrapped to `<body>`'s children when the html/head/body
/// scaffolding is pure parser boilerplate — no doctype in the source, empty
/// `<head>`, attribute-less `<html>`/`<body>` (plan §2).
fn parse_roots(html: &str, opts: &Options) -> Roots {
    if let Some(ctx) = &opts.fragment {
        let name = QualName::new(None, ns!(html), LocalName::from(ctx.as_str()));
        let dom = parse_fragment(
            RcDom::default(),
            Default::default(),
            name,
            Vec::new(),
            false,
        )
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .expect("reading from a byte slice cannot fail");
        // fragment parse: document → <html> → the fragment's nodes
        let roots = {
            let doc = dom.document.children.borrow();
            match doc.iter().find(|n| element_name(n).is_some()) {
                Some(root) => root.children.borrow().clone(),
                None => Vec::new(),
            }
        };
        return Roots { dom, roots };
    }

    let dom = parse_dom(html);
    let doc_children = dom.document.children.borrow().clone();
    let has_doctype = doc_children
        .iter()
        .any(|n| matches!(n.data, NodeData::Doctype { .. }));
    let html_el = doc_children
        .iter()
        .find(|n| element_name(n).as_deref() == Some("html"))
        .cloned();

    if let Some(html_el) = &html_el {
        if !has_doctype && attrs_of(html_el).is_empty() {
            let kids = html_el.children.borrow();
            let head = kids
                .iter()
                .find(|n| element_name(n).as_deref() == Some("head"));
            let body = kids
                .iter()
                .find(|n| element_name(n).as_deref() == Some("body"));
            let head_empty = head.is_none_or(|h| {
                h.children.borrow().iter().all(|c| match &c.data {
                    NodeData::Text { contents } => {
                        contents.borrow().chars().all(|c| c.is_ascii_whitespace())
                    }
                    _ => false,
                })
            });
            if head_empty && body.is_some_and(|b| attrs_of(b).is_empty()) {
                // Boilerplate only: doc-level comments, then body's children.
                let mut roots: Vec<Handle> = doc_children
                    .iter()
                    .filter(|n| matches!(n.data, NodeData::Comment { .. }))
                    .cloned()
                    .collect();
                roots.extend(body.unwrap().children.borrow().iter().cloned());
                drop(kids);
                return Roots { dom, roots };
            }
        }
    }
    Roots {
        dom,
        roots: doc_children,
    }
}

fn element_name(handle: &Handle) -> Option<String> {
    match &handle.data {
        NodeData::Element { name, .. } if name.ns == ns!(html) => {
            Some(name.local.as_ref().to_string())
        }
        _ => None,
    }
}

fn attrs_of(handle: &Handle) -> Vec<(String, String)> {
    match &handle.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .map(|a| (attr_name(a), a.value.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn attr_name(a: &Attribute) -> String {
    match &a.name.prefix {
        Some(p) => format!("{p}:{}", a.name.local),
        None => a.name.local.as_ref().to_string(),
    }
}

// ── Sibling pass: text merging and whitespace policy (plan §4) ──────────────

enum Item {
    /// Adjacent DOM text nodes merged, whitespace-only runs dropped,
    /// interior whitespace collapsed, edges trimmed at block boundaries.
    Text(String),
    Node(Handle),
}

fn items_of(children: &[Handle]) -> Vec<Item> {
    // Merge adjacent text nodes, keep everything else in order.
    let mut merged: Vec<Item> = Vec::new();
    for child in children {
        if let NodeData::Text { contents } = &child.data {
            let s = contents.borrow().to_string();
            if let Some(Item::Text(prev)) = merged.last_mut() {
                prev.push_str(&s);
            } else {
                merged.push(Item::Text(s));
            }
        } else {
            merged.push(Item::Node(child.clone()));
        }
    }
    // Drop whitespace-only text (inter-element formatting, non-contractual).
    merged.retain(|item| match item {
        Item::Text(s) => !s.chars().all(|c| c.is_ascii_whitespace()),
        Item::Node(_) => true,
    });
    // Collapse, then trim the edges that touch the parent's boundary — a
    // leading/trailing space only renders next to a *sibling* (plan §4.2).
    let last = merged.len().saturating_sub(1);
    for (i, item) in merged.iter_mut().enumerate() {
        if let Item::Text(s) = item {
            let mut t = collapse_ws(s);
            if i == 0 {
                t = t.trim_start().to_string();
            }
            if i == last {
                t = t.trim_end().to_string();
            }
            *s = t;
        }
    }
    merged.retain(|item| !matches!(item, Item::Text(s) if s.is_empty()));
    merged
}

/// Newline runs (with surrounding indentation) → one space; the rest of the
/// value is untouched. For attribute values, which fhtml can't hold
/// multi-line (plan §3).
fn collapse_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\n' || c == '\r' {
            while out.ends_with([' ', '\t']) {
                out.pop();
            }
            while it.peek().is_some_and(|&n| n.is_ascii_whitespace()) {
                it.next();
            }
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// HTML whitespace collapse: runs of ASCII whitespace → one space. NBSP and
/// other Unicode spaces are *not* HTML whitespace and pass through.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_ascii_whitespace() {
            if !in_ws {
                out.push(' ');
            }
            in_ws = true;
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

// ── Conversion ──────────────────────────────────────────────────────────────

/// Attributes from the WHATWG boolean list: presence is the value, so they
/// render as bare fhtml attrs when their HTML value is `""` or the name itself.
const BOOLEAN_ATTRS: [&str; 25] = [
    "allowfullscreen",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "defer",
    "disabled",
    "formnovalidate",
    "hidden",
    "inert",
    "ismap",
    "itemscope",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "selected",
];

struct Conv<'a> {
    opts: &'a Options,
    warnings: Vec<String>,
}

impl Conv<'_> {
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    /// Contracts one class token for the shorthand output; the result always
    /// reads back as `class` under `#!shorthand` (codes where they round-trip, `=`-escapes where the class would decode).
    fn contract_class(&self, class: &str) -> String {
        if !self.opts.shorthand {
            return class.to_string();
        }
        crate::shorthand::contract(class)
    }

    fn convert_item(&mut self, item: &Item, out: &mut Vec<Node>) {
        match item {
            Item::Text(t) => match t.strip_suffix(' ') {
                // Trailing whitespace on a source line is never significant
                // (parser trims it), so a rendered trailing space rides as an
                // explicit empty `|` line: its newline collapses to the space.
                Some(stripped) => out.push(Node::TextBlock(vec![lit_parts(stripped), Vec::new()])),
                None => out.push(Node::TextBlock(vec![lit_parts(t)])),
            },
            Item::Node(handle) => match &handle.data {
                NodeData::Element { .. } => self.convert_element(handle, out),
                NodeData::Comment { contents } => {
                    out.push(comment_node(contents));
                }
                NodeData::Doctype { .. } => out.push(Node::Doctype),
                NodeData::ProcessingInstruction { target, contents } => {
                    self.warn(format!(
                        "processing instruction `<?{target} …>` passed through as raw"
                    ));
                    out.push(Node::Raw(vec![format!("<?{target} {contents}>")]));
                }
                NodeData::Document | NodeData::Text { .. } => unreachable!(),
            },
        }
    }

    fn convert_element(&mut self, handle: &Handle, out: &mut Vec<Node>) {
        let NodeData::Element {
            name,
            attrs,
            template_contents,
            ..
        } = &handle.data
        else {
            unreachable!()
        };

        // Foreign content (svg/math): raw by default — fidelity-safe, and
        // whitespace shifts are irrelevant there (plan §3).
        if name.ns != ns!(html) && !self.opts.convert_svg {
            out.push(raw_multiline(&serialize_subtree(handle)));
            return;
        }

        let tag = name.local.as_ref().to_string();

        match tag.as_str() {
            // Whitespace-exact content can't survive multi-line raw nesting
            // (indent re-anchoring) — one physical line, newlines
            // entity-encoded, is byte-exact and indentation-immune (plan §3).
            "pre" | "textarea" => {
                out.push(Node::Raw(vec![single_line_raw(handle)]));
                return;
            }
            // Raw-text elements: entities don't decode inside them, so these
            // stay multi-line; re-indentation is safe for CSS/JS except
            // template literals — hence the backtick warning.
            "script" | "style" => {
                let html = serialize_subtree(handle);
                if text_content(handle).contains('`') {
                    self.warn(format!(
                        "<{tag}> contains a backtick — if it's a template literal, \
                         re-indentation may alter the string"
                    ));
                }
                out.push(raw_multiline(&html));
                return;
            }
            _ => {}
        }

        let attr_list = attrs.borrow();

        // Structural escape hatch (plan §3): tags fhtml can't say (reserved
        // words, exotic names) and attrs it can't hold (newline + `//`,
        // unrepresentable names) become raw open/close tag lines with full
        // fhtml children between them — only the tag lines are raw.
        if self.needs_tag_fallback(&tag, &attr_list) {
            let children = self.convert_children(handle, template_contents);
            if children.is_empty() && !crate::parser::is_void(&tag) {
                out.push(raw_multiline(&format!(
                    "{}</{tag}>",
                    open_tag_html(&tag, &attr_list)
                )));
            } else {
                out.push(raw_multiline(&open_tag_html(&tag, &attr_list)));
                out.extend(children);
                if !crate::parser::is_void(&tag) {
                    out.push(Node::Raw(vec![format!("</{tag}>")]));
                }
            }
            return;
        }

        let mut el = Element {
            tag: tag.clone(),
            id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
            text: None,
            chain: None,
            children: Vec::new(),
            raw_body: None,
            line: 0,
        };

        for attr in attr_list.iter() {
            let aname = attr_name(attr);
            let mut value = attr.value.to_string();
            if value.contains('\n') {
                self.warn(format!(
                    "<{tag} {aname}> value contains newlines — normalized to spaces"
                ));
                value = collapse_newlines(&value);
            }
            match aname.as_str() {
                "id" if id_token_ok(&value) => el.id = Some(value),
                "class" => el.classes.extend(
                    value
                        .split_ascii_whitespace()
                        .map(|c| ClassItem::Lit(self.contract_class(c))),
                ),
                n if BOOLEAN_ATTRS.contains(&n)
                    && (value.is_empty() || value.eq_ignore_ascii_case(n)) =>
                {
                    el.attrs.push((aname, AttrValue::Bool));
                }
                _ => el.attrs.push((aname, AttrValue::Str(lit_parts(&value)))),
            }
        }
        drop(attr_list);

        let mut children = self.convert_children(handle, template_contents);

        // Sole short text child → inline `"text"`; long text or text with a
        // quote reads better as a `|` line (plan §4.3).
        if let [Node::TextBlock(lines)] = children.as_slice() {
            if let [[TextPart::Lit(t)]] = lines
                .iter()
                .map(Vec::as_slice)
                .collect::<Vec<_>>()
                .as_slice()
            {
                if t.len() <= 80 && !t.contains('"') {
                    el.text = Some(lit_parts(t));
                    children.clear();
                }
            }
        }

        // `>` chain synthesis (plan §3): only for a childless-text wrapper
        // with exactly one child *of any kind*, that child being a plain
        // element — a comment sibling would otherwise migrate into the chain.
        if self.opts.chains && el.text.is_none() && children.len() == 1 {
            if let Node::Element(_) = &children[0] {
                let Some(Node::Element(child)) = children.pop() else {
                    unreachable!()
                };
                el.chain = Some(Box::new(child));
                out.push(Node::Element(el));
                return;
            }
        }

        el.children = children;
        out.push(Node::Element(el));
    }

    fn convert_children(
        &mut self,
        handle: &Handle,
        template_contents: &std::cell::RefCell<Option<Handle>>,
    ) -> Vec<Node> {
        // <template> children live in the separate template-contents fragment.
        let kids: Vec<Handle> = match template_contents.borrow().as_ref() {
            Some(tc) => tc.children.borrow().clone(),
            None => handle.children.borrow().clone(),
        };
        let mut out = Vec::new();
        for item in items_of(&kids) {
            self.convert_item(&item, &mut out);
        }
        out
    }

    fn needs_tag_fallback(&mut self, tag: &str, attrs: &[Attribute]) -> bool {
        if RESERVED.contains(&tag) || tag == "doctype" || !tag_name_ok(tag) {
            self.warn(format!(
                "<{tag}> is not expressible as an fhtml element — raw tag lines emitted"
            ));
            return true;
        }
        for attr in attrs {
            let name = attr_name(attr);
            if !attr_name_ok(&name) {
                self.warn(format!(
                    "<{tag}> attribute `{name}` is not expressible in fhtml — raw tag lines emitted"
                ));
                return true;
            }
            // Collapsing newlines in a value that holds a `//` line comment
            // would comment out the rest of the code (Alpine x-data etc.);
            // raw tag lines preserve the newlines instead (plan §3).
            if attr.value.contains('\n') && attr.value.contains("//") {
                self.warn(format!(
                    "<{tag} {name}> holds multi-line code with a `//` comment — \
                     raw tag lines emitted to preserve its newlines"
                ));
                return true;
            }
        }
        false
    }
}

fn tag_name_ok(tag: &str) -> bool {
    let mut chars = tag.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn attr_name_ok(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c.is_ascii_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | '=' | '{'))
}

/// A DOM id usable as the `#id` token: no whitespace and none of the
/// characters that would reparse as something else (plan §3).
fn id_token_ok(id: &str) -> bool {
    !id.is_empty()
        && !id
            .chars()
            .any(|c| c.is_ascii_whitespace() || matches!(c, '"' | '\'' | '{'))
}

fn comment_node(contents: &str) -> Node {
    let mut lines: Vec<String> = contents.split('\n').map(|l| l.trim_end().into()).collect();
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines[0] = lines[0].trim().to_string();
    // Continuation lines are stored with the relative indent a reparse would
    // keep; two spaces puts them inside the comment block.
    for l in &mut lines[1..] {
        if !l.trim().is_empty() {
            *l = format!("  {}", l.trim());
        } else {
            l.clear();
        }
    }
    Node::Comment { lines, emit: true }
}

// ── Raw construction ────────────────────────────────────────────────────────

fn serialize_subtree(handle: &Handle) -> String {
    let mut buf = Vec::new();
    let s: SerializableHandle = handle.clone().into();
    serialize(
        &mut buf,
        &s,
        SerializeOpts {
            traversal_scope: TraversalScope::IncludeNode,
            ..Default::default()
        },
    )
    .expect("serializing to a Vec cannot fail");
    String::from_utf8(buf).expect("serializer output is UTF-8")
}

/// Serialized HTML → one `Node::Raw`. Continuation lines are stored two
/// spaces deep so a reparse keeps the whole block together; the shift is
/// invisible in whitespace-insensitive content (svg, script, style — the
/// whitespace-exact elements never come through here).
fn raw_multiline(html: &str) -> Node {
    let mut lines: Vec<String> = Vec::new();
    for (i, l) in html.split('\n').enumerate() {
        let l = l.trim_end_matches('\r');
        if i == 0 {
            lines.push(l.to_string());
        } else if l.is_empty() {
            lines.push(String::new());
        } else {
            lines.push(format!("  {l}"));
        }
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    Node::Raw(lines)
}

/// `pre`/`textarea` on one physical raw line, newlines and tabs
/// entity-encoded — byte-exact rendering, immune to indent re-anchoring.
fn single_line_raw(handle: &Handle) -> String {
    let mut html = serialize_subtree(handle);
    // The HTML parser drops one newline right after a `pre`/`textarea` open
    // tag — even a `&#10;` reference, since references resolve before tree
    // construction. Re-add it so the round-trip DOM keeps the leading \n.
    if let Some(gt) = html.find('>') {
        if html[gt + 1..].starts_with('\n') {
            html.insert(gt + 1, '\n');
        }
    }
    html.replace('\n', "&#10;")
        .replace('\t', "&#9;")
        .replace('\r', "&#13;")
}

/// Open tag for the raw-fallback path. Values keep literal newlines (that is
/// the point of the fallback); `&`/`"`/`<` are entity-escaped.
fn open_tag_html(tag: &str, attrs: &[Attribute]) -> String {
    let mut s = format!("<{tag}");
    for attr in attrs {
        let v = attr
            .value
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;");
        s.push_str(&format!(" {}=\"{v}\"", attr_name(attr)));
    }
    s.push('>');
    s
}

fn text_content(handle: &Handle) -> String {
    let mut out = String::new();
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => out.push_str(&contents.borrow()),
            _ => out.push_str(&text_content(child)),
        }
    }
    out
}

// ── Normalized-DOM comparator (plan §4 fidelity contract) ───────────────────

#[derive(Debug, PartialEq)]
enum NTree {
    El {
        tag: String,
        attrs: Vec<(String, String)>,
        children: Vec<NTree>,
    },
    Text(String),
}

fn normalize_roots(html: &str, opts: &Options) -> Vec<NTree> {
    let roots = parse_roots(html, opts);
    normalize_children(&roots.roots, false)
}

fn normalize_children(children: &[Handle], exact: bool) -> Vec<NTree> {
    let mut out = Vec::new();
    if exact {
        // Whitespace-exact context: keep text verbatim (adjacent nodes still
        // merge — entity boundaries differ between parses).
        for child in children {
            match &child.data {
                NodeData::Text { contents } => {
                    let s = contents.borrow().to_string();
                    if let Some(NTree::Text(prev)) = out.last_mut() {
                        prev.push_str(&s);
                    } else {
                        out.push(NTree::Text(s));
                    }
                }
                NodeData::Element { .. } => out.push(normalize_element(child)),
                _ => {}
            }
        }
        return out;
    }
    for item in items_of(children) {
        match item {
            Item::Text(t) => match out.last_mut() {
                // Comments were dropped between these; re-merge.
                Some(NTree::Text(prev)) => {
                    prev.push(' ');
                    prev.push_str(&t);
                }
                _ => out.push(NTree::Text(t)),
            },
            Item::Node(h) => {
                if matches!(h.data, NodeData::Element { .. }) {
                    out.push(normalize_element(&h));
                }
            }
        }
    }
    out
}

fn normalize_element(handle: &Handle) -> NTree {
    let NodeData::Element {
        name,
        attrs,
        template_contents,
        ..
    } = &handle.data
    else {
        unreachable!()
    };
    let tag = name.local.as_ref().to_string();

    let mut nattrs: Vec<(String, String)> = attrs
        .borrow()
        .iter()
        .map(|a| {
            let n = attr_name(a);
            let v = a.value.to_string();
            let v = if n == "class" {
                v.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
            } else if BOOLEAN_ATTRS.contains(&n.as_str()) && v.eq_ignore_ascii_case(&n) {
                // `disabled="disabled"` ≡ `disabled` — presence is the value.
                String::new()
            } else {
                // The converter normalizes newlines away (warned) or shifts
                // indentation inside raw-fallback values — both intentional,
                // so values compare newline-collapsed.
                collapse_newlines(&v)
            };
            (n, v)
        })
        .collect();
    nattrs.sort();

    let exact = name.ns == ns!(html) && matches!(tag.as_str(), "pre" | "textarea");
    let kids: Vec<Handle> = match template_contents.borrow().as_ref() {
        Some(tc) => tc.children.borrow().clone(),
        None => handle.children.borrow().clone(),
    };
    NTree::El {
        tag,
        attrs: nattrs,
        children: normalize_children(&kids, exact),
    }
}

fn compare(a: &[NTree], b: &[NTree], path: &str) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!(
            "{path}: child count differs: {} (source) vs {} (round-trip)\n  source:     {}\n  round-trip: {}",
            a.len(),
            b.len(),
            summarize(a),
            summarize(b),
        ));
    }
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        match (x, y) {
            (NTree::Text(t1), NTree::Text(t2)) => {
                if t1 != t2 {
                    return Err(format!("{path} > text[{i}]: {t1:?} vs {t2:?}"));
                }
            }
            (
                NTree::El {
                    tag: tag1,
                    attrs: at1,
                    children: c1,
                },
                NTree::El {
                    tag: tag2,
                    attrs: at2,
                    children: c2,
                },
            ) => {
                let p = format!("{path} > {tag1}[{i}]");
                if tag1 != tag2 {
                    return Err(format!(
                        "{path}: tag differs at [{i}]: <{tag1}> vs <{tag2}>"
                    ));
                }
                if at1 != at2 {
                    return Err(format!("{p}: attributes differ: {at1:?} vs {at2:?}"));
                }
                compare(c1, c2, &p)?;
            }
            _ => {
                return Err(format!(
                    "{path}: node kind differs at [{i}]: {} vs {}",
                    kind(x),
                    kind(y)
                ))
            }
        }
    }
    Ok(())
}

fn kind(t: &NTree) -> String {
    match t {
        NTree::El { tag, .. } => format!("<{tag}>"),
        NTree::Text(t) => format!("text {t:?}"),
    }
}

fn summarize(nodes: &[NTree]) -> String {
    nodes.iter().map(kind).collect::<Vec<_>>().join(", ")
}

/// Ensures a fresh `Options` keeps chains on — the comparator relies on
/// defaults matching converter behavior.
#[cfg(test)]
mod tests {
    #[test]
    fn default_options() {
        let o = super::Options::default();
        assert!(o.chains && !o.convert_svg && o.fragment.is_none());
    }
}
