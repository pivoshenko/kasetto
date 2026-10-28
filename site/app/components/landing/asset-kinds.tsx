import type { IconType } from "react-icons";
import { GoArrowDown, GoBook, GoNote, GoPlug, GoTerminal } from "react-icons/go";

// Discovery conventions mirror src/source/mod.rs; destinations are the claude-code and cursor
// presets from src/model/agent.rs, picked because the pair shows the per-agent rewrite
const KINDS: { icon: IconType; name: string; desc: string; from: string; to: string[] }[] = [
  {
    icon: GoBook,
    name: "Skills",
    desc: "A directory with a SKILL.md, copied whole",
    from: "skills/pdf/SKILL.md",
    to: [".claude/skills/pdf/", ".cursor/skills/pdf/"],
  },
  {
    icon: GoTerminal,
    name: "Commands",
    desc: "Markdown, rewritten to each agent's format",
    from: "commands/git/commit.md",
    to: [".claude/commands/git/commit.md", ".gemini/commands/git/commit.toml"],
  },
  {
    icon: GoPlug,
    name: "MCPs",
    desc: "A server definition, merged into agent settings",
    from: "mcps/github.json",
    to: [".mcp.json", ".cursor/mcp.json"],
  },
  {
    icon: GoNote,
    name: "Instructions",
    desc: "Rules, as files or managed blocks in shared ones",
    from: "instructions/testing.md",
    to: ["CLAUDE.md  # managed block", ".cursor/rules/testing.mdc"],
  },
];

export function AssetKinds() {
  return (
    <ol className="lp-kinds">
      {KINDS.map((k) => {
        const Icon = k.icon;
        return (
          <li key={k.name} className="lp-kind">
            <div className="lp-kind-head">
              <Icon aria-hidden />
              <h3 className="lp-kind-title display">{k.name}</h3>
            </div>
            <p className="lp-kind-desc">{k.desc}</p>
            <div className="lp-kind-flow">
              <span className="lp-kind-label">in the source</span>
              <code className="lp-kind-path">{k.from}</code>
              <GoArrowDown className="lp-kind-arrow" aria-hidden />
              <span className="lp-kind-label">installed</span>
              {k.to.map((t) => (
                <code key={t} className="lp-kind-path" data-out="">
                  {t}
                </code>
              ))}
            </div>
          </li>
        );
      })}
    </ol>
  );
}
