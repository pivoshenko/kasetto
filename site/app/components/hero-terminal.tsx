"use client";

import { useEffect, useRef, useState } from "react";

type Source = { name: string; source: string };

const COMMAND = "kst sync";

// One row per skill, run-length-grouped by source so the URL appears once
// per contiguous run — matches the new `kst sync -v` output exactly.
const SOURCES: Source[] = [
  { name: "code-review", source: "github.com/anthropics/skills" },
  { name: "pr-description", source: "" },
  { name: "release-notes", source: "" },
  { name: "next-best-practices", source: "github.com/vercel-labs/next-skills" },
  { name: "next-upgrade", source: "" },
  { name: "git-commit", source: "github.com/pivoshenko/pivoshenko.ai" },
];

const SUMMARY = { count: "17", elapsed: "412ms" };

type Phase = "idle" | "typing" | "running" | "done";

export function HeroTerminal() {
  const ref = useRef<HTMLElement>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [typed, setTyped] = useState(0);
  // -1 = pending, 0..n-1 = currently resolving index n, n = all resolved
  const [resolving, setResolving] = useState<number>(-1);
  const [resolved, setResolved] = useState<number>(0);
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
      { threshold: 0.3 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  useEffect(() => {
    if (phase !== "typing") return;
    if (reduced) {
      setTyped(COMMAND.length);
      setPhase("running");
      return;
    }
    if (typed >= COMMAND.length) {
      const t = setTimeout(() => setPhase("running"), 380);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setTyped((n) => n + 1), 65 + Math.random() * 50);
    return () => clearTimeout(t);
  }, [phase, typed, reduced]);

  useEffect(() => {
    if (phase !== "running") return;
    if (reduced) {
      setResolving(SOURCES.length);
      setResolved(SOURCES.length);
      setPhase("done");
      return;
    }
    if (resolved >= SOURCES.length) {
      setResolving(-1);
      const t = setTimeout(() => setPhase("done"), 280);
      return () => clearTimeout(t);
    }
    setResolving(resolved);
    const t = setTimeout(() => setResolved((n) => n + 1), 360);
    return () => clearTimeout(t);
  }, [phase, resolved, reduced]);

  const showCursor = phase === "typing" || phase === "running";

  return (
    <figure className="hero-terminal" ref={ref} aria-label="Example kst sync output">
      <div className="hero-terminal-bar">
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-title">~/projects/my-app</span>
      </div>
      <div className="hero-terminal-body">
        <div className="t-rows">
          <div className="t-line">
            <span className="t-prompt">$</span>
            <span>{COMMAND.slice(0, typed)}</span>
            {showCursor && phase === "typing" && <span className="t-cursor" aria-hidden />}
          </div>

          {SOURCES.map((row, idx) => {
            const isDone = idx < resolved;
            const isResolving = idx === resolving && phase === "running";
            const visible = isDone || isResolving;
            return (
              <div key={row.name} className="t-row t-fade" data-shown={visible}>
                {isDone ? <span className="t-ok">+</span> : <span className="t-spin" aria-hidden />}
                <strong>{row.name}</strong>
                {row.source && <span className="t-dim">{row.source}</span>}
              </div>
            );
          })}

          <div className="t-summary-line t-fade" data-shown={phase === "done"}>
            <span className="t-ok">Installed</span>
            <span className="t-summary">{SUMMARY.count}</span>
            <span className="t-dim">items in</span>
            <span className="t-summary">{SUMMARY.elapsed}</span>
          </div>
        </div>
      </div>
    </figure>
  );
}
