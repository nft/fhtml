#!/usr/bin/env python3
"""LLM generation-error benchmark: can models *write* fhtml correctly?

Task: translate each corpus component's canonical pretty HTML into a target
syntax (fhtml or pug — Pug is the control: the incumbent indentation
language). Grading is fully automatic:

  compile  the model's output compiles (fhtml binary / real Pug compiler)
  dom      the compiled HTML is normalized-DOM-equivalent to the reference
           (`html2fhtml --dom-eq`, the same comparator `--check` uses)

A model that "knows" a syntax should score high on both; the thesis is that
fhtml's verbatim class tokens avoid the escaping errors Pug forces on
Tailwind markup.

Requires ANTHROPIC_API_KEY. Zero Python deps (stdlib urllib). Pug grading
needs `npm install --prefix bench/.tools pug`.

Usage:
  export ANTHROPIC_API_KEY=…
  python3 bench/run.py                        # populates bench/out/ first
  python3 bench/generate.py --models claude-haiku-4-5-20251001 \
      --targets fhtml,pug [--limit 10]

Results: printed table + bench/out/gen/results.json (+ raw completions).
"""

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
H2F = os.path.join(ROOT, "target", "release", "html2fhtml")
FHTML = os.path.join(ROOT, "target", "release", "fhtml")
OUT = os.path.join(ROOT, "bench", "out")
GEN = os.path.join(OUT, "gen")
PUG_MOD = os.path.join(ROOT, "bench", ".tools", "node_modules", "pug")
API = "https://api.anthropic.com/v1/messages"

FEWSHOT = ["pricing-card"]  # from tests/corpus — never part of the eval set

PROMPT = {
    "fhtml": (
        "You translate HTML into fhtml, a whitespace-based markup language. "
        "The complete syntax reference:\n\n{cheatsheet}\n\n"
        "Reply with ONLY the fhtml source — no code fences, no commentary."
    ),
    "pug": (
        "You translate HTML into Pug (pug 3.x). Remember that Pug class "
        "literals only allow [A-Za-z0-9_-], so Tailwind classes containing "
        "':', '/', '.', or '[' must go in a (class=\"...\") attribute. "
        "Reply with ONLY the Pug source — no code fences, no commentary."
    ),
}


def run(cmd, stdin=None):
    p = subprocess.run(
        cmd, input=stdin, capture_output=True, text=True, check=False
    )
    return p.returncode, p.stdout, p.stderr


def api_call(model, system, messages, key, max_retries=3):
    body = json.dumps({
        "model": model,
        "max_tokens": 8192,
        "temperature": 0.0,
        "system": system,
        "messages": messages,
    }).encode()
    req = urllib.request.Request(API, data=body, headers={
        "content-type": "application/json",
        "x-api-key": key,
        "anthropic-version": "2023-06-01",
    })
    for attempt in range(max_retries + 1):
        try:
            with urllib.request.urlopen(req, timeout=300) as resp:
                data = json.load(resp)
            return "".join(
                b["text"] for b in data["content"] if b["type"] == "text"
            )
        except urllib.error.HTTPError as e:
            if e.code in (429, 529, 500) and attempt < max_retries:
                time.sleep(5 * (attempt + 1))
                continue
            raise RuntimeError(f"API {e.code}: {e.read().decode()[:500]}")


def strip_fences(text):
    text = text.strip()
    if text.startswith("```"):
        lines = text.splitlines()
        if lines[-1].strip().startswith("```"):
            lines = lines[1:-1]
        else:
            lines = lines[1:]
        text = "\n".join(lines)
    return text + "\n"


def compile_output(target, source, workdir, stem):
    """Model output → HTML, or an error string."""
    src_path = os.path.join(workdir, f"{stem}.{target}")
    with open(src_path, "w") as fh:
        fh.write(source)
    if target == "fhtml":
        code, html, err = run([FHTML, "--min", src_path])
        return (html, None) if code == 0 else (None, err.strip())
    script = (
        "const pug=require(process.argv[1]);"
        "process.stdout.write(pug.renderFile(process.argv[2]));"
    )
    code, html, err = run(["node", "-e", script, PUG_MOD, src_path])
    return (html, None) if code == 0 else (None, err.strip().splitlines()[0])


def dom_eq(html_a_path, html_b_text, workdir, stem):
    b_path = os.path.join(workdir, f"{stem}.out.html")
    with open(b_path, "w") as fh:
        fh.write(html_b_text)
    code, _, err = run([H2F, "--dom-eq", html_a_path, b_path])
    return code == 0, err.strip()


def fewshot_messages(target, pretty_dir):
    msgs = []
    for stem in FEWSHOT:
        html_path = os.path.join(ROOT, "tests", "corpus", stem + ".html")
        with open(html_path) as fh:
            html = fh.read()
        if target == "fhtml":
            _, out, _ = run([H2F, html_path])
        else:
            sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
            import pug_emit
            out = pug_emit.convert(html)
        msgs.append({"role": "user", "content": f"Translate to {target}:\n\n{html}"})
        msgs.append({"role": "assistant", "content": out.rstrip()})
    return msgs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", default="claude-haiku-4-5-20251001")
    ap.add_argument("--targets", default="fhtml,pug")
    ap.add_argument("--limit", type=int, default=0, help="first N components")
    args = ap.parse_args()

    key = os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        sys.exit("ANTHROPIC_API_KEY is not set")
    pretty_dir = os.path.join(OUT, "html-pretty")
    if not os.path.isdir(pretty_dir):
        sys.exit("bench/out/ missing — run `python3 bench/run.py` first")

    with open(os.path.join(ROOT, "bench", "cheatsheet.md")) as fh:
        cheatsheet = fh.read()

    stems = sorted(
        os.path.splitext(f)[0].replace(".pretty", "")
        for f in os.listdir(pretty_dir)
    )
    if args.limit:
        stems = stems[: args.limit]
    models = args.models.split(",")
    targets = args.targets.split(",")

    os.makedirs(GEN, exist_ok=True)
    results = []
    for model in models:
        for target in targets:
            system = PROMPT[target].format(cheatsheet=cheatsheet)
            shots = fewshot_messages(target, pretty_dir)
            n_compile = n_dom = 0
            for stem in stems:
                ref_path = os.path.join(pretty_dir, stem + ".pretty.html")
                with open(ref_path) as fh:
                    ref_html = fh.read()
                messages = shots + [{
                    "role": "user",
                    "content": f"Translate to {target}:\n\n{ref_html}",
                }]
                raw = api_call(model, system, messages, key)
                source = strip_fences(raw)
                case_dir = os.path.join(GEN, model, target)
                os.makedirs(case_dir, exist_ok=True)
                html, err = compile_output(target, source, case_dir, stem)
                ok_dom, dom_err = (False, "did not compile")
                if html is not None:
                    n_compile += 1
                    ok_dom, dom_err = dom_eq(ref_path, html, case_dir, stem)
                    if ok_dom:
                        n_dom += 1
                results.append({
                    "model": model, "target": target, "component": stem,
                    "compile": err is None, "dom": ok_dom,
                    "error": err or (None if ok_dom else dom_err),
                })
                mark = "✓" if ok_dom else ("c" if err is None else "✗")
                print(f"{model} {target} {stem}: {mark}", flush=True)
            n = len(stems)
            print(
                f"\n== {model} / {target}: compile {n_compile}/{n}, "
                f"DOM-equivalent {n_dom}/{n}\n", flush=True,
            )

    with open(os.path.join(GEN, "results.json"), "w") as fh:
        json.dump(results, fh, indent=2)
    print(f"raw results: {os.path.join(GEN, 'results.json')}")


if __name__ == "__main__":
    main()
