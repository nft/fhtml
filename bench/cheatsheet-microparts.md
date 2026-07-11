# Micro-parts JSON

You emit ONE JSON object with exactly two keys:

- `"body"` (required): an HTML string — the whole page, top to bottom.
- `"parts"` (optional): an object whose values are HTML strings — one entry
  per markup shape that repeats.

Inside any of these strings, `{{name}}` and `{{name key="value" …}}` are the
only special forms; everything else is verbatim HTML.

```json
{
  "parts": {
    "feature": "<li class=\"rounded-xl bg-white p-6 shadow\"><h3 class=\"text-lg font-semibold\">{{title}}</h3><p class=\"mt-2 text-sm text-gray-600\">{{body}}</p></li>"
  },
  "body": "<ul class=\"grid grid-cols-3 gap-6\">{{feature title=\"Fast\" body=\"Ships in milliseconds.\"}}{{feature title=\"Safe\" body=\"Every change is previewed.\"}}</ul>"
}
```

- **In `body`, `{{name …}}` calls a part.** Inside a part, a bare `{{name}}`
  that is not a part name is a **slot**, filled by the call's `name="…"`
  argument. Every call must bind every slot of the part it calls — no more,
  no less.
- **Names use underscores, never hyphens**: lowercase letters, digits, `_`,
  starting with a letter or `_`. `blog-post` is an ERROR — write `blog_post`.
- **Argument values are double-quoted.** Single quotes are an ERROR. Inside a
  value the only escapes are `\"` (a literal quote) and `\\` (a literal
  backslash). Whitespace is allowed inside the braces and around `=`.
- **Escaping happens at two layers — this is the one trap.** The strings live
  in JSON, so every HTML attribute quote is written `\"` in the file:
  `<div class=\"p-6\">`. A literal quote inside an *argument value* needs
  both layers: the template wants `\"`, and JSON escapes each of those two
  characters, so the file shows `\\\"`. Example, calling
  `{{quote text="He said \"hi\""}}` — in the JSON file that argument is
  written `text=\"He said \\\"hi\\\"\"`.
- **Values are spliced verbatim** — never HTML-escaped, never re-scanned for
  `{{…}}`. So do NOT pass a value containing a quote into a slot that sits
  inside a quoted attribute (`title="{{msg}}"`) — the quote ends the
  attribute early and corrupts the markup. Keep such text in element content,
  or escape nothing and split the part differently.
- Parts may call other parts (nesting depth up to 8, no cycles). Entities
  (`&amp;`, `&#8594;`) are copied through exactly as written in the input.
- **Factor a part only for true repeats**: same markup shape, only text or
  attribute values differing. Give every difference between the instances its
  own slot — ids, `aria-label`s, a `checked` flag, the selected item's extra
  classes; a difference you flatten away corrupts the output. If instances
  differ *structurally* (an extra badge, a different wrapper), leave them
  inline in `body` — never wrap single-use markup in a part.
- Write the HTML minified: no indentation, no newlines between tags.
