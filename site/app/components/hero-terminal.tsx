"use client";

import { useEffect, useRef, useState } from "react";

const COMMAND = "kasetto sync";

// status: undefined → unchanged; otherwise updated/added/removed.
type Status =
  | { s: "updated"; v: [string, string] }
  | { s: "added"; v: [string] }
  | { s: "removed" };

const STATUS: Record<string, Status> = {
  "git-commit": { s: "updated", v: ["2.1.0", "2.2.0"] },
  "git-pr-create": { s: "updated", v: ["1.4.2", "1.5.0"] },
  "pivoshenko-brand": { s: "updated", v: ["0.9.0", "1.0.0"] },
  "command:gsd": { s: "added", v: ["1.0.0"] },
  "web-design-guidelines": { s: "updated", v: ["3.0.1", "3.1.0"] },
  "rust-best-practices": { s: "added", v: ["1.0.0"] },
  "grill-me": { s: "removed" },
};

type Group = { repo: string; items: string[] };

const GROUPS: Group[] = [
  {
    repo: "github.com/pivoshenko/pivoshenko.ai",
    items: [
      "git-branch-create",
      "git-branch-sync",
      "git-branches-cleanup",
      "git-commit",
      "git-pr-create",
      "pivoshenko-brand",
      "command:gsd",
      "mcp:github.json",
      "mcp:vercel.json",
    ],
  },
  {
    repo: "github.com/anthropics/skills",
    items: ["design-thinking", "doc-coauthoring", "pdf", "pptx", "skill-creator"],
  },
  {
    repo: "github.com/vercel-labs/agent-skills",
    items: ["deploy-to-vercel", "web-design-guidelines"],
  },
  { repo: "github.com/apollographql/skills", items: ["rust-best-practices"] },
  { repo: "github.com/mattpocock/skills", items: ["grill-me"] },
];

const TOTAL = GROUPS.reduce((n, g) => n + g.items.length, 0);

const COUNTS = (() => {
  let updated = 0;
  let added = 0;
  let removed = 0;
  for (const s of Object.values(STATUS)) {
    if (s.s === "updated") updated++;
    else if (s.s === "added") added++;
    else if (s.s === "removed") removed++;
  }
  return { updated, added, removed, unchanged: TOTAL - updated - added - removed };
})();

function glyphFor(s: Status | undefined): { g: string; cls: string } {
  if (!s) return { g: "✓", cls: "t-faint" };
  if (s.s === "updated") return { g: "↑", cls: "t-amber" };
  if (s.s === "added") return { g: "+", cls: "t-green" };
  return { g: "−", cls: "t-red" };
}

type Phase = "idle" | "typing" | "resolving" | "running" | "done";

export function HeroTerminal() {
  const ref = useRef<HTMLElement>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [typed, setTyped] = useState(0);
  // group-level progress; runs serially through GROUPS, each group fills items then settles
  const [groupIdx, setGroupIdx] = useState(-1);
  const [itemIdx, setItemIdx] = useState(0);
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    const m = window.matchMedia("(prefers-reduced-motion: reduce)");
    setReduced(m.matches);
    const handler = (e: MediaQueryListEvent) => setReduced(e.matches);
    m.addEventListener("change", handler);
    return () => m.removeEventListener("change", handler);
  }, []);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setPhase("typing");
            obs.disconnect();
            break;
          }
        }
      },
      { threshold: 0.25 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  useEffect(() => {
    if (phase !== "typing") return;
    if (reduced) {
      setTyped(COMMAND.length);
      setPhase("done");
      setGroupIdx(GROUPS.length);
      return;
    }
    if (typed >= COMMAND.length) {
      const t = setTimeout(() => setPhase("resolving"), 320);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setTyped((n) => n + 1), 55 + Math.random() * 40);
    return () => clearTimeout(t);
  }, [phase, typed, reduced]);

  useEffect(() => {
    if (phase !== "resolving") return;
    const t = setTimeout(() => {
      setPhase("running");
      setGroupIdx(0);
      setItemIdx(0);
    }, 740);
    return () => clearTimeout(t);
  }, [phase]);

  useEffect(() => {
    if (phase !== "running") return;
    if (groupIdx < 0) return;
    if (groupIdx >= GROUPS.length) {
      setPhase("done");
      return;
    }
    const g = GROUPS[groupIdx];
    if (itemIdx < g.items.length) {
      const t = setTimeout(() => setItemIdx((i) => i + 1), 90);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => {
      setGroupIdx((i) => i + 1);
      setItemIdx(0);
    }, 180);
    return () => clearTimeout(t);
  }, [phase, groupIdx, itemIdx]);

  const showCursor = phase === "typing";
  const summaryVisible = phase === "done";
  const resolvedVisible = phase === "resolving" || phase === "running" || phase === "done";

  return (
    <figure className="hero-terminal" ref={ref} aria-label="Example kasetto sync output">
      <div className="hero-terminal-bar">
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-title">~/dev/kasetto</span>
      </div>
      <div className="hero-terminal-body">
        <div className="t-rows">
          <div className="t-line">
            <span className="t-prompt">❯</span>
            <span>
              <span className="t-fg">kasetto </span>
              <span className="t-amber">{typed > 8 ? COMMAND.slice(8, typed) : ""}</span>
            </span>
            {showCursor && <span className="t-cursor" aria-hidden />}
          </div>

          {(phase === "resolving" || phase === "running" || phase === "done") && (
            <div className="t-line">
              {phase === "resolving" ? (
                <span className="t-spin" aria-hidden />
              ) : (
                <span className="t-green">✓</span>
              )}
              <span>
                {phase === "resolving" ? (
                  <span>Resolving sources</span>
                ) : (
                  <span>
                    <span className="t-fg">Resolved </span>
                    <span className="t-amber">{GROUPS.length} sources</span>
                    <span className="t-dim"> · {TOTAL} items</span>
                  </span>
                )}
              </span>
            </div>
          )}

          {resolvedVisible &&
            (phase === "running" || phase === "done") &&
            GROUPS.map((g, gi) => {
              const inflight = gi === groupIdx;
              const past = gi < groupIdx || phase === "done";
              const visible = inflight || past;
              if (!visible) return null;
              return (
                <div key={g.repo} className="t-group">
                  <div className="t-srch">
                    {past ? (
                      <span className="t-green">✓</span>
                    ) : (
                      <span className="t-spin" aria-hidden />
                    )}
                    <span className="t-cyan">{g.repo}</span>
                  </div>
                  {g.items.map((slug, i) => {
                    const isLast = i === g.items.length - 1;
                    const itemShown = past || (inflight && i < itemIdx);
                    if (!itemShown) return null;
                    const st = STATUS[slug];
                    const { g: gl, cls } = glyphFor(st);
                    return (
                      <div key={slug} className="t-row">
                        <span className="t-faint">{isLast ? "└─" : "├─"}</span>
                        <span className={cls}>{gl}</span>
                        <strong className={st?.s === "removed" ? "t-strike" : "t-fg"}>
                          {slug}
                        </strong>
                        <span className="t-tail">
                          {!st && <span className="t-faint">unchanged</span>}
                          {st?.s === "updated" && <span className="t-amber">updated</span>}
                          {st?.s === "added" && <span className="t-green">added</span>}
                          {st?.s === "removed" && <span className="t-red">removed</span>}
                        </span>
                      </div>
                    );
                  })}
                </div>
              );
            })}

          {summaryVisible && (
            <div className="t-line t-summary-chips">
                <span />
                <span>
                  {"  "}
                  <span className="t-amber">●</span>
                  <span className="t-fg"> {COUNTS.updated} </span>
                  <span className="t-dim">updated</span>
                  {"  "}
                  <span className="t-green">●</span>
                  <span className="t-fg"> {COUNTS.added} </span>
                  <span className="t-dim">added</span>
                  {"  "}
                  <span className="t-red">●</span>
                  <span className="t-fg"> {COUNTS.removed} </span>
                  <span className="t-dim">removed</span>
                  {"  "}
                  <span className="t-faint">●</span>
                  <span className="t-fg"> {COUNTS.unchanged} </span>
                  <span className="t-dim">unchanged</span>
                </span>
              </div>
          )}
        </div>
      </div>
    </figure>
  );
}
