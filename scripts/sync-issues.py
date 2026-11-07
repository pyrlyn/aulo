#!/usr/bin/env python3
"""Create GitHub labels, milestones and issues from the active tasks in plan/.

plan/ stays the source of truth: the script only creates what is missing and
writes each new issue number back into the task's Stage line ("Issue: #N"), so
a second run creates nothing. Dry run by default; --apply talks to GitHub
through `gh`.
"""
import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

PLAN = Path(__file__).resolve().parent.parent / "plan"
# GitHub throttles bursts of content creation; one call every PAUSE_S stays under it.
PAUSE_S = 1.5
CARD_META = re.compile(r"^Stage: (S\d+) · Area: (\S+) · Depends on: (.+?) · Blocks: \d+ task\(s\)(?: · Issue: #(\d+))?$", re.M)
FRONT = re.compile(r"\A---\n(.*?)\n---\n", re.S)


def parse_task(path):
    text = path.read_text()
    front = FRONT.match(text)
    if not front:
        sys.exit(f"{path}: active task has no front matter")
    keys = dict(re.findall(r"^(\w+): ?(.*)$", front[1], re.M))
    head = re.search(r"^# (T\d+(?:\.\d+)*)\. (.+)$", text, re.M)
    if not head or head[1] != path.parent.name:
        sys.exit(f"{path}: heading does not name {path.parent.name}")
    meta = CARD_META.search(text)
    if not meta:
        sys.exit(f"{path}: task has no Stage/Area/Depends line")
    pri, cx = keys.get("priority", ""), keys.get("complexity", "")
    if not re.fullmatch(r"P[0-3]", pri) or not re.fullmatch(r"[1-5]", cx):
        sys.exit(f"{path}: priority or complexity is missing or malformed")
    stage, area, deps, issue = meta.groups()
    body = "\n".join(l for l in text[head.end():].split("\n") if not CARD_META.match(l)).strip()
    return dict(id=head[1], title=head[2], stage=stage, area=area,
                deps=[] if deps == "none" else [d.strip() for d in deps.split(",")],
                issue=int(issue) if issue else None, body=body, pri=pri, cx=cx, path=path)


def parse():
    stages = dict(re.findall(r"^- (S\d+)\. (.+)$", (PLAN / "README.md").read_text(), re.M))
    tasks = [parse_task(d / "task.md") for d in sorted(PLAN.iterdir())
             if d.is_dir() and not (d / "done.md").exists()]
    for t in tasks:
        if t["stage"] not in stages:
            sys.exit(f"{t['path']}: stage {t['stage']} is not listed in plan/README.md")
    return stages, tasks


def gh(args, apply):
    if not apply:
        print("dry-run: gh " + " ".join(args[:6]) + (" …" if len(args) > 6 else ""))
        return ""
    time.sleep(PAUSE_S)
    run = subprocess.run(["gh", *args], capture_output=True, text=True)
    if run.returncode != 0:
        sys.exit(f"gh {' '.join(args[:3])} failed: {run.stderr.strip()}")
    return run.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True, help="owner/name, e.g. pyrlyn/aulo")
    ap.add_argument("--apply", action="store_true", help="really create on GitHub")
    a = ap.parse_args()
    stages, tasks = parse()
    known = {t["id"]: t["issue"] for t in tasks}

    labels = {f"P{i}": "b60205" for i in range(4)} | {f"complexity:{i}": "c5def5" for i in range(1, 6)}
    labels |= {f"area:{t['area']}": "fbca04" for t in tasks}
    have = set()
    if a.apply:
        # Fail before creating anything when the repository is missing or unreachable.
        gh(["repo", "view", a.repo, "--json", "name"], True)
        have = {l["name"] for l in json.loads(gh(["label", "list", "-R", a.repo, "-L", "500", "--json", "name"], True))}
    for name, color in sorted(labels.items()):
        if name not in have:
            gh(["label", "create", name, "-R", a.repo, "-c", color], a.apply)

    ms = {}
    if a.apply:
        ms = {m["title"]: m["number"] for m in json.loads(gh(["api", f"repos/{a.repo}/milestones?state=all&per_page=100"], True))}
    for sid, name in stages.items():
        title = f"{sid}. {name}"
        if title not in ms:
            gh(["api", f"repos/{a.repo}/milestones", "-f", f"title={title}"], a.apply)

    for t in tasks:
        if t["issue"]:
            continue
        deps = ", ".join(f"{d} (#{known[d]})" if known.get(d) else d for d in t["deps"]) or "none"
        body = f"**Task:** {t['id']} · **Priority:** {t['pri']} · **Complexity:** {t['cx']}/5 · **Depends on:** {deps}\n\n{t['body']}\n\nSource of truth: `plan/{t['id']}/task.md`."
        out = gh(["issue", "create", "-R", a.repo, "-t", f"{t['id']}. {t['title']}", "-b", body,
                  "-l", f"{t['pri']},complexity:{t['cx']},area:{t['area']}",
                  "-m", f"{t['stage']}. {stages[t['stage']]}"], a.apply)
        if a.apply:
            num = int(out.strip().rsplit("/", 1)[-1])
            known[t["id"]] = num
            text = t["path"].read_text()
            text = CARD_META.sub(lambda m: m[0] + f" · Issue: #{num}", text, count=1)
            t["path"].write_text(text)
            print(f"{t['id']} -> #{num}")


if __name__ == "__main__":
    main()
