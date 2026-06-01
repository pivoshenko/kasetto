import type { ReactNode } from "react";

type Kind = "info" | "note" | "tip" | "success" | "warning" | "warn" | "danger" | "error";

type Props = {
  type?: Kind;
  title?: ReactNode;
  children: ReactNode;
};

const MAP: Record<string, { cls: string; glyph: string; label: string }> = {
  info: { cls: "kc-info", glyph: "→", label: "Note" },
  note: { cls: "kc-info", glyph: "→", label: "Note" },
  tip: { cls: "kc-tip", glyph: "✓", label: "Tip" },
  success: { cls: "kc-tip", glyph: "✓", label: "Tip" },
  warning: { cls: "kc-warn", glyph: "⚠", label: "Warning" },
  warn: { cls: "kc-warn", glyph: "⚠", label: "Warning" },
  danger: { cls: "kc-danger", glyph: "✗", label: "Danger" },
  error: { cls: "kc-danger", glyph: "✗", label: "Danger" },
};

export function Callout({ type = "info", title, children }: Props) {
  const m = MAP[type] ?? MAP.info;
  return (
    <aside className={`kasetto-callout ${m.cls}`} role="note">
      <div className="kasetto-callout-head">
        <span className="kasetto-callout-glyph" aria-hidden>
          {m.glyph}
        </span>
        <span className="kasetto-callout-title">{title ?? m.label}</span>
      </div>
      <div className="kasetto-callout-body">{children}</div>
    </aside>
  );
}
