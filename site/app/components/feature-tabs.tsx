const FEATURES = [
  {
    title: "DECLARATIVE",
    desc: "One YAML file replaces every manual setup script you have ever written. Define your skills, MCP servers, and agents once — then forget about them. Configs compose with `extends`, so an org base, a team overlay, and a per-project file stay in sync without copy-paste.",
  },
  {
    title: "ENTERPRISE & PRIVATE REPOS",
    desc: "Works with GitHub, GitLab, Bitbucket, Codeberg, Gitea, and self-hosted instances out of the box. Onboard new engineers in one command. Everyone gets the exact same environment — zero drift, zero surprises.",
  },
  {
    title: "MULTI-AGENT",
    desc: "Ship to 21 agents at once — Claude Code, Cursor, Codex, Windsurf, Copilot, and beyond. Stop maintaining separate configs for each tool. One sync, every agent updated, every time.",
  },
  {
    title: "SKILLS, MCP & COMMANDS",
    desc: "Any directory with a SKILL.md is a skill — no registry, no boilerplate. MCP server configs are auto-merged into every supported format, and slash commands transform into each agent's native format. Distribute rules, tools, and prompts as easily as sharing a repo link.",
  },
  {
    title: "SPEED",
    desc: "Built in Rust for instant startup. SHA-256 hashing and lock file diffing mean only what changed gets touched. Full sync across all 21 agents finishes in seconds, not coffee breaks.",
  },
  {
    title: "UNIVERSAL",
    desc: "One static binary — macOS, Linux, Windows. Drop it into CI pipelines with --json output and proper exit codes. Same behavior on a laptop, a Docker container, or a GitHub Actions runner.",
  },
];

type Token = { t: string; v?: string };

const CONFIG_LINES: Token[] = [
  { t: "cmt", v: "# inherit a shared base config — overrides merge on top" },
  { t: "nl" },
  { t: "cmt", v: "# extends: github.com/acme/kasetto-base/raw/main/kasetto.yaml" },
  { t: "nl" },
  { t: "nl" },

  { t: "key", v: "agent" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "str", v: "claude-code" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "str", v: "cursor" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "str", v: "opencode" },
  { t: "nl" },
  { t: "nl" },
  { t: "key", v: "scope" },
  { t: "punct", v: ": " },
  { t: "str", v: "project" },
  { t: "cmt", v: " # or global" },
  { t: "nl" },
  { t: "nl" },
  { t: "cmt", v: "# destination: ./.agents/skills  # optional, override install path" },
  { t: "nl" },
  { t: "nl" },

  { t: "key", v: "skills" },
  { t: "punct", v: ":" },
  { t: "nl" },

  // Source 1: wildcard, no branch/ref
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/acme/frontend-pack" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "skills" },
  { t: "punct", v: ": " },
  { t: "str", v: '"*"' },
  { t: "nl" },
  { t: "nl" },

  // Source 2: branch
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "gitlab.com/team/internal-tools" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "branch" },
  { t: "punct", v: ": " },
  { t: "str", v: "master" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "skills" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "react-patterns" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "go-standards" },
  { t: "nl" },
  { t: "nl" },

  // Source 3: ref + custom paths
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "codeberg.org/oss/shared" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "ref" },
  { t: "punct", v: ": " },
  { t: "str", v: "v2.1.0" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "skills" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "key", v: "name" },
  { t: "punct", v: ": " },
  { t: "str", v: "custom-lint" },
  { t: "nl" },
  { t: "dash", v: "        " },
  { t: "key", v: "path" },
  { t: "punct", v: ": " },
  { t: "str", v: "rules/custom-lint" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "key", v: "name" },
  { t: "punct", v: ": " },
  { t: "str", v: "format-helpers" },
  { t: "nl" },
  { t: "dash", v: "        " },
  { t: "key", v: "path" },
  { t: "punct", v: ": " },
  { t: "str", v: "rules/format" },
  { t: "nl" },
  { t: "nl" },

  { t: "key", v: "mcps" },
  { t: "punct", v: ":" },
  { t: "nl" },

  // MCP 1: wildcard — auto-discover all packs
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/acme/mcp-pack" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "mcps" },
  { t: "punct", v: ': "' },
  { t: "str", v: "*" },
  { t: "punct", v: '"' },
  { t: "nl" },
  { t: "nl" },

  // MCP 2: monorepo — pick by name, resolved from mcps/
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/acme/monorepo" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "ref" },
  { t: "punct", v: ": " },
  { t: "str", v: "v1.4.0" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "mcps" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "github" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "linear" },
  { t: "nl" },
  { t: "nl" },

  { t: "key", v: "commands" },
  { t: "punct", v: ":" },
  { t: "nl" },

  // Commands 1: wildcard — sync every slash command in the repo
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/acme/prompt-pack" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "commands" },
  { t: "punct", v: ': "' },
  { t: "str", v: "*" },
  { t: "punct", v: '"' },
  { t: "nl" },
  { t: "nl" },

  // Commands 2: pick by name
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/acme/monorepo" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "commands" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "review" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "commit" },
  { t: "nl" },
];

function renderTokens(tokens: Token[]) {
  const lines: React.ReactNode[][] = [[]];
  let key = 0;
  for (const tok of tokens) {
    if (tok.t === "nl") {
      lines.push([]);
    } else {
      const cls =
        tok.t === "key"
          ? "sy-key"
          : tok.t === "str"
            ? "sy-str"
            : tok.t === "url"
              ? "sy-url"
              : tok.t === "cmt"
                ? "sy-cmt"
                : tok.t === "dash"
                  ? "sy-dash"
                  : "sy-punct";
      lines[lines.length - 1].push(
        <span key={key++} className={cls}>
          {tok.v}
        </span>
      );
    }
  }
  if (lines[lines.length - 1].length === 0) lines.pop();
  return lines.map((line, i) => (
    <div key={i} className="sy-line">
      <span className="sy-ln">{i + 1}</span>
      <span className="sy-line-content">{line}</span>
    </div>
  ));
}

export function FeatureList() {
  return (
    <div className="grid-box">
      <div className="feat-grid">
        {FEATURES.map((f) => (
          <div key={f.title} className="feat-cell">
            <p className="feat-cell-title">{f.title}</p>
            <p className="feat-cell-desc">{f.desc}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

export function ConfigExample() {
  return (
    <div className="feat-code-block">
      <div className="feat-code-header">kasetto.yaml</div>
      <div className="feat-code-body">{renderTokens(CONFIG_LINES)}</div>
    </div>
  );
}
