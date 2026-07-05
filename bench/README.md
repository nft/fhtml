# fhtml benchmark harness

Measure, don't believe. Three questions, three tools:

1. **How many tokens does fhtml actually save?** — `run.py`
2. **Does the HTML→fhtml round-trip preserve the DOM?** — `run.py` (via `html2fhtml --check`)
3. **Can LLMs write fhtml without more errors than the incumbent?** — `generate.py`

## Corpus

`corpus/` holds 48 realistic Tailwind v4 components (marketing, application
UI, ecommerce, misc/hard cases) in the style of Tailwind UI: responsive and
state variants, `dark:`, arbitrary values (`max-w-[72rem]`, `bg-[#0f172a]`),
data-attribute variants, inline SVG icons, tables, `<pre><code>`, mixed
inline text. Each file is a pretty-printed HTML fragment that passes
`html2fhtml --check`.

## Token counts + round-trip: `run.py`

```sh
cargo build --release --features convert
pip3 install tiktoken
python3 bench/run.py --validate-pug   # pug validation needs: npm install --prefix bench/.tools pug
```

For every corpus file it derives four representations of the **identical
DOM** and counts bytes plus BPE tokens (tiktoken `o200k_base` and
`cl100k_base`):

| form | produced by | stands for |
|---|---|---|
| `html-pretty` | `fhtml --pretty` of the converted source | the clean HTML an agent is normally asked to write |
| `html-min` | `fhtml --min` | the aggressive HTML baseline |
| `pug` | `pug_emit.py` | conservative idiomatic Pug (the incumbent) |
| `fhtml` | `html2fhtml` | canonical fhtml |

Deriving all forms from the same AST keeps formatting differences out of the
comparison — this measures syntax, not indentation style. The Pug emitter is
deliberately fair: `.class` shorthand whenever every class on the element is
a legal Pug class literal, `(class="…")` otherwise (Tailwind variants and
arbitrary values are never legal literals — that asymmetry is fhtml's core
bet). `--validate-pug` compiles every emitted `.pug` with the real Pug 3.x
compiler. Intermediates land in `out/`; the report in [RESULTS.md](RESULTS.md).

## LLM generation errors: `generate.py`

```sh
export ANTHROPIC_API_KEY=…
python3 bench/run.py                  # populates out/ (references + few-shot)
python3 bench/generate.py --models claude-haiku-4-5-20251001,claude-sonnet-5 --targets fhtml,pug
```

Task: translate each component's canonical pretty HTML into fhtml (and, as
control, Pug). The model gets the complete syntax reference
([cheatsheet.md](cheatsheet.md)) and one few-shot pair (from `tests/corpus/`,
outside the eval set). Two automatic grades per completion:

- **compile** — the output compiles (`fhtml` binary / real Pug compiler);
- **dom** — the compiled HTML is normalized-DOM-equivalent to the reference
  (`html2fhtml --dom-eq`: comments and inter-element whitespace are
  non-contractual, attribute order and boolean forms unified — the same
  comparator `--check` trusts).

Translation (not free-form generation) is used because it's exactly
auto-gradable: the reference DOM is known. Caveats: one-model-family
harness (Anthropic API, stdlib-only); Pug's whitespace semantics differ
slightly from fhtml's, so a Pug completion can lose a contractual interior
space that fhtml's `|`-idiom preserves — that counts against it, and is a
real fidelity difference, not grader bias.

Not yet measured (needs a separate task design): free-form generation from a
visual/text brief, and exact-match *edit* tasks on existing fhtml files.

## Files

- `run.py` — token + round-trip benchmark, writes `RESULTS.md` and `out/`
- `pug_emit.py` — HTML → conservative idiomatic Pug
- `generate.py` — LLM translation benchmark (needs `ANTHROPIC_API_KEY`)
- `cheatsheet.md` — the fhtml syntax reference given to models
- `corpus/` — the 48-component corpus
- `out/`, `.tools/` — generated artifacts and the local Pug install (gitignored)
