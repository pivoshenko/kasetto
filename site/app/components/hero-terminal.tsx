"use client";

import { useEffect, useRef, useState } from "react";

const SYNC_COMMAND = "kasetto sync";
const ADD_COMMAND = "kst add github.com/anthropics/skills@v1.2.0 --skill pdf";
const ADD_SOURCE = "https://github.com/anthropics/skills";
const ADD_REPO = "github.com/anthropics/skills";
const ADD_ITEM = "pdf";
const REMOVE_COMMAND = "kst remove github.com/mattpocock/skills --skill grill-me";
const REMOVE_SOURCE = "https://github.com/mattpocock/skills";
const REMOVE_REPO = "github.com/mattpocock/skills";
const REMOVE_ITEM = "grill-me";

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

// scene "sync" runs the original kasetto sync animation; "edit" runs the
// continuation cargo-style `kst add ...` then `kst remove ...` session; "to-*"
// are brief clear-screen transitions between scenes so the loop reads as one
// terminal. Within "edit" the EditStep state machine threads add → remove.
type Scene = "sync" | "to-edit" | "edit" | "to-sync";
type SyncPhase = "idle" | "typing" | "resolving" | "running" | "done";
type EditStep =
  | "add-typing"
  | "add-resolving"
  | "add-running"
  | "add-done"
  | "rm-typing"
  | "rm-resolving"
  | "rm-running"
  | "rm-done";

export function HeroTerminal() {
  const ref = useRef<HTMLElement>(null);
  const [scene, setScene] = useState<Scene>("sync");
  // sync-scene state
  const [phase, setPhase] = useState<SyncPhase>("idle");
  const [typed, setTyped] = useState(0);
  const [groupIdx, setGroupIdx] = useState(-1);
  const [itemIdx, setItemIdx] = useState(0);
  // edit-scene state (threads add → remove in one terminal session)
  const [step, setStep] = useState<EditStep>("add-typing");
  const [addTyped, setAddTyped] = useState(0);
  const [addItemShown, setAddItemShown] = useState(false);
  const [rmTyped, setRmTyped] = useState(0);
  const [rmItemShown, setRmItemShown] = useState(false);
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

  // --- sync scene state machine ---
  useEffect(() => {
    if (scene !== "sync" || phase !== "typing") return;
    if (reduced) {
      setTyped(SYNC_COMMAND.length);
      setPhase("done");
      setGroupIdx(GROUPS.length);
      return;
    }
    if (typed >= SYNC_COMMAND.length) {
      const t = setTimeout(() => setPhase("resolving"), 320);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setTyped((n) => n + 1), 55 + Math.random() * 40);
    return () => clearTimeout(t);
  }, [scene, phase, typed, reduced]);

  useEffect(() => {
    if (scene !== "sync" || phase !== "resolving") return;
    const t = setTimeout(() => {
      setPhase("running");
      setGroupIdx(0);
      setItemIdx(0);
    }, 740);
    return () => clearTimeout(t);
  }, [scene, phase]);

  useEffect(() => {
    if (scene !== "sync" || phase !== "running") return;
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
  }, [scene, phase, groupIdx, itemIdx]);

  // sync done → wait, then transition to edit scene
  useEffect(() => {
    if (scene !== "sync" || phase !== "done" || reduced) return;
    const t = setTimeout(() => setScene("to-edit"), 4200);
    return () => clearTimeout(t);
  }, [scene, phase, reduced]);

  // brief blank "clear screen", then begin edit scene at the add step
  useEffect(() => {
    if (scene !== "to-edit") return;
    const t = setTimeout(() => {
      setStep("add-typing");
      setAddTyped(0);
      setAddItemShown(false);
      setRmTyped(0);
      setRmItemShown(false);
      setScene("edit");
    }, 280);
    return () => clearTimeout(t);
  }, [scene]);

  // --- edit scene state machine: add → remove, in one session ---
  useEffect(() => {
    if (scene !== "edit" || step !== "add-typing") return;
    if (reduced) {
      setAddTyped(ADD_COMMAND.length);
      setAddItemShown(true);
      setRmTyped(REMOVE_COMMAND.length);
      setRmItemShown(true);
      setStep("rm-done");
      return;
    }
    if (addTyped >= ADD_COMMAND.length) {
      const t = setTimeout(() => setStep("add-resolving"), 280);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setAddTyped((n) => n + 1), 30 + Math.random() * 22);
    return () => clearTimeout(t);
  }, [scene, step, addTyped, reduced]);

  useEffect(() => {
    if (scene !== "edit" || step !== "add-resolving") return;
    const t = setTimeout(() => setStep("add-running"), 600);
    return () => clearTimeout(t);
  }, [scene, step]);

  useEffect(() => {
    if (scene !== "edit" || step !== "add-running") return;
    const t1 = setTimeout(() => setAddItemShown(true), 240);
    const t2 = setTimeout(() => setStep("add-done"), 460);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [scene, step]);

  // add settled → start typing the remove command in the same session
  useEffect(() => {
    if (scene !== "edit" || step !== "add-done") return;
    const t = setTimeout(() => setStep("rm-typing"), 1400);
    return () => clearTimeout(t);
  }, [scene, step]);

  useEffect(() => {
    if (scene !== "edit" || step !== "rm-typing") return;
    if (rmTyped >= REMOVE_COMMAND.length) {
      const t = setTimeout(() => setStep("rm-resolving"), 280);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setRmTyped((n) => n + 1), 30 + Math.random() * 22);
    return () => clearTimeout(t);
  }, [scene, step, rmTyped]);

  useEffect(() => {
    if (scene !== "edit" || step !== "rm-resolving") return;
    const t = setTimeout(() => setStep("rm-running"), 600);
    return () => clearTimeout(t);
  }, [scene, step]);

  useEffect(() => {
    if (scene !== "edit" || step !== "rm-running") return;
    const t1 = setTimeout(() => setRmItemShown(true), 240);
    const t2 = setTimeout(() => setStep("rm-done"), 460);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [scene, step]);

  // remove settled → loop back to sync
  useEffect(() => {
    if (scene !== "edit" || step !== "rm-done" || reduced) return;
    const t = setTimeout(() => setScene("to-sync"), 4200);
    return () => clearTimeout(t);
  }, [scene, step, reduced]);

  useEffect(() => {
    if (scene !== "to-sync") return;
    const t = setTimeout(() => {
      setPhase("typing");
      setTyped(0);
      setGroupIdx(-1);
      setItemIdx(0);
      setScene("sync");
    }, 280);
    return () => clearTimeout(t);
  }, [scene]);

  const syncTypingCursor = scene === "sync" && phase === "typing";
  const syncSummaryVisible = scene === "sync" && phase === "done";
  const syncResolvedVisible =
    scene === "sync" && (phase === "resolving" || phase === "running" || phase === "done");

  // True once we've moved past the corresponding sub-phase (current step's
  // ordinal compared against each anchor). Drives "remember everything I've
  // already shown" semantics so the session stays on screen as we type the
  // next command.
  const stepOrder: EditStep[] = [
    "add-typing",
    "add-resolving",
    "add-running",
    "add-done",
    "rm-typing",
    "rm-resolving",
    "rm-running",
    "rm-done",
  ];
  const stepIdx = scene === "edit" ? stepOrder.indexOf(step) : -1;
  const past = (s: EditStep) => stepIdx > stepOrder.indexOf(s);
  const inOrPast = (s: EditStep) => stepIdx >= stepOrder.indexOf(s);

  const addTypingCursor = scene === "edit" && step === "add-typing";
  const addLineVisible = scene === "edit" && past("add-typing");
  const addResolvedVisible = scene === "edit" && inOrPast("add-resolving");
  const addGroupVisible = scene === "edit" && inOrPast("add-running");
  const addSummaryVisible = scene === "edit" && inOrPast("add-done");

  const rmPromptVisible = scene === "edit" && inOrPast("rm-typing");
  const rmTypingCursor = scene === "edit" && step === "rm-typing";
  const rmLineVisible = scene === "edit" && past("rm-typing");
  const rmResolvedVisible = scene === "edit" && inOrPast("rm-resolving");
  const rmGroupVisible = scene === "edit" && inOrPast("rm-running");
  const rmSummaryVisible = scene === "edit" && inOrPast("rm-done");

  return (
    <figure
      className="hero-terminal"
      ref={ref}
      aria-label="Example kasetto sync, then kst add and kst remove output"
    >
      <div className="hero-terminal-bar">
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-dot" />
        <span className="hero-terminal-title">~/dev/kasetto</span>
      </div>
      <div className="hero-terminal-body">
        <div className="t-rows">
          {scene === "sync" && (
            <>
              <div className="t-line">
                <span className="t-prompt">❯</span>
                <span>
                  <span className="t-fg">kasetto </span>
                  <span className="t-amber">{typed > 8 ? SYNC_COMMAND.slice(8, typed) : ""}</span>
                </span>
                {syncTypingCursor && <span className="t-cursor" aria-hidden />}
              </div>

              {syncResolvedVisible && (
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

              {syncResolvedVisible &&
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

              {syncSummaryVisible && (
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
            </>
          )}

          {scene === "edit" && (
            <>
              <div className="t-line">
                <span className="t-prompt">❯</span>
                <span>
                  <span className="t-fg">kst </span>
                  <span className="t-amber">
                    {addTyped > 4 ? ADD_COMMAND.slice(4, addTyped) : ""}
                  </span>
                </span>
                {addTypingCursor && <span className="t-cursor" aria-hidden />}
              </div>

              {addLineVisible && (
                <div className="t-line">
                  <span />
                  <span>
                    <span className="t-fg">Adding </span>
                    <span className="t-cyan">{ADD_SOURCE}</span>
                    <span className="t-fg"> to skills</span>
                  </span>
                </div>
              )}

              {addResolvedVisible && (
                <div className="t-line">
                  {step === "add-resolving" ? (
                    <span className="t-spin" aria-hidden />
                  ) : (
                    <span className="t-green">✓</span>
                  )}
                  <span>
                    {step === "add-resolving" ? (
                      <span>Resolving sources</span>
                    ) : (
                      <span>
                        <span className="t-fg">Resolved </span>
                        <span className="t-amber">1 source</span>
                        <span className="t-dim"> · 1 item</span>
                      </span>
                    )}
                  </span>
                </div>
              )}

              {addGroupVisible && (
                <div className="t-group">
                  <div className="t-srch">
                    {past("add-running") ? (
                      <span className="t-green">✓</span>
                    ) : (
                      <span className="t-spin" aria-hidden />
                    )}
                    <span className="t-cyan">{ADD_REPO}</span>
                  </div>
                  {addItemShown && (
                    <div className="t-row">
                      <span className="t-faint">└─</span>
                      <span className="t-green">+</span>
                      <strong className="t-fg">{ADD_ITEM}</strong>
                      <span className="t-tail">
                        <span className="t-green">added</span>
                      </span>
                    </div>
                  )}
                </div>
              )}

              {addSummaryVisible && (
                <div className="t-line t-summary-chips">
                  <span />
                  <span>
                    {"  "}
                    <span className="t-green">●</span>
                    <span className="t-fg"> 1 </span>
                    <span className="t-dim">added</span>
                  </span>
                </div>
              )}

              {rmPromptVisible && (
                <div className="t-line">
                  <span className="t-prompt">❯</span>
                  <span>
                    <span className="t-fg">kst </span>
                    <span className="t-amber">
                      {rmTyped > 4 ? REMOVE_COMMAND.slice(4, rmTyped) : ""}
                    </span>
                  </span>
                  {rmTypingCursor && <span className="t-cursor" aria-hidden />}
                </div>
              )}

              {rmLineVisible && (
                <div className="t-line">
                  <span />
                  <span>
                    <span className="t-fg">Removing </span>
                    <span className="t-cyan">{REMOVE_SOURCE}</span>
                    <span className="t-fg"> from skills</span>
                  </span>
                </div>
              )}

              {rmResolvedVisible && (
                <div className="t-line">
                  {step === "rm-resolving" ? (
                    <span className="t-spin" aria-hidden />
                  ) : (
                    <span className="t-green">✓</span>
                  )}
                  <span>
                    {step === "rm-resolving" ? (
                      <span>Resolving sources</span>
                    ) : (
                      <span>
                        <span className="t-fg">Resolved </span>
                        <span className="t-amber">1 source</span>
                        <span className="t-dim"> · 1 item</span>
                      </span>
                    )}
                  </span>
                </div>
              )}

              {rmGroupVisible && (
                <div className="t-group">
                  <div className="t-srch">
                    {past("rm-running") ? (
                      <span className="t-green">✓</span>
                    ) : (
                      <span className="t-spin" aria-hidden />
                    )}
                    <span className="t-cyan">{REMOVE_REPO}</span>
                  </div>
                  {rmItemShown && (
                    <div className="t-row">
                      <span className="t-faint">└─</span>
                      <span className="t-red">−</span>
                      <strong className="t-fg t-strike">{REMOVE_ITEM}</strong>
                      <span className="t-tail">
                        <span className="t-red">removed</span>
                      </span>
                    </div>
                  )}
                </div>
              )}

              {rmSummaryVisible && (
                <div className="t-line t-summary-chips">
                  <span />
                  <span>
                    {"  "}
                    <span className="t-red">●</span>
                    <span className="t-fg"> 1 </span>
                    <span className="t-dim">removed</span>
                  </span>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </figure>
  );
}
