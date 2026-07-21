"""Workspace lifecycle for the agent benchmark: seed, git init, skill install.

A cell's workspace is a throwaway git repo under bench/out/agents/work/ that
the agent CLI runs inside. Grading intermediates never land here (they go to
a sibling grade/ dir) so a finished workspace stays inspectable as the agent
left it.
"""

import os
import shutil
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "bench", "out", "agents")
TASKS = os.path.join(HERE, "tasks")

# Where each agent expects project instructions (the skill arm's install site).
SKILL_DEST = {
    "claude": "CLAUDE.md",
    "codex": "AGENTS.md",
    "gemini": "GEMINI.md",
    "antigravity": "AGENTS.md",
    # harness-validation pseudo-agents read nothing, but installing keeps the
    # two arms' workspaces byte-comparable
    "null": "AGENTS.md",
    "noop": "AGENTS.md",
}

DEFAULT_SKILL = os.path.join(ROOT, "skills", "fhtml", "AGENTS.md")


def _git(ws, *args):
    subprocess.run(["git", *args], cwd=ws, check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def make_workspace(agent, task, arm, rep, skill_file=None):
    """Create and seed the workspace for one cell; returns its path."""
    ws = os.path.join(OUT, "work", agent, task, arm, f"rep{rep}")
    if os.path.exists(ws):
        shutil.rmtree(ws)
    os.makedirs(ws)

    seed = os.path.join(TASKS, task, "seed")
    if os.path.isdir(seed):
        shutil.copytree(seed, ws, dirs_exist_ok=True)
    shutil.copy(os.path.join(TASKS, task, "task.md"),
                os.path.join(ws, "task.md"))

    if arm == "skill":
        src = skill_file or DEFAULT_SKILL
        shutil.copy(src, os.path.join(ws, SKILL_DEST[agent]))

    # A repo makes `git status --porcelain` a free change log and satisfies
    # agent CLIs that refuse to run outside version control.
    _git(ws, "init", "-q")
    _git(ws, "add", "-A")
    _git(ws, "-c", "user.email=bench@fhtml", "-c", "user.name=bench",
         "commit", "-q", "-m", "seed", "--allow-empty")
    return ws


def changed_files(ws):
    """Paths the agent added or modified, from git's point of view."""
    p = subprocess.run(["git", "status", "--porcelain"], cwd=ws,
                       capture_output=True, text=True)
    return [ln[3:].strip() for ln in p.stdout.splitlines() if ln.strip()]
