# Task: add a fourth blog card

`index.fhtml` is a blog-cards grid written in fhtml (the `fhtml` compiler is
on PATH; `fhtml index.fhtml` compiles it). It currently shows three article
cards. Add a **fourth card** after the third, with exactly the same
structure and styling as the existing three, using this content:

- link: `/blog/edge-caching`
- image: `/img/blog/edge-cache.jpg`
- date: `Apr 22, 2026` (datetime attribute `2026-04-22`)
- category: `Engineering`, linking to `/blog/category/engineering`
- title: `Caching sensor data at the edge without lying to farmers`
- excerpt: `Stale readings are worse than no readings. The cache design
  that keeps dashboards fast on rural connections while never showing
  yesterday's soil moisture as today's.`
- avatar: `/img/team/marcus.jpg`
- author: `Marcus Webb`, role `Platform Engineer`

Change nothing else. The file must still compile cleanly with
`fhtml index.fhtml --deny-warnings`.
