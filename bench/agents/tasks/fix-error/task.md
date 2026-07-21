# Task: fix the broken pricing page

`index.fhtml` is a pricing section written in fhtml, but it does not
compile. The `fhtml` compiler is on PATH; running it currently reports:

```
index.fhtml:32:293: error: unclosed attribute list — missing `)`
```

There is **more than one** problem in the file — fix an error, re-run
`fhtml index.fhtml`, and repeat until it compiles. Every fix must restore
the clearly intended markup (do not delete content or restructure the page
to silence an error).

Done when `fhtml index.fhtml --deny-warnings` exits cleanly.
