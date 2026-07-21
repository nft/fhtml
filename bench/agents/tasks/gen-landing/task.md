# Task: landing page

Create `index.fhtml` — a complete landing page written in fhtml (the `fhtml`
compiler is on PATH; the page must compile cleanly with `fhtml index.fhtml
--deny-warnings`).

Requirements:

- A full HTML document: doctype, `html`, `head` (charset, viewport, title),
  `body`.
- A `nav` bar with the brand name and links: Home (`/`), Features
  (`/features`), Pricing (`/pricing`).
- A hero section whose `h1` reads exactly: `Ship faster with fhtml`, a
  supporting paragraph, and a call-to-action link to `/signup`.
- A features section with three `article` cards titled exactly: `Zero
  config`, `Token cheap`, `Tailwind native` — each with a short description.
- A `footer` with a copyright line.
- The nav needs a mobile menu toggle. Its JavaScript must live in a
  **separate file** `js/menu.js`, referenced from the page with
  `script(src=/js/menu.js defer)` — do not write any inline script.
- Style with Tailwind utility classes.

When the page compiles cleanly you are done.
