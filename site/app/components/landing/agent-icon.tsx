import { AGENT_ICONS } from "@/lib/agent-icons";

/** An agent's platform mark in `currentColor`, or a mono monogram for presets without one. */
export function AgentIcon({
  preset,
  name,
  className,
}: {
  preset: string;
  name: string;
  className?: string;
}) {
  const icon = AGENT_ICONS[preset];
  if (!icon) {
    return (
      <span className={`lp-monogram ${className ?? ""}`} aria-hidden="true">
        {name.charAt(0)}
      </span>
    );
  }
  return (
    <svg
      viewBox={icon.viewBox}
      fill="currentColor"
      fillRule="evenodd"
      className={className}
      aria-hidden="true"
      focusable="false"
      // biome-ignore lint/security/noDangerouslySetInnerHtml: static SVG bodies from lib/agent-icons.ts
      dangerouslySetInnerHTML={{ __html: icon.body }}
    />
  );
}
