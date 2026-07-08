# fhtml components

When the same markup shape repeats with only the text or attribute values
changing, factor it once with `def` and instantiate it with `+name(…)`:

```
def feature_card(title href badge_text=null)
  li rounded-xl bg-white p-6 shadow
    h3 text-lg font-semibold > a(href={href}) "{title}"
    if badge_text
      span rounded-full bg-indigo-50 px-2 text-xs "{badge_text}"
    p mt-2 text-sm text-gray-600
      children

ul grid grid-cols-3 gap-6
  +feature_card(title="Fast" href="/fast" badge_text="New")
    | Ships in milliseconds.
  +feature_card(title="Safe" href="/safe")
    | Every change is previewed.
```

- **Names use underscores, never hyphens.** Component and parameter names are
  expression identifiers (letters, digits, `_`); `-` is minus. `def
  blog-post(img-src)` is an ERROR — write `def blog_post(img_src)`.
- **`def name(param param=default)`** — top level only. The body sees ONLY its
  parameters (interpolate them: `{title}`); it cannot see other variables.
  Defaults follow call-argument quoting: `variant="emerald"`, not
  `variant=emerald` (that's a variable reference, which is null).
- **`+name(args)` instantiates.** Arguments are named-only. **Quoting differs
  from tag attributes — this is the one trap:** in a call, an unquoted value
  is an *expression*, not a string. `n=3` is the number 3, `wide=false` is a
  boolean, but `title=Fast` and `href=/fast` are ERRORS. **Every string
  argument must be double-quoted**, including URLs: `href="/fast"` (even
  though `a(href=/fast)` on a plain tag is fine).
  - A bare argument name means `true`: `+card(compact)`.
- **`children`** in the body marks where the caller's indented block goes.
  Put the longest varying content (a sentence, a paragraph) in the block
  instead of a parameter. A block is only allowed if the def uses `children`.
- A parameter without a default is required at every call.
- **Parameterize EVERY difference between the repeats** — ids, `aria-label`s,
  a `checked` flag, the selected item's extra classes. Compare the instances
  token by token; a difference you flatten away corrupts the output. If the
  blocks differ in structure, leave them plain.
- Byte-identical repeated `<svg>` icons factor well: put the raw `<svg …>`
  lines inside a def's body. Identical ONLY — interpolation does not run
  inside raw `<` lines, so a `{path_d}` there stays literal text.
- A text-only child line starts with `|`. A bare quoted line is an ERROR —
  quotes only attach text to an element's own line.
- Markup that does **not** repeat stays plain — never wrap single-use markup
  in a def; that costs tokens instead of saving them.
