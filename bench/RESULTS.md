# Benchmark results — token counts & round-trip fidelity

Corpus: 48 components in `bench/corpus/`. All four columns render the identical DOM (see `bench/README.md` for methodology). `svg%` is the share of the minified HTML occupied by inline `<svg>` payload — bytes no syntax can save on.

## Tokens per component (o200k_base)

|                component |   svg% | html-pretty | html-min |    pug |  fhtml | vs pretty | vs min | vs pug |
|--------------------------|--------|-------------|----------|--------|--------|-----------|--------|--------|
|            activity-feed |    15% |        1536 |     1411 |   1329 |   1284 |      -16% |    -9% |    -3% |
|               blog-cards |     0% |        1519 |     1382 |   1274 |   1256 |      -17% |    -9% |    -1% |
|              breadcrumbs |    61% |         984 |      951 |    906 |    880 |      -11% |    -7% |    -3% |
|           calendar-month |    14% |        1716 |     1589 |   1526 |   1403 |      -18% |   -12% |    -8% |
|               cart-panel |     4% |        1596 |     1439 |   1330 |   1256 |      -21% |   -13% |    -6% |
|         category-filters |     4% |        2019 |     1801 |   1649 |   1592 |      -21% |   -12% |    -3% |
|              chat-thread |    31% |        1498 |     1422 |   1372 |   1326 |      -11% |    -7% |    -3% |
|            checkout-form |    11% |        2178 |     1995 |   1851 |   1759 |      -19% |   -12% |    -5% |
|            cookie-banner |    31% |        1196 |     1154 |   1134 |   1109 |       -7% |    -4% |    -2% |
|                 cta-band |    28% |         589 |      551 |    526 |    498 |      -15% |   -10% |    -5% |
|          dashboard-stats |    16% |        2159 |     2001 |   1942 |   1819 |      -16% |    -9% |    -6% |
|               data-table |     0% |        1879 |     1752 |   1685 |   1557 |      -17% |   -11% |    -8% |
|         description-list |    25% |        1634 |     1528 |   1430 |   1389 |      -15% |    -9% |    -3% |
|                error-404 |    44% |        1738 |     1663 |   1588 |   1535 |      -12% |    -8% |    -3% |
|            faq-accordion |    29% |        1095 |     1017 |    989 |    944 |      -14% |    -7% |    -5% |
|             feature-grid |    26% |        1085 |      995 |    924 |    900 |      -17% |   -10% |    -3% |
|            file-dropzone |    56% |        2362 |     2278 |   2197 |   2132 |      -10% |    -6% |    -3% |
|           footer-columns |    32% |        2295 |     2141 |   2018 |   1935 |      -16% |   -10% |    -4% |
|               hero-split |     8% |         738 |      697 |    680 |    652 |      -12% |    -6% |    -4% |
|                  invoice |     0% |        1425 |     1240 |   1091 |   1065 |      -25% |   -14% |    -2% |
|             kanban-board |    10% |        1685 |     1536 |   1458 |   1372 |      -19% |   -11% |    -6% |
|               login-form |    30% |        1992 |     1895 |   1837 |   1767 |      -11% |    -7% |    -4% |
|               logo-cloud |     0% |         673 |      641 |    628 |    592 |      -12% |    -8% |    -6% |
|             modal-dialog |    21% |         861 |      822 |    786 |    743 |      -14% |   -10% |    -5% |
|         navbar-marketing |    18% |        1341 |     1260 |   1230 |   1161 |      -13% |    -8% |    -6% |
|        newsletter-signup |    17% |        1152 |     1079 |   1022 |    984 |      -15% |    -9% |    -4% |
|            notifications |    31% |        1820 |     1758 |   1672 |   1596 |      -12% |    -9% |    -5% |
|            order-history |     8% |        2154 |     1999 |   1921 |   1754 |      -19% |   -12% |    -9% |
|            order-summary |     8% |        1235 |     1116 |    998 |    982 |      -20% |   -12% |    -2% |
|               pagination |    17% |        1113 |     1064 |   1048 |   1015 |       -9% |    -5% |    -3% |
|            pricing-tiers |    40% |        2671 |     2521 |   2390 |   2331 |      -13% |    -8% |    -2% |
|           product-detail |    12% |        2132 |     1982 |   1908 |   1817 |      -15% |    -8% |    -5% |
|             product-grid |    31% |        2259 |     2066 |   1942 |   1890 |      -16% |    -9% |    -3% |
|           progress-steps |    20% |        1023 |      951 |    896 |    848 |      -17% |   -11% |    -5% |
|             promo-banner |    25% |        1496 |     1412 |   1368 |   1312 |      -12% |    -7% |    -4% |
|               quick-view |    35% |        2557 |     2424 |   2349 |   2268 |      -11% |    -6% |    -3% |
|          reviews-section |    59% |        4872 |     4681 |   4428 |   4380 |      -10% |    -6% |    -1% |
|            settings-form |     0% |        2021 |     1836 |   1671 |   1631 |      -19% |   -11% |    -2% |
|              sidebar-nav |    52% |        3014 |     2920 |   2882 |   2796 |       -7% |    -4% |    -3% |
|               slide-over |    16% |        2058 |     1927 |   1790 |   1716 |      -17% |   -11% |    -4% |
|             stacked-list |    17% |        2134 |     1983 |   1863 |   1775 |      -17% |   -10% |    -5% |
|               stats-band |     0% |         591 |      535 |    497 |    481 |      -19% |   -10% |    -3% |
|                store-nav |    34% |        2447 |     2311 |   2245 |   2151 |      -12% |    -7% |    -4% |
|                     tabs |    53% |        2179 |     2117 |   2093 |   2046 |       -6% |    -3% |    -2% |
|         team-empty-state |    40% |        1035 |      994 |    948 |    956 |       -8% |    -4% |    +1% |
|             testimonials |     0% |         832 |      753 |    685 |    675 |      -19% |   -10% |    -1% |
|               video-card |    37% |        1102 |     1059 |   1028 |    986 |      -11% |    -7% |    -4% |
|                 wishlist |    32% |        3008 |     2863 |   2737 |   2651 |      -12% |    -7% |    -3% |
|                **total** |        |       82698 |    77512 |  73761 |  70967 |      -14% |    -8% |    -4% |
|      **svg-light (n=9)** |        |       12555 |    11379 |  10510 |  10105 |      -20% |   -11% |    -4% |

## Totals across tokenizers

|     measure | html-pretty |    html-min |         pug |       fhtml |   vs pretty |      vs min |      vs pug |
|-------------|-------------|-------------|-------------|-------------|-------------|-------------|-------------|
|  o200k_base |       82698 |       77512 |       73761 |       70967 |        -14% |         -8% |         -4% |
| cl100k_base |       83450 |       78265 |       74552 |       71845 |        -14% |         -8% |         -4% |
|       bytes |      242442 |      220950 |      219819 |      206100 |        -15% |         -7% |         -6% |

## Round-trip fidelity

- `html2fhtml --check` (normalized-DOM equality): **48/48 pass**
- converter warnings across the corpus: 0
- emitted Pug validated with pug 3.x: all compiled

## Generation reliability (models writing fhtml)

Can models *write* fhtml correctly? `bench/generate.py`: each model translates the 48
pretty-HTML components (temperature 0, one few-shot example, cheatsheet in the system
prompt), output graded by compiling it and comparing normalized DOMs against the source.
`ws-only` = DOM-equal except text whitespace. Run 2026-07-07 via OpenRouter
(nemotron added 2026-07-10).

| model | target | compiles | strict DOM-eq | + ws-only | DOM-valid |
|-------|--------|---------:|--------------:|----------:|----------:|
| claude-haiku-4.5 | fhtml | **46/48** | 25 | 5 | **30/48** |
| claude-haiku-4.5 | pug | 14/48 | 5 | 3 | 8/48 |
| deepseek-v4-pro | fhtml | **42/48** | 20 | 14 | **34/48** |
| deepseek-v4-pro | pug | 35/48 | 10 | 20 | 30/48 |
| tencent/hy3 | fhtml | **47/48** | 23 | 14 | **37/48** |
| tencent/hy3 | pug | 11/48 | 2 | 4 | 6/48 |
| nemotron-3-ultra | fhtml | **39/48** | 19 | 5 | **24/48** |
| nemotron-3-ultra | pug | 25/48 | 8 | 12 | 20/48 |

Pooled over the four models: fhtml compiles **90.6%** vs Pug's 44.3%; strict
DOM-equivalence **45.3%** vs 13.0%; DOM-valid 65.1% vs 33.3%. fhtml wins every model on
every metric, though nemotron is where Pug comes closest (24 vs 20 DOM-valid).
Pug's failures are dominated by its lexer rejecting Tailwind class syntax
(`w-1/2`, `data-[state=open]:…`); fhtml's residual misses are mostly cosmetic whitespace
plus one recurring real hazard — attributes written as bare tokens
(`div aria-hidden=true …` becomes a class; 11 of haiku's 16 DOM misses), which the
compiler now flags with a warning. The html-minification control (same grading, the
syntax models already know) is wired but unswept.

## Components in generation (`fhtml-def`)

Does the hand-written −46% (blog-cards with `def`) survive real generation? Same
translation task with the components cheatsheet; **compression** = token reduction vs the
same model-agnostic plain-fhtml reference (o200k). The gate: on the repetitive half of
the corpus (24/48, split at median structural-repetition), ≥15% median compression with a
DOM-valid rate within 10 points of the model's plain-fhtml run. Compression counts only
DOM-valid output — shrinking by dropping elements is not compression. Run 2026-07-09.

| model | compiles | DOM-valid | vs plain fhtml | median compression (DOM-valid) | repetitive half | gate |
|-------|---------:|----------:|---------------:|-------------------------------:|----------------:|------|
| tencent/hy3 (reasoning) | 45/48 | **33/48 (69%)** | −8.3 pts | +11.3% (33 cases) | **+25.4%** (15 cases) | **PASS** |
| claude-haiku-4.5 | 28/48 | 10/48 (21%) | −41.7 pts | +13.7% (10 cases) | +29.3% (4 cases) | FAIL (DOM) |
| nemotron-3-ultra | 19/48 | 10/48 (21%) | −29.2 pts | −2.2% (10 cases) | +20.5% (2 cases) | FAIL (DOM) |

hy3's DOM-valid repetitive-half output totals **24.1% fewer tokens** than plain fhtml
(15,648 vs 20,621), with per-component wins up to +48% (product-grid) and +43%
(pricing-tiers) — at essentially no reliability cost (45/48 vs 47/48 compiles, DOM gap
within the gate).

The split is by model class, not by syntax. When any model's def output is DOM-valid, the
compression is there (haiku's median on its valid cases: +29.3%). But under the
components prompt haiku regresses on *base* syntax — Pug mixin habits resurface (`h2#id`,
`.flex-auto`, `details.open`) and per-item differences get flattened when factoring
(`checked`, `aria-label`s, the selected item's classes). A reasoning model plans the
factoring and keeps the differences. Verdict: **components hold for reasoning-class
models and stay a human/review feature for fast non-reasoning models**; plain fhtml
remains the reliable agent floor for the latter (haiku: 30/48 DOM-valid).

*Addendum (2026-07-14):* the micro-parts sweep (next section) added two models to this
gate: **qwen/qwen3.7-max PASS** — 38/48 DOM-valid, −6.2 pts vs its plain run,
repetitive-half +18.3% (21 cases) — the second model to clear it; xiaomi/mimo-v2.5-pro
FAIL on DOM (28/48, −12.5 pts, despite +21.9% on its repetitive half).

## Plan-first scaffold vs the micro-parts JSON control (`fhtml-def-plan`, `microparts`)

Two hypotheses, one sweep
(2026-07-11 → 07-14, 3 models × 4 targets × 48). **H1**: does forcing a plan — def
signatures plus a per-instance *differences ledger*, written before the source —
recover the factoring that non-reasoning models flatten? **H2**: is "keep everything in
JSON" (`{"body": …, "parts": {…}}` with `{{part key="value"}}` calls and `{{slot}}`
slots, assembled by `bench/microparts_assemble.py`) competitive with fhtml? Models:
qwen/qwen3.7-max, xiaomi/mimo-v2.5-pro, tencent/hy3:free (hy3 replaced the pinned
kimi-k2.7-code mid-sweep; its plain/def baselines are its recorded 2026-07-09 runs —
the one cross-date comparison here). No model hit a completion-budget exhaustion, so
the reasoning-class split is not observable from the records; verdicts are per model.

| model | target | compiles | strict DOM-eq | + ws-only | DOM-valid | median compression (DOM-valid) | repetitive half |
|-------|--------|---------:|--------------:|----------:|----------:|-------------------------------:|----------------:|
| qwen3.7-max | fhtml | 46/48 | 24 | 17 | **41/48** | — | — |
| qwen3.7-max | fhtml-def | 42/48 | 21 | 17 | 38/48 | +7.7% (38) | +18.3% (21) |
| qwen3.7-max | fhtml-def-plan | 43/48 | 15 | 18 | 33/48 | +7.7% (33) | +27.9% (15) |
| qwen3.7-max | microparts | 47/48 | 31 | 14 | **45/48** | +0.4% (45) | +14.7% (23) |
| mimo-v2.5-pro | fhtml | 41/48 | 20 | 14 | 34/48 | — | — |
| mimo-v2.5-pro | fhtml-def | 41/48 | 14 | 14 | 28/48 | +10.7% (28) | +21.9% (15) |
| mimo-v2.5-pro | fhtml-def-plan | 42/48 | 21 | 10 | 31/48 | +8.0% (31) | +16.9% (15) |
| mimo-v2.5-pro | microparts | 47/48 | 31 | 10 | **41/48** | +2.8% (41) | +13.0% (20) |
| tencent/hy3 | fhtml | 47/48 | 23 | 14 | 37/48 | — | — |
| tencent/hy3 | fhtml-def | 45/48 | 23 | 10 | 33/48 | +11.3% (33) | +25.4% (15) |
| tencent/hy3 | fhtml-def-plan | 44/48 | 15 | 17 | 32/48 | +7.7% (32) | +23.2% (15) |
| tencent/hy3 | microparts | 43/48 | 23 | 17 | **40/48** | +11.2% (40) | +20.4% (21) |

Compression is vs the plain-fhtml reference (o200k), DOM-valid cases only. For
`fhtml-def-plan` it counts the SOURCE section; the plan's own tokens (median 100–128
per case) are priced separately in `total_compression`. For `microparts` it counts the
whole JSON — the envelope is the format.

### H1: the plan-first scaffold fails the gate — for all three models

Gate per model: DOM-valid within 10 points of its plain-fhtml run, ≥15% median source
compression and ≥10% median total (plan + source) compression on the repetitive half.

| model | DOM gap vs plain | src rep-half (≥15%) | total rep-half (≥10%) | Δ DOM-valid vs fhtml-def | gate |
|-------|-----------------:|--------------------:|----------------------:|-------------------------:|------|
| qwen3.7-max | −16.7 pts | +27.9% | +12.7% | **−5** | FAIL (DOM) |
| mimo-v2.5-pro | −6.2 pts | +16.9% | +3.1% | **+3** | FAIL (total) |
| tencent/hy3 | −10.4 pts | +23.2% | +9.4% | −1 | FAIL (both, marginal) |

The instructive part is *why* it fails. Protocol adherence was essentially perfect —
144/144 completions produced a `PLAN:` header and a clean `SOURCE:` split (zero
extraction failures, zero decorated markers), 144/144 wrote the skeleton line, 142/144
the stays-plain list, and in 143/144 the def names in the plan exactly matched the defs
in the source. The models do everything the scaffold asks — **and it doesn't help.**
qwen, which passes the plain components gate on its own, got *worse* under the
scaffold (38 → 33 DOM-valid, violating the ≤3 non-regression tolerance); its
compile failures are the same Pug habits as ever (`input#…`), unmoved by planning.
mimo moved in the hypothesized direction (+3 DOM-valid over `fhtml-def`) but pays more
in plan tokens than the economic floor allows (+3.1% total vs the ≥10% gate). hy3 was
flat (−1, within its non-regression tolerance). Per the plan's attribution rule
(decision 2): the failure is not non-adherence — writing a correct-looking differences
ledger does not make a model honor it while writing source. **Verdict: the single-call
plan-first scaffold is not a components rescue; the components verdict above stands
unamended.** The deferred two-call variant remains the only untested follow-up, and the
transcripts qualify for it (the models demonstrably state plans they then under-execute).

### H2: micro-parts is the most *reliable* format in the benchmark — and loses on tokens to fhtml-def where fhtml-def works

Two pinned populations: reliability over all 48, and total output tokens
on the pairwise DOM-valid intersection with the model's better def target.

| model | DOM-valid, all 48 (plain fhtml) | comparison target | intersection tokens: microparts vs target | competitive? |
|-------|--------------------------------:|-------------------|------------------------------------------:|--------------|
| qwen3.7-max | **45/48** (41/48) | fhtml-def | 43,987 vs 43,125 (+2.0%, n=36) | no — more tokens |
| mimo-v2.5-pro | **41/48** (34/48) | fhtml-def-plan | 33,504 vs 37,080 (−9.6%, n=28) | **yes** |
| tencent/hy3 | **40/48** (37/48) | fhtml-def | 32,899 vs 30,240 (+8.8%, n=29) | no — more tokens |

By the pinned rule (≥1 model within 10 points of plain fhtml *and* fewer intersection
tokens) the formal verdict is **competitive** — mimo satisfies both. The honest reading
is narrower and more interesting:

- **Reliability is the real result.** Micro-parts beat *plain fhtml* on DOM-validity
  for all three models (45/41/40 vs 41/34/37) — the highest rates in the whole
  generation benchmark. Models factored willingly (41–45 of 48 completions define
  parts) and executed the scheme's semantics almost flawlessly. The likely mechanism
  is unflattering to the scheme, though: the output is ~95% verbatim HTML — the syntax
  models know best — and this benchmark's `html`-minification control (the direct
  "just let them write HTML" baseline) is **still unswept**, so "the parts scheme
  helps" and "HTML is easy" cannot yet be separated. That control is now the most
  important missing number in the table.
- **Tokens favor fhtml.** Micro-parts' whole-document output lands at par with the
  plain-fhtml reference (median +0.4% / +2.8% / +11.2%) — JSON-escaping plus dedup
  roughly cancels fhtml's syntax savings — and on the intersections it emits *more*
  tokens than `fhtml-def` for the two models where fhtml-def works well.
- **Failures skew silent.** Of the 18 micro-parts failures with a graded cause, 7 were
  caught loud by the assembler (`bad-template` ×4, `unused-arg` ×2, `bad-json` ×1) and
  11 assembled cleanly into a wrong DOM — the shorthand lesson in miniature, minus the
  scale. fhtml's failures stay predominantly compile-loud.
- **Finding #0** (recorded before the sweep): making the "just JSON" idea gradeable at
  all required inventing a template micro-language — grammar, resolution rules, depth
  and cycle semantics, eleven error codes. The simplicity is the pitch, not the spec.

**Verdict: dominated as a format** — it never beats fhtml-def on tokens where
fhtml-def is healthy, its failures are quieter, and it smuggles in an unspecified
compiler — **but its reliability numbers are a finding fhtml has to answer**: staying
close to raw HTML bought 4–11 DOM-valid cases per model over plain fhtml. Whether that
premium comes from the JSON scaffold or just from HTML familiarity is exactly what the
unswept `html` control measures; run it next (`--targets html`, same three models,
144 calls).

## Class shorthand in generation (`shorthand`)

Can models *emit* the class shorthand?
Same translation task with the legend appended to the system prompt; the model writes
`#!shorthand` files. The deterministic economics are settled separately
(`bench/shorthand_economics.py`: legend costs 1,021 tokens, saves ~133/component,
break-even ≈ 8 components) — this sweep asks only whether models apply the legend
*correctly*. Run 2026-07-10.

| model | compiles | strict DOM-eq | + ws-only | DOM-valid | (plain fhtml) | forgot `#!shorthand` |
|-------|---------:|--------------:|----------:|----------:|--------------:|---------------------:|
| tencent/hy3 (reasoning) | **48/48** | 10 | 1 | 11/48 | 37/48 | 0 |
| claude-haiku-4.5 | 42/48 | 1 | 0 | 1/48 | 30/48 | 0 |
| nemotron-3-ultra | 39/48 | 3 | 2 | 5/48 | 24/48 | 2 |

**Verdict: fails, decisively and deceptively.** Compile rate is fine (pooled 89.6%,
essentially plain-fhtml's; hy3's 48/48 is the only perfect compile run in the whole
benchmark) and the directive is remembered — but DOM validity collapses to **11.8%
pooled vs 63.2%** on plain fhtml, and the failure is *silent*. 97 of the 112 DOM
failures first mismatch on a `class` attribute, in two modes:

- **Invented codes.** Despite the legend's "never guess a code", models coin codes that
  don't exist — `rxl` for `rounded-xl` (the code is `rx`), `r3x` for `rounded-3xl` (no
  code exists), `te6` for `text-emerald-600` (it's `tem6`). Bare tokens are
  classes-verbatim (SPEC §3), so an unknown code compiles cleanly into a garbage class.
  At the first mismatch alone, ≥35 of the 112 failures show a surviving unknown code —
  a lower bound, since only the first diff is recorded.
- **Confused decodes.** Near-collisions in the table get crossed: `gy`(gray) vs
  `gn`(green) produced `text-green-900` where the source has `text-gray-900`.

This is the mirror image of the components result: there, compile failures were loud
and DOM-valid output kept its compression; here, every error is invisible until
rendered. Even the reasoning model that passes the components gate manages 11/48. The
shorthand stays what the economics script already priced it as: a **deterministic
write-time compression** — tooling can apply it mechanically at zero risk (0/48
regressions when applied by script), but models must not be asked to emit it.
