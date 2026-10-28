"use client";

import { useEffect, useRef, useState } from "react";

const COMMAND = "kst sync --audit high";
const LEVELS = ["safe", "low", "medium", "high", "critical"] as const;
const THRESHOLD = 3;

type Level = (typeof LEVELS)[number];
type Outcome = "ok" | "warn" | "block";

type Row = {
  name: string;
  kind: "skill" | "mcp";
  level: Level | null;
  note: string;
  outcome: Outcome;
};

// Mirrors the gate's policy: at or above the threshold fails the run, an asset nothing could
// judge only warns, and anything below the threshold passes even when it is flagged
const ROWS: Row[] = [
  { name: "pdf", kind: "skill", level: "safe", note: "3 partners", outcome: "ok" },
  { name: "frontend-design", kind: "skill", level: "low", note: "3 partners", outcome: "ok" },
  { name: "vercel", kind: "mcp", level: "medium", note: "floating npx version", outcome: "ok" },
  { name: "team-playbook", kind: "skill", level: null, note: "not on skills.sh", outcome: "warn" },
  { name: "db-admin", kind: "mcp", level: "critical", note: "2 OSV advisories", outcome: "block" },
];

const OUTCOME_LABEL: Record<Outcome, string> = { ok: "pass", warn: "warn", block: "block" };

// One timeline drives the whole scene; each beat is a stage index the markup reads
const TYPE_MS = 48;
const BEATS = {
  rail: COMMAND.length + 4,
  scan: COMMAND.length + 12,
  rowStep: 7,
};
const LAST_ROW = BEATS.scan + ROWS.length * BEATS.rowStep;
const VERDICT = LAST_ROW + 4;
const END = VERDICT + 70;

function tone(level: Level | null): string {
  if (level === null) return "unknown";
  if (LEVELS.indexOf(level) >= THRESHOLD) return "danger";
  if (level === "medium") return "warning";
  return "success";
}

export function AuditGate() {
  const ref = useRef<HTMLElement>(null);
  const [tick, setTick] = useState(-1);
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    const m = window.matchMedia("(prefers-reduced-motion: reduce)");
    setReduced(m.matches);
    if (m.matches) {
      setTick(END);
      return;
    }
    const el = ref.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          setTick(0);
          obs.disconnect();
        }
      },
      { threshold: 0.35 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  useEffect(() => {
    if (tick < 0 || reduced) return;
    const delay = tick < COMMAND.length ? TYPE_MS : 90;
    const t = setTimeout(() => setTick((n) => (n >= END ? 0 : n + 1)), delay);
    return () => clearTimeout(t);
  }, [tick, reduced]);

  const typed = Math.max(0, Math.min(tick, COMMAND.length));
  const railOn = tick >= BEATS.rail;
  const auditing = tick >= BEATS.scan && tick < LAST_ROW;
  const audited = tick >= LAST_ROW;
  const verdict = tick >= VERDICT;
  const blocks = ROWS.filter((r) => r.outcome === "block").length;
  const warns = ROWS.filter((r) => r.outcome === "warn").length;

  return (
    <figure
      ref={ref}
      className="lp-pane lp-audit"
      data-verdict={verdict ? "" : undefined}
      aria-label="kst sync --audit high: four assets pass, one unverified skill warns, and a critical MCP fails the run"
    >
      <figcaption className="lp-pane-bar">
        <span>~/dev/team-app</span>
        <span className="lp-pane-tag">audit gate</span>
      </figcaption>

      <div className="lp-audit-body">
        <div className="lp-audit-cmd">
          <span className="t-prompt">❯</span>
          <span className="t-fg">{COMMAND.slice(0, typed)}</span>
          {tick >= 0 && tick < COMMAND.length + 2 ? (
            <span className="t-cursor" aria-hidden />
          ) : null}
        </div>

        <div className="lp-audit-rail" data-on={railOn ? "" : undefined} aria-hidden>
          {LEVELS.map((l, i) => (
            <span key={l} className="lp-audit-seg" data-fail={i >= THRESHOLD ? "" : undefined}>
              {l}
            </span>
          ))}
          <span
            className="lp-audit-notch"
            style={{ left: `${(THRESHOLD / LEVELS.length) * 100}%` }}
          >
            fail-on
          </span>
        </div>

        <div className="lp-audit-status">
          {auditing ? <span className="t-spin" aria-hidden /> : null}
          {audited ? <span className="t-success">✓</span> : null}
          <span className={audited || auditing ? "t-fg" : "t-subtle"}>
            {audited ? "Audited" : "Auditing"} {ROWS.length} assets
          </span>
          <span className="t-subtle">skills.sh · OSV · deps.dev</span>
        </div>

        <ol className="lp-audit-rows">
          {ROWS.map((r, i) => {
            const at = BEATS.scan + i * BEATS.rowStep;
            const state = tick < at ? "idle" : tick < at + BEATS.rowStep - 2 ? "scan" : "done";
            return (
              <li
                key={r.name}
                className="lp-audit-row"
                data-state={state}
                data-outcome={state === "done" ? r.outcome : undefined}
              >
                <span className="lp-audit-id">
                  <span className="lp-audit-name">
                    <strong>{r.name}</strong>
                    <span className="lp-audit-kind">{r.kind}</span>
                  </span>
                  <span className="lp-audit-note">{r.note}</span>
                </span>
                <span className="lp-audit-level" data-tone={tone(r.level)}>
                  {r.level ?? "unverified"}
                </span>
                <span className="lp-audit-outcome">{OUTCOME_LABEL[r.outcome]}</span>
              </li>
            );
          })}
        </ol>

        <div className="lp-audit-verdict" aria-hidden={!verdict}>
          <p className="lp-audit-line" data-tone="danger">
            <span className="lp-audit-mark">✗</span>
            <span>
              {blocks} asset at or above <b>high</b>: run fails
            </span>
            <span className="lp-audit-exit">exit 1</span>
          </p>
          <p className="lp-audit-line" data-tone="warning">
            <span className="lp-audit-mark">!</span>
            <span>{warns} asset could not be verified: warn only</span>
          </p>
        </div>
      </div>
    </figure>
  );
}

// The config form of the gate; `kst sync --audit high` is the one-run equivalent
const POLICY = [
  { key: "audit", value: "" },
  { key: "  fail-on", value: " high" },
  { key: "  fail-on-unverified", value: " true" },
];

/** How the gate is switched on: three lines of config, or one flag for a single run. */
export function AuditPolicy() {
  return (
    <figure className="lp-pane">
      <figcaption className="lp-pane-bar">
        <span>kasetto.yaml</span>
        <span className="lp-pane-tag">policy</span>
      </figcaption>
      <div className="lp-pane-body lp-code">
        {POLICY.map((l, i) => (
          <div key={l.key} className="sy-line">
            <span className="sy-ln" aria-hidden="true">
              {i + 1}
            </span>
            <span>
              <span className="sy-key">{l.key}</span>
              <span className="sy-punct">:</span>
              <span className="sy-str">{l.value}</span>
            </span>
          </div>
        ))}
      </div>
      <div className="lp-pane-foot">
        <span className="lp-pane-prompt">$</span> kst sync --audit high
        <span className="lp-pane-note">the same gate for one run, no config</span>
      </div>
    </figure>
  );
}
