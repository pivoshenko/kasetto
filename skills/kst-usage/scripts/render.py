#!/usr/bin/env python3
"""Render usage.json as a self-contained, kasetto-branded HTML dashboard.

Palette and roles come from `src/colors.rs`, which the site also derives from,
so the dashboard reads as the same tool as the CLI. The role semantics carry
over directly: SUCCESS marks what is alive, ERROR marks removal candidates,
ATTENTION marks coverage gaps and freshly installed assets, INFO labels sources
and unmanaged finds, BRAND violet is reserved for the wordmark, and INFRA draws
structure only - never content.

Layout is a 12-column panel grid with a sticky table-of-contents rail, rather
than a stack of sections. Panels render at full height - nothing scrolls inside
a panel - so the page is complete when printed or saved, and the rail carries
navigation instead.

Charts are hand-rolled inline SVG. No chart library, no CDN, no webfont fetch:
the output is one file that opens offline and can be mailed as-is.
"""

import argparse
import html
import json
import re
import sys
from datetime import date, timedelta
from pathlib import Path

# == Palette (src/colors.rs is the source of truth) ==
C = {
    "crust": "#151514", "base": "#1f1f1e", "mantle": "#1a1a19",
    "s0": "#262625", "s1": "#2e2e2c", "s2": "#373634",
    "text": "#e4e2de", "sub": "#b8b3a8", "secondary": "#a8a195", "infra": "#6e6759",
    "attention": "#e8a94d", "success": "#84c578", "error": "#e87e6c",
    "info": "#6cbfd3", "brand": "#b6a6ef",
}

WORDMARK = r"""██╗  ██╗ █████╗ ███████╗███████╗████████╗████████╗ ██████╗
██║ ██╔╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝╚══██╔══╝██╔═══██╗
█████╔╝ ███████║███████╗█████╗     ██║      ██║   ██║   ██║
██╔═██╗ ██╔══██║╚════██║██╔══╝     ██║      ██║   ██║   ██║
██║  ██╗██║  ██║███████║███████╗   ██║      ██║   ╚██████╔╝
╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝      ╚═╝    ╚═════╝"""

CSS = """
*{box-sizing:border-box}
html,body{margin:0;padding:0}
body{background:%(crust)s;color:%(text)s;
  font-family:'JetBrains Mono',Menlo,Consolas,'Noto Sans Mono',monospace;
  font-size:12.5px;line-height:1.55;-webkit-font-smoothing:antialiased}
.shell{max-width:1440px;margin:0 auto;padding-left:20px;padding-right:20px}
code{background:%(s0)s;border:1px solid %(s1)s;border-radius:3px;padding:0 4px;font-size:11.5px}

/* == Header == */
header.site{background:%(mantle)s;border-bottom:1px solid %(s2)s;padding:16px 0}
.hrow{display:flex;align-items:center;gap:22px;flex-wrap:wrap}
.mark{border:1px solid %(brand)s;border-radius:3px;padding:8px 11px;flex:0 0 auto}
.mark pre{margin:0;color:%(brand)s;font-size:6px;line-height:1.1;font-weight:700}
.htitle{flex:1 1 220px;min-width:180px}
.htitle .t{font-size:16px;font-weight:700;letter-spacing:-.01em;line-height:1.3}
.hmeta{flex:0 0 auto;text-align:right;color:%(infra)s;font-size:11px;line-height:1.8}
.hmeta b{color:%(sub)s;font-weight:400}

/* == Grid == */
main{padding-top:28px;padding-bottom:44px}
.layout{display:flex;gap:22px;align-items:flex-start}
.grid{display:grid;grid-template-columns:repeat(12,1fr);gap:12px;flex:1 1 auto;min-width:0}

/* == TOC == */
.toc{position:sticky;top:18px;flex:0 0 176px;width:176px}
.toc-h{color:%(infra)s;font-size:9.5px;letter-spacing:.12em;text-transform:uppercase;
  padding:0 0 7px;border-bottom:1px solid %(s1)s;margin-bottom:6px}
.toc a{display:block;color:%(secondary)s;text-decoration:none;font-size:11.5px;
  padding:3px 9px;border-left:2px solid transparent;line-height:1.45}
.toc a:hover{color:%(text)s;background:%(s0)s}
.toc a.on{color:%(text)s;border-left-color:%(attention)s}
@media(max-width:1080px){.toc{display:none}}
.c3{grid-column:span 3}.c4{grid-column:span 4}.c5{grid-column:span 5}
.c6{grid-column:span 6}.c7{grid-column:span 7}.c8{grid-column:span 8}
.c9{grid-column:span 9}.c12{grid-column:span 12}
@media(max-width:1080px){.c3,.c4,.c5,.c6,.c7{grid-column:span 6}
  .c8,.c9{grid-column:span 12}}
@media(max-width:680px){.grid>*{grid-column:span 12 !important}.mark{display:none}}

/* == Panel == */
.panel{background:%(base)s;border:1px solid %(s1)s;border-radius:4px;display:flex;
  flex-direction:column;overflow:hidden;min-width:0}
.phead{display:flex;align-items:baseline;justify-content:space-between;gap:10px;
  padding:9px 13px;border-bottom:1px solid %(s1)s;background:%(mantle)s;flex:0 0 auto}
.phead h2{margin:0;font-size:12px;font-weight:700;letter-spacing:0;color:%(text)s}
.phead .hint{color:%(infra)s;font-size:10.5px;text-align:right;white-space:nowrap}
.pbody{padding:13px;overflow:auto;flex:1 1 auto;min-height:0}
.pbody.flush{padding:0}
.mix-i{text-align:center;padding:16px 8px 14px}
.mix-i+.mix-i{border-top:1px solid %(s1)s}
.mix-l{color:%(sub)s;font-size:11.5px;margin-top:4px}

/* == KPI strip == */
.kstrip{display:grid;grid-template-columns:repeat(5,1fr);gap:12px}
@media(max-width:1080px){.kstrip{grid-template-columns:repeat(2,1fr)}}
@media(max-width:520px){.kstrip{grid-template-columns:1fr}}
.kpi{background:%(base)s;border:1px solid %(s1)s;border-radius:4px;padding:13px 15px}
.kpi .n{font-size:26px;font-weight:700;line-height:1.1;font-variant-numeric:tabular-nums}
.kpi .l{color:%(sub)s;font-size:11px;margin-top:3px}
.kpi .s{color:%(infra)s;font-size:10.5px}

/* == Tables == */
table{width:100%%;border-collapse:collapse}
th{text-align:left;font-weight:400;font-size:10.5px;letter-spacing:.02em;
  color:%(infra)s;padding:8px 12px;border-bottom:1px solid %(s1)s;white-space:nowrap}
td{padding:6px 12px;border-bottom:1px solid %(s0)s;vertical-align:middle}
tbody tr:last-child td{border-bottom:none}
tbody tr:hover td{background:%(s0)s}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.dim{color:%(secondary)s}.faint{color:%(infra)s}
.src{color:%(info)s;font-size:10.5px}
.nm{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:210px;display:block}

/* == Bars == */
.track{background:%(s1)s;border-radius:2px;height:7px;width:100%%;min-width:60px;
  overflow:hidden;display:flex}
.track i{height:7px;display:block}
.legend{display:flex;gap:14px;flex-wrap:wrap;color:%(secondary)s;font-size:10.5px;
  margin-top:10px;justify-content:center}
.legend span{display:flex;align-items:center;gap:5px}
.sw{width:8px;height:8px;border-radius:2px;display:inline-block}

.tag{display:inline-block;border:1px solid;border-radius:3px;padding:0 5px;
  font-size:10px;line-height:16px;white-space:nowrap}
.ok{color:%(success)s;border-color:%(success)s}
.dead{color:%(error)s;border-color:%(error)s}
.warn{color:%(attention)s;border-color:%(attention)s}
.nfo{color:%(info)s;border-color:%(info)s}
.idle{color:%(secondary)s;border-color:%(s2)s}

.more{padding:8px 12px;color:%(infra)s;font-size:10.5px;border-top:1px solid %(s1)s}

/* == Insights == */
.ins{border-left:2px solid;border-bottom:1px solid %(s0)s;padding:11px 14px}
.ins:last-child{border-bottom:none}
.ins-h{display:flex;align-items:baseline;gap:9px;flex-wrap:wrap}
.ins-b{border:1px solid;border-radius:3px;padding:0 6px;font-size:9.5px;line-height:15px;
  letter-spacing:.05em;flex:0 0 auto}
.ins-t{font-weight:700;font-size:12.5px}
.ins-d{margin:4px 0 0;color:%(sub)s;font-size:11.5px}
.ins-i{margin-top:7px;line-height:1.9}

footer.site{border-top:1px solid %(s1)s;padding:20px 0 40px;color:%(infra)s;font-size:10.5px}
"""


def esc(s):
    return html.escape(str(s if s is not None else ""))


def short(src):
    if not src or src == "-":
        return "-"
    s = str(src).rstrip("/")
    return "/".join(s.split("/")[-2:]) if "/" in s else s


AGE_RE = re.compile(r"(\d+)\s*([smhdwy])")
AGE_UNIT = {"s": 0.0, "m": 0.0, "h": 1 / 24, "d": 1.0, "w": 7.0, "y": 365.0}


def age_rank(age):
    """Install age in days from `kst list`'s "12h ago" / "35d ago" rendering."""
    m = AGE_RE.search(age or "")
    return int(m.group(1)) * AGE_UNIT[m.group(2)] if m else 0.0


def day_only(ts):
    return ts.split("T")[0] if ts else "Never"


def slug(title):
    return re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")


def panel(title, body, hint="", cls="c6", body_cls=""):
    h = f'<div class="hint">{hint}</div>' if hint else ""
    return (f'<section id="{slug(title)}" class="panel {cls}">'
            f'<div class="phead"><h2>{title}</h2>{h}</div>'
            f'<div class="pbody {body_cls}">{body}</div></section>')


# == Charts ==

def svg_timeline(skills, mcps, height=132):
    """Daily invocation volume, skills stacked over MCP calls.

    Empty days are drawn as gaps rather than skipped, because "nothing happened
    for two weeks" is the signal an evenly spaced series would hide.
    """
    width = 900
    days = sorted(set(skills) | set(mcps))
    if not days:
        return '<p class="faint" style="margin:0">No dated activity recorded.</p>'

    start, end = date.fromisoformat(days[0]), date.fromisoformat(days[-1])
    span = [(start + timedelta(days=i)).isoformat() for i in range((end - start).days + 1)]
    span = span[-180:]

    pad_l, pad_b, pad_t = 30, 18, 6
    pw, ph = width - pad_l - 6, height - pad_b - pad_t
    top = max([skills.get(d, 0) + mcps.get(d, 0) for d in span] + [1])
    step = pw / len(span)
    bw = max(1.5, step - 1.4)

    o = [f'<svg viewBox="0 0 {width} {height}" width="100%" height="{height}" '
         f'preserveAspectRatio="none" role="img" aria-label="daily invocations">']
    for frac in (0, 0.5, 1):
        y = pad_t + ph * (1 - frac)
        o.append(f'<line x1="{pad_l}" y1="{y:.1f}" x2="{width-6}" y2="{y:.1f}" '
                 f'stroke="{C["s1"]}" stroke-width="1"/>')
        o.append(f'<text x="{pad_l-6}" y="{y+3.5:.1f}" text-anchor="end" font-size="9" '
                 f'fill="{C["infra"]}" font-family="monospace">{round(top*frac)}</text>')
    for i, dd in enumerate(span):
        s, m = skills.get(dd, 0), mcps.get(dd, 0)
        if not (s or m):
            continue
        x = pad_l + i * step
        hm, hs = ph * m / top, ph * s / top
        if m:
            o.append(f'<rect x="{x:.1f}" y="{pad_t+ph-hm:.1f}" width="{bw:.1f}" height="{hm:.1f}" '
                     f'fill="{C["info"]}"><title>{dd}: {m} mcp</title></rect>')
        if s:
            o.append(f'<rect x="{x:.1f}" y="{pad_t+ph-hm-hs:.1f}" width="{bw:.1f}" height="{hs:.1f}" '
                     f'fill="{C["success"]}"><title>{dd}: {s} skill</title></rect>')
    o.append(f'<text x="{pad_l}" y="{height-4}" font-size="9" fill="{C["infra"]}" '
             f'font-family="monospace">{span[0]}</text>')
    o.append(f'<text x="{width-6}" y="{height-4}" text-anchor="end" font-size="9" '
             f'fill="{C["infra"]}" font-family="monospace">{span[-1]}</text>')
    o.append('</svg>')
    return "".join(o)


def svg_donut(segments, size=118, thickness=14):
    """Flat composition ring. segments = [(label, value, color)]."""
    total = sum(v for _, v, _ in segments) or 1
    r = (size - thickness) / 2
    circ = 2 * 3.141592653589793 * r
    cx = cy = size / 2
    o = [f'<svg viewBox="0 0 {size} {size}" width="{size}" height="{size}" role="img">',
         f'<circle cx="{cx}" cy="{cy}" r="{r:.2f}" fill="none" stroke="{C["s1"]}" '
         f'stroke-width="{thickness}"/>']
    off = 0.0
    for label, val, col in segments:
        if not val:
            continue
        frac = val / total
        o.append(f'<circle cx="{cx}" cy="{cy}" r="{r:.2f}" fill="none" stroke="{col}" '
                 f'stroke-width="{thickness}" stroke-dasharray="{circ*frac:.2f} {circ:.2f}" '
                 f'stroke-dashoffset="{-circ*off:.2f}" transform="rotate(-90 {cx} {cy})">'
                 f'<title>{esc(label)}: {val}</title></circle>')
        off += frac
    live = segments[0][1] if segments else 0
    o.append(f'<text x="{cx}" y="{cy}" text-anchor="middle" font-size="20" font-weight="700" '
             f'fill="{C["text"]}" font-family="monospace">{live}</text>')
    o.append(f'<text x="{cx}" y="{cy+13}" text-anchor="middle" font-size="8" '
             f'fill="{C["infra"]}" font-family="monospace">Of {total}</text>')
    o.append('</svg>')
    return "".join(o)


def stacked(parts):
    """Inline composition bar. parts = [(value, color, title)]."""
    total = sum(p[0] for p in parts) or 1
    cells = "".join(f'<i style="width:{100*v/total:.2f}%;background:{col}" title="{esc(t)}"></i>'
                    for v, col, t in parts if v)
    return f'<span class="track">{cells}</span>'


def legend(items):
    return ('<div class="legend">' + "".join(
        f'<span><i class="sw" style="background:{col}"></i>{esc(l)}</span>'
        for l, col in items) + '</div>')


def table(head, rows, sticky=True):
    th = "".join(f'<th class="{c}">{t}</th>' for t, c in head)
    thead = f'<thead><tr>{th}</tr></thead>' if sticky else f'<tr>{th}</tr>'
    return f'<table>{thead}<tbody>{"".join(rows)}</tbody></table>'


# == Page ==

def render(d):
    c = d["counts"]
    skills, mcps = d["skills"], d["mcps"]
    used = {k: v for k, v in skills.items() if v["total"]}
    fresh = {k: v for k, v in skills.items() if not v["total"] and v.get("fresh")}
    idle = {k: v for k, v in skills.items() if not v["total"] and not v.get("fresh")}
    mcp_idle = {k: v for k, v in mcps.items() if not v["total"]}

    o = []
    A = o.append
    A(f'<!doctype html><html lang="en"><head><meta charset="utf-8">'
      f'<meta name="viewport" content="width=device-width,initial-scale=1">'
      f'<meta name="color-scheme" content="dark"><meta name="theme-color" content="{C["crust"]}">'
      f'<title>Kasetto Usage</title><style>{CSS % C}</style></head><body>')

    A(f'<header class="site"><div class="shell hrow">'
      f'<div class="mark"><pre>{esc(WORDMARK)}</pre></div>'
      f'<div class="htitle"><div class="t">Kasetto Usage</div></div>'
      f'<div class="hmeta">Generated <b>{esc(d["generated_at"][:16].replace("T", " "))}</b></div>'
      f'</div></header><main class="shell"><div class="layout"><div class="grid">')

    # Coverage is not banner-ed here: `insights` already carries it as a Review
    # finding naming the unread agents, and repeating it above the fold was the
    # third telling on the same page.

    # == KPI strip ==
    A('<div id="overview" class="c12 kstrip">')
    for n, lab, sub, col in [
        (c.get("total_calls", 0), "Invocations", "Across all agents", C["text"]),
        (f'{c["skills_used"]}/{c["skills_installed"]}', "Skills Live", "Invoked at least once", C["success"]),
        (c.get("skills_idle", 0), "Skills Idle", "Older than 7d, never called",
         C["secondary"]),
        (c.get("skills_fresh", 0), "Skills New", "Newer than 7d, too soon to judge", C["info"]),
        (f'{c.get("mcps_idle", 0)}/{c["mcps_installed"]}', "MCP Packs Idle", "Standing context cost",
         C["attention"] if c.get("mcps_idle") else C["success"]),
    ]:
        A(f'<div class="kpi"><div class="n" style="color:{col}">{n}</div>'
          f'<div class="l">{lab}</div><div class="s">{sub}</div></div>')
    A('</div>')

    # == Insights: the read, stated before the charts that support it ==
    sev = {"bad": C["error"], "warn": C["attention"], "info": C["info"], "good": C["success"]}
    words = {"bad": "Waste", "warn": "Review", "info": "Note", "good": "Clear"}
    cards = []
    for ins in d.get("insights", []):
        col = sev.get(ins["severity"], C["secondary"])
        items = ("".join(f'<span class="tag" style="color:{col};border-color:{col}">'
                         f'{esc(i)}</span> ' for i in ins.get("items", []))
                 if ins.get("items") else "")
        cards.append(
            f'<div class="ins" style="border-left-color:{col}">'
            f'<div class="ins-h"><span class="ins-b" style="color:{col};border-color:{col}">'
            f'{words.get(ins["severity"], "Note")}</span>'
            f'<span class="ins-t">{esc(ins["title"])}</span></div>'
            f'<p class="ins-d">{esc(ins["detail"])}</p>'
            f'{f"<div class=ins-i>{items}</div>" if items else ""}</div>')
    if cards:
        A(panel("Insights", "".join(cards), hint="Computed, not guessed",
                cls="c12", body_cls="flush"))

    # == Activity ==
    # Full width: coverage lives in the banner and the insights, so the timeline
    # gets the whole row rather than sharing it with a table that repeats them.
    A(panel("Activity", svg_timeline(d["activity"]["skills"], d["activity"]["mcps"])
            + legend([("Skill invocations", C["success"]), ("MCP tool calls", C["info"])]),
            hint="Daily invocations", cls="c12"))

    # == Composition ==
    # Both rings in one panel, stacked, so skills and MCP packs are read as two
    # views of the same portfolio rather than two unrelated widgets.
    mix = (
        '<div class="mix-i">'
        + svg_donut([("live", len(used), C["success"]), ("new", len(fresh), C["info"]),
                     ("idle", len(idle), C["secondary"])])
        + '<div class="mix-l">Skills</div>'
        + legend([("Live", C["success"]), ("New", C["info"]), ("Idle", C["secondary"])])
        + '</div><div class="mix-i">'
        + svg_donut([("live", c["mcps_used"], C["success"]),
                     ("idle", c.get("mcps_idle", 0), C["attention"])])
        + '<div class="mix-l">MCP Packs</div>'
        + legend([("Live", C["success"]), ("Idle", C["attention"])])
        + '</div>')
    A(panel("Asset Mix", mix, hint="Skills and MCP packs", cls="c3", body_cls="flush"))

    # == Top skills ==
    if used:
        top = max(v["total"] for v in used.values())
        rows = []
        for k, v in sorted(used.items(), key=lambda x: -x[1]["total"]):
            agents = ", ".join(f"{a}&nbsp;{n}" for a, n in sorted(v["by_agent"].items()))
            # A stale asset reads as live on every other view; flag it where it shows.
            lastc = C["attention"] if v.get("stale") else C["secondary"]
            since = v.get("days_since")
            ago = f"{since}d ago" if since is not None else "Never"
            rows.append(
                f'<tr><td><span class="nm">{esc(k)}</span></td>'
                f'<td style="width:26%">{stacked([(v["total"], C["success"], "{} calls".format(v["total"])), (top - v["total"], C["s1"], "")])}</td>'
                f'<td class="num" style="color:{C["success"]}">{v["total"]}</td>'
                f'<td class="num dim">{v.get("sessions", 0)}</td>'
                f'<td style="color:{lastc}">{esc(ago)}</td>'
                f'<td class="faint">{agents}</td></tr>')
        body = table([("Skill", ""), ("Calls", ""), ("N", "num"), ("Sessions", "num"),
                      ("Last Used", ""), ("Agents", "")], rows)
    else:
        body = '<p class="faint" style="margin:0">No installed skill was invoked.</p>'
    A(panel("Skills in Use", body, hint=f"Live: {len(used)}", cls="c9", body_cls="flush"))

    # == By source ==
    rows = []
    for src, s in sorted(d["by_source"].items(), key=lambda x: (-x[1]["idle"], -x[1]["calls"])):
        allde = s["used"] == 0 and s["fresh"] == 0
        nm = (f'<span class="nm" style="color:{C["error"]}">{esc(short(src))}</span>'
              if allde else f'<span class="nm">{esc(short(src))}</span>')
        rows.append(
            f'<tr><td>{nm}</td><td style="width:30%">'
            f'{stacked([(s["used"], C["success"], "live"), (s["fresh"], C["info"], "new"), (s["idle"], C["secondary"], "idle")])}</td>'
            f'<td class="num" style="color:{C["success"]}">{s["used"] or ""}</td>'
            f'<td class="num" style="color:{C["info"]}">{s["fresh"] or ""}</td>'
            f'<td class="num dim">{s["idle"] or ""}</td>'
            f'<td class="num dim">{s["calls"]}</td></tr>')
    A(panel("By Source", table([("Source", ""), ("Composition", ""), ("Live", "num"),
                                ("New", "num"), ("Idle", "num"), ("Calls", "num")], rows),
            hint="A fully idle repo is one config block", cls="c6", body_cls="flush"))

    # == By project ==
    proj = d.get("by_project") or {}
    if proj:
        ptop = max(v["calls"] for v in proj.values())
        rows = []
        for name, v in sorted(proj.items(), key=lambda x: -x[1]["calls"]):
            rows.append(
                f'<tr><td><span class="nm">{esc(name)}</span></td>'
                f'<td style="width:30%">{stacked([(v["calls"], C["success"], str(v["calls"])), (ptop - v["calls"], C["s1"], "")])}</td>'
                f'<td class="num" style="color:{C["success"]}">{v["calls"]}</td>'
                f'<td class="num dim">{v["skills"]}</td>'
                f'<td class="num dim">{v["mcps"]}</td></tr>')
        A(panel("By Project", table([("Project", ""), ("Calls", ""), ("N", "num"),
                                     ("Skills", "num"), ("MCPs", "num")], rows),
                hint="Where the toolkit gets used", cls="c6", body_cls="flush"))

    # == MCP detail ==
    rows = []
    for k, v in sorted(mcps.items(), key=lambda x: -x[1]["total"]):
        col = C["success"] if v["total"] else C["attention"]
        tools = v.get("by_tool") or {}
        tl = (" ".join(f'<span class="tag ok">{esc(t)}&nbsp;{n}</span>'
                       for t, n in sorted(tools.items(), key=lambda x: -x[1])[:6])
              or '<span class="faint">None called</span>')
        rows.append(f'<tr><td><span class="nm">{esc(k)}</span></td>'
                    f'<td class="num" style="color:{col}">{v["total"]}</td>'
                    f'<td class="dim">{esc(day_only(v["last_used"]))}</td>'
                    f'<td>{tl}</td></tr>')
    A(panel("MCP Packs",
            table([("Pack", ""), ("Calls", "num"), ("Last", ""), ("Tools Used", "")], rows),
            hint="Idle packs cost context every request", cls="c6", body_cls="flush"))

    # == Cut candidates ==
    # Every candidate has zero calls, so usage cannot rank them. MCP packs go
    # first because they cost context on every request, then skills by longest
    # time installed - the ones that have had the most chance to be used.
    if idle or mcp_idle:
        ranked = ([(k, v, "MCP") for k, v in mcp_idle.items()]
                  + sorted(((k, v, "Skill") for k, v in idle.items()),
                           key=lambda x: -age_rank(x[1].get("age"))))
        total_cut = len(ranked)
        rows = []
        for k, v, kind in ranked[:5]:
            tag = "warn" if kind == "MCP" else "idle"
            rows.append(f'<tr><td><span class="nm">{esc(k)}</span></td>'
                        f'<td><span class="tag {tag}">{kind}</span></td>'
                        f'<td class="dim">{esc(v.get("age") or "-")}</td>'
                        f'<td class="src">{esc(short(v.get("source")))}</td></tr>')
        body = table([("Asset", ""), ("Kind", ""), ("Installed", ""), ("Source", "")], rows)
        # Never let a truncated list read as the whole list.
        if total_cut > 5:
            body += (f'<div class="more">{total_cut - 5} more not shown &middot; '
                     f'full list in usage.json</div>')
        A(panel("Cut Candidates", body,
                hint=f"Top 5 of {total_cut}", cls="c6", body_cls="flush"))
    else:
        A(panel("Cut Candidates", '<p class="faint" style="margin:0">Nothing installed is idle.</p>',
                cls="c6"))

    # == Unmanaged ==
    um_s, um_m = d["unmanaged"]["skills"], d["unmanaged"]["mcps"]
    if um_s or um_m:
        allrows = ([(k, "MCP", n) for k, n in um_m.items()]
                   + [(k, "Skill", n) for k, n in um_s.items()])
        top = max([n for _, _, n in allrows] + [1])
        rows = [f'<tr><td><span class="nm">{esc(k)}</span></td>'
                f'<td><span class="tag nfo">{kind}</span></td>'
                f'<td style="width:30%">{stacked([(n, C["info"], str(n)), (top - n, C["s1"], "")])}</td>'
                f'<td class="num dim">{n}</td></tr>'
                for k, kind, n in sorted(allrows, key=lambda x: -x[2])[:24]]
        A(panel("Not Managed by Kasetto",
                table([("Name", ""), ("Kind", ""), ("Calls", ""), ("N", "num")], rows),
                hint="Candidates to bring under kasetto.yaml", cls="c6",
                body_cls="flush"))

    # == TOC ==
    # Built by reading back what was actually emitted, so a panel that was
    # skipped for lack of data can never leave a dead link in the rail.
    emitted = re.findall(r'id="([^"]+)" class="panel[^"]*"><div class="phead"><h2>([^<]+)</h2>',
                         "".join(o))
    links = [('overview', 'Overview')] + emitted
    rail = "".join(f'<a href="#{i}">{esc(t)}</a>' for i, t in links)
    A(f'</div><aside class="toc"><div class="toc-h">Contents</div>{rail}</aside>'
      f'</div></main>')

    # Year comes from the report rather than a literal so the footer cannot go stale.
    year = (d.get("generated_at") or "")[:4] or ""
    A('<footer class="site"><div class="shell">'
      f'<a href="https://kasetto.dev">kasetto.dev</a> &middot; {esc(year)}'
      '</div></footer>')

    # Highlight the rail entry for whatever is on screen. Plain observer, no
    # scroll handler, so it costs nothing while reading.
    A('<script>'
      'var L=[].slice.call(document.querySelectorAll(".toc a")),'
      'M={};L.forEach(function(a){var e=document.getElementById(a.hash.slice(1));'
      'if(e)M[a.hash.slice(1)]=a;});'
      'var seen={};'
      'var io=new IntersectionObserver(function(es){'
      'es.forEach(function(e){seen[e.target.id]=e.isIntersecting;});'
      'var first=Object.keys(M).filter(function(k){return seen[k];})[0];'
      'L.forEach(function(a){a.classList.toggle("on",a.hash.slice(1)===first);});'
      '},{rootMargin:"-10% 0px -70% 0px"});'
      'Object.keys(M).forEach(function(k){io.observe(document.getElementById(k));});'
      '</script></body></html>')
    return "".join(o)


def main():
    ap = argparse.ArgumentParser(description="Render usage.json as an HTML dashboard.")
    ap.add_argument("usage_json", nargs="?", default="usage.json")
    ap.add_argument("--out", default="usage.html")
    args = ap.parse_args()
    try:
        data = json.loads(Path(args.usage_json).read_text())
    except (OSError, json.JSONDecodeError) as exc:
        print(f"error: cannot read {args.usage_json}: {exc}", file=sys.stderr)
        return 2
    Path(args.out).write_text(render(data))
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
