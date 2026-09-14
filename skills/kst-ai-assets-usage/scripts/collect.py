#!/usr/bin/env python3
"""Collect local usage telemetry for kasetto-installed skills and MCP servers.

Reads the installed inventory from `kst list --json`, scans each supported
agent's local session store for tool invocations, and emits a single JSON
document joining the two. Standard library only.

The scan is deliberately pattern-based rather than schema-based. Agent session
formats churn (Goose moved JSONL -> SQLite, OpenCode JSON -> SQLite, Copilot CLI
flat -> per-session dirs), but the tool-name strings inside them do not move.
Matching on `mcp__<server>__<tool>` and on known asset names survives a schema
change that would break a field-by-field parser.
"""

import argparse
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from datetime import date
from pathlib import Path

CACHE_VERSION = 2
STALE_DAYS = 60

# == Providers ==

# Each provider: where its session store lives and how to read it. `kind` picks
# the reader. Paths honour the agent's own env override where one exists.
def providers():
    home = Path.home()
    codex_home = Path(os.environ.get("CODEX_HOME", home / ".codex"))
    return [
        {
            "agent": "claude-code",
            "kind": "jsonl",
            "root": home / ".claude" / "projects",
            "glob": "**/*.jsonl",
            "verified": True,
        },
        {
            "agent": "codex",
            "kind": "jsonl",
            "root": codex_home / "sessions",
            "glob": "**/*.jsonl",
            "verified": False,
        },
        {
            "agent": "cursor",
            "kind": "jsonl",
            "root": home / ".cursor" / "projects",
            "glob": "**/agent-transcripts/**/*.jsonl",
            "verified": False,
        },
        {
            "agent": "github-copilot",
            "kind": "jsonl",
            "root": home / ".copilot" / "session-state",
            "glob": "**/*.jsonl",
            "verified": False,
        },
        {
            "agent": "opencode",
            "kind": "sqlite",
            "root": home / ".local" / "share" / "opencode" / "opencode.db",
            "sql": "SELECT time_created, data FROM part",
            "verified": True,
        },
        {
            "agent": "goose",
            "kind": "sqlite",
            "root": home / ".local" / "share" / "goose" / "sessions.db",
            "sql": None,  # table discovered at runtime; schema unverified
            "verified": False,
        },
    ]


# == Matching ==

# Anchored on a name-key position on purpose. Session logs also carry arrays of
# the tools *offered* each turn (`"mcp__logfire__alert_create","mcp__logfire__..."`),
# which repeat once per request. Matching the bare `mcp__x__y` string anywhere
# counted those as calls and inflated one server from 0 real invocations to
# 38,743. Any new provider must anchor the same way.
MCP_RE = re.compile(r'"name"\s*:\s*"mcp__([A-Za-z0-9_.-]+?)__([A-Za-z0-9_.-]+)"')
# ISO-8601 with optional fractional seconds and zone; the first one on a line is
# close enough to the event for a "last used" readout.
TS_RE = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?")


def skill_pattern(names):
    """Match an installed skill name only in a structured key position.

    A bare name like `init` appears constantly in prose, so anchoring on
    `"skill": "init"` / `"tool": "init"` is what separates an invocation from a
    mention. The generic `"name"` key is deliberately excluded: it is the most
    common key in these logs and matches file names, agent names and tool
    definitions. Over-matching marks a dead skill alive, and a skill wrongly
    shown as used is the one error that costs the user nothing to keep and
    everything to trust.
    """
    if not names:
        return None
    alt = "|".join(sorted((re.escape(n) for n in names), key=len, reverse=True))
    return re.compile(r'"(?:skill|skill_name|tool)"\s*:\s*"(' + alt + r')"')


def scan_text(text, skill_re, acc, ts_hint):
    """Accumulate matches from one chunk of raw session text.

    Per-day buckets are kept alongside the totals so the report can show when an
    asset was used, not just how often. A total alone cannot distinguish a tool
    used heavily last year from one used steadily this week.
    """
    ts = None
    m = TS_RE.search(text)
    if m:
        ts = m.group(0)
    ts = ts or ts_hint
    day = ts[:10] if ts else None

    for server, tool in MCP_RE.findall(text):
        e = acc["mcp_servers"].setdefault(
            server, {"count": 0, "tools": {}, "last": None, "days": {}})
        e["count"] += 1
        e["tools"][tool] = e["tools"].get(tool, 0) + 1
        if day:
            e["days"][day] = e["days"].get(day, 0) + 1
        if ts and (e["last"] is None or ts > e["last"]):
            e["last"] = ts

    if skill_re:
        for name in skill_re.findall(text):
            e = acc["skills"].setdefault(name, {"count": 0, "last": None, "days": {}})
            e["count"] += 1
            if day:
                e["days"][day] = e["days"].get(day, 0) + 1
            if ts and (e["last"] is None or ts > e["last"]):
                e["last"] = ts


def empty_acc():
    return {"skills": {}, "mcp_servers": {}}


def attribute(acc, project):
    """Stamp one file's results with its session and project.

    One JSONL file is one session, so presence in the file means one session -
    which separates a skill run 16 times in a single burst from one run once a
    week for 16 weeks. The project label comes from the directory name the agent
    encodes the working directory into.
    """
    for group in ("skills", "mcp_servers"):
        for e in acc[group].values():
            e["sessions"] = 1
            if project:
                e["projects"] = {project: e["count"]}


def merge_days(into, other):
    for d, n in other.items():
        into[d] = into.get(d, 0) + n


def merge_acc(into, other):
    for name, e in other["skills"].items():
        t = into["skills"].setdefault(
            name, {"count": 0, "last": None, "days": {}, "sessions": 0, "projects": {}})
        t["count"] += e["count"]
        t["sessions"] = t.get("sessions", 0) + e.get("sessions", 0)
        merge_days(t["projects"], e.get("projects", {}))
        merge_days(t["days"], e.get("days", {}))
        if e["last"] and (t["last"] is None or e["last"] > t["last"]):
            t["last"] = e["last"]
    for server, e in other["mcp_servers"].items():
        t = into["mcp_servers"].setdefault(
            server, {"count": 0, "tools": {}, "last": None, "days": {},
                     "sessions": 0, "projects": {}})
        t["count"] += e["count"]
        t["sessions"] = t.get("sessions", 0) + e.get("sessions", 0)
        merge_days(t["projects"], e.get("projects", {}))
        for tool, n in e["tools"].items():
            t["tools"][tool] = t["tools"].get(tool, 0) + n
        merge_days(t["days"], e.get("days", {}))
        if e["last"] and (t["last"] is None or e["last"] > t["last"]):
            t["last"] = e["last"]


# == Readers ==

def project_label(path):
    """Readable project name from the directory an agent stores sessions under.

    Claude Code encodes the working directory into the folder name by replacing
    every non-alphanumeric run with `-`, which is lossy: a real hyphen and a path
    separator become the same character, so `pivoshenko-wallpapers` cannot be
    told from a `pivoshenko/wallpapers` directory. Rather than reconstruct a path
    and get it confidently wrong, strip the encoded home prefix and show what is
    left verbatim. It groups correctly, which is what the rollup needs.
    """
    name = path.parent.name
    if not name.startswith("-"):
        return name
    home = re.sub(r"[^A-Za-z0-9]+", "-", str(Path.home()))
    if name.startswith(home):
        name = name[len(home):]
    return name.strip("-") or "-"


def read_jsonl_tree(root, pattern, skill_re, cache):
    """Scan a tree of JSONL session files, reusing cached per-file results.

    Closed sessions never change, so caching on (size, mtime) means a rerun only
    touches the handful of files the agent actually appended to.
    """
    acc, files, events = empty_acc(), 0, 0
    for path in sorted(root.glob(pattern)):
        if not path.is_file():
            continue
        files += 1
        try:
            st = path.stat()
        except OSError:
            continue
        key = str(path)
        hit = cache.get(key)
        if hit and hit.get("size") == st.st_size and hit.get("mtime") == int(st.st_mtime):
            merge_acc(acc, hit["result"])
            events += hit.get("events", 0)
            continue

        one = empty_acc()
        ts_hint = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(st.st_mtime))
        n = 0
        try:
            with path.open("r", encoding="utf-8", errors="replace") as fh:
                for line in fh:
                    if "mcp__" not in line and '"skill' not in line and '"tool"' not in line and '"name"' not in line:
                        continue
                    scan_text(line, skill_re, one, ts_hint)
                    n += 1
        except OSError:
            continue
        attribute(one, project_label(path))
        cache[key] = {
            "size": st.st_size,
            "mtime": int(st.st_mtime),
            "events": n,
            "result": one,
        }
        merge_acc(acc, one)
        events += n
    return acc, files, events


def read_sqlite(path, sql, skill_re):
    """Scan a SQLite session store. Rows are opaque JSON blobs; same patterns apply."""
    acc, rows = empty_acc(), 0
    try:
        con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    except sqlite3.Error:
        return acc, 0, 0

    try:
        queries = [sql] if sql else discover_sqlite_queries(con)
        for q in queries:
            try:
                for ts, data in con.execute(q):
                    if not data:
                        continue
                    text = data if isinstance(data, str) else str(data)
                    hint = None
                    if isinstance(ts, int) and ts > 0:
                        secs = ts / 1000 if ts > 10_000_000_000 else ts
                        hint = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(secs))
                    scan_text(text, skill_re, acc, hint)
                    rows += 1
            except sqlite3.Error:
                continue
    finally:
        con.close()
    return acc, 1, rows


def discover_sqlite_queries(con):
    """Find (timestamp, blob) column pairs in an unknown schema.

    Goose's post-1.10 schema is not pinned here on purpose. Rather than guess
    table names that may be wrong, look for any text column wide enough to hold
    a serialized message and pair it with a plausible time column.
    """
    out = []
    try:
        tables = [r[0] for r in con.execute(
            "SELECT name FROM sqlite_master WHERE type='table'")]
    except sqlite3.Error:
        return out
    for t in tables:
        try:
            cols = [(r[1], (r[2] or "").upper()) for r in con.execute(f'PRAGMA table_info("{t}")')]
        except sqlite3.Error:
            continue
        names = [c for c, _ in cols]
        blob = next((c for c in names if c.lower() in
                     ("data", "content", "body", "message", "payload", "json")), None)
        if not blob:
            continue
        tcol = next((c for c in names if "time" in c.lower() or c.lower() in ("created", "ts")), None)
        out.append(f'SELECT {tcol or "NULL"}, "{blob}" FROM "{t}"')
    return out


# == Inventory ==

def installed_inventory():
    """Ask kasetto what it installed. `kst list --json` is the contract."""
    for exe in ("kst", "kasetto"):
        try:
            proc = subprocess.run([exe, "list", "--json"], capture_output=True, text=True, timeout=60)
        except (FileNotFoundError, subprocess.SubprocessError):
            continue
        if proc.returncode == 0 and proc.stdout.strip():
            try:
                return json.loads(proc.stdout), None
            except json.JSONDecodeError as exc:
                return None, f"{exe} list --json returned unparseable output: {exc}"
    return None, "kasetto CLI not found on PATH (tried `kst`, `kasetto`)"


FRESH_DAYS = 7
AGE_RE = re.compile(r"(\d+)\s*([smhdwy])")


def days_since(last):
    """Whole days since an ISO timestamp, or None when never used."""
    if not last:
        return None
    try:
        return max(0, (date.today() - date.fromisoformat(last[:10])).days)
    except ValueError:
        return None


def is_stale(last, total):
    """A skill used heavily long ago still reads as alive without this.

    Live/idle is a binary that hides the middle: something invoked twenty times
    a year ago and never since is not in use, but it is also not unused, and it
    escapes the idle list forever. Anything past STALE_DAYS gets its own tier.
    """
    if not total:
        return False
    n = days_since(last)
    return n is not None and n > STALE_DAYS


def is_fresh(age):
    """True when an asset was installed too recently to judge as unused.

    `kst list` renders age as "12h ago" / "3d ago" / "35d ago". Something
    installed yesterday has had no chance to be invoked, and listing it as a
    removal candidate is how a report loses the user's trust.
    """
    if not age:
        return False
    m = AGE_RE.search(age)
    if not m:
        return False
    n, unit = int(m.group(1)), m.group(2)
    days = {"s": 0, "m": 0, "h": n / 24, "d": n, "w": n * 7, "y": n * 365}[unit]
    return days <= FRESH_DAYS


def skill_inventory(entries):
    """Normalize skill rows from `kst list --json`.

    A row carries both a display `name` ("/analyze - Answer Data Questions") and
    the invocable slug `skill` ("analyze"). Matching must use the slug; the
    display name never appears in a session log.
    """
    out = []
    for e in entries or []:
        if isinstance(e, str):
            out.append({"slug": e, "label": e, "source": "", "scope": "", "age": ""})
            continue
        slug = e.get("skill") or e.get("name")
        if not slug:
            continue
        out.append({
            "slug": slug,
            "label": e.get("name") or slug,
            "source": e.get("source", ""),
            "scope": e.get("scope", ""),
            "age": e.get("updated_ago", ""),
        })
    return out


MCP_LOCK_RE = re.compile(
    r"^\s{2}mcp::(?P<src>[^:\n]*(?:::[^:\n]*)*?)::(?P<pack>[^:\n]+):\s*$"
    r"(?P<body>(?:\n\s{4}\S.*)*)", re.MULTILINE)


def lock_mcp_servers():
    """Map MCP pack -> server names by reading `kasetto.lock`.

    `kst list --json` reports the pack but not the servers it merged, and the
    telemetry only ever sees server names (`mcp__<server>__<tool>`). The lock is
    the only place that mapping exists. It is YAML and the standard library has
    no YAML parser, but these entries are machine-generated with a fixed shape,
    so a narrow regex over just the `mcp::` blocks is safer than taking on a
    dependency. If the shape ever changes this returns nothing and callers fall
    back to assuming pack name == server name.
    """
    candidates = [
        Path.cwd() / "kasetto.lock",
        Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))
        / "kasetto" / "kasetto.lock",
    ]
    mapping = {}
    for lock in candidates:
        if not lock.exists():
            continue
        try:
            text = lock.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for m in MCP_LOCK_RE.finditer(text):
            body = m.group("body")
            dest = re.search(r"^\s{4}destination:\s*(.+)$", body, re.MULTILINE)
            name = re.search(r"^\s{4}name:\s*(.+)$", body, re.MULTILINE)
            pack = (name.group(1).strip() if name else m.group("pack")).strip()
            pack = pack[:-5] if pack.endswith(".json") else pack
            servers = [s.strip() for s in dest.group(1).split(",")] if dest else []
            if servers:
                mapping.setdefault(pack, []).extend(s for s in servers if s)
    return mapping


def mcp_inventory(entries, lock_map):
    """Normalize MCP rows, resolving each pack to the servers it installed."""
    out = []
    for e in entries or []:
        if isinstance(e, str):
            pack = e[:-5] if e.endswith(".json") else e
            out.append({"pack": pack, "servers": lock_map.get(pack, [pack]),
                        "source": "", "scope": ""})
            continue
        pack = e.get("name") or e.get("id") or ""
        pack = pack[:-5] if pack.endswith(".json") else pack
        if not pack:
            continue
        out.append({
            "pack": pack,
            "servers": lock_map.get(pack, [pack]),
            "source": e.get("source", ""),
            "scope": e.get("scope", ""),
        })
    return out


# == Insights ==

# Severity drives colour in the dashboard, so the vocabulary is deliberately
# narrow. "bad" is reserved for concrete waste the user can delete; an unused
# skill is dormant, not a defect, and colouring it red would spend the strongest
# signal on the least urgent finding.
SEV_BAD, SEV_WARN, SEV_INFO, SEV_GOOD = "bad", "warn", "info", "good"


def insights(skills, mcps, by_source, unmanaged, coverage, activity, idle, fresh,
             by_project=None):
    """Derive the handful of statements worth acting on.

    Computed here rather than left to the reader because these are arithmetic
    over the whole report, and a number stated once beats a chart the reader has
    to integrate by eye.
    """
    out = []

    # Whole source repos with nothing in use: the cheapest large cleanup, since
    # each is a single `source:` block rather than N individual removals.
    dead_src = {s: v for s, v in by_source.items()
                if v["used"] == 0 and v["fresh"] == 0 and (v["idle"] or 0) > 0}
    if dead_src:
        n = sum(v["idle"] for v in dead_src.values())
        out.append({
            "severity": SEV_BAD,
            "title": f"{len(dead_src)} source repos have no asset in use",
            "detail": f"They account for {n} idle assets. Each is one `source:` block, "
                      f"so removing them is {len(dead_src)} edits rather than {n}.",
            "items": [short_src(s) for s in sorted(
                dead_src, key=lambda x: -dead_src[x]["idle"])][:6],
        })

    # Idle MCP packs carry a standing cost that idle skills do not.
    mcp_idle = [k for k, v in mcps.items() if not v["total"]]
    if mcp_idle:
        out.append({
            "severity": SEV_WARN,
            "title": f"{len(mcp_idle)} MCP packs load into context but are never called",
            "detail": "MCP tool definitions are sent on every request whether or not the "
                      "server is used, so an idle pack is a recurring cost, not just clutter.",
            "items": sorted(mcp_idle),
        })

    # Coverage gaps invalidate every "idle" claim for the agents involved.
    absent = [c["agent"] for c in coverage if c["status"] != "ok"]
    unver = [c["agent"] for c in coverage if c["status"] == "ok"
             and not c.get("verified_format")]
    if absent:
        out.append({
            "severity": SEV_WARN,
            "title": f"{len(absent)} of {len(coverage)} known agents left no readable history",
            "detail": "Assets driven from these agents appear idle here and are not. "
                      "Treat the idle list as a shortlist to review, not a cut list.",
            "items": sorted(absent),
        })
    if unver:
        out.append({
            "severity": SEV_WARN,
            "title": f"{len(unver)} providers parsed an unverified format",
            "detail": "Their layout has not been confirmed against a real sample, so a zero "
                      "from them is weak evidence rather than proof.",
            "items": sorted(unver),
        })

    # Used, but long ago - invisible in a live/idle split.
    stale = [k for k, v in skills.items() if v.get("stale")]
    if stale:
        out.append({
            "severity": SEV_WARN,
            "title": f"{len(stale)} skills were used once but not in the last {STALE_DAYS} days",
            "detail": "They count as live on every other view, which is how a tool you "
                      "stopped reaching for stays in the config indefinitely.",
            "items": sorted(stale, key=lambda k: -(skills[k].get("days_since") or 0))[:6],
        })

    # One-burst assets: a high total from a single session is not a habit.
    burst = [k for k, v in skills.items()
             if v["total"] >= 5 and v.get("sessions", 0) == 1]
    if burst:
        out.append({
            "severity": SEV_INFO,
            "title": f"{len(burst)} skills look busy but ran in a single session",
            "detail": "A high call count from one sitting is a trial, not a habit. "
                      "Session count separates the two.",
            "items": sorted(burst),
        })

    # Where the toolkit actually gets exercised.
    if by_project:
        top = sorted(by_project.items(), key=lambda x: -x[1]["calls"])
        share = round(100 * top[0][1]["calls"] / max(1, sum(v["calls"] for v in by_project.values())))
        out.append({
            "severity": SEV_INFO,
            "title": f"{len(by_project)} projects used these assets, and the top one is "
                     f"{share}% of all calls",
            "detail": "Assets that only ever fire in one project may belong in that "
                      "project's kasetto.yaml rather than the global config.",
            "items": [t[0] for t in top[:4]],
        })

    # Concentration: how much of the toolkit is actually carrying the work.
    used = {k: v["total"] for k, v in skills.items() if v["total"]}
    total_calls = sum(used.values())
    if used and total_calls:
        top3 = sorted(used.values(), reverse=True)[:3]
        pct = round(100 * sum(top3) / total_calls)
        out.append({
            "severity": SEV_INFO,
            "title": f"Your top 3 skills are {pct}% of all skill invocations",
            "detail": f"{len(used)} of {len(skills)} installed skills have ever run. "
                      f"Usage concentrates hard, which is normal - it just means the long "
                      f"tail is cheaper to review than it looks.",
            "items": [k for k, _ in sorted(used.items(), key=lambda x: -x[1])[:3]],
        })

    # Unmanaged assets in heavy use are the inverse finding: not what to remove,
    # but what is missing from the config.
    um = sorted(unmanaged["mcps"].items(), key=lambda x: -x[1])[:1]
    if um and used and um[0][1] > max(used.values() or [0]):
        name, n = um[0]
        out.append({
            "severity": SEV_INFO,
            "title": f"`{name}` is your most-used asset and kasetto does not manage it",
            "detail": f"{n} calls, more than any managed skill. Adding it to kasetto.yaml "
                      f"would sync it across machines and agents like the rest.",
            "items": [],
        })

    if fresh:
        out.append({
            "severity": SEV_INFO,
            "title": f"{len(fresh)} skills were installed in the last 7 days and not yet run",
            "detail": "Held out of the cut list deliberately - a zero here means no chance "
                      "to be used, not a verdict.",
            "items": sorted(fresh)[:6],
        })

    # A long silence is worth naming; a sparse chart is easy to misread as dense.
    days = sorted(set(activity["skills"]) | set(activity["mcps"]))
    if days:
        try:
            last = date.fromisoformat(days[-1])
            quiet = (date.today() - last).days
            if quiet >= 14:
                out.append({
                    "severity": SEV_INFO,
                    "title": f"No recorded invocation in {quiet} days",
                    "detail": "Either the toolkit is idle or sessions are being written "
                              "somewhere this run did not read.",
                    "items": [],
                })
        except ValueError:
            pass

    if not idle and not mcp_idle:
        out.append({
            "severity": SEV_GOOD,
            "title": "Everything installed has been used",
            "detail": "No idle assets in the agents this run could read.",
            "items": [],
        })

    order = {SEV_BAD: 0, SEV_WARN: 1, SEV_INFO: 2, SEV_GOOD: 3}
    out.sort(key=lambda x: order.get(x["severity"], 9))
    return out


def short_src(src):
    if not src or src == "-":
        return "-"
    s = str(src).rstrip("/")
    return "/".join(s.split("/")[-2:]) if "/" in s else s


# == Main ==

def main():
    ap = argparse.ArgumentParser(description="Collect kasetto asset usage telemetry.")
    ap.add_argument("--out", default="usage.json", help="where to write the report JSON")
    ap.add_argument("--cache", default=None, help="scan cache path (default: XDG cache)")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    cache_path = Path(args.cache) if args.cache else (
        Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
        / "kasetto" / "usage-scan.json")
    cache = {}
    if cache_path.exists():
        try:
            blob = json.loads(cache_path.read_text())
            if blob.get("version") == CACHE_VERSION:
                cache = blob.get("files", {})
        except (OSError, json.JSONDecodeError):
            cache = {}

    inventory, inv_err = installed_inventory()
    if inventory is None:
        print(f"error: {inv_err}", file=sys.stderr)
        return 2

    skills = skill_inventory(inventory.get("skills"))
    mcps = mcp_inventory(inventory.get("mcps"), lock_mcp_servers())
    skill_re = skill_pattern([s["slug"] for s in skills])

    # server name -> owning pack, so per-server counts roll up to the pack
    pack_of = {}
    for entry in mcps:
        for server in entry["servers"]:
            pack_of[server] = entry["pack"]

    coverage, totals = [], {}
    for prov in providers():
        root = prov["root"]
        if not root.exists():
            coverage.append({
                "agent": prov["agent"], "status": "absent",
                "reason": f"no session store at {root}",
                "verified_format": prov["verified"],
            })
            continue
        if prov["kind"] == "jsonl":
            acc, files, events = read_jsonl_tree(root, prov["glob"], skill_re, cache)
        else:
            acc, files, events = read_sqlite(root, prov.get("sql"), skill_re)
        totals[prov["agent"]] = acc
        coverage.append({
            "agent": prov["agent"], "status": "ok", "root": str(root),
            "sources": files, "events": events,
            "verified_format": prov["verified"],
        })

    # == Join ==
    covered = {c["agent"] for c in coverage if c["status"] == "ok"}

    skill_usage = {}
    for entry in skills:
        slug = entry["slug"]
        by_agent, days, projects, last, total, sessions = {}, {}, {}, None, 0, 0
        for agent, acc in totals.items():
            e = acc["skills"].get(slug)
            if not e:
                continue
            by_agent[agent] = e["count"]
            total += e["count"]
            sessions += e.get("sessions", 0)
            merge_days(days, e.get("days", {}))
            merge_days(projects, e.get("projects", {}))
            if e["last"] and (last is None or e["last"] > last):
                last = e["last"]
        skill_usage[slug] = {
            "total": total, "last_used": last, "by_agent": by_agent, "days": days,
            "sessions": sessions, "projects": projects,
            "days_since": days_since(last), "stale": is_stale(last, total),
            "label": entry["label"], "source": entry["source"],
            "scope": entry["scope"], "age": entry["age"],
            "fresh": is_fresh(entry["age"]),
        }

    mcp_usage = {}
    for entry in mcps:
        by_agent, per_server, per_tool, days, projects = {}, {}, {}, {}, {}
        last, total, sessions = None, 0, 0
        for server in entry["servers"]:
            for agent, acc in totals.items():
                e = acc["mcp_servers"].get(server)
                if not e:
                    continue
                by_agent[agent] = by_agent.get(agent, 0) + e["count"]
                per_server[server] = per_server.get(server, 0) + e["count"]
                total += e["count"]
                merge_days(days, e.get("days", {}))
                merge_days(projects, e.get("projects", {}))
                sessions += e.get("sessions", 0)
                for tool, n in e.get("tools", {}).items():
                    per_tool[tool] = per_tool.get(tool, 0) + n
                if e["last"] and (last is None or e["last"] > last):
                    last = e["last"]
            per_server.setdefault(server, 0)
        mcp_usage[entry["pack"]] = {
            "total": total, "last_used": last, "days": days,
            "sessions": sessions, "projects": projects,
            "days_since": days_since(last), "stale": is_stale(last, total),
            "by_agent": by_agent, "by_server": per_server, "by_tool": per_tool,
            "source": entry["source"], "scope": entry["scope"],
        }

    # Assets seen in the logs that kasetto did not install.
    managed_servers = set(pack_of)
    unmanaged = {"skills": {}, "mcps": {}}
    for acc in totals.values():
        for name, e in acc["skills"].items():
            if name not in skill_usage:
                unmanaged["skills"][name] = unmanaged["skills"].get(name, 0) + e["count"]
        for server, e in acc["mcp_servers"].items():
            if server not in managed_servers:
                unmanaged["mcps"][server] = unmanaged["mcps"].get(server, 0) + e["count"]

    # Daily activity across everything managed, for the timeline.
    activity = {"skills": {}, "mcps": {}}
    for v in skill_usage.values():
        merge_days(activity["skills"], v["days"])
    for v in mcp_usage.values():
        merge_days(activity["mcps"], v["days"])

    # Whole source repos often go idle together, and a dead source is one block
    # to delete from kasetto.yaml rather than N rows to prune individually.
    by_source = {}
    for slug, v in skill_usage.items():
        s = by_source.setdefault(v["source"] or "-", {
            "used": 0, "idle": 0, "fresh": 0, "calls": 0, "assets": []})
        s["calls"] += v["total"]
        s["assets"].append(slug)
        if v["total"]:
            s["used"] += 1
        elif v["fresh"]:
            s["fresh"] += 1
        else:
            s["idle"] += 1
    for pack, v in mcp_usage.items():
        s = by_source.setdefault(v["source"] or "-", {
            "used": 0, "idle": 0, "fresh": 0, "calls": 0, "assets": []})
        s["calls"] += v["total"]
        s["assets"].append(pack)
        s["used" if v["total"] else "idle"] += 1

    # Which projects the toolkit is actually exercised in.
    by_project = {}
    for group, kind in ((skill_usage, "skills"), (mcp_usage, "mcps")):
        for name, v in group.items():
            for proj, cnt in v.get("projects", {}).items():
                e = by_project.setdefault(proj, {"calls": 0, "skills": 0, "mcps": 0,
                                                 "assets": []})
                e["calls"] += cnt
                e[kind] += 1
                e["assets"].append(name)

    idle_skills = [k for k, v in skill_usage.items() if not v["total"] and not v["fresh"]]
    fresh_skills = [k for k, v in skill_usage.items() if not v["total"] and v["fresh"]]

    findings = insights(skill_usage, mcp_usage, by_source, unmanaged, coverage,
                        activity, idle_skills, fresh_skills, by_project)

    report = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "insights": findings,
        "coverage": coverage,
        "agents_covered": sorted(covered),
        "counts": {
            "skills_installed": len(skill_usage),
            "skills_used": sum(1 for v in skill_usage.values() if v["total"] > 0),
            "skills_idle": len(idle_skills),
            "skills_fresh": len(fresh_skills),
            "skills_stale": sum(1 for v in skill_usage.values() if v.get("stale")),
            "projects": len(by_project),
            "sessions": sum(v.get("sessions", 0) for v in skill_usage.values())
                        + sum(v.get("sessions", 0) for v in mcp_usage.values()),
            "mcps_installed": len(mcp_usage),
            "mcps_used": sum(1 for v in mcp_usage.values() if v["total"] > 0),
            "mcps_idle": sum(1 for v in mcp_usage.values() if not v["total"]),
            "total_calls": sum(v["total"] for v in skill_usage.values())
                           + sum(v["total"] for v in mcp_usage.values()),
        },
        "activity": activity,
        "by_source": by_source,
        "by_project": by_project,
        "skills": skill_usage,
        "mcps": mcp_usage,
        "unmanaged": unmanaged,
    }

    Path(args.out).write_text(json.dumps(report, indent=2) + "\n")
    try:
        cache_path.parent.mkdir(parents=True, exist_ok=True)
        cache_path.write_text(json.dumps({"version": CACHE_VERSION, "files": cache}))
    except OSError:
        pass

    if not args.quiet:
        c = report["counts"]
        print(f"wrote {args.out}")
        print(f"  agents covered : {', '.join(sorted(covered)) or 'none'}")
        print(f"  skills         : {c['skills_used']}/{c['skills_installed']} used")
        print(f"  mcps           : {c['mcps_used']}/{c['mcps_installed']} used")
    return 0


if __name__ == "__main__":
    sys.exit(main())
