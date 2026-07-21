"""Grading for the agent benchmark: correctness checks + mechanical quality.

Each task ships a checks.json:

    {
      "primary": "index.fhtml",          # file the checks compile (default index.fhtml)
      "data": "data.json",               # optional --data for every compile
      "checks": [ {"kind": ..., ...} ],  # correctness, in order (compile first)
      "quality": { ... }                 # mechanical quality switches
    }

Check kinds: compile, dom_eq, tag_count, class_present, text_contains,
attr_present, deps_count, defs, alt_data. Property kinds parse the compiled
`fhtml --min` output with the stdlib html.parser — no selector engine.

Correctness and quality are scored separately and never blended.
"""

import glob
import json
import os
import re
import subprocess
from html.parser import HTMLParser

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
FHTML = os.path.join(ROOT, "target", "release", "fhtml")
H2F = os.path.join(ROOT, "target", "release", "html2fhtml")

# 1-3 quoted-line bootstraps are tolerated by the skill; benchmark tasks never
# need one, so any inline script body counts against quality here.
INLINE_JS_ALLOWED_LINES = 0


def run(cmd, cwd=None):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    return p.returncode, p.stdout, p.stderr


# ---------------------------------------------------------------- HTML model

class _Walker(HTMLParser):
    """Flat element list + document text + per-script bodies."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.elements = []          # (tag, {attr: value})
        self.text = []
        self._script_depth = 0
        self.script_bodies = []

    def handle_starttag(self, tag, attrs):
        self.elements.append((tag, dict(attrs)))
        if tag == "script":
            self._script_depth += 1
            self.script_bodies.append("")

    def handle_endtag(self, tag):
        if tag == "script" and self._script_depth:
            self._script_depth -= 1

    def handle_data(self, data):
        if self._script_depth:
            self.script_bodies[-1] += data
        else:
            self.text.append(data)


def parse_html(html):
    w = _Walker()
    w.feed(html)
    return w


def _classes(attrs):
    return (attrs.get("class") or "").split()


def _match(elements, tag=None, attr=None, value=None, classes=None):
    hits = []
    for t, attrs in elements:
        if tag and t != tag:
            continue
        if attr is not None:
            if attr not in attrs:
                continue
            if value is not None and attrs.get(attr) != value:
                continue
        if classes and not set(classes) <= set(_classes(attrs)):
            continue
        hits.append((t, attrs))
    return hits


_OPS = {">=": lambda a, b: a >= b, "<=": lambda a, b: a <= b,
        "==": lambda a, b: a == b, ">": lambda a, b: a > b}


# ------------------------------------------------------------------- compile

def compile_primary(ws, spec, grade_dir, data_override=None):
    """Compile the task's primary file; returns (html_text|None, err)."""
    primary = os.path.join(ws, spec.get("primary", "index.fhtml"))
    if not os.path.exists(primary):
        return None, f"missing {os.path.basename(primary)}"
    cmd = [FHTML, "--min", primary]
    data = data_override or spec.get("data")
    if data:
        cmd += ["--data", os.path.join(ws, data)]
    if spec.get("deny_warnings", True):
        cmd.append("--deny-warnings")
    code, html, err = run(cmd)
    if code != 0:
        return None, err.strip()
    out = os.path.join(grade_dir, "out.html")
    with open(out, "w") as fh:
        fh.write(html)
    return html, None


def dom_eq(golden_path, html_path):
    code, _, err = run([H2F, "--dom-eq", golden_path, html_path])
    return code == 0, err.strip()


# -------------------------------------------------------------------- checks

def run_checks(ws, task_dir, spec, grade_dir):
    """Execute spec['checks']; returns (compile_ok, dom_ok, props, results)."""
    results = []
    html, err = compile_primary(ws, spec, grade_dir)
    compile_ok = html is not None
    results.append({"id": "compile", "pass": compile_ok, "detail": err or ""})
    doc = parse_html(html) if compile_ok else None

    dom_ok = None
    props_passed = props_total = 0
    failed = []

    for i, c in enumerate(spec.get("checks", [])):
        kind = c["kind"]
        cid = c.get("id", f"{kind}-{i}")
        if kind == "compile":
            continue  # implicit, already run
        if not compile_ok:
            ok, detail = False, "did not compile"
        elif kind == "dom_eq":
            ok, detail = dom_eq(os.path.join(task_dir, c["golden"]),
                                os.path.join(grade_dir, "out.html"))
        elif kind == "tag_count":
            n = len(_match(doc.elements, c.get("tag"), c.get("attr"),
                           c.get("value"), c.get("classes")))
            ok = _OPS[c.get("op", ">=")](n, c["n"])
            detail = f"found {n}"
        elif kind == "class_present":
            ok = bool(_match(doc.elements, c.get("tag"),
                             classes=c["classes"]))
            detail = "" if ok else f"no <{c.get('tag') or '*'}> with {c['classes']}"
        elif kind == "attr_present":
            ok = bool(_match(doc.elements, c.get("tag"), c["attr"],
                             c.get("value")))
            detail = "" if ok else f"no [{c['attr']}]"
        elif kind == "text_contains":
            text = "".join(doc.text)
            missing = [t for t in c["texts"] if t not in text]
            ok, detail = not missing, f"missing {missing}" if missing else ""
        elif kind == "deps_count":
            primary = os.path.join(ws, spec.get("primary", "index.fhtml"))
            code, out, _ = run([FHTML, "deps", primary])
            n = len([ln for ln in out.splitlines() if ln.strip()])
            ok = code == 0 and n >= c["min"]
            detail = f"{n} includes"
        elif kind == "defs":
            defs, calls = count_defs(ws)
            ok = len(defs) >= c.get("min_defs", 1) and calls >= c.get("min_calls", 1)
            detail = f"{len(defs)} defs, {calls} calls"
        elif kind == "alt_data":
            # held-out data lives in the task dir, not the workspace — the
            # agent never sees it, which is what makes it prove templating
            alt_dir = os.path.join(grade_dir, f"alt-{i}")
            os.makedirs(alt_dir, exist_ok=True)
            alt_html, alt_err = compile_primary(
                ws, spec, alt_dir,
                data_override=os.path.join(task_dir, c["data"]))
            if alt_html is None:
                ok, detail = False, alt_err
            else:
                text = "".join(parse_html(alt_html).text)
                missing = [t for t in c["texts"] if t not in text]
                ok = not missing
                detail = f"missing {missing}" if missing else ""
        else:
            raise ValueError(f"unknown check kind {kind!r}")

        results.append({"id": cid, "pass": ok, "detail": detail})
        if kind == "dom_eq":
            dom_ok = ok
        else:
            props_total += 1
            props_passed += ok or 0
            if not ok:
                failed.append(cid)

    props = {"passed": props_passed, "total": props_total, "failed": failed}
    return compile_ok, dom_ok, props, results


# ------------------------------------------------------------------- quality

def fhtml_files(ws):
    files = glob.glob(os.path.join(ws, "**", "*.fhtml"), recursive=True)
    return sorted(f for f in files if ".git" not in f)


DEF_RE = re.compile(r"^\s*def\s+([a-z_][a-z0-9_]*)\s*\(", re.M)
CALL_RE = re.compile(r"^\s*\+([a-z_][a-z0-9_]*)\s*[(\s]", re.M)


def count_defs(ws):
    defs, calls = set(), 0
    for f in fhtml_files(ws):
        src = open(f).read()
        defs |= set(DEF_RE.findall(src))
        calls += len(CALL_RE.findall(src))
    return defs, calls


def repetition_score(fhtml_src):
    """Fraction of structural lines that repeat (copied from bench/generate.py
    — kept in sync by eye; it is 20 lines of frozen scoring logic)."""
    lines = []
    for ln in fhtml_src.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        if ln.startswith("|"):
            ln = "|"
        ln = re.sub(r'"[^"]*"', '""', ln)
        ln = re.sub(r"\([^)]*\)", "()", ln)
        lines.append(ln)
    if not lines:
        return 0.0
    counts = {}
    for ln in lines:
        counts[ln] = counts.get(ln, 0) + 1
    return sum(n for n in counts.values() if n > 1) / len(lines)


def run_quality(ws, spec, grade_dir, compiled_html):
    q = spec.get("quality", {})
    results = []

    def add(qid, ok, detail=""):
        results.append({"id": qid, "pass": bool(ok), "detail": detail})

    if q.get("no_inline_js", True):
        if compiled_html is None:
            add("no_inline_js", False, "did not compile")
        else:
            doc = parse_html(compiled_html)
            bodies = [b for b in doc.script_bodies if b.strip()]
            handlers = [a for _, attrs in doc.elements for a in attrs
                        if a.startswith("on")]
            add("no_inline_js", not bodies and not handlers,
                f"{len(bodies)} inline script bodies, "
                f"{len(handlers)} on* attributes" if bodies or handlers else "")

    if q.get("fmt_clean", True):
        dirty = []
        for f in fhtml_files(ws):
            rel = os.path.relpath(f, ws)
            cp = os.path.join(grade_dir, "fmt", rel)
            os.makedirs(os.path.dirname(cp), exist_ok=True)
            with open(f) as src, open(cp, "w") as dst:
                dst.write(src.read())
            code, _, _ = run([FHTML, "fmt", cp])
            if code != 0 or open(cp).read() != open(f).read():
                dirty.append(rel)
        add("fmt_clean", not dirty, f"needs fmt: {dirty}" if dirty else "")

    if "repetition_max" in q:
        worst = max((repetition_score(open(f).read()) for f in fhtml_files(ws)),
                    default=0.0)
        add("repetition", worst <= q["repetition_max"],
            f"score {worst:.2f} > {q['repetition_max']}"
            if worst > q["repetition_max"] else f"score {worst:.2f}")

    if q.get("def_usage", True):
        defs, _ = count_defs(ws)
        dead = set()
        if defs:
            called = set()
            for f in fhtml_files(ws):
                called |= set(CALL_RE.findall(open(f).read()))
            dead = defs - called
        add("def_usage", not dead, f"dead defs: {sorted(dead)}" if dead else "")

    if "require_defs" in q:
        r = q["require_defs"]
        defs, calls = count_defs(ws)
        ok = len(defs) >= r.get("min_defs", 1) and calls >= r.get("min_calls", 1)
        add("require_defs", ok, f"{len(defs)} defs, {calls} calls")

    if "max_fhtml_files" in q:
        n = len(fhtml_files(ws))
        add("file_count", n <= q["max_fhtml_files"], f"{n} .fhtml files")
    if "min_fhtml_files" in q:
        n = len(fhtml_files(ws))
        add("min_file_count", n >= q["min_fhtml_files"], f"{n} .fhtml files")

    passed = sum(r["pass"] for r in results)
    score = passed / len(results) if results else 1.0
    return {"score": round(score, 3),
            "failed": [r["id"] for r in results if not r["pass"]]}, results


# ----------------------------------------------------------------- top level

def grade(ws, task, grade_dir):
    """Grade one finished workspace; returns the record fragment."""
    task_dir = os.path.join(HERE, "tasks", task)
    with open(os.path.join(task_dir, "checks.json")) as fh:
        spec = json.load(fh)
    os.makedirs(grade_dir, exist_ok=True)

    compile_ok, dom_ok, props, check_results = run_checks(
        ws, task_dir, spec, grade_dir)
    html = None
    out = os.path.join(grade_dir, "out.html")
    if compile_ok and os.path.exists(out):
        html = open(out).read()
    quality, quality_results = run_quality(ws, spec, grade_dir, html)

    all_props_ok = props["total"] == 0 or props["passed"] == props["total"]
    if not compile_ok:
        status = "compile-fail"
    elif dom_ok is False:
        status = "dom-fail"
    elif all_props_ok:
        status = "pass"
    elif props["passed"] == 0:
        status = "prop-fail"
    else:
        status = "prop-partial"

    with open(os.path.join(grade_dir, "checks.out.json"), "w") as fh:
        json.dump({"checks": check_results, "quality": quality_results},
                  fh, indent=2)

    return {"status": status, "compile": compile_ok, "dom": dom_ok,
            "props": props, "quality": quality}
