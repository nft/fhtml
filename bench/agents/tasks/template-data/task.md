# Task: data-driven team section

Create `team.fhtml` — a team section written in fhtml that renders from the
JSON in `data.json` (the `fhtml` compiler is on PATH; render with
`fhtml team.fhtml --data data.json`).

Requirements:

- An `h2` showing `{heading}` from the data.
- One `li` per entry in `team`, rendered with a `for` loop — showing the
  member's avatar (`img`), name, and role. Do **not** hardcode any member's
  name or role in the markup; the section must render correctly for any
  team list the data file provides.
- When `team` is empty, render a single `li` reading exactly:
  `No team members yet.` (use the loop's `empty` branch).
- Style with Tailwind utility classes.

The file must compile cleanly with
`fhtml team.fhtml --data data.json --deny-warnings`.
