# Task: factor the repeated card into a component

`index.fhtml` is a blog-cards grid written in fhtml (the `fhtml` compiler is
on PATH; `fhtml index.fhtml` compiles it). The same article-card markup is
written out three times, differing only in values.

Refactor it: define the card **once** as a `def` component and instantiate
it for each post with `+name(…)` calls. The rendered output must not change
— every link, image, date, category, title, excerpt, author, and role must
survive exactly. Put each post's excerpt in the component's `children`
block.

The file must still compile cleanly with `fhtml index.fhtml
--deny-warnings`. You can verify your refactor preserved the output by
comparing `fhtml index.fhtml` before and after.
