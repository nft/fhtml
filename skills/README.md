# fhtml agent skill

`fhtml/` is a distributable skill that teaches a coding agent to write clean,
idiomatic fhtml: the write → `fmt` → build loop, partials and `include`
structure, `def` hygiene, and no inline JavaScript. The syntax reference
inside it (`fhtml/references/language.md`) is a generated copy of the
repo-root [`llms.md`](../llms.md) — the single source of truth.

## Install

| Agent | How |
|---|---|
| Claude Code | `cp -r skills/fhtml ~/.claude/skills/fhtml` (or into a project's `.claude/skills/`) |
| Codex CLI | append `skills/fhtml/AGENTS.md` to the project's `AGENTS.md` |
| Gemini CLI | append `skills/fhtml/AGENTS.md` to the project's `GEMINI.md` |
| Antigravity | add `skills/fhtml/AGENTS.md` to the workspace rules / `AGENTS.md` |
| Anything else | `skills/fhtml/AGENTS.md` is self-contained plain markdown (practices + full syntax); it is also served at [fhtml.dev skill.md](https://nft.github.io/fhtml/skill.md) |

`AGENTS.md` is the degraded single-file form — the SKILL.md practices with the
complete language reference appended — so every non-Claude agent consumes
identical content.

## Layout

```
fhtml/
  SKILL.md                 # practices + workflow (hand-written)
  references/
    language.md            # generated copy of llms.md — do not edit
    examples.md            # annotated idiomatic examples (hand-written)
  AGENTS.md                # generated single-file form — do not edit
```

## Maintenance

Edit `SKILL.md`, `references/examples.md`, or the repo-root `llms.md`, then:

```sh
skills/build.sh          # regenerate language.md + AGENTS.md, mirror to .claude/skills/
skills/build.sh --check  # CI: fail if committed copies drifted
```
