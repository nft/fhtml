#!/usr/bin/env python3
"""Unit tests for microparts_assemble.py.

The assembler is the GRADER for the microparts benchmark control — it must
be more trustworthy than the completions it grades, so every rule and every
structured error of the grammar is exercised here, plus the two
checked-in few-shot documents (assembled and DOM-compared to the corpus
HTML via `html2fhtml --dom-eq`, the same comparator the benchmark uses).

Run: python3 bench/test_microparts.py   (stdlib only; DOM checks need the
release binaries from `cargo build --release --features convert`).
"""

import json
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from microparts_assemble import assemble  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
H2F = os.path.join(ROOT, "target", "release", "html2fhtml")

FAILS = []


def check(name, cond, detail=""):
    print(("ok  " if cond else "FAIL") + f" {name}"
          + (f"  [{detail}]" if detail and not cond else ""))
    if not cond:
        FAILS.append(name)


def ok(name, doc, expect_html):
    html, err = assemble(json.dumps(doc) if isinstance(doc, dict) else doc)
    check(name, err is None and html == expect_html,
          err or f"got {html!r}, want {expect_html!r}")


def fails(name, doc, code, needle=""):
    html, err = assemble(json.dumps(doc) if isinstance(doc, dict) else doc)
    check(name, html is None and err is not None
          and err.startswith(code + ":") and needle in err,
          f"got ({html!r}, {err!r}), want {code} error with {needle!r}")


# ---- happy paths ---------------------------------------------------------
ok("body only, no parts", {"body": "<p>hi</p>"}, "<p>hi</p>")
ok("zero-arg call from body",
   {"parts": {"hr": "<hr>"}, "body": "<div>{{hr}}{{hr}}</div>"},
   "<div><hr><hr></div>")
ok("slots bound by args",
   {"parts": {"card": "<li>{{title}}: {{body}}</li>"},
    "body": '<ul>{{card title="A" body="one"}}{{card title="B" body="two"}}</ul>'},
   "<ul><li>A: one</li><li>B: two</li></ul>")
ok("whitespace-tolerant forms: {{ name }} and key = \"v\"",
   {"parts": {"tag": "<b>{{ x }}</b>"},
    "body": '{{ tag  x = "v" }}'}, "<b>v</b>")
ok("newlines as whitespace inside a node",
   {"parts": {"t": "<i>{{a}}</i>"},
    "body": '{{t\n  a="1"\n}}'}, "<i>1</i>")
ok("escapes in values: \\\" and \\\\",
   {"parts": {"t": "<span>{{v}}</span>"},
    "body": '{{t v="a \\"q\\" b\\\\c"}}'},
   '<span>a "q" b\\c</span>')
ok("markup and }} inside a quoted value",
   {"parts": {"t": "<td>{{v}}</td>"},
    "body": '{{t v="<b>x</b> close}}braces"}}'},
   "<td><b>x</b> close}}braces</td>")
ok("substituted value containing {{...}} stays literal (opaque)",
   {"parts": {"t": "<p>{{v}}</p>", "other": "<hr>"},
    "body": '{{t v="{{other}}"}}'},
   "<p>{{other}}</p>")
ok("part calling another part (zero-arg)",
   {"parts": {"icon": "<svg/>", "row": "<li>{{icon}}{{txt}}</li>"},
    "body": '{{row txt="x"}}'},
   "<li><svg/>x</li>")
ok("same name: slot inside a part, unknown in body context",
   {"parts": {"t": "<p>{{title}}</p>"}, "body": '{{t title="s"}}'},
   "<p>s</p>")
ok("bare part name inside a part is a zero-arg call, not a slot",
   {"parts": {"hr": "<hr>", "t": "<div>{{hr}}{{x}}</div>"},
    "body": '{{t x="v"}}'},
   "<div><hr>v</div>")
ok("lone { and } and }} outside a node are literal",
   {"body": "<script>if (a) { b(); }} </script>"},
   "<script>if (a) { b(); }} </script>")
ok("entities verbatim",
   {"body": "<p>&amp; &#8594; &nbsp;</p>"}, "<p>&amp; &#8594; &nbsp;</p>")

# determinism regardless of parts order (same doc, both key orders)
a1, e1 = assemble('{"parts": {"a": "<i>{{b}}</i>", "b": "<b>x</b>"}, '
                  '"body": "{{a}}"}')
a2, e2 = assemble('{"parts": {"b": "<b>x</b>", "a": "<i>{{b}}</i>"}, '
                  '"body": "{{a}}"}')
check("output independent of parts dictionary order",
      e1 is None and e2 is None and a1 == a2 == "<i><b>x</b></i>",
      f"{e1} {e2} {a1!r} {a2!r}")

# depth: a chain of 8 calls passes, 9 is an error
def chain_doc(n):
    parts = {f"p{i}": f"<x{i}>{{{{p{i + 1}}}}}</x{i}>" for i in range(1, n)}
    parts[f"p{n}"] = "<leaf/>"
    return {"parts": parts, "body": "{{p1}}"}

ok("nesting at depth 8 passes", chain_doc(8),
   "".join(f"<x{i}>" for i in range(1, 8)) + "<leaf/>"
   + "".join(f"</x{i}>" for i in range(7, 0, -1)))
fails("nesting at depth 9 is an error", chain_doc(9), "depth", "p9")

# ---- template errors -----------------------------------------------------
fails("single-quoted value", {"parts": {"t": "<p>{{v}}</p>"},
      "body": "{{t v='x'}}"}, "bad-template", "single-quoted")
fails("dangling {{", {"body": "<p>{{</p>"}, "bad-template", "")
fails("kebab-case node name", {"body": "{{blog-post}}"}, "bad-template", "")
fails("uppercase node name", {"body": "{{Card}}"}, "bad-template", "")
fails("unterminated value", {"parts": {"t": "{{v}}"},
      "body": '{{t v="x'}, "bad-template", "unterminated")
fails("missing = between key and value", {"parts": {"t": "{{v}}"},
      "body": '{{t v "x"}}'}, "bad-template", "'='")
fails("bad escape \\n in a value", {"parts": {"t": "{{v}}"},
      "body": '{{t v="a\\nb"}}'}, "bad-template", "escape")
fails("unquoted value", {"parts": {"t": "{{v}}"},
      "body": "{{t v=3}}"}, "bad-template", "double-quoted")
fails("duplicate arg keys", {"parts": {"t": "{{v}}"},
      "body": '{{t v="a" v="b"}}'}, "duplicate-arg", "'v'")
fails("bad-template inside a part string",
      {"parts": {"t": "<p>{{</p>"}, "body": "{{t}}"},
      "bad-template", "part 't'")

# ---- resolution errors ---------------------------------------------------
fails("unknown name in body", {"body": "{{ghost}}"},
      "unknown-name", "body")
fails("slot-looking bare name in body is unknown-name",
      {"parts": {"t": "<p>{{x}}</p>"}, "body": "{{x}}"},
      "unknown-name", "slots only exist inside parts")
fails("call with args to a non-part",
      {"body": '{{ghost x="1"}}'}, "unknown-name", "ghost")
fails("unbound slot", {"parts": {"t": "<p>{{a}}{{b}}</p>"},
      "body": '{{t a="1"}}'}, "unbound-slot", "b")
fails("unused arg", {"parts": {"t": "<p>{{a}}</p>"},
      "body": '{{t a="1" z="2"}}'}, "unused-arg", "z")
fails("arg name shadowing a part name",
      {"parts": {"icon": "<svg/>", "t": "<p>{{icon}}{{x}}</p>"},
       "body": '{{t x="1" icon="not allowed"}}'},
      "shadowed-name", "icon")
fails("direct cycle, chain in message",
      {"parts": {"a": "<p>{{a}}</p>"}, "body": "{{a}}"},
      "cycle", "a -> a")
fails("indirect cycle, chain in message",
      {"parts": {"a": "{{b}}", "b": "{{c}}", "c": "{{a}}"},
       "body": "{{a}}"},
      "cycle", "a -> b -> c -> a")

# ---- envelope errors -----------------------------------------------------
fails("not JSON at all", "here is your document: {", "bad-json", "")
fails("top-level array", "[1, 2]", "bad-envelope", "object")
fails("missing body", {"parts": {}}, "bad-envelope", "body")
fails("empty body", {"body": "   "}, "bad-envelope", "non-empty")
fails("non-string body", {"body": 42}, "bad-envelope", "body")
fails("non-object parts", {"body": "<p/>", "parts": ["x"]},
      "bad-envelope", "parts")
fails("non-string part value", {"body": "<p/>", "parts": {"t": 1}},
      "bad-envelope", "'t'")
fails("unknown top-level key",
      {"body": "<p/>", "header": "<h1/>"}, "bad-envelope", "header")
fails("kebab-case part name",
      {"body": "<p/>", "parts": {"blog-post": "<x/>"}},
      "bad-envelope", "underscores")
fails("duplicate JSON keys",
      '{"body": "<p/>", "body": "<q/>"}', "duplicate-key", "body")

# ---- few-shot documents assemble and match the corpus DOM ----------------
for stem in ("pricing-card", "feature-list"):
    path = os.path.join(ROOT, "tests", "corpus", stem + ".microparts.json")
    with open(path) as fh:
        html, err = assemble(fh.read())
    check(f"few-shot {stem}: assembles", err is None, str(err))
    if err is None:
        ref = os.path.join(ROOT, "tests", "corpus", stem + ".html")
        with tempfile.NamedTemporaryFile("w", suffix=".html",
                                         delete=False) as fh:
            fh.write(html)
        p = subprocess.run([H2F, "--dom-eq", ref, fh.name],
                           capture_output=True, text=True)
        os.unlink(fh.name)
        check(f"few-shot {stem}: DOM-eq to corpus html", p.returncode == 0,
              p.stderr.strip())

print()
if FAILS:
    sys.exit(f"{len(FAILS)} FAILURES: {FAILS}")
print(f"all checks passed")
