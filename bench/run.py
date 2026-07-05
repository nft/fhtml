#!/usr/bin/env python3
"""fhtml benchmark: token counts and round-trip fidelity over bench/corpus/.

For every corpus file, produces four same-DOM representations —

  html-pretty  canonical 2-space-indented HTML (fhtml's Pretty emitter),
               the form an agent is normally asked to write
  html-min     minified HTML (fhtml's Min emitter), the aggressive baseline
  pug          conservative idiomatic Pug (pug_emit.py)
  fhtml        canonical fhtml (html2fhtml)

— then counts bytes and BPE tokens (tiktoken o200k_base and cl100k_base)
for each, and verifies the HTML→fhtml→HTML round-trip with
`html2fhtml --check`. Intermediates land in bench/out/ for inspection;
results in bench/RESULTS.md.

Usage:
  python3 bench/run.py [--validate-pug] [--corpus DIR]

--validate-pug compiles every emitted .pug with the real Pug compiler
(needs `npm install --prefix bench/.tools pug` once).
"""

import argparse
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pug_emit  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
H2F = os.path.join(ROOT, "target", "release", "html2fhtml")
FHTML = os.path.join(ROOT, "target", "release", "fhtml")
OUT = os.path.join(ROOT, "bench", "out")
FORMATS = ["html-pretty", "html-min", "pug", "fhtml"]


def run(cmd, stdin=None):
    p = subprocess.run(
        cmd, input=stdin, capture_output=True, text=True, check=False
    )
    return p.returncode, p.stdout, p.stderr


def build_representations(path):
    """name → source text for one corpus file, plus round-trip status.

    All forms derive from `--convert-svg` output so each syntax expresses
    SVG natively (the default raw-passthrough would carry the source file's
    own indentation verbatim into three of the four columns — measuring the
    corpus author's formatting, not the syntax)."""
    code, fhtml_src, err = run([H2F, "--convert-svg", path])
    if code != 0:
        raise RuntimeError(f"{path}: html2fhtml failed:\n{err}")
    warnings = sum(1 for line in err.splitlines() if ": warning: " in line)
    rt_code, _, rt_err = run([H2F, "--convert-svg", "--check", path])
    _, pretty, _ = run([FHTML, "--pretty"], stdin=fhtml_src)
    _, minified, _ = run([FHTML, "--min"], stdin=fhtml_src)
    reps = {
        "html-pretty": pretty,
        "html-min": minified,
        "pug": pug_emit.convert(pretty),
        "fhtml": fhtml_src,
    }
    return reps, rt_code == 0, rt_err, warnings


def svg_share(html_min):
    """Fraction of the minified HTML taken up by inline <svg> subtrees —
    incompressible payload no syntax can save on."""
    svg = sum(len(m) for m in
              re.findall(r"<svg\b.*?</svg>", html_min, flags=re.S))
    return svg / len(html_min) if html_min else 0.0


def validate_pug(pug_path):
    script = (
        "const pug=require(process.argv[1]);"
        "pug.renderFile(process.argv[2]);"
    )
    pug_mod = os.path.join(ROOT, "bench", ".tools", "node_modules", "pug")
    code, _, err = run(["node", "-e", script, pug_mod, pug_path])
    return code == 0, err


def fmt_row(cells, widths):
    return "| " + " | ".join(str(c).rjust(w) for c, w in zip(cells, widths)) + " |"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=os.path.join(ROOT, "bench", "corpus"))
    ap.add_argument("--validate-pug", action="store_true")
    args = ap.parse_args()

    try:
        import tiktoken
    except ImportError:
        sys.exit("tiktoken is required: pip3 install tiktoken")
    encoders = {
        "o200k_base": tiktoken.get_encoding("o200k_base"),
        "cl100k_base": tiktoken.get_encoding("cl100k_base"),
    }

    for binary in (H2F, FHTML):
        if not os.path.exists(binary):
            sys.exit(
                f"{binary} not found — run "
                "`cargo build --release --features convert` first"
            )

    files = sorted(
        f for f in os.listdir(args.corpus) if f.endswith((".html", ".htm"))
    )
    if not files:
        sys.exit(f"no .html files in {args.corpus}")

    ext = {"html-pretty": ".pretty.html", "html-min": ".min.html",
           "pug": ".pug", "fhtml": ".fhtml"}
    for fmt in FORMATS:
        os.makedirs(os.path.join(OUT, fmt), exist_ok=True)

    rows = []
    totals = {f: {"bytes": 0, "o200k_base": 0, "cl100k_base": 0}
              for f in FORMATS}
    rt_pass = 0
    warn_total = 0
    pug_fail = []

    for name in files:
        path = os.path.join(args.corpus, name)
        reps, rt_ok, rt_err, warnings = build_representations(path)
        warn_total += warnings
        if rt_ok:
            rt_pass += 1
        else:
            print(f"ROUND-TRIP FAIL {name}\n{rt_err}", file=sys.stderr)

        stem = os.path.splitext(name)[0]
        row = {"name": stem, "rt": rt_ok, "warnings": warnings,
               "svg": svg_share(reps["html-min"])}
        for fmt in FORMATS:
            text = reps[fmt]
            out_path = os.path.join(OUT, fmt, stem + ext[fmt])
            with open(out_path, "w") as fh:
                fh.write(text)
            row[fmt] = {
                "bytes": len(text.encode()),
                **{k: len(e.encode(text)) for k, e in encoders.items()},
            }
            for k in totals[fmt]:
                totals[fmt][k] += row[fmt][k]
        rows.append(row)

        if args.validate_pug:
            ok, err = validate_pug(os.path.join(OUT, "pug", stem + ".pug"))
            if not ok:
                pug_fail.append((name, err.strip().splitlines()[-1]))

    # ── Report ──────────────────────────────────────────────────────────────
    enc = "o200k_base"
    header = ["component", "svg%", "html-pretty", "html-min", "pug", "fhtml",
              "vs pretty", "vs min", "vs pug"]
    lines = [
        "# Benchmark results — token counts & round-trip fidelity",
        "",
        f"Corpus: {len(files)} components in `bench/corpus/`. "
        "All four columns render the identical DOM "
        "(see `bench/README.md` for methodology). "
        "`svg%` is the share of the minified HTML occupied by inline "
        "`<svg>` payload — bytes no syntax can save on.",
        "",
        f"## Tokens per component ({enc})",
        "",
    ]
    widths = [max(len(h), 24) if i == 0 else max(len(h), 6)
              for i, h in enumerate(header)]
    lines.append(fmt_row(header, widths))
    lines.append("|" + "|".join("-" * (w + 2) for w in widths) + "|")

    def pct(fh, other):
        return f"{(fh / other - 1) * 100:+.0f}%" if other else "n/a"

    for row in rows:
        fh = row["fhtml"][enc]
        lines.append(fmt_row(
            [row["name"], f"{row['svg'] * 100:.0f}%",
             row["html-pretty"][enc], row["html-min"][enc],
             row["pug"][enc], fh,
             pct(fh, row["html-pretty"][enc]),
             pct(fh, row["html-min"][enc]),
             pct(fh, row["pug"][enc])],
            widths,
        ))

    def total_line(label, subset):
        t = {f: sum(r[f][enc] for r in subset) for f in FORMATS}
        return fmt_row(
            [label, "", t["html-pretty"], t["html-min"], t["pug"],
             t["fhtml"],
             pct(t["fhtml"], t["html-pretty"]),
             pct(t["fhtml"], t["html-min"]), pct(t["fhtml"], t["pug"])],
            widths,
        )

    lines.append(total_line("**total**", rows))
    light = [r for r in rows if r["svg"] < 0.05]
    if light and len(light) < len(rows):
        lines.append(total_line(f"**svg-light (n={len(light)})**", light))

    lines += ["", "## Totals across tokenizers", ""]
    thead = ["measure", "html-pretty", "html-min", "pug", "fhtml",
             "vs pretty", "vs min", "vs pug"]
    twidths = [max(len(h), 11) for h in thead]
    lines.append(fmt_row(thead, twidths))
    lines.append("|" + "|".join("-" * (w + 2) for w in twidths) + "|")
    for measure in ("o200k_base", "cl100k_base", "bytes"):
        vals = {f: totals[f][measure] for f in FORMATS}
        lines.append(fmt_row(
            [measure, vals["html-pretty"], vals["html-min"], vals["pug"],
             vals["fhtml"],
             pct(vals["fhtml"], vals["html-pretty"]),
             pct(vals["fhtml"], vals["html-min"]),
             pct(vals["fhtml"], vals["pug"])],
            twidths,
        ))

    lines += [
        "",
        "## Round-trip fidelity",
        "",
        f"- `html2fhtml --check` (normalized-DOM equality): "
        f"**{rt_pass}/{len(files)} pass**",
        f"- converter warnings across the corpus: {warn_total}",
    ]
    if args.validate_pug:
        status = ("all compiled" if not pug_fail
                  else f"{len(pug_fail)} FAILED")
        lines.append(f"- emitted Pug validated with pug 3.x: {status}")
        for name, err in pug_fail:
            lines.append(f"  - {name}: {err}")
    lines.append("")

    report = "\n".join(lines)
    with open(os.path.join(ROOT, "bench", "RESULTS.md"), "w") as fh:
        fh.write(report)
    print(report)


if __name__ == "__main__":
    main()
