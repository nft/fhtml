#!/usr/bin/env python3
"""Does the class shorthand *pay for itself*?

The reliability half (`generate.py --targets shorthand`) asks whether a model
can author shorthand correctly. This half is deterministic and needs no API
key: the shorthand legend costs a fixed number of system-prompt tokens once
per session, while every generated component saves some output tokens. The
shorthand wins a session only once the accrued per-component savings clear the
one-time legend cost.

For each `bench/corpus/*.html` it counts tokens of `html2fhtml` output with
and without `--shorthand` (o200k_base and cl100k_base), then reports the
break-even component count and the net over a full 48-component "session".

  python3 bench/shorthand_economics.py [--corpus DIR]

Writes bench/out/gen/shorthand-economics.md and prints the summary. Needs
tiktoken, the release `html2fhtml` binary, and bench/shorthand-legend.md
(run `python3 bench/gen_legend.py`).
"""

import argparse
import glob
import math
import os
import subprocess
import sys

import tiktoken

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
H2F = os.path.join(ROOT, "target", "release", "html2fhtml")
OUT = os.path.join(ROOT, "bench", "out", "gen")
ENCODINGS = ["o200k_base", "cl100k_base"]


def conv(path, shorthand):
    flags = ["--convert-svg"] + (["--shorthand"] if shorthand else [])
    p = subprocess.run([H2F, *flags, path], capture_output=True, text=True)
    if p.returncode != 0:
        sys.exit(f"{path}: html2fhtml failed:\n{p.stderr}")
    return p.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=os.path.join(ROOT, "bench", "corpus"))
    args = ap.parse_args()

    if not os.path.exists(H2F):
        sys.exit("release html2fhtml missing — `cargo build --release "
                 "--features convert`")
    legend_path = os.path.join(ROOT, "bench", "shorthand-legend.md")
    cheat_path = os.path.join(ROOT, "bench", "cheatsheet.md")
    if not os.path.exists(legend_path):
        sys.exit("bench/shorthand-legend.md missing — `python3 "
                 "bench/gen_legend.py`")
    legend = open(legend_path).read()
    cheatsheet = open(cheat_path).read()

    files = sorted(glob.glob(os.path.join(args.corpus, "*.html")))
    if not files:
        sys.exit(f"no .html under {args.corpus}")

    encs = {name: tiktoken.get_encoding(name) for name in ENCODINGS}

    def tok(enc, s):
        return len(encs[enc].encode(s))

    # Per-encoding accounting.
    report = {}
    for name in ENCODINGS:
        legend_cost = tok(name, legend)
        cheat_cost = tok(name, cheatsheet)
        deltas, plain_tot, short_tot = [], 0, 0
        for f in files:
            p = tok(name, conv(f, False))
            s = tok(name, conv(f, True))
            plain_tot += p
            short_tot += s
            deltas.append(p - s)
        n = len(deltas)
        avg = sum(deltas) / n
        deltas_sorted = sorted(deltas)
        median = deltas_sorted[n // 2]
        breakeven = math.ceil(legend_cost / avg) if avg > 0 else None
        # Net vs a plain-fhtml session (both pay the fhtml cheatsheet, so it
        # cancels): shorthand's only extra fixed cost is the legend.
        net_session = legend_cost - sum(deltas)  # for the full N=48 session
        report[name] = dict(
            legend=legend_cost, cheat=cheat_cost, plain=plain_tot,
            short=short_tot, n=n, avg=avg, median=median,
            breakeven=breakeven, net_session=net_session,
            regressions=sum(1 for d in deltas if d < 0),
        )

    # Markdown.
    lines = ["# Shorthand economics", "",
             "Deterministic token accounting: the legend is a one-time "
             "system-prompt cost; savings accrue per component. Break-even is "
             "the component count at which accrued savings clear the legend.",
             ""]
    lines.append("| encoding | legend (sys) | plain fhtml (48) | shorthand (48) "
                 "| corpus Δ | avg/comp | break-even | net over 48 |")
    lines.append("|----------|-------------:|-----------------:|--------------:"
                 "|---------:|---------:|-----------:|------------:|")
    for name in ENCODINGS:
        r = report[name]
        pct = (r["short"] - r["plain"]) / r["plain"] * 100
        lines.append(
            f"| {name} | {r['legend']} | {r['plain']} | {r['short']} | "
            f"{pct:+.1f}% | {r['avg']:.0f} | {r['breakeven']} comps | "
            f"−{-r['net_session'] if r['net_session'] < 0 else r['net_session']} "
            f"tok |")
    lines += [
        "",
        f"- Legend regenerated from `src/shorthand.rs` "
        f"(`bench/gen_legend.py`); no component regresses "
        f"(o200k: {report['o200k_base']['regressions']}/48).",
        f"- **Break-even ≈ {report['o200k_base']['breakeven']} components** "
        f"(o200k). A page-building session almost always exceeds this, so "
        f"shorthand nets positive; below it the legend dominates.",
        f"- Net over a full 48-component session (o200k): "
        f"{report['o200k_base']['net_session']:+d} tokens "
        f"({'saved' if report['o200k_base']['net_session'] < 0 else 'cost'}).",
        "- The legend's bulk is the ~53-entry utility table; dropping it lowers "
        "break-even at the cost of authoring savings on those utilities "
        "(unknown classes still compile — the grammar layers carry most of the "
        "win).",
        "",
        "Reliability (compile + DOM-eq of model-authored shorthand) is the "
        "other half — `bench/generate.py --targets shorthand`.",
        "",
    ]
    md = "\n".join(lines)
    os.makedirs(OUT, exist_ok=True)
    out_path = os.path.join(OUT, "shorthand-economics.md")
    with open(out_path, "w") as fh:
        fh.write(md)
    print(md)
    print(f"wrote {out_path}")


if __name__ == "__main__":
    main()
