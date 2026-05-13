#!/usr/bin/env python3
"""TASMail traceability extractor + CI gate (TMAIL-208).

Walks the live source tree, builds a UI ⇄ backend traceability matrix, then
diffs the current orphan-endpoint set against the checked-in baseline at
docs/traceability/orphans-baseline.json.

Exit codes
----------
0 — clean. No new orphans, or new orphans only in the "warn" categories.
1 — drift detected in a "block" category (auth, billing, folders, messages,
    signatures, contacts). CI should fail.

Usage
-----
  python3 scripts/trace-check.py            # diff against baseline
  python3 scripts/trace-check.py --update   # regenerate baseline + report
  python3 scripts/trace-check.py --json     # emit machine-readable result
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ROUTER = ROOT / "backend/src/router.rs"
API_DIR = ROOT / "frontend/src/api"
COMP_DIR = ROOT / "frontend/src/components"
BASELINE = ROOT / "docs/traceability/orphans-baseline.json"

# Endpoints whose absence from the SPA must block the build.
BLOCK_CATEGORIES = {"auth", "billing", "folders", "messages", "signatures", "contacts"}

# Endpoints intentionally consumed via non-static patterns. They get filtered
# OUT of the orphan set before diffing so adding documented dynamic imports
# doesn't keep tripping the gate.
KNOWN_NONSTATIC = {
    # background-sync.ts dynamic imports (see file-level docstring there)
    ("POST", "/api/messages/schedule"),
    ("POST", "/api/folders/{folder}/messages/{uid}/move"),
    ("DELETE", "/api/folders/{folder}/messages/{uid}"),
    ("POST", "/api/folders/{folder}/messages/{uid}/flag"),
    ("POST", "/api/drafts"),
}


# ── parser ────────────────────────────────────────────────────────────────────
METHOD_RE = re.compile(
    r"\b(get|post|put|patch|delete)\s*\(\s*handlers::([\w:]+)\s*\)"
)
ROUTE_START = re.compile(r'\.route\(\s*(?:"([^"]+)")?')

CLIENT_CALL_RE = re.compile(
    r"apiClient\.(get|post|put|patch|delete)\s*(?:<[^>]+>)?\(\s*[`'\"]([^`'\"]+)[`'\"]",
    re.DOTALL,
)
FETCH_RE = re.compile(r"fetch\(\s*[`'\"](/api/[^`'\"]+)[`'\"]")


def normalise(p: str) -> str:
    p = p.split("?", 1)[0]
    p = re.sub(r"\$\{[^}]+\}", ":p", p)
    p = re.sub(r"\{[^}]+\}", ":p", p)
    if p.endswith("/") and len(p) > 1:
        p = p[:-1]
    return p


def feature_of(path: str) -> str:
    parts = path.strip("/").split("/")
    if parts and parts[0] == "api":
        parts = parts[1:]
    if parts and parts[0] == "admin" and len(parts) > 1:
        return f"admin/{parts[1]}"
    return parts[0] if parts else "(root)"


def parse_router(lines: list[str], gated: bool) -> list[dict]:
    out: list[dict] = []
    i = 0
    while i < len(lines):
        m = ROUTE_START.search(lines[i])
        if not m:
            i += 1
            continue
        path = m.group(1)
        depth = 0
        buf: list[str] = []
        j = i
        while j < len(lines):
            chunk = lines[j]
            for ch in chunk:
                if ch == "(":
                    depth += 1
                elif ch == ")":
                    depth -= 1
            buf.append(chunk)
            if depth == 0:
                break
            j += 1
        body = " ".join(buf)
        if path is None:
            mp = re.search(r'"([^"]+)"', body)
            if mp:
                path = mp.group(1)
        if path:
            for verb, handler in METHOD_RE.findall(body):
                out.append({
                    "path": path,
                    "method": verb.upper(),
                    "handler": handler,
                    "auth": gated,
                })
        i = j + 1
    return out


def extract_routes() -> list[dict]:
    src = ROUTER.read_text().splitlines()
    split = next(
        (i for i, line in enumerate(src) if "PROTECTED ROUTES" in line),
        len(src),
    )
    routes = parse_router(src[:split], gated=False) + parse_router(src[split:], gated=True)
    # dedup
    seen: set[tuple[str, str]] = set()
    uniq: list[dict] = []
    for r in routes:
        key = (r["path"], r["method"])
        if key in seen:
            continue
        seen.add(key)
        uniq.append(r)
    return uniq


def extract_client_paths() -> set[tuple[str, str]]:
    keys: set[tuple[str, str]] = set()
    for f in API_DIR.glob("*.ts"):
        if f.name.endswith(".test.ts") or f.name == "client.ts":
            continue
        text = f.read_text()
        for verb, path in CLIENT_CALL_RE.findall(text):
            if not path.startswith("/"):
                continue
            if not path.startswith("/api"):
                path = "/api" + path
            keys.add((verb.upper(), normalise(path)))
        for path in FETCH_RE.findall(text):
            # raw fetch only — best-effort, can't tell method without context
            for verb in ("GET", "POST", "PUT", "PATCH", "DELETE"):
                keys.add((verb, normalise(path)))
    return keys


def find_orphans() -> list[dict]:
    routes = extract_routes()
    client_keys = extract_client_paths()
    orphans = []
    for r in routes:
        key = (r["method"], normalise(r["path"]))
        if key in client_keys:
            continue
        if (r["method"], r["path"]) in KNOWN_NONSTATIC:
            continue
        orphans.append({
            "method": r["method"],
            "path": r["path"],
            "handler": r["handler"],
            "auth": r["auth"],
            "feature": feature_of(r["path"]),
        })
    orphans.sort(key=lambda o: (o["feature"], o["path"], o["method"]))
    return orphans


def group_by_feature(orphans: list[dict]) -> dict[str, list[dict]]:
    g: dict[str, list[dict]] = defaultdict(list)
    for o in orphans:
        g[o["feature"]].append(o)
    return dict(sorted(g.items()))


def load_baseline() -> dict:
    if not BASELINE.exists():
        return {"orphans": [], "by_feature": {}}
    return json.loads(BASELINE.read_text())


def dump_baseline(orphans: list[dict]) -> None:
    BASELINE.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "generated_by": "scripts/trace-check.py --update",
        "orphan_count": len(orphans),
        "by_feature_count": {feat: len(rows) for feat, rows in group_by_feature(orphans).items()},
        "orphans": orphans,
    }
    BASELINE.write_text(json.dumps(payload, indent=2) + "\n")


def diff(baseline: dict, current: list[dict]) -> tuple[list[dict], list[dict]]:
    base_keys = {(o["method"], o["path"]) for o in baseline.get("orphans", [])}
    cur_keys = {(o["method"], o["path"]) for o in current}
    added = [o for o in current if (o["method"], o["path"]) not in base_keys]
    removed_keys = base_keys - cur_keys
    removed = [o for o in baseline.get("orphans", []) if (o["method"], o["path"]) in removed_keys]
    return added, removed


def main() -> int:
    ap = argparse.ArgumentParser(description="TASMail traceability gate")
    ap.add_argument("--update", action="store_true", help="rewrite the baseline")
    ap.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = ap.parse_args()

    current = find_orphans()
    if args.update:
        dump_baseline(current)
        print(f"Baseline updated: {len(current)} orphans recorded at {BASELINE.relative_to(ROOT)}")
        return 0

    baseline = load_baseline()
    added, removed = diff(baseline, current)
    block = [o for o in added if o["feature"] in BLOCK_CATEGORIES]
    warn = [o for o in added if o["feature"] not in BLOCK_CATEGORIES]

    if args.json:
        print(json.dumps({
            "total_orphans": len(current),
            "baseline_orphans": len(baseline.get("orphans", [])),
            "added": added,
            "removed": removed,
            "block": block,
            "warn": warn,
        }, indent=2))
        return 1 if block else 0

    print(f"Trace check — {len(current)} orphan endpoints "
          f"(baseline: {len(baseline.get('orphans', []))})")
    if removed:
        print(f"\n✓ {len(removed)} orphan(s) closed since baseline:")
        for o in removed:
            print(f"    {o['method']:6} {o['path']:60} [{o['feature']}]")
    if warn:
        print(f"\n⚠ {len(warn)} new orphan(s) in warn-only categories:")
        for o in warn:
            print(f"    {o['method']:6} {o['path']:60} [{o['feature']}]")
    if block:
        print(f"\n✘ {len(block)} new orphan(s) in BLOCK categories "
              f"({', '.join(sorted(BLOCK_CATEGORIES))}):")
        for o in block:
            print(f"    {o['method']:6} {o['path']:60} [{o['feature']}]  → {o['handler']}")
        print("\nA shipped backend endpoint in one of the block categories no "
              "longer has a SPA consumer. Either add the apiClient call, "
              "wire it through a documented dynamic-import path, or update "
              "the baseline if the route is being retired:")
        print("    python3 scripts/trace-check.py --update")
        return 1

    if not added and not removed:
        print("✓ No drift — orphan set matches baseline exactly.")
    elif not block:
        print("\n✓ Drift confined to warn-only categories. No action required.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
