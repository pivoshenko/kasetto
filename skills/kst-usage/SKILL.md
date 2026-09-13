---
name: kst-usage
description: Report which kasetto-installed skills and MCP servers are actually being used across the AI agents on this machine, and render a branded HTML dashboard of the result. Use whenever the user asks what agent assets they actually use, which skills or MCPs are dead weight, what to prune or clean up from kasetto.yaml, why their context is bloated with unused MCP servers, whether a skill has ever been invoked, or wants a usage report, audit, or dashboard of their agent setup. Also trigger on "kst usage", "what skills do I actually use", "which MCPs are worth keeping", "audit my agent assets", "am I using all these skills", or any request to review, trim, or justify an installed agent toolkit.
---

# kst-usage

Kasetto knows what you installed. Each agent knows what it ran. Nothing on the
machine joins the two, so installed-and-forgotten assets accumulate silently -
skills nobody has ever invoked, MCP servers loading tool definitions into every
request for a server the user stopped using months ago.

This skill closes that loop: read the lock, read each agent's local session
store, join them, and say plainly what is alive and what is dead.

## Run It

Two scripts, both standard-library Python 3, both offline.

```bash
python3 scripts/collect.py --out usage.json        # scan + join
python3 scripts/render.py usage.json --out usage.html
```

`collect.py` is the slow one on a cold cache (a few seconds for a large history)
and near-instant afterwards - it caches per-file results in
`$XDG_CACHE_HOME/kasetto/usage-scan.json` keyed on size and mtime, so a rerun
only touches files the agent appended to. Delete that file to force a full
rescan.

Open the dashboard when it is written, then give the user the short version in
the terminal: how many assets are idle, the biggest surprises, and what you would
cut. The HTML is the artifact; your reading of it is the value.

## The Rule That Matters: Never Overstate Coverage

`collect.py` reports a `coverage` block listing every agent it knows about and
whether it found a readable session store. Honour it.

An asset the user drives daily from an agent that has no provider **looks
completely dead in this report**. If you present "55 skills never used" without
saying which agents that claim covers, and they act on it, they delete something
they rely on. That is the one way this skill can actively hurt someone.

So:

- State the coverage before any count that depends on it. The dashboard carries
  this as a Review insight naming the unread agents rather than a banner, so it
  is easy to scroll past - say it out loud in your summary
- Phrase the finding as "never invoked in *the agents kasetto can read*", not
  "never invoked"
- When a `status: absent` agent is one the user actually uses, say so directly
  and treat the idle list as a shortlist to review rather than a cut list
- A `verified_format: false` provider parsed a store whose layout has not been
  confirmed against a real sample. A zero from it is weak evidence, not proof

## Reading the Output

Start with `insights` - `collect.py` computes the findings worth acting on
rather than leaving them to be read off a chart. Each carries a `severity`, a
`title` already phrased as a claim, a `detail` explaining why it matters, and
the `items` it refers to. Lead your summary with these, in order; they are
already sorted by severity. Then use the rest of the JSON to answer follow-ups.

Severity is a deliberately narrow vocabulary, and it drives colour in the
dashboard:

- `bad` (red) - Concrete waste, meaning a source repo with nothing in use. This
  is the only thing red is spent on, because an unused skill is dormant, not
  broken, and colouring it red would burn the strongest signal on the least
  urgent finding
- `warn` (amber) - Act on it: idle MCP packs, coverage gaps, unverified formats
- `info` (cyan) - Context: concentration, freshly installed assets, unmanaged
  finds
- `good` (green) - Nothing idle

Idle skills are grey throughout. Keep that distinction when you talk about them:
"dormant" and "worth a look", not "bad".

`usage.json` carries more than the dashboard shows. Worth reading directly when
the user asks something specific:

- `skills` / `mcps` - Per asset: `total`, `last_used`, `by_agent`, `days` (a
  date-keyed histogram), plus `source` and `scope` from the lock. Skills carry
  `age` and a `fresh` flag; MCP packs carry `by_server` and `by_tool`
- `by_source` - Per source repo, how many of its assets are live, fresh and idle.
  **Usually the most useful view.** Assets go idle in clusters, because a repo
  gets added for one skill and brings twelve. A wholly idle source is a single
  `source:` block to delete rather than N rows to prune, so lead with this when
  recommending cuts
- `activity` - Daily invocation totals for skills and MCPs, which is what
  separates "used heavily last year" from "used steadily this week"
- `unmanaged` - Names called locally that are not in the lock: installed by hand
  or shipped by the agent. A heavily used one is worth bringing under
  `kasetto.yaml` so it syncs everywhere
- `counts` - Headline numbers, with `skills_idle` and `skills_fresh` already
  separated

An MCP pack can merge several servers, so usage is counted per server and rolled
up. A pack alive on one server and idle on another is worth calling out; that
detail is invisible in the pack-level total.

## Recommending Removals

The idle list is a starting point, not a verdict. Before suggesting a cut:

- Skills cost almost nothing when idle - only their description is loaded until
  invoked. An unused skill is clutter, not a real tax
- **MCP servers are different.** Their tool definitions load into context on
  every single request whether or not you call them. An idle MCP pack is a
  standing cost, so it is the far stronger cut candidate and worth leading with
- An asset installed last week that has not been used yet is not dead, it is new.
  `collect.py` already separates these into `skills_fresh` using a 7-day
  threshold, and the dashboard holds them in their own section. Never move one
  into a cut list because its count is zero
- Some assets exist for rare high-stakes moments (an incident runbook, a release
  procedure). Low count is the design, not a defect

To act on a decision, the existing CLI does it - `kst remove` edits
`kasetto.yaml` in place and preserves comments and key order, then `kst sync`
uninstalls the asset. Do not hand-edit the lock.

## Adding an Agent

The provider list is a table at the top of `scripts/collect.py`. Adding one is a
new entry plus, sometimes, a reader. `references/providers.md` documents each
known agent's store, which are verified against real data, and which are
inferred from documentation.

Two things to hold onto when you extend it:

**Match patterns, not schemas.** Session formats churn - Goose moved JSONL to
SQLite, OpenCode JSON to SQLite, Copilot CLI flat files to per-session
directories. What did *not* move through any of those migrations is the tool-name
string. Matching `mcp__<server>__<tool>` survives a schema rewrite that would
break a field-by-field parser. Resist the urge to "properly" parse these files.

**Anchor on an invocation position.** Session logs also record the tools
*offered* each turn, as arrays of names repeated on every request. An early
version matched the bare `mcp__x__y` string anywhere and reported 38,743 calls
for a server with zero real invocations. Every pattern must require a key
position (`"name": "mcp__..."`), and any new provider needs a sanity check
against an independent count before its numbers are trusted.

## Scope

Skills and MCP servers only. Commands and instructions are deliberately out:
slash commands in the logs are mostly agent built-ins rather than kasetto assets,
and instructions are injected as context and never "invoked", so usage is
undefined for them. If asked about those, explain why rather than guessing.

Everything stays local. The scripts read files and run `kst list --json`; nothing
is uploaded, and the dashboard makes no network requests so it stays readable
offline and safe to share.
