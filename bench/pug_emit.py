#!/usr/bin/env python3
"""HTML → conservative idiomatic Pug, for token-count benchmarking.

Emits the compact form a competent Pug author would write:

- `.class` shorthand only when *every* class on the element is a legal Pug
  class literal; otherwise the whole list goes into `(class="…")`. Mixing the
  two forms on one element is legal Pug but not what people write. Tailwind's
  variant and arbitrary-value classes (`:`, `/`, `[`, `]`, `.`) are never
  legal literals — that asymmetry is the thing being measured.
- `div` is elided when a class/id shorthand carries the line.
- An element whose only child is a short text run gets Pug trailing text
  (`p Hello`); mixed content falls back to `|` pipe lines.
- `<pre>`/`<script>`/`<style>` use Pug's `tag.` raw-text block form.

The output is meant to be *fair* Pug, not adversarial: it is validated with
the real Pug compiler by `run.py --validate-pug`. Note: html.parser
lowercases attribute names (viewBox → viewbox); this never changes token
counts and Pug does not care.
"""

import re
import sys
from html import escape
from html.parser import HTMLParser

VOID = {
    "area", "base", "br", "col", "embed", "hr", "img", "input",
    "link", "meta", "param", "source", "track", "wbr",
}
RAW_TEXT = {"script", "style", "pre", "textarea"}
CLASS_LIT = re.compile(r"^-?[A-Za-z_][A-Za-z0-9_-]*$")


class El:
    def __init__(self, tag, attrs):
        self.tag = tag
        self.attrs = attrs  # list of (name, value-or-None)
        self.children = []  # El | str


class TreeBuilder(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = El("#root", [])
        self.stack = [self.root]

    def handle_starttag(self, tag, attrs):
        el = El(tag, attrs)
        self.stack[-1].children.append(el)
        if tag not in VOID:
            self.stack.append(el)

    def handle_startendtag(self, tag, attrs):
        self.stack[-1].children.append(El(tag, attrs))

    def handle_endtag(self, tag):
        # Tolerant close: pop to the nearest matching open tag.
        for i in range(len(self.stack) - 1, 0, -1):
            if self.stack[i].tag == tag:
                del self.stack[i:]
                return

    def handle_data(self, data):
        self.stack[-1].children.append(data)


def parse(html):
    tb = TreeBuilder()
    tb.feed(html)
    tb.close()
    return tb.root


def collapse_ws(text):
    return re.sub(r"[ \t\r\n\f]+", " ", text)


def clean_children(el, raw):
    """Merge/collapse text per the same non-contractual-whitespace policy the
    fhtml converter uses; keep raw-text elements exact."""
    out = []
    for child in el.children:
        if isinstance(child, str):
            if raw:
                out.append(child)
                continue
            t = collapse_ws(child)
            if t.strip() == "":
                continue
            if out and isinstance(out[-1], str):
                out[-1] += t
            else:
                out.append(t)
        else:
            out.append(child)
    if not raw:
        out = [c.strip() if isinstance(c, str) else c for c in out]
        out = [c for c in out if not (isinstance(c, str) and c == "")]
    return out


def quote_attr(value):
    if '"' not in value:
        return '"' + value + '"'
    if "'" not in value:
        return "'" + value + "'"
    return '"' + value.replace('"', "&quot;") + '"'


def pug_text(text):
    """Text as Pug sees it: raw HTML with #{} interpolation — escape both."""
    return escape(text, quote=False).replace("#{", "\\#{").replace("#[", "\\#[")


def head_of(el):
    """The tag+shorthand+attrs part of a Pug line."""
    attrs = list(el.attrs)
    classes = None
    id_val = None
    for i, (k, v) in enumerate(attrs):
        if k == "class" and v:
            toks = v.split()
            if toks and all(CLASS_LIT.match(t) for t in toks):
                classes = toks
                attrs[i] = None
        elif k == "id" and v and CLASS_LIT.match(v):
            id_val = v
            attrs[i] = None
    attrs = [a for a in attrs if a is not None]

    head = el.tag
    if id_val:
        head += "#" + id_val
    if classes:
        head += "." + ".".join(classes)
    if (id_val or classes) and el.tag == "div":
        head = head[len("div"):]

    if attrs:
        parts = []
        for k, v in attrs:
            if v is None or v == "":
                parts.append(k)
            else:
                parts.append(k + "=" + quote_attr(v))
        head += "(" + " ".join(parts) + ")"
    return head


def emit(el, depth, lines):
    pad = "  " * depth
    head = head_of(el)

    if el.tag in RAW_TEXT:
        text = "".join(c for c in el.children if isinstance(c, str))
        if text.strip() == "" and not any(
            not isinstance(c, str) for c in el.children
        ):
            lines.append(pad + head)
        elif any(not isinstance(c, str) for c in el.children):
            # e.g. <pre><code>…</code></pre> — recurse normally.
            lines.append(pad + head)
            for child in clean_children(el, raw=False):
                emit_child(child, depth + 1, lines)
        else:
            lines.append(pad + head + ".")
            for raw_line in text.strip("\n").split("\n"):
                lines.append(pad + "  " + raw_line)
        return

    children = clean_children(el, raw=False)
    if not children:
        lines.append(pad + head)
        return
    if len(children) == 1 and isinstance(children[0], str):
        lines.append(pad + head + " " + pug_text(children[0]))
        return
    lines.append(pad + head)
    for child in children:
        emit_child(child, depth + 1, lines)


def emit_child(child, depth, lines):
    if isinstance(child, str):
        lines.append("  " * depth + "| " + pug_text(child))
    else:
        emit(child, depth, lines)


def convert(html):
    root = parse(html)
    lines = []
    for child in clean_children(root, raw=False):
        emit_child(child, 0, lines)
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    source = sys.stdin.read() if len(sys.argv) < 2 else open(sys.argv[1]).read()
    sys.stdout.write(convert(source))
