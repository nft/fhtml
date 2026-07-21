# Task: split a monolithic page into components

`index.fhtml` is a complete marketing page written in fhtml (the `fhtml`
compiler is on PATH; `fhtml index.fhtml` compiles it): header, hero, an
install section, and a footer, all in one file.

Split it into separate component files. In fhtml, `include ./partials/x`
splices another file at top level — its `def`s join the namespace — and a
`+name(…)` call instantiates a `def` where you need it. The idiomatic split
is def-only partial files:

- `partials/header.fhtml` — a def holding the site header
- `partials/install.fhtml` — a def holding the install section (the code
  block and the plans table)
- `partials/footer.fhtml` — a def holding the footer

`index.fhtml` keeps the document shell and the hero, includes the partials
at the top of the file, and instantiates each component where its markup
used to be.

The rendered output must not change — compare `fhtml index.fhtml` before
and after. It must still compile cleanly with `fhtml index.fhtml
--deny-warnings`.
