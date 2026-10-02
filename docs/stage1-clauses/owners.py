#!/usr/bin/env python3
"""Assigns every Core conformance id of the botster-contracts ledger to exactly one Stage 1 package.

Usage: owners.py <path to botster-contracts>. Writes docs/stage1-clauses/<package>.txt and prints the counts.
The rules are the ones in docs/stage1-plan.md section 6; the first rule that matches wins.
"""
import collections, json, os, re, sys

contracts = sys.argv[1]
ids = json.load(open(os.path.join(contracts, "conformance/ledger.json")))["ids"]
pending = set(open(os.path.join(contracts, "conformance/pending.txt")).read().split())
core = [x for x in ids if x["contract"] == "core"]


def owner(x):
    i, c = x["id"], x["clause"] or ""
    fam = c.split("-")[0]
    if c.startswith("E2-"):
        return "p3-worker"
    if "withheld_control_link" in i:
        return "p5-adoption"
    if c == "A6-1":
        return "p7-services"
    if c == "A6-2":
        return "p5-adoption"
    if c == "A6-3":
        return "p4b-queries-files"
    if re.search(r"adopt|restart|survive", i) or fam == "AD" or c in ("DP-8", "ID-2", "LC-11") \
            or i == "conf::lc_12_drop_leaves_workers_running":
        return "p5-adoption"
    if fam == "A5" or c == "OR-3":
        return "p6-testkit"
    if fam == "SV" or c in ("A2-5", "A4-1"):
        return "p7-services"
    if c in ("TH-1", "E1-1"):
        return "p0-skeleton"
    if c in ("A2-8", "ST-6b"):
        return "p2-terminal"
    if re.search(r"webrtc|datachannel|_t2_|connect_deadline", i) or c in ("DP-10", "DP-11"):
        return "p4c-webrtc-perf"
    if c in ("EV-8", "DP-5b", "A2-9") or re.search(r"query|file", i):
        return "p4b-queries-files"
    if fam in ("OU", "DP", "A3") or c in ("A2-3", "TH-3"):
        return "p4a-routes"
    if fam in ("IN", "SZ", "TP") or c in ("AM-2", "A2-2", "A2-4", "ST-1", "ST-2", "ST-3", "ST-5", "ST-7",
                                          "EV-1", "EV-3", "EV-4", "EV-7"):
        return "p3-worker"
    return "p1-lifecycle"


out = collections.defaultdict(list)
for x in core:
    out[owner(x)].append(x)
here = os.path.dirname(os.path.abspath(__file__))
for pkg, xs in sorted(out.items()):
    with open(os.path.join(here, f"{pkg}.txt"), "w") as f:
        for x in sorted(xs, key=lambda x: x["id"]):
            f.write(f"{x['id']}\t{x['clause']}\t{'pending' if x['id'] in pending else 'transcript'}\n")
    print(f"{pkg}\t{len(xs)}\tpending {sum(x['id'] in pending for x in xs)}\tclauses {' '.join(sorted({x['clause'] for x in xs}))}")
print("total", len(core))
