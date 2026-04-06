# Supported agents

Set the `agent` field in your [config](./configuration.md) and Kasetto handles the rest — skills
are installed to the correct directory for each agent.

## Agent presets

| Agent | Config value | Install path |
| --- | --- | --- |
| Amp | `amp` | `~/.config/agents/skills/` |
| Antigravity | `antigravity` | `~/.gemini/antigravity/skills/` |
| Augment | `augment` | `~/.augment/skills/` |
| Claude Code | `claude-code` | `~/.claude/skills/` |
| Cline | `cline` | `~/.agents/skills/` |
| Codex | `codex` | `~/.codex/skills/` |
| Continue | `continue` | `~/.continue/skills/` |
| Cursor | `cursor` | `~/.cursor/skills/` |
| Gemini CLI | `gemini-cli` | `~/.gemini/skills/` |
| GitHub Copilot | `github-copilot` | `~/.copilot/skills/` |
| Goose | `goose` | `~/.config/goose/skills/` |
| Junie | `junie` | `~/.junie/skills/` |
| Kiro CLI | `kiro-cli` | `~/.kiro/skills/` |
| OpenClaw | `openclaw` | `~/.openclaw/skills/` |
| OpenCode | `opencode` | `~/.config/opencode/skills/` |
| OpenHands | `openhands` | `~/.openhands/skills/` |
| Replit | `replit` | `~/.config/agents/skills/` |
| Roo Code | `roo` | `~/.roo/skills/` |
| Trae | `trae` | `~/.trae/skills/` |
| Warp | `warp` | `~/.agents/skills/` |
| Windsurf | `windsurf` | `~/.codeium/windsurf/skills/` |

## Custom paths

Need an agent that isn't listed? Use the `destination` field to point at any path:

```yaml
destination: ~/.my-custom-agent/skills
```

This overrides the `agent` field if both are set. See the
[configuration reference](./configuration.md#agent-vs-destination) for details.
