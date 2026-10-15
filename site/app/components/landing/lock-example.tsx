// Mirrors the LockFile / SkillEntry / AssetEntry field order in src/lock.rs and
// src/model/types.rs; hashes are sha256 hex, cut short so the pane fits a phone
const LOCK = `version: 3
skills:
  github.com/anthropics/skills::pdf:
    destination: .claude/skills/pdf,.cursor/skills/pdf
    hash: 9b2e41c07d5fa8e3...
    skill: pdf
    description: Read, fill, and merge PDF files
    source: github.com/anthropics/skills
    source_revision: ref:v1.2.0
    scope: project
assets:
  mcp::github.com/pivoshenko/pivoshenko.ai::github.json:
    kind: mcp
    name: github.json
    hash: 31f0c8ae52b97d14...
    source: github.com/pivoshenko/pivoshenko.ai
    destination: github
    source_revision: branch:main
    has_secrets: true`;

function line(text: string) {
  // Keys may contain "::" (asset ids), so split on the first colon followed by a space or EOL
  const m = /^(\s*)(.+?):( .*|)$/.exec(text);
  if (!m) return <span>{text}</span>;
  const [, indent, key, value] = m;
  return (
    <>
      {indent}
      <span className="sy-key">{key}</span>
      <span className="sy-punct">:</span>
      <span className="sy-str">{value}</span>
    </>
  );
}

export function LockExample() {
  return (
    <figure className="lp-pane">
      <figcaption className="lp-pane-bar">
        <span>kasetto.lock</span>
        <span className="lp-pane-tag">committed</span>
      </figcaption>
      <div className="lp-pane-body lp-code">
        {LOCK.split("\n").map((text, i) => (
          <div key={i} className="sy-line">
            <span className="sy-ln" aria-hidden="true">
              {i + 1}
            </span>
            <span>{line(text)}</span>
          </div>
        ))}
      </div>
      <div className="lp-pane-foot">
        <span className="lp-pane-prompt">$</span> kst sync --locked
        <span className="lp-pane-note">no network, fails if the lock drifts from the config</span>
      </div>
    </figure>
  );
}
