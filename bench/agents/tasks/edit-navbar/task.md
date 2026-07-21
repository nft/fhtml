# Task: extend the navbar

`index.fhtml` is a marketing site navbar written in fhtml (the `fhtml`
compiler is on PATH; `fhtml index.fhtml` compiles it). It has a desktop link
row and a mobile menu, both listing: Product, Solutions, Pricing, Customers,
Docs. Make two changes:

1. Add a **Changelog** link (`href=/changelog`) after **Docs**, with exactly
   the same styling as the neighbouring links — in **both** the desktop nav
   and the mobile menu.
2. The two "Start free" call-to-action links should read **"Start for
   free"** instead (both of them).

Change nothing else. The file must still compile cleanly with
`fhtml index.fhtml --deny-warnings`.
