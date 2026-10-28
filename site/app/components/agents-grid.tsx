import { AgentIcon } from "./landing/agent-icon";

export const AGENTS = [
  { name: "Claude Code", value: "claude-code" },
  { name: "Cursor", value: "cursor" },
  { name: "Codex", value: "codex" },
  { name: "Antigravity", value: "antigravity" },
  { name: "Amp", value: "amp" },
  { name: "Augment", value: "augment" },
  { name: "Cline", value: "cline" },
  { name: "Continue", value: "continue" },
  { name: "Gemini CLI", value: "gemini-cli" },
  { name: "GitHub Copilot", value: "github-copilot" },
  { name: "Goose", value: "goose" },
  { name: "Junie", value: "junie" },
  { name: "Kiro CLI", value: "kiro-cli" },
  { name: "OpenClaw", value: "openclaw" },
  { name: "OpenCode", value: "opencode" },
  { name: "OpenHands", value: "openhands" },
  { name: "Pi", value: "pi" },
  { name: "Replit", value: "replit" },
  { name: "Roo Code", value: "roo" },
  { name: "Trae", value: "trae" },
  { name: "Warp", value: "warp" },
  { name: "Windsurf", value: "windsurf" },
  { name: "ZCode", value: "zcode" },
];

export function AgentsGrid() {
  return (
    <ul className="lp-wall">
      {AGENTS.map((agent) => (
        <li key={agent.value} className="lp-wall-cell">
          <AgentIcon preset={agent.value} name={agent.name} className="lp-wall-icon" />
          <span className="lp-wall-name">{agent.name}</span>
          <code className="lp-wall-preset">{agent.value}</code>
        </li>
      ))}
      <li className="lp-wall-cell lp-wall-cell--ask">
        <a
          href="https://github.com/pivoshenko/kasetto/issues"
          target="_blank"
          rel="noopener noreferrer"
          className="lp-wall-ask"
        >
          <span className="lp-wall-name">Your agent?</span>
          <span className="lp-wall-preset">Open an issue -&gt;</span>
        </a>
      </li>
    </ul>
  );
}
