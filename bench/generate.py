#!/usr/bin/env python3
"""LLM generation-error benchmark: can models *write* fhtml correctly?

Task: translate each corpus component's canonical pretty HTML into a target
syntax (fhtml or pug — Pug is the control: the incumbent indentation
language). Grading is fully automatic:

  compile  the model's output compiles (fhtml binary / real Pug compiler)
  dom      the compiled HTML is normalized-DOM-equivalent to the reference
           (`html2fhtml --dom-eq`, the same comparator `--check` uses)

Per-case marks: ✓ pass · ~ whitespace-only miss (DOM-equivalent once all
whitespace significance is erased — a render-visible space slipped, but the
structure, attributes, and text are right) · c structural DOM mismatch ·
✗ did not compile.

A model that "knows" a syntax should score high on both; the thesis is that
fhtml's verbatim class tokens avoid the escaping errors Pug forces on
Tailwind markup.

Requires ANTHROPIC_API_KEY or OPENROUTER_API_KEY (Anthropic wins if both
are set; OpenRouter model ids look like `anthropic/claude-haiku-4.5`).
Zero Python deps (stdlib urllib). Pug grading needs
`npm install --prefix bench/.tools pug`.

Usage:
  export ANTHROPIC_API_KEY=…                  # or OPENROUTER_API_KEY=…
  python3 bench/run.py                        # populates bench/out/ first
  python3 bench/generate.py --models claude-haiku-4-5-20251001 \
      --targets fhtml,pug [--limit 10] [--verbose] [--resume]

results.json is rewritten after every graded case, so an interrupted run
loses nothing; rerun with --resume to skip already-graded cases.

Results: printed table + bench/out/gen/results.json (+ raw completions).
"""

import argparse
import http.client
import json
import os
import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request


class TransientAPIError(Exception):
    """A 200 response whose payload is unusable — retry."""


class FatalAPIError(Exception):
    """The API rejected us for a reason retrying can't fix (e.g. credits)."""

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
H2F = os.path.join(ROOT, "target", "release", "html2fhtml")
FHTML = os.path.join(ROOT, "target", "release", "fhtml")
OUT = os.path.join(ROOT, "bench", "out")
GEN = os.path.join(OUT, "gen")
PUG_MOD = os.path.join(ROOT, "bench", ".tools", "node_modules", "pug")
API = {
    "anthropic": "https://api.anthropic.com/v1/messages",
    "openrouter": "https://openrouter.ai/api/v1/chat/completions",
}

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


def vblock(title, text):
    """Verbose helper: print `text` framed by a titled ruler."""
    print(f"\n──── {title} " + "─" * max(0, 60 - len(title)))
    print(text.rstrip())
    print("─" * 66, flush=True)


def api_call(provider, model, system, messages, key, max_retries=3,
             verbose=False, max_tokens=8192):
    if provider == "anthropic":
        body = {
            "model": model,
            "max_tokens": max_tokens,
            "temperature": 0.0,
            "system": system,
            "messages": messages,
        }
        headers = {
            "content-type": "application/json",
            "x-api-key": key,
            "anthropic-version": "2023-06-01",
        }
    else:  # openrouter — OpenAI chat-completions format
        body = {
            "model": model,
            "max_tokens": max_tokens,
            "temperature": 0.0,
            "messages": [{"role": "system", "content": system}] + messages,
        }
        headers = {
            "content-type": "application/json",
            "authorization": f"Bearer {key}",
        }
    req = urllib.request.Request(
        API[provider], data=json.dumps(body).encode(), headers=headers
    )
    for attempt in range(max_retries + 1):
        try:
            with urllib.request.urlopen(req, timeout=300) as resp:
                data = json.load(resp)
            if provider == "anthropic":
                return "".join(
                    b["text"] for b in data["content"] if b["type"] == "text"
                )
            # OpenRouter can 200 with an error payload, an empty choice, or
            # `content: null` (upstream provider hiccup) — all retryable.
            if "choices" not in data or not data["choices"]:
                raise TransientAPIError(str(data.get("error", data))[:300])
            content = data["choices"][0].get("message", {}).get("content")
            if not content:
                raise TransientAPIError(
                    f"empty completion (finish_reason="
                    f"{data['choices'][0].get('finish_reason')})")
            return content
        except urllib.error.HTTPError as e:
            if e.code in (429, 500, 502, 503, 529) and attempt < max_retries:
                delay = 5 * (attempt + 1)
                if verbose:
                    print(f"[api] HTTP {e.code}, retry "
                          f"{attempt + 1}/{max_retries} in {delay}s", flush=True)
                time.sleep(delay)
                continue
            detail = e.read().decode()[:500]
            if e.code == 402:  # OpenRouter: key limit can't afford max_tokens
                raise FatalAPIError(
                    f"402 Payment Required — the key's remaining credit "
                    f"cannot cover max_tokens={max_tokens}. Top up / raise "
                    f"the key limit, or rerun with a smaller --max-tokens. "
                    f"Detail: {detail}")
            raise RuntimeError(f"API {e.code}: {detail}")
        except (http.client.HTTPException, urllib.error.URLError,
                ConnectionError, socket.timeout, json.JSONDecodeError,
                TransientAPIError) as e:
            # Dropped/truncated connections (IncompleteRead), DNS blips,
            # malformed bodies: transient — retry the same request.
            if attempt < max_retries:
                delay = 5 * (attempt + 1)
                if verbose:
                    print(f"[api] {type(e).__name__}: {e} — retry "
                          f"{attempt + 1}/{max_retries} in {delay}s", flush=True)
                time.sleep(delay)
                continue
            raise RuntimeError(
                f"API network error after {max_retries} retries: {e!r}")


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


def erase_ws(html):
    """Kill ALL whitespace significance — for the lenient grading tier that
    separates render-visible-space slips from structural errors."""
    s = re.sub(r"\s+", " ", html)
    s = re.sub(r"> ", ">", s)
    return re.sub(r" <", "<", s)


def dom_eq_lenient(ref_html, html_b_text, workdir, stem):
    a_path = os.path.join(workdir, f"{stem}.ws.ref.html")
    with open(a_path, "w") as fh:
        fh.write(erase_ws(ref_html))
    ok, _ = dom_eq(a_path, erase_ws(html_b_text), workdir, f"{stem}.ws")
    return ok


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
    ap.add_argument("--verbose", "-v", action="store_true",
                    help="print prompts, raw completions, grading details")
    ap.add_argument("--resume", action="store_true",
                    help="skip cases already graded in results.json "
                         "(continue an interrupted run)")
    ap.add_argument("--max-tokens", type=int, default=8192,
                    help="completion budget per request (lower it if your "
                         "key's credit limit rejects requests with 402)")
    args = ap.parse_args()

    key = os.environ.get("ANTHROPIC_API_KEY")
    provider = "anthropic"
    if not key:
        key = os.environ.get("OPENROUTER_API_KEY")
        provider = "openrouter"
    if not key:
        sys.exit("neither ANTHROPIC_API_KEY nor OPENROUTER_API_KEY is set")
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
    # results.json is the accumulator across runs and models; it is rewritten
    # after every case so a crash or ^C never loses graded work (--resume
    # picks up from it).
    res_path = os.path.join(GEN, "results.json")
    merged = {}
    if os.path.exists(res_path):
        with open(res_path) as fh:
            for r in json.load(fh):
                merged[(r["model"], r["target"], r["component"])] = r

    def save():
        tmp = res_path + ".tmp"
        with open(tmp, "w") as fh:
            json.dump(sorted(merged.values(), key=lambda r: (
                r["model"], r["target"], r["component"])), fh, indent=2)
        os.replace(tmp, res_path)

    MARKS = {"pass": "✓", "ws-only": "~", "dom-fail": "c",
             "compile-fail": "✗"}

    def mark_of(rec):  # tolerate pre-`status` records from older runs
        status = rec.get("status")
        if status:
            return MARKS[status]
        return "✓" if rec["dom"] else ("c" if rec["compile"] else "✗")

    for model in models:
        for target in targets:
            system = PROMPT[target].format(cheatsheet=cheatsheet)
            shots = fewshot_messages(target, pretty_dir)
            if args.verbose:
                vblock(f"system prompt · {model} / {target}", system)
                print(f"[gen] few-shot examples: {len(shots) // 2} "
                      f"({', '.join(FEWSHOT)}), components: {len(stems)}",
                      flush=True)
            for stem in stems:
                case_key = (model, target, stem)
                if args.resume and case_key in merged:
                    print(f"{model} {target} {stem}: "
                          f"{mark_of(merged[case_key])} (resumed)", flush=True)
                    continue
                ref_path = os.path.join(pretty_dir, stem + ".pretty.html")
                with open(ref_path) as fh:
                    ref_html = fh.read()
                messages = shots + [{
                    "role": "user",
                    "content": f"Translate to {target}:\n\n{ref_html}",
                }]
                if args.verbose:
                    vblock(f"input · {stem} ({len(ref_html)} bytes)", ref_html)
                t0 = time.time()
                try:
                    raw = api_call(provider, model, system, messages, key,
                                   verbose=args.verbose,
                                   max_tokens=args.max_tokens)
                except (RuntimeError, FatalAPIError) as e:
                    sys.exit(
                        f"\n{e}\n\nAll graded cases are saved — rerun the "
                        f"same command with --resume to continue from "
                        f"`{stem}`.")
                source = strip_fences(raw)
                if args.verbose:
                    vblock(f"completion · {model} / {target} / {stem} "
                           f"({time.time() - t0:.1f}s, {len(raw)} bytes)", raw)
                case_dir = os.path.join(GEN, model, target)
                os.makedirs(case_dir, exist_ok=True)
                html, err = compile_output(target, source, case_dir, stem)
                ok_dom, dom_err = (False, "did not compile")
                status = "compile-fail"
                if html is not None:
                    ok_dom, dom_err = dom_eq(ref_path, html, case_dir, stem)
                    if ok_dom:
                        status = "pass"
                    elif dom_eq_lenient(ref_html, html, case_dir, stem):
                        status = "ws-only"
                    else:
                        status = "dom-fail"
                merged[case_key] = {
                    "model": model, "target": target, "component": stem,
                    "compile": err is None, "dom": ok_dom, "status": status,
                    "error": err or (None if ok_dom else dom_err),
                }
                save()
                print(f"{model} {target} {stem}: {MARKS[status]}", flush=True)
                if args.verbose and not ok_dom:
                    kind = "compile error" if err else "DOM mismatch"
                    vblock(f"{kind} · {stem}", err or dom_err or "(no detail)")
            # Summarize from the accumulator so resumed cases count too.
            recs = [merged[(model, target, s)] for s in stems
                    if (model, target, s) in merged]
            n = len(recs)
            n_compile = sum(1 for r in recs if r["compile"])
            n_dom = sum(1 for r in recs if r["dom"])
            n_ws = sum(1 for r in recs if r.get("status") == "ws-only")
            print(
                f"\n== {model} / {target}: compile {n_compile}/{n}, "
                f"DOM-equivalent {n_dom}/{n} "
                f"(+{n_ws} whitespace-only misses)\n", flush=True,
            )

    print(f"raw results ({len(merged)} cases): {res_path}")


if __name__ == "__main__":
    main()
