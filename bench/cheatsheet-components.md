# fhtml components

When the same markup shape repeats with only the text or attribute values
changing, factor it once with `def` and instantiate it with `+name(…)`:

```
def card(title href badge=null)
  li rounded-xl bg-white p-6 shadow
    h3 text-lg font-semibold > a(href={href}) "{title}"
    if badge
      span rounded-full bg-indigo-50 px-2 text-xs "{badge}"
    p mt-2 text-sm text-gray-600
      children

ul grid grid-cols-3 gap-6
  +card(title="Fast" href="/fast" badge="New")
    | Ships in milliseconds.
  +card(title="Safe" href="/safe")
    | Every change is previewed.
```

- **`def name(param param=default)`** — top level only. The body sees ONLY its
  parameters (interpolate them: `{title}`); it cannot see other variables.
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
- Repeated inline `<svg>` icons factor well: put the raw `<svg …>` lines
  inside a def's body and call it wherever the icon repeats.
- Markup that does **not** repeat stays plain — never wrap single-use markup
  in a def; that costs tokens instead of saving them.
