# fhtml benchmark harness

Measure, don't believe. Four questions, three tools:

1. **How many tokens does fhtml actually save?** — `run.py`
2. **Does the HTML→fhtml round-trip preserve the DOM?** — `run.py` (via `html2fhtml --check`)
3. **Can LLMs write fhtml without more errors than the incumbent?** — `generate.py`
4. **Does Tailwind's scanner see fhtml classes?** — `tailwind_scan.sh`

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

The **`fhtml-def`** target answers the components question: same translation task, but the system prompt adds
[cheatsheet-components.md](cheatsheet-components.md) and the few-shot adds a
hand-written repetitive example (`tests/corpus/feature-list-def.fhtml`).
Grading is unchanged — the compiler expands calls before `--dom-eq` — plus a
third first-class metric, **compression**: the model's output tokens (o200k,
needs `pip3 install tiktoken`) vs the plain-fhtml reference in `out/fhtml/`.
Gate: ≥15% median compression on the repetitive half of the corpus (split at
the median structural-repetition score of the references) with a DOM rate
within 10 points of the plain `fhtml` target's.

Two further targets test scaffolding ideas:
**`fhtml-def-plan`** is `fhtml-def` behind a two-section protocol — the model
writes a `PLAN:` (skeleton line, def signatures with a per-instance
differences ledger, stays-plain list) and then the `SOURCE:`; only the source
compiles, the plan's tokens are priced separately (`tokens_plan`,
`total_compression`), and mechanical adherence fields (`plan_present`,
`defs_match_plan`, …) make any improvement attributable to the ledger.
**`microparts`** is a competing-format control — the whole completion is one
JSON document (`{"body": …, "parts": {…}}` with `{{part key="value"}}` calls
and `{{slot}}` slots) assembled by `microparts_assemble.py`
([cheatsheet-microparts.md](cheatsheet-microparts.md) is the scheme spec;
`test_microparts.py` pins the grammar). A structured assembler error is the
compile-fail analog; `tokens_out` counts the whole JSON. No target gets any
repair beyond fence-stripping — grading symmetry. All new records carry the
run date and the response's `api_model`/`api_provider` strings.

Free-form generation from a text brief and exact-match *edit* tasks on
existing fhtml files are measured by the separate **agent harness** in
[agents/](agents/) — it runs local CLI coding agents (Claude Code, Codex,
Gemini CLI) end-to-end in throwaway workspaces, with and without the
[fhtml skill](../skills/) installed.

## Tailwind `@source` scanning: `tailwind_scan.sh`

```sh
npm install --prefix bench/.tools tailwindcss @tailwindcss/cli
bench/tailwind_scan.sh
```

Builds CSS twice from the same corpus — `@source` pointed at the HTML
originals, then at their fhtml conversions (`source(none)` isolates the
test) — and diffs. Passes when the fhtml scan covers every utility the HTML
scan found. Verified against tailwindcss v4.3.2: full coverage including
arbitrary values, `data-[…]:` variants, fractions, and hex colors; one
harmless superset artifact (bare tag tokens that name a utility, e.g.
`table`, add dead CSS).

## Files

- `run.py` — token + round-trip benchmark, writes `RESULTS.md` and `out/`
- `pug_emit.py` — HTML → conservative idiomatic Pug
- `generate.py` — LLM translation benchmark (needs `ANTHROPIC_API_KEY`)
- `tailwind_scan.sh` — Tailwind `@source` coverage check (needs the npm install above)
- `cheatsheet.md` — the fhtml syntax reference given to models. **Frozen between
  re-baselinings**: it is the recorded sweeps' prompt, and comparability dies with any
  edit. It predates raw-text elements (SPEC §6.3, added 2026-07-16) — add that section
  (and switch `html2fhtml` to emitting the `script` + `|` form) only at the next full
  re-baselining, never alone.
- `cheatsheet-components.md` — the components section (`fhtml-def` + `fhtml-def-plan`)
- `cheatsheet-microparts.md` — the micro-parts scheme spec (`microparts` target)
- `microparts_assemble.py` — the micro-parts grammar/assembler (the control's grader)
- `test_microparts.py` — assembler unit tests (48 checks; run before trusting a sweep)
- `corpus/` — the 48-component corpus
- `out/`, `.tools/` — generated artifacts and local npm installs (gitignored)
