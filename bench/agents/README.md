# Agent benchmark: local coding agents writing fhtml

The API benchmark ([../README.md](../README.md)) measures single-turn
translation. This harness measures the thing users actually experience:
**a CLI coding agent, in a workspace, doing fhtml tasks end-to-end** —
free-form generation, exact edits, refactors, debugging — and whether
installing the [fhtml skill](../../skills/) measurably helps.

```sh
cargo build --release --features convert
python3 bench/agents/run.py --agents null,noop           # validate the graders
python3 bench/agents/run.py --agents claude --tasks fix-error --limit 1
python3 bench/agents/run.py --agents claude,codex,gemini # full sweep (paid!)
```

## Protocol

A **cell** is agent × task × arm × rep. Per cell the harness:

1. creates a fresh git workspace under `bench/out/agents/work/`, seeds it
   from `tasks/<id>/seed/` plus the task's `task.md`;
2. in the **skill** arm, installs `skills/fhtml/AGENTS.md` under the agent's
   convention (`CLAUDE.md` / `AGENTS.md` / `GEMINI.md`); the **bare** arm
   gets only `task.md`. The prompt is identical everywhere: *"Read task.md in
   this directory and complete the task."* — any uplift is attributable to
   the context file alone (the skill file's sha256 is recorded per cell);
3. runs the agent CLI headlessly with the fhtml binaries on PATH (agents can
   self-check), 600 s timeout, whole-process-group kill;
4. grades the workspace against `tasks/<id>/checks.json` — grading
   intermediates go to `bench/out/agents/grade/`, never into the workspace;
5. appends to `bench/out/agents/results.json` (rewritten after every cell;
   interrupted sweeps resume — done cells are skipped unless `--force`).

Marks: `✓` full pass · `~` properties partial · `c` compiles but
DOM/property fail · `✗` did not compile · `t` timeout.

**Correctness** = compile (`--deny-warnings`) + DOM-equivalence against a
committed golden (`html2fhtml --dom-eq`) where one exists + per-task property
checks (tag counts, texts, attributes, held-out `--data` renders).
**Quality** is scored separately, never blended: no inline JS, `fhtml fmt`
produces no diff, low structural repetition, no dead defs, expected file
structure.

## Tasks

| task | type | key grading |
|---|---|---|
| `gen-pricing` | free-form generation | compile + properties (tiers, highlight, CTAs); quality wants a `def` |
| `gen-landing` | free-form generation | compile + properties (nav/hero/features/footer, external `js/menu.js`) |
| `edit-add-card` | exact edit | DOM-eq vs golden |
| `edit-navbar` | exact edit | DOM-eq vs golden |
| `refactor-def` | refactor to component | DOM-eq vs the seed's own output + `def` used ≥3× |
| `split-components` | split into `include` partials | DOM-eq + `fhtml deps` ≥3 files |
| `template-data` | `--data` templating | properties + held-out `data.alt.json` / `data.empty.json` renders |
| `fix-error` | debugging (3 seeded errors) | compile + DOM-eq vs the pre-breakage golden |

Task fixtures (`task.md`, `seed/`, `golden/`, `golden-workspace/`,
`checks.json`) are **committed and frozen** — comparability across sweeps
dies if they drift. `golden/` regenerates from `golden-workspace/` via
`regen_goldens.py` (only when a task deliberately changes; eyeball the diff).

## Validating the graders

Two network-free pseudo-agents pin the harness itself: **null** copies each
task's committed `golden-workspace/` into place and must grade `✓` with
quality 1.00 everywhere; **noop** touches nothing and must fail correctness
everywhere. Run them after any grader or task change.

## Agents

| agent | invocation | cost/tokens |
|---|---|---|
| claude | `claude -p … --dangerously-skip-permissions --output-format json` | first-class (cost, tokens, turns) |
| codex | `codex exec … --dangerously-bypass-approvals-and-sandbox --json` | tokens best-effort from the event stream |
| gemini | `gemini -p … --yolo --output-format json` | tokens best-effort from the stats block |
| antigravity | manual protocol (below) | wall-clock only |

Flags match claude 2.1.x / codex-cli 0.144.x / gemini-cli 0.50.x and move
fast — re-verify after upgrading any of them. The bypass/yolo flags run the
agent without approval prompts **inside the throwaway workspace**; don't
point the harness at anything you care about.

**Antigravity** is an agentic IDE with no headless CLI, so it runs
first-class but manually: `run.py --agents antigravity` prepares each
workspace identically (including the skill arm's `AGENTS.md`), prints the
path and the standard prompt, and waits for Enter while a human drives the
IDE against it; grading is identical. Its records carry `"manual": true`,
wall-clock only — excluded from cost comparisons.

## Known confounds (documented, not solved)

- **Global agent config leaks in**: `~/.claude` (memory, user CLAUDE.md),
  `~/.codex/config.toml`, `~/.gemini` all apply to harness runs. Config-dir
  env overrides tend to break auth, so instead: run sweeps with minimal
  global context, and compare arms only within the same machine/setup.
  `agent_version` and `skill_sha` are recorded per cell so mismatched sweeps
  are at least detectable.
- **Models differ across agents** by default; `--model X` pins one per
  sweep, recorded per cell. Cross-agent comparisons are agent+model bundles,
  not model comparisons.
- **One rep is noisy.** `--reps 3` gives paired per-task marks; the report
  shows all reps' marks per cell.

## Results

`run.py` regenerates [RESULTS.md](RESULTS.md) (per-agent mark tables and
bare→skill uplift metrics) after every sweep. Raw agent transcripts land in
`bench/out/agents/logs/`, workspaces stay under `bench/out/agents/work/`
for post-hoc inspection.
