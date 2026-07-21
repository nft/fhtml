# Idiomatic fhtml examples

Both examples below are adapted from files in the fhtml repo that compile and
pass its test suite (`tests/corpus/blog-cards*.fhtml`,
`integrations/vite/example/src/`).

## Flat vs component-factored

The same blog-card grid, written twice. The flat version repeats the card
shape three times; the factored version writes the shape once as a `def` and
instantiates it per post. **Factor only because the markup repeats** — the
section header around the grid appears once, so it stays plain in both.

### Flat (what to avoid when the shape repeats)

```fhtml
. mx-auto mt-16 grid max-w-2xl grid-cols-1 gap-x-8 gap-y-16 lg:grid-cols-3
  article group flex flex-col items-start
    a(href=/blog/soil-probe-v3) relative w-full overflow-hidden rounded-2xl
      img(src=/img/blog/soil-probe-field.jpg alt="") aspect-[16/9] w-full object-cover
    . mt-6 flex items-center gap-x-4 text-xs
      time(datetime=2026-06-18) text-slate-500 "Jun 18, 2026"
      a(href=/blog/category/hardware) rounded-full bg-slate-100 px-3 py-1.5 "Hardware"
    h3 mt-3 text-lg font-semibold text-slate-900 > a(href=/blog/soil-probe-v3) "Designing a soil probe that survives five winters underground"
    p mt-3 line-clamp-3 text-sm text-slate-600
      | Frost heave destroyed 30% of our v2 probes in the first season.
  article group flex flex-col items-start
    // ... the exact same 8-line shape, repeated for every post ...
```

### Factored (idiomatic)

```fhtml
def post(href img datetime date cathref cat title)
  article group flex flex-col items-start
    a(href={href}) relative w-full overflow-hidden rounded-2xl
      img(src={img} alt="") aspect-[16/9] w-full object-cover
    . mt-6 flex items-center gap-x-4 text-xs
      time(datetime={datetime}) text-slate-500 "{date}"
      a(href={cathref}) rounded-full bg-slate-100 px-3 py-1.5 "{cat}"
    h3 mt-3 text-lg font-semibold text-slate-900 > a(href={href}) "{title}"
    p mt-3 line-clamp-3 text-sm text-slate-600
      children

. mx-auto mt-16 grid max-w-2xl grid-cols-1 gap-x-8 gap-y-16 lg:grid-cols-3
  +post(href="/blog/soil-probe-v3" img="/img/blog/soil-probe-field.jpg" datetime="2026-06-18" date="Jun 18, 2026" cathref="/blog/category/hardware" cat="Hardware" title="Designing a soil probe that survives five winters underground")
    | Frost heave destroyed 30% of our v2 probes in the first season.
  +post(href="/blog/lora-mesh-lessons" img="/img/blog/lora-gateway.jpg" datetime="2026-05-30" date="May 30, 2026" cathref="/blog/category/engineering" cat="Engineering" title="What 40,000 LoRa nodes taught us about mesh networking")
    | Textbook mesh routing collapses when half your nodes sleep 99% of the time.
```

What to notice:

- **Every difference between the repeats became a parameter** — href, image,
  datetime, display date, category link and label, title. Nothing was
  flattened away.
- **The longest varying content (the excerpt) is the `children` block**, a
  `|` line under each call — not a parameter.
- **Every string argument is double-quoted, including URLs** —
  `href="/blog/…"` in the call, even though the flat version writes
  `a(href=/blog/…)` unquoted on the tag.
- The def name and parameters use underscores/plain identifiers, no hyphens.

## Multi-file layout with `include`

A page splits shared components into partial files. `include` splices the
file: its `def`s join the namespace, its top-level markup (if any) emits at
the include site.

`src/partials/badge.fhtml` — a def-only component partial:

```fhtml
def badge(label)
  span inline-block mt-3 px-2 py-0.5 text-xs rounded-full bg-emerald-100 text-emerald-700 "{label}"
```

`src/card.fhtml` — a page fragment using it:

```fhtml
include ./partials/badge
article max-w-xl mx-auto mt-4 px-4 py-4 bg-white rounded-xl shadow-sm
  h2 text-lg font-semibold "{title}"
  p mt-1 text-gray-600 "{body}"
  +badge(label="live")
```

A full page follows the same pattern at larger scale — layout defs (head,
nav, footer) in `partials/layout.fhtml`, included by every page:

```fhtml
include ./partials/layout
include ./partials/badge

doctype
html(lang=en)
  +head(title="Dashboard")
  body min-h-screen bg-slate-50
    +nav(active="/dashboard")
    main mx-auto max-w-7xl px-6 py-12
      h1 text-2xl font-semibold "Dashboard"
      +badge(label="beta")
    +footer()
    script(src=/js/menu.js defer)
```

What to notice:

- Behavior is a **separate `.js` file** referenced with
  `script(src=/js/menu.js defer)` — no inline script body.
- Include paths are relative to the including file.
- One component family per partial; pages stay short and scannable.
