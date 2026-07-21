#!/usr/bin/env python3
"""Local coding-agent benchmark: how do CLI agents handle fhtml end-to-end?

    cargo build --release --features convert
    python3 bench/agents/run.py --agents null,noop            # validate graders
    python3 bench/agents/run.py --agents claude --tasks fix-error --limit 1
    python3 bench/agents/run.py --agents claude,codex,gemini  # full sweep

Cells are agent × task × arm × rep. Arms: `bare` (task.md only) and `skill`
(the fhtml skill installed as the agent's context file). Graded cells land in
bench/out/agents/results.json, rewritten after every cell — an interrupted
sweep resumes by default (already-graded cells are skipped; --force redoes
them). See README.md for the protocol and known confounds.
"""

import argparse
import datetime
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import drivers
import grade as grader
import report
import workspace

OUT = workspace.OUT
ALL_ARMS = ["bare", "skill"]


def all_tasks():
    return sorted(d for d in os.listdir(workspace.TASKS)
                  if os.path.isdir(os.path.join(workspace.TASKS, d)))


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        h.update(fh.read())
    return h.hexdigest()[:16]


def load_results(path):
    merged = {}
    if os.path.exists(path):
        with open(path) as fh:
            for r in json.load(fh):
                merged[(r["agent"], r["task"], r["arm"], r["rep"])] = r
    return merged


def save_results(path, merged):
    tmp = path + ".tmp"
    with open(tmp, "w") as fh:
        json.dump(sorted(merged.values(), key=lambda r: (
            r["agent"], r["task"], r["arm"], r["rep"])), fh, indent=2)
    os.replace(tmp, path)


MARKS = {"pass": "✓", "prop-partial": "~", "prop-fail": "c",
         "dom-fail": "c", "compile-fail": "✗", "timeout": "t"}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--agents", default="null",
                    help=f"comma list of {sorted(drivers.DRIVERS)}")
    ap.add_argument("--tasks", default="", help="comma list (default: all)")
    ap.add_argument("--arms", default="bare,skill")
    ap.add_argument("--reps", type=int, default=1)
    ap.add_argument("--limit", type=int, default=0,
                    help="stop after N newly graded cells")
    ap.add_argument("--timeout", type=int, default=600,
                    help="seconds per agent run")
    ap.add_argument("--model", default=None,
                    help="model override passed to every agent CLI")
    ap.add_argument("--skill-file", default=workspace.DEFAULT_SKILL,
                    help="context file the skill arm installs")
    ap.add_argument("--force", action="store_true",
                    help="re-run cells already in results.json")
    args = ap.parse_args()

    agents = [a.strip() for a in args.agents.split(",") if a.strip()]
    for a in agents:
        if a not in drivers.DRIVERS:
            sys.exit(f"unknown agent {a!r} (have {sorted(drivers.DRIVERS)})")
    tasks = ([t.strip() for t in args.tasks.split(",") if t.strip()]
             or all_tasks())
    for t in tasks:
        if not os.path.isdir(os.path.join(workspace.TASKS, t)):
            sys.exit(f"unknown task {t!r} (have {all_tasks()})")
    arms = [a.strip() for a in args.arms.split(",") if a.strip()]

    skill_sha = sha256(args.skill_file)
    res_path = os.path.join(OUT, "results.json")
    os.makedirs(OUT, exist_ok=True)
    merged = load_results(res_path)
    versions = {a: drivers.agent_version(a) for a in agents}
    done = 0

    cells = [(agent, task, arm, rep)
             for agent in agents for task in tasks for arm in arms
             for rep in range(1, args.reps + 1)]
    for key in cells:
        agent, task, arm, rep = key
        if key in merged and not args.force:
            continue
        if args.limit and done >= args.limit:
            break

        ws = workspace.make_workspace(agent, task, arm, rep,
                                      args.skill_file)
        log = os.path.join(OUT, "logs", agent, f"{task}-{arm}-rep{rep}.log")
        driver = drivers.DRIVERS[agent]
        if agent in drivers.PSEUDO_AGENTS:
            run = driver(ws, args.timeout, log, task=task)
        else:
            run = driver(ws, args.timeout, log, model=args.model)

        grade_dir = os.path.join(OUT, "grade", agent, task, arm, f"rep{rep}")
        g = grader.grade(ws, task, grade_dir)
        if run["timed_out"]:
            g["status"] = "timeout"

        rec = {
            "agent": agent, "agent_version": versions[agent],
            "model": args.model, "task": task, "arm": arm,
            "rep": rep, "skill_sha": skill_sha,
            "manual": agent in drivers.MANUAL_AGENTS,
            "changed_files": workspace.changed_files(ws),
            "date": datetime.date.today().isoformat(),
            "log": os.path.relpath(run["raw_log"], OUT)
                   if run["raw_log"] else None,
            **g,
            **{k: run[k] for k in ("duration_s", "cost_usd", "tokens_in",
                                   "tokens_out", "turns", "timed_out",
                                   "exit_code")},
        }
        merged[key] = rec
        save_results(res_path, merged)
        done += 1
        print(f"{MARKS[g['status']]} {agent:12s} {task:18s} "
              f"{arm:5s} rep{rep}  q={g['quality']['score']:.2f} "
              f"{rec['duration_s']:.0f}s")

    report.write(res_path, os.path.join(os.path.dirname(
        os.path.abspath(__file__)), "RESULTS.md"))
    print(f"\n{done} cells graded → {res_path}")
    print("report → bench/agents/RESULTS.md")


if __name__ == "__main__":
    main()
