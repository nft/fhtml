#!/usr/bin/env python3
"""Emit bench/shorthand-legend.md from the codebook in src/shorthand.rs.

The legend is the extra system-prompt text a model needs to *author* shorthand
fhtml (`ti4` → `text-indigo-400`). Generating it from the Rust source is the
only way to guarantee it can never drift from the compiler's actual codebook —
a legend that taught a code the compiler doesn't honor would silently poison
the generation benchmark. Run after any edit to the `const` tables:

  python3 bench/gen_legend.py

Zero deps (stdlib re). Prints the token count (o200k_base) if tiktoken is
importable, since the legend's size is the fixed cost the economics must beat.
"""

import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "src", "shorthand.rs")
OUT = os.path.join(ROOT, "bench", "shorthand-legend.md")


def const_pairs(name, text):
    """Extract the `(code, full)` tuples from a `const NAME: &[...] = &[ ... ];`."""
    m = re.search(rf"const {name}[^=]*=\s*&\[(.*?)\];", text, re.S)
    if not m:
        raise SystemExit(f"could not find const {name} in {SRC}")
    return re.findall(r'\("([^"]*)",\s*"([^"]*)"\)', m.group(1))


def const_strs(name, text):
    """Extract a flat `const NAME: &[&str] = &[ "a", "b", ... ];`."""
    m = re.search(rf"const {name}[^=]*=\s*&\[(.*?)\];", text, re.S)
    if not m:
        raise SystemExit(f"could not find const {name} in {SRC}")
    return re.findall(r'"([^"]*)"', m.group(1))


def main():
    text = open(SRC).read()
    props = const_pairs("PROPS", text)
    colors = const_pairs("COLORS", text)
    noshade = const_pairs("COLORS_NOSHADE", text)
    spacing_props = const_pairs("SPACING_PROPS", text)
    scale = const_strs("SPACING_SCALE", text)
    table = const_pairs("TABLE", text)

    def row(pairs):
        return " ".join(f"{c}={f.rstrip('-')}" for c, f in pairs)

    lines = []
    A = lines.append
    A("# fhtml class shorthand — legend")
    A("")
    A("An **optional** contraction of Tailwind class tokens: a short code stands")
    A("in for a long class (`ti4` → `text-indigo-400`). It is a *superset* of")
    A("plain fhtml — **any class you don't have a code for, just write in full**;")
    A("both compile identically. Use codes where you know them to save tokens;")
    A("never guess a code.")
    A("")
    A("**Turn it on:** the file's first line must be exactly `#!shorthand`.")
    A("")
    A("## Colors — `{property}{color}{shade}`, no separators")
    A("")
    A("Concatenate a property code, a color code, and a shade digit:")
    A("`bg-indigo-400` → `b`+`i`+`4` = `bi4`. `text-slate-900` → `tsl9`.")
    A("")
    A(f"- **property:** {row(props)}")
    A(f"- **color:** {row(colors)}")
    A(f"- **shade:** 100–900 → one digit (`4`=400, `9`=900); `50`/`950` written in full "
      f"(`ti50`=text-indigo-50). {row(noshade)} carry no shade (`tw`=text-white).")
    A("")
    A("## Spacing / sizing — `{property}{value}`, drop the hyphens")
    A("")
    A("`px-4` → `px4`, `gap-x-6` → `gx6`, `-mt-4` → `-mt4` (keep the leading `-`).")
    A(f"- **property:** {row(spacing_props)}")
    A(f"- **value** (Tailwind scale only): {' '.join(scale)}")
    A("")
    A("## Common utilities — exact codes")
    A("")
    A(" ".join(f"`{c}`={f}" for c, f in table))
    A("")
    A("## Variants (`hover:`, `dark:`, `sm:`, stacked `dark:hover:`)")
    A("")
    A("Keep the `variant:` prefix **verbatim** and encode only the base after the")
    A("last colon: `hover:bg-blue-500` → `hover:bb5`, `dark:hover:bg-slate-800` →")
    A("`dark:hover:bsl8`. Never abbreviate the variant word itself.")
    A("")
    A("## Escape")
    A("")
    A("If a literal class would collide with a code, prefix it with `=`")
    A("(`=ic` compiles to the literal class `ic`, not `items-center`). Rare.")
    A("")

    body = "\n".join(lines)
    with open(OUT, "w") as fh:
        fh.write(body)
    print(f"wrote {OUT} ({len(body)} bytes)")
    try:
        import tiktoken
        n = len(tiktoken.get_encoding("o200k_base").encode(body))
        print(f"legend tokens (o200k_base): {n}")
    except ImportError:
        pass


if __name__ == "__main__":
    main()
