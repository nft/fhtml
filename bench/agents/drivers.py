"""Agent drivers: launch a CLI agent headlessly in a workspace and report
what can be measured about the run.

Every driver returns:

    {"exit_code": int, "timed_out": bool, "duration_s": float,
     "cost_usd": float|None, "tokens_in": int|None, "tokens_out": int|None,
     "turns": int|None, "raw_log": str}

Cost fields are best-effort: claude's JSON result carries them first-class,
codex/gemini expose token counts in their stream output (parsed defensively),
null/noop/antigravity have none. CLI flags below match claude 2.1.x,
codex-cli 0.144.x, gemini-cli 0.50.x — these move fast; re-verify on upgrade.
"""

import json
import os
import shutil
import signal
import subprocess
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))

PROMPT = ("Read task.md in this directory and complete the task. "
          "Do not ask questions; when done, stop.")


def _result(exit_code=0, timed_out=False, duration_s=0.0, cost_usd=None,
            tokens_in=None, tokens_out=None, turns=None, raw_log=""):
    return {"exit_code": exit_code, "timed_out": timed_out,
            "duration_s": round(duration_s, 1), "cost_usd": cost_usd,
            "tokens_in": tokens_in, "tokens_out": tokens_out,
            "turns": turns, "raw_log": raw_log}


def _exec(cmd, ws, timeout, log_path):
    """Run cmd in ws with the fhtml binaries on PATH; kill the whole process
    group on timeout so agent-spawned children die too."""
    env = dict(os.environ)
    env["PATH"] = os.path.join(ROOT, "target", "release") + os.pathsep + env["PATH"]
    start = time.monotonic()
    timed_out = False
    proc = subprocess.Popen(cmd, cwd=ws, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            start_new_session=True)
    try:
        out, errout = proc.communicate(timeout=timeout)
        code = proc.returncode
    except subprocess.TimeoutExpired:
        timed_out, code = True, -1
        # the agent runs in its own session: kill the whole group so any
        # children it spawned (editors, node, cargo) die with it
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        out, errout = proc.communicate()
    duration = time.monotonic() - start
    os.makedirs(os.path.dirname(log_path), exist_ok=True)
    with open(log_path, "w") as fh:
        fh.write(out)
        if errout:
            fh.write("\n--- stderr ---\n" + errout)
    return code, out, timed_out, duration


# ------------------------------------------------------------- real agents

def run_claude(ws, timeout, log_path, model=None):
    cmd = ["claude", "-p", PROMPT, "--dangerously-skip-permissions",
           "--output-format", "json"]
    if model:
        cmd += ["--model", model]
    code, out, timed_out, dur = _exec(cmd, ws, timeout, log_path)
    cost = tin = tout = turns = None
    try:
        r = json.loads(out)
        if isinstance(r, list):  # some versions emit the message stream as an array
            r = next((m for m in reversed(r) if isinstance(m, dict)
                      and "total_cost_usd" in m), {})
        cost = r.get("total_cost_usd")
        turns = r.get("num_turns")
        usage = r.get("usage") or {}
        tin = usage.get("input_tokens")
        tout = usage.get("output_tokens")
    except (json.JSONDecodeError, AttributeError):
        pass
    return _result(code, timed_out, dur, cost, tin, tout, turns, log_path)


def run_codex(ws, timeout, log_path, model=None):
    cmd = ["codex", "exec", PROMPT,
           "--dangerously-bypass-approvals-and-sandbox", "--json"]
    if model:
        cmd += ["-m", model]
    code, out, timed_out, dur = _exec(cmd, ws, timeout, log_path)
    tin = tout = None
    # JSONL event stream; token totals appear on token_count-ish events whose
    # shape has changed across versions — take the last one that parses.
    for ln in out.splitlines():
        try:
            ev = json.loads(ln)
        except json.JSONDecodeError:
            continue
        info = ev.get("info") or ev.get("msg") or ev
        usage = (info.get("total_token_usage") or info.get("usage")
                 or {}) if isinstance(info, dict) else {}
        if usage.get("input_tokens") is not None:
            tin = usage.get("input_tokens")
            tout = usage.get("output_tokens")
    return _result(code, timed_out, dur, None, tin, tout, None, log_path)


def run_gemini(ws, timeout, log_path, model=None):
    cmd = ["gemini", "-p", PROMPT, "--yolo", "--output-format", "json"]
    if model:
        cmd += ["-m", model]
    code, out, timed_out, dur = _exec(cmd, ws, timeout, log_path)
    tin = tout = turns = None
    try:
        r = json.loads(out)
        stats = r.get("stats") or {}
        models = stats.get("models") or {}
        for m in models.values():
            tok = m.get("tokens") or {}
            tin = (tin or 0) + (tok.get("prompt") or 0)
            tout = (tout or 0) + (tok.get("candidates") or 0)
        turns = (stats.get("tools") or {}).get("totalCalls")
    except (json.JSONDecodeError, AttributeError):
        pass
    return _result(code, timed_out, dur, None, tin, tout, turns, log_path)


# --------------------------------------------- harness-validation pseudo-agents

def run_null(ws, timeout, log_path, model=None, task=None):
    """Copies the committed known-good solution into place. Must grade 100%
    on every task — it validates the graders, not any agent."""
    golden_ws = os.path.join(HERE, "tasks", task, "golden-workspace")
    start = time.monotonic()
    shutil.copytree(golden_ws, ws, dirs_exist_ok=True)
    return _result(duration_s=time.monotonic() - start, raw_log=log_path)


def run_noop(ws, timeout, log_path, model=None, task=None):
    """Touches nothing. Must fail correctness on every task."""
    return _result(raw_log=log_path)


# -------------------------------------------------------------------- manual

def run_manual(ws, timeout, log_path, model=None, agent="antigravity"):
    """Manual protocol for IDE-only agents (Antigravity has no headless CLI):
    the harness prepares the workspace, a human drives the IDE against it,
    and grading proceeds identically. Wall-clock only, no cost fields."""
    print(f"\n=== manual run: {agent} ===")
    print(f"workspace: {ws}")
    print(f"prompt:    {PROMPT}")
    print("Open the workspace in the IDE, run the agent, then press Enter "
          "here when it has finished.")
    start = time.monotonic()
    input()
    dur = time.monotonic() - start
    with open(log_path, "w") as fh:
        fh.write(f"manual run, {dur:.0f}s\n")
    return _result(duration_s=dur, raw_log=log_path)


DRIVERS = {
    "claude": run_claude,
    "codex": run_codex,
    "gemini": run_gemini,
    "null": run_null,
    "noop": run_noop,
    "antigravity": run_manual,
}

MANUAL_AGENTS = {"antigravity"}
PSEUDO_AGENTS = {"null", "noop"}


def agent_version(agent):
    if agent in PSEUDO_AGENTS | MANUAL_AGENTS:
        return "n/a"
    try:
        p = subprocess.run([agent, "--version"], capture_output=True,
                           text=True, timeout=30)
        return p.stdout.strip().splitlines()[0] if p.stdout.strip() else "unknown"
    except (OSError, subprocess.TimeoutExpired, IndexError):
        return "unknown"
