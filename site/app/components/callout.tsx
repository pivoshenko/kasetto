import type { ReactNode } from "react";
import type { IconType } from "react-icons";
import {
  IoAlertCircleOutline,
  IoBulbOutline,
  IoInformationCircleOutline,
  IoWarningOutline,
} from "react-icons/io5";

type Kind = "info" | "note" | "tip" | "success" | "warning" | "warn" | "danger" | "error";

type Props = {
  type?: Kind;
  title?: ReactNode;
  children: ReactNode;
};

const INFO = { cls: "kc-info", Icon: IoInformationCircleOutline, label: "Note" };
const TIP = { cls: "kc-tip", Icon: IoBulbOutline, label: "Tip" };
const WARN = { cls: "kc-warn", Icon: IoWarningOutline, label: "Warning" };
const DANGER = { cls: "kc-danger", Icon: IoAlertCircleOutline, label: "Danger" };

const MAP: Record<Kind, { cls: string; Icon: IconType; label: string }> = {
  info: INFO,
  note: INFO,
  tip: TIP,
  success: TIP,
  warning: WARN,
  warn: WARN,
  danger: DANGER,
  error: DANGER,
};

export function Callout({ type = "info", title, children }: Props) {
  const { cls, Icon, label } = MAP[type] ?? INFO;
  return (
    <aside className={`kasetto-callout ${cls}`} role="note">
      <Icon className="kasetto-callout-icon" aria-hidden />
      <div className="kasetto-callout-content">
        <p className="kasetto-callout-title">{title ?? label}</p>
        <div className="kasetto-callout-body">{children}</div>
      </div>
    </aside>
  );
}
