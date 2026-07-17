# fhtml + Vite + Tailwind — example

A minimal hot-reloading page: a `?html` hero, a templated card component
rendered from data, and an `include`d partial, styled by Tailwind v4.

## Run it

1. Install the fhtml compiler (once):

   ```sh
   cargo install --path ../../..     # from this directory; or any clone of the fhtml repo
   ```

   Any way that puts `fhtml` on `$PATH` works — or set `FHTML_BIN` to the
   binary, or pass `fhtml({ bin: "…" })` in `vite.config.js`.

2. Install and start:

   ```sh
   npm install
   npm run dev
   ```

Then try it: edit `src/partials/badge.fhtml` — every card on the page
hot-reloads (the include graph is watched). Break the syntax in
`src/card.fhtml` — the overlay points at the `.fhtml` line. Classes are
plain Tailwind utilities; the `@source "./**/*.fhtml"` line in
`src/style.css` is all Tailwind needs.

`npm run build` produces the same page statically in `dist/`.
