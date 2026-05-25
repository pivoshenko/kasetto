"use client";

import { useState } from "react";

const AGENTS = [
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
  { name: "Replit", value: "replit" },
  { name: "Roo Code", value: "roo" },
  { name: "Trae", value: "trae" },
  { name: "Warp", value: "warp" },
  { name: "Windsurf", value: "windsurf" },
];

const INITIAL = 4;

export function AgentsGrid() {
  const [showAll, setShowAll] = useState(false);
  const visible = showAll ? AGENTS : AGENTS.slice(0, INITIAL);
  const remaining = AGENTS.length - INITIAL;

  return (
    <div className="grid-box">
      <div className="agents-header">
        <span>21 PRESETS, BUILT IN</span>
      </div>
      <div className="agents-cells">
        {visible.map((agent) => (
          <div key={agent.value} className="agent-cell">
            {agent.name.toUpperCase()}
          </div>
        ))}
        {showAll &&
          Array.from({ length: (4 - (AGENTS.length % 4)) % 4 }).map((_, i) => (
            <div key={`filler-${i}`} className="agent-cell agent-cell--empty" />
          ))}
      </div>
      {!showAll && (
        <button type="button" className="show-more-row" onClick={() => setShowAll(true)}>
          + SHOW {remaining} MORE
        </button>
      )}
    </div>
  );
}
