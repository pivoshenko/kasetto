const FEATURES = [
  {
    title: "DECLARATIVE",
    desc: "One YAML file, your whole setup — skills, commands, MCPs, instructions, and agents. Apply globally or scope to a project; configs compose with `extends`, so org, team, and project stay in sync.",
  },
  {
    title: "ENTERPRISE & PRIVATE REPOSITORIES",
    desc: "Pull from anywhere — GitHub, GitLab, Bitbucket, self-hosted, public or private. Onboard a new engineer with one command; everyone gets the same environment, zero drift. Ship MCP packs with `kst_*` secret placeholders — injected at sync time from env, a credentials file, or a manager like 1Password, Vault, AWS, GCP, Azure, KeePass, pass, or the macOS Keychain; never committed and never written to the lock.",
  },
  {
    title: "MULTI-AGENT",
    desc: "Write once, ship everywhere. Claude Code, Cursor, Codex, Copilot, and every other major agent — one sync keeps them all current.",
  },
  {
    title: "SKILLS, COMMANDS, MCPS & INSTRUCTIONS",
    desc: "All four asset kinds, one source — skills, commands, MCPs, and instructions. Everything is transformed into each agent's native format, and auto-merged. Distribute instructions, tools, and prompts as easily as sharing a repository link.",
  },
  {
    title: "SPEED",
    desc: "Instant by design. Built in Rust, it hashes content and diffs a lock file so only what changed gets touched — full syncs finish in seconds.",
  },
  {
    title: "UNIVERSAL",
    desc: "One static binary for macOS, Linux, and Windows. CI-friendly with JSON output and real exit codes — same behavior on your laptop or a CI runner.",
  },
];

type Token = { t: string; v?: string };

// kasetto-config:start — generated from kasetto.example.yaml; run `just sync-config`
const CONFIG_LINES: Token[] = [
  {
    t: "cmt",
    v: "# Option A: preset destination by agent (see README for supported agent values)",
  },
  { t: "nl" },
  { t: "key", v: "agent" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "str", v: "codex" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "str", v: "claude-code" },
  { t: "nl" },
  { t: "nl" },
  { t: "cmt", v: "# Option B: manual destination (takes precedence if both are set)" },
  { t: "nl" },
  { t: "cmt", v: "# destination: ./.agents/skills" },
  { t: "nl" },
  { t: "nl" },
  { t: "key", v: "skills" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "cmt", v: '# "*" syncs every skill in the source — each is a directory with a SKILL.md,' },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "cmt", v: "# discovered in the source root or its skills/ subdirectory" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/vercel-labs/next-skills" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "cmt", v: "# ref: v1.0.0   # pin to a tag or commit; omit to track the default branch" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "skills" },
  { t: "punct", v: ": " },
  { t: "str", v: '"*"' },
  { t: "nl" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "cmt", v: "# or list skills by name" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/anthropics/skills" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "skills" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "doc-coauthoring" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "frontend-design" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "pptx" },
  { t: "nl" },
  { t: "nl" },
  { t: "dash", v: "  " },
  {
    t: "cmt",
    v: "# sub-dir: resolve the named skills under this path, e.g. skills/productivity/grill-me/",
  },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/mattpocock/skills" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "sub-dir" },
  { t: "punct", v: ": " },
  { t: "str", v: "skills/productivity" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "skills" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "grill-me" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "caveman" },
  { t: "nl" },
  { t: "nl" },
  { t: "dash", v: "  " },
  {
    t: "cmt",
    v: "# path: a skill in a non-standard location → <path>/<name>/, here skills/engineering/improve-codebase-architecture/",
  },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/mattpocock/skills" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "skills" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "key", v: "name" },
  { t: "punct", v: ": " },
  { t: "str", v: "improve-codebase-architecture" },
  { t: "nl" },
  { t: "dash", v: "        " },
  { t: "key", v: "path" },
  { t: "punct", v: ": " },
  { t: "str", v: "skills/engineering" },
  { t: "nl" },
  { t: "nl" },
  { t: "key", v: "commands" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "  " },
  {
    t: "cmt",
    v: "# names resolve to commands/<name>.md in the source (nested dirs namespace, e.g. git:commit)",
  },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/gsd-build/get-shit-done" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "commands" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "key", v: "gsd" },
  { t: "punct", v: ": " },
  { t: "str", v: "explore" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "key", v: "gsd" },
  { t: "punct", v: ": " },
  { t: "str", v: "fast" },
  { t: "nl" },
  { t: "nl" },
  { t: "key", v: "instructions" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "  " },
  {
    t: "cmt",
    v: "# instructions wire CLAUDE.md / .cursor/rules / AGENTS.md etc. from instructions/<name>.{md,mdc}",
  },
  { t: "nl" },
  { t: "dash", v: "  " },
  {
    t: "cmt",
    v: '# "*" syncs every instruction; aggregate files (CLAUDE.md, AGENTS.md) get managed blocks',
  },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/pivoshenko/pivoshenko.ai" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "instructions" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "docs-autoupdate" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "multi-agent-dispatch" },
  { t: "nl" },
  { t: "nl" },
  { t: "key", v: "mcps" },
  { t: "punct", v: ":" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "cmt", v: "# names resolve to mcps/<name>.json in the source" },
  { t: "nl" },
  { t: "dash", v: "  " },
  { t: "punct", v: "- " },
  { t: "key", v: "source" },
  { t: "punct", v: ": " },
  { t: "url", v: "github.com/pivoshenko/pivoshenko.ai" },
  { t: "nl" },
  { t: "dash", v: "    " },
  { t: "key", v: "branch" },
  { t: "punct", v: ": " },
  { t: "str", v: "main" },
  { t: "cmt", v: "   # track a specific branch (use ref: to pin a tag or commit)" },
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
  { t: "str", v: "vercel" },
  { t: "nl" },
  { t: "dash", v: "      " },
  { t: "punct", v: "- " },
  { t: "str", v: "kaggle" },
  { t: "nl" },
];
// kasetto-config:end

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
