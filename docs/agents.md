# Supported agents

Set the `agent` field in your [config](./configuration.md) and Kasetto handles the rest — skills
are installed to the correct directory for each agent.

## Agent presets

| Agent | Config value | Install path |
| --- | --- | --- |
| AdaL | `adal` | `~/.adal/skills/` |
| Amp | `amp` | `~/.config/agents/skills/` |
| Antigravity | `antigravity` | `~/.gemini/antigravity/skills/` |
| Augment | `augment` | `~/.augment/skills/` |
| Claude Code | `claude-code` | `~/.claude/skills/` |
| Cline | `cline` | `~/.agents/skills/` |
| CodeBuddy | `codebuddy` | `~/.codebuddy/skills/` |
| Codex | `codex` | `~/.codex/skills/` |
| Command Code | `command-code` | `~/.commandcode/skills/` |
| Continue | `continue` | `~/.continue/skills/` |
| Cortex Code | `cortex` | `~/.snowflake/cortex/skills/` |
| Crush | `crush` | `~/.config/crush/skills/` |
| Cursor | `cursor` | `~/.cursor/skills/` |
| Deep Agents | `deepagents` | `~/.deepagents/agent/skills/` |
| Droid | `droid` | `~/.factory/skills/` |
| Gemini CLI | `gemini-cli` | `~/.gemini/skills/` |
| GitHub Copilot | `github-copilot` | `~/.copilot/skills/` |
| Goose | `goose` | `~/.config/goose/skills/` |
| iFlow CLI | `iflow-cli` | `~/.iflow/skills/` |
| Junie | `junie` | `~/.junie/skills/` |
| Kilo Code | `kilo` | `~/.kilocode/skills/` |
| Kimi Code CLI | `kimi-cli` | `~/.config/agents/skills/` |
| Kiro CLI | `kiro-cli` | `~/.kiro/skills/` |
| Kode | `kode` | `~/.kode/skills/` |
| MCPJam | `mcpjam` | `~/.mcpjam/skills/` |
| Mistral Vibe | `mistral-vibe` | `~/.vibe/skills/` |
| Mux | `mux` | `~/.mux/skills/` |
| Neovate | `neovate` | `~/.neovate/skills/` |
| OpenClaw | `openclaw` | `~/.openclaw/skills/` |
| OpenCode | `opencode` | `~/.config/opencode/skills/` |
| OpenHands | `openhands` | `~/.openhands/skills/` |
| Pi | `pi` | `~/.pi/agent/skills/` |
| Pochi | `pochi` | `~/.pochi/skills/` |
| Qoder | `qoder` | `~/.qoder/skills/` |
| Qwen Code | `qwen-code` | `~/.qwen/skills/` |
| Replit | `replit` | `~/.config/agents/skills/` |
| Roo Code | `roo` | `~/.roo/skills/` |
| Trae | `trae` | `~/.trae/skills/` |
| Trae CN | `trae-cn` | `~/.trae-cn/skills/` |
| Universal | `universal` | `~/.config/agents/skills/` |
| Warp | `warp` | `~/.agents/skills/` |
| Windsurf | `windsurf` | `~/.codeium/windsurf/skills/` |
| Zencoder | `zencoder` | `~/.zencoder/skills/` |

## Custom paths

Need an agent that isn't listed? Use the `destination` field to point at any path:

```yaml
destination: ~/.my-custom-agent/skills
```

This overrides the `agent` field if both are set. See the
[configuration reference](./configuration.md#agent-vs-destination) for details.
