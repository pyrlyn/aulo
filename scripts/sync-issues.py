#!/usr/bin/env python3
"""Create GitHub labels, milestones and issues from plan.md.

plan.md stays the source of truth: the script only creates what is missing and
writes each new issue number back into its card ("Issue: #N"), so a second run
creates nothing. Dry run by default; --apply talks to GitHub through `gh`.
"""
import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

PLAN = Path(__file__).resolve().parent.parent / "plan.md"
# GitHub throttles bursts of content creation; one call every PAUSE_S stays under it.
PAUSE_S = 1.5
CARD_META = re.compile(r"^Stage: (S\d+) · Area: (\S+) · Depends on: (.+?) · Blocks: \d+ task\(s\)(?: · Issue: #(\d+))?$")


def parse(text):
    table = {tid: (pri, cx) for tid, pri, cx in re.findall(r"^\| (T\d+\.\d+) \| [^|]+ \| (P\d) \| (\d) \|", text, re.M)}
    stages = dict(re.findall(r"^## (S\d+)\. (.+)$", text, re.M))
    tasks = []
    for block in re.split(r"^### ", text, flags=re.M)[1:]:
        lines = block.split("\n")
        head = re.match(r"(T\d+\.\d+)\. (.+)", lines[0])
        if not head:
            continue
        meta = next((CARD_META.match(l) for l in lines if CARD_META.match(l)), None)
        if not meta:
            sys.exit(f"{PLAN}: card {head[1]} has no Stage/Area/Depends line")
        stage, area, deps, issue = meta.groups()
        body = "\n".join(l for l in lines[1:] if not CARD_META.match(l) and not l.startswith("## ")).strip()
        tasks.append(dict(id=head[1], title=head[2], stage=stage, area=area,
                          deps=[] if deps == "none" else [d.strip() for d in deps.split(",")],
                          issue=int(issue) if issue else None, body=body,
                          pri=table[head[1]][0], cx=table[head[1]][1]))
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
    text = PLAN.read_text()
    stages, tasks = parse(text)
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
        body = f"**Task:** {t['id']} · **Priority:** {t['pri']} · **Complexity:** {t['cx']}/5 · **Depends on:** {deps}\n\n{t['body']}\n\nSource of truth: `plan.md` card {t['id']}."
        out = gh(["issue", "create", "-R", a.repo, "-t", f"{t['id']}. {t['title']}", "-b", body,
                  "-l", f"{t['pri']},complexity:{t['cx']},area:{t['area']}",
                  "-m", f"{t['stage']}. {stages[t['stage']]}"], a.apply)
        if a.apply:
            num = int(out.strip().rsplit("/", 1)[-1])
            known[t["id"]] = num
            text = re.sub(rf"^(### {re.escape(t['id'])}\. .+\n\nStage: .+ task\(s\))$", rf"\1 · Issue: #{num}", text, count=1, flags=re.M)
            PLAN.write_text(text)
            print(f"{t['id']} -> #{num}")


if __name__ == "__main__":
    main()
