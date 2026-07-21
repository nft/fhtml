#!/usr/bin/env python3
"""Regenerate tasks/*/golden/*.html from each task's golden-workspace.

Goldens are committed fixtures; rerun this only when a task's
golden-workspace deliberately changes, then eyeball the diff — silently
regenerated goldens would hide grading drift.
"""

import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
FHTML = os.path.join(ROOT, "target", "release", "fhtml")


def main():
    for task in sorted(os.listdir(os.path.join(HERE, "tasks"))):
        task_dir = os.path.join(HERE, "tasks", task)
        spec_path = os.path.join(task_dir, "checks.json")
        if not os.path.isfile(spec_path):
            continue
        spec = json.load(open(spec_path))
        goldens = [c["golden"] for c in spec.get("checks", [])
                   if c["kind"] == "dom_eq"]
        if not goldens:
            continue
        primary = spec.get("primary", "index.fhtml")
        src = os.path.join(task_dir, "golden-workspace", primary)
        cmd = [FHTML, "--min", src, "--deny-warnings"]
        if spec.get("data"):
            cmd += ["--data", os.path.join(task_dir, "seed", spec["data"])]
        p = subprocess.run(cmd, capture_output=True, text=True)
        if p.returncode != 0:
            sys.exit(f"{task}: golden-workspace does not compile:\n{p.stderr}")
        for rel in goldens:
            out = os.path.join(task_dir, rel)
            os.makedirs(os.path.dirname(out), exist_ok=True)
            with open(out, "w") as fh:
                fh.write(p.stdout)
            print(f"{task}: {rel} ({len(p.stdout)} B)")


if __name__ == "__main__":
    main()
