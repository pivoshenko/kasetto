const FEATURES = [
  {
    title: "Team baselines",
    desc: "Publish a team config and have every project `extends` it. Everyone starts from the same skills and servers, then layers their own on top.",
  },
  {
    title: "Private and enterprise repos",
    desc: "Pull from GitHub, GitLab, Bitbucket, or self-hosted forges, public or private. Secrets resolve from `kst_*` placeholders at sync time, so they are never committed or locked.",
  },
  {
    title: "Agent-agnostic",
    desc: "Use Claude Code today and Cursor tomorrow, or both. Presets know where each agent keeps its files, so switching never means rebuilding your setup.",
  },
  {
    title: "Four asset kinds",
    desc: "Skills, commands, MCPs, and instructions from one source, transformed into each agent's native format and merged where agents share a file.",
  },
  {
    title: "Fast",
    desc: "Built in Rust. Content is hashed and diffed against the lock, so only what changed is touched. Syncs finish in seconds.",
  },
  {
    title: "Same everywhere",
    desc: "One static binary for macOS, Linux, and Windows. JSON output and real exit codes, so your laptop and CI behave the same.",
  },
];

// Renders `code` spans in feature copy without pulling in a markdown parser
function inlineCode(text: string) {
  return text
    .split("`")
    .map((part, i) => (i % 2 === 1 ? <code key={i}>{part}</code> : <span key={i}>{part}</span>));
}

type Token = { t: string; v?: string };

// kasetto-config:start - generated from kasetto.example.yaml; run `just sync-config`
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
  { t: "cmt", v: '# "*" syncs every skill in the source. Each is a directory with a SKILL.md,' },
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

const TOKEN_CLASS: Record<string, string> = {
  key: "sy-key",
  str: "sy-str",
  url: "sy-url",
  cmt: "sy-cmt",
  dash: "sy-dash",
};

function renderTokens(tokens: Token[]) {
  const lines: React.ReactNode[][] = [[]];
  let key = 0;
  for (const tok of tokens) {
    if (tok.t === "nl") {
      lines.push([]);
    } else {
      lines[lines.length - 1].push(
        <span key={key++} className={TOKEN_CLASS[tok.t] ?? "sy-punct"}>
          {tok.v}
        </span>
      );
    }
  }
  if (lines[lines.length - 1].length === 0) lines.pop();
  return lines.map((line, i) => (
    <div key={i} className="sy-line">
      <span className="sy-ln" aria-hidden="true">
        {i + 1}
      </span>
      <span>{line}</span>
    </div>
  ));
}

export function FeatureList() {
  return (
    <ol className="lp-grid">
      {FEATURES.map((f, i) => (
        <li key={f.title} className="lp-grid-cell">
          <span className="lp-grid-index">{String(i + 1).padStart(2, "0")}</span>
          <h3 className="lp-grid-title display">{f.title}</h3>
          <p className="lp-grid-desc">{inlineCode(f.desc)}</p>
        </li>
      ))}
    </ol>
  );
}

export function ConfigExample() {
  return (
    <figure className="lp-pane lp-pane--scroll">
      <figcaption className="lp-pane-bar">
        <span>kasetto.yaml</span>
        <span className="lp-pane-tag">yaml</span>
      </figcaption>
      <div className="lp-pane-body lp-code">{renderTokens(CONFIG_LINES)}</div>
    </figure>
  );
}
