#!/usr/bin/env python3
"""Ad-hoc token-economics probe — run the RESULTS.md measurement on YOUR
input instead of the fixed corpus.

Point it at any HTML (a file, several files, or stdin) and it reports how
many BPE tokens that exact markup costs as canonical pretty HTML, minified
HTML, idiomatic Pug, and fhtml, under both tokenizers:

  o200k_base   GPT-4o / o-series / recent OpenAI models
  cl100k_base  GPT-4 / GPT-3.5

The fhtml column and its `vs pretty / vs min / vs pug` deltas are the win.
Conversion and counting are byte-for-byte the same pipeline bench/run.py
uses for the corpus, so numbers here are comparable to bench/RESULTS.md.

Usage:
  python3 bench/tokens.py page.html                  # one file
  python3 bench/tokens.py a.html b.html c.html       # several + a total
  pbpaste | python3 bench/tokens.py                   # clipboard / stdin
  python3 bench/tokens.py --show fhtml page.html      # also print the fhtml
  python3 bench/tokens.py --show pug   page.html      # ...or the Pug, etc.

Requires the release binaries (`cargo build --release --features convert`)
and tiktoken (`pip3 install tiktoken`).
"""

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import run as bench  # noqa: E402  (build_representations, svg_share, FORMATS)


def get_encoders():
    try:
        import tiktoken
    except ImportError:
        sys.exit("tiktoken is required: pip3 install tiktoken")
    return {
        "o200k_base": tiktoken.get_encoding("o200k_base"),
        "cl100k_base": tiktoken.get_encoding("cl100k_base"),
    }


def measure(path, encoders):
    """Return (reps, counts, svg_share, round_trip_ok) for one HTML file."""
    reps, rt_ok, _rt_err, _warn = bench.build_representations(path)
    svg = bench.svg_share(reps["html-min"])
    counts = {
        fmt: {
            "bytes": len(reps[fmt].encode()),
            **{name: len(enc.encode(reps[fmt])) for name, enc in encoders.items()},
        }
        for fmt in bench.FORMATS
    }
    return reps, counts, svg, rt_ok


def pct(fhtml_val, other_val):
    return f"{(fhtml_val / other_val - 1) * 100:+.0f}%" if other_val else "n/a"


MEASURES = ("bytes", "o200k_base", "cl100k_base")


def print_table(label, counts, svg, rt_ok):
    tag = "" if rt_ok else "  [!] round-trip differs — see note below"
    svg_note = f"   (svg {svg * 100:.0f}% of minified HTML)" if svg is not None else ""
    print(f"\n{label}{svg_note}{tag}")
    head = f"  {'format':<12}" + "".join(f"{m:>13}" for m in MEASURES)
    print(head)
    print("  " + "-" * (len(head) - 2))
    for fmt in bench.FORMATS:
        cells = "".join(f"{counts[fmt][m]:>13,}" for m in MEASURES)
        print(f"  {fmt:<12}{cells}")
    # fhtml deltas vs the other three, per tokenizer
    print()
    for base in ("html-pretty", "html-min", "pug"):
        deltas = "   ".join(
            f"{enc}: {pct(counts['fhtml'][enc], counts[base][enc])}"
            for enc in ("o200k_base", "cl100k_base")
        )
        print(f"  fhtml vs {base:<11} {deltas}")


def main():
    ap = argparse.ArgumentParser(
        description="Token cost of your HTML as pretty/min/pug/fhtml.")
    ap.add_argument("files", nargs="*",
                    help="HTML files (omit to read one document from stdin)")
    ap.add_argument("--show", metavar="FORMAT", choices=bench.FORMATS,
                    help="also print the converted source in this format "
                         f"({', '.join(bench.FORMATS)})")
    args = ap.parse_args()

    for binary in (bench.H2F, bench.FHTML):
        if not os.path.exists(binary):
            sys.exit(f"{binary} not found — run "
                     "`cargo build --release --features convert` first")

    encoders = get_encoders()

    # Resolve inputs to a list of (label, path); stdin lands in a temp file.
    tmp = None
    inputs = []
    if args.files:
        for f in args.files:
            if not os.path.isfile(f):
                sys.exit(f"not a file: {f}")
            inputs.append((f, f))
    else:
        data = sys.stdin.read()
        if not data.strip():
            sys.exit("no input: pass HTML file(s) or pipe HTML on stdin")
        tmp = tempfile.NamedTemporaryFile(
            mode="w", suffix=".html", delete=False)
        tmp.write(data)
        tmp.close()
        inputs.append(("<stdin>", tmp.name))

    try:
        totals = {fmt: {m: 0 for m in MEASURES} for fmt in bench.FORMATS}
        any_rt_fail = False
        for label, path in inputs:
            reps, counts, svg, rt_ok = measure(path, encoders)
            any_rt_fail |= not rt_ok
            for fmt in bench.FORMATS:
                for m in MEASURES:
                    totals[fmt][m] += counts[fmt][m]
            print_table(label, counts, svg, rt_ok)
            if args.show:
                print(f"\n--- {label} as {args.show} "
                      f"{'-' * max(0, 40 - len(label) - len(args.show))}")
                print(reps[args.show].rstrip())
                print("-" * 44)

        if len(inputs) > 1:
            print_table(f"TOTAL ({len(inputs)} files)", totals, None, True)
    finally:
        if tmp:
            os.unlink(tmp.name)

    if any_rt_fail:
        print("\nNote: [!] means html→fhtml→html didn't reproduce the exact "
              "normalized DOM for that input — the fhtml token count is still "
              "valid, but inspect it with `--show fhtml` before trusting it as "
              "a faithful translation.", file=sys.stderr)


if __name__ == "__main__":
    main()
