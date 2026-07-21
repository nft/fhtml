# Task: pricing section

Create `index.fhtml` — a pricing section written in fhtml (the language of
the `.fhtml` files; the `fhtml` compiler is on PATH: `fhtml index.fhtml`
compiles it, and it must compile cleanly with `fhtml index.fhtml
--deny-warnings`).

Requirements:

- Three pricing tiers, in this order: **Starter** ($19/mo), **Growth**
  ($49/mo), **Enterprise** (Custom pricing).
- Each tier is a card with the tier name, the price, a feature list (`ul`
  with at least 3 items), and a call-to-action link (`a` with
  `href=/signup`).
- The **Growth** tier is visually highlighted: its card carries the classes
  `ring-2 ring-indigo-600`.
- Style with Tailwind utility classes; a clean, modern SaaS look.
- The three cards share the same structure — factor the repeated markup
  instead of writing it three times.

Do not add other pages or build tooling. When the file compiles cleanly you
are done.
