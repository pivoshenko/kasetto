# Agent Session Stores

Where each agent keeps local session history, and how far the claim is verified.
Read this before adding or fixing a provider in `scripts/collect.py`.

## Status of Each Provider

`verified` means the path and format were confirmed against a real session store
and the resulting counts were checked against an independent count. `inferred`
means the path and format come from documentation or published research and have
not been run against real data - those providers set `verified_format: false`, and
the dashboard labels their rows so a zero from them is not read as proof.

| agent | store | format | status |
|---|---|---|---|
| `claude-code` | `~/.claude/projects/<encoded-cwd>/<id>.jsonl` | JSONL | verified |
| `opencode` | `~/.local/share/opencode/opencode.db` | SQLite | verified |
| `codex` | `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl` (default `~/.codex`) | JSONL | inferred |
| `cursor` | `~/.cursor/projects/*/agent-transcripts/**/*.jsonl` | JSONL | inferred |
| `github-copilot` | `~/.copilot/session-state/<id>/events.jsonl` | JSONL | inferred |
| `goose` | `~/.local/share/goose/sessions.db` | SQLite | inferred |

Every other agent in kasetto's roster is unmapped. That is "nobody has checked",
not "impossible" - most were simply not installed on the machine where this was
built. Adding one means finding its store, confirming tool calls are recoverable,
and checking the counts against a manual grep before marking it verified.

## Details Worth Knowing

**Claude Code.** Skill invocations appear as `"skill":"<name>"` inside the `Skill`
tool's input. MCP calls appear as tool names shaped `mcp__<server>__<tool>`. Both
are reliable. History can run to hundreds of megabytes, which is why the
per-file cache exists.

**OpenCode.** The `part` table holds one row per event with the payload in a
`data` text column of JSON; `json_extract(data,'$.type')='tool'` selects tool
calls and `$.tool` is the name. Migrated from per-file JSON to SQLite in v1.2.
Builtin tools (`edit`, `read`, `bash`) dominate, so a machine with OpenCode
installed but no MCP or skill use reports a working provider with zero findings -
which is correct, and deliberately renders differently from a missing provider.

**Codex.** `CODEX_HOME` relocates the whole tree, so never hardcode `~/.codex`.
The project is explicitly tolerant of schema drift and field names vary between
client versions, which is another argument for pattern matching over parsing.

**Cursor.** Splits storage. The agent transcripts are readable JSONL. The IDE
chat lives in protobuf blobs inside `state.vscdb` under `workspaceStorage` and is
not decoded by anything public - so Cursor coverage is real but partial, and a
user who works mainly in the IDE chat will look less active than they are.

**Goose.** Moved from per-session JSONL to a `sessions.db` SQLite database in
1.10.0. Legacy JSONL files may still sit in `~/.local/share/goose/sessions/`
untouched. The SQLite schema is not pinned here; the reader discovers plausible
(timestamp, payload) column pairs instead of guessing table names.

**Copilot CLI.** Restructured from flat JSONL to per-session directories in
v1.0.11, so the glob has to cover both shapes.

## Why This Churns

Three of the six formats above changed shape recently - Goose, OpenCode and
Copilot CLI all migrated within a short window. These are internal formats with
no compatibility contract and no deprecation notice, so treat any provider here
as a moving target.

That churn is the reason this lives in a skill rather than in the kasetto binary.
Fixing a drifted format is editing a file that reaches users on their next `kst
sync`, instead of cutting a release and waiting for everyone to upgrade.

It is also why the readers match tool-name strings rather than parsing records.
Every one of those three migrations changed the record structure. None of them
changed what a tool call is named.
