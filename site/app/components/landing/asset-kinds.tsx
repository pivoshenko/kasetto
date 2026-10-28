import type { IconType } from "react-icons";
import { GoBook, GoNote, GoPlug, GoTerminal } from "react-icons/go";

const KINDS: { icon: IconType; name: string; desc: string }[] = [
  { icon: GoBook, name: "Skills", desc: "A directory with a SKILL.md, copied as is" },
  {
    icon: GoTerminal,
    name: "Commands",
    desc: "Markdown files, rewritten into each agent's format",
  },
  { icon: GoPlug, name: "MCPs", desc: "Server definitions merged into the agent's settings" },
  {
    icon: GoNote,
    name: "Instructions",
    desc: "Rules written as files, or as managed blocks in shared ones",
  },
];

export function AssetKinds() {
  return (
    <ul className="lp-kinds">
      {KINDS.map((k) => {
        const Icon = k.icon;
        return (
          <li key={k.name} className="lp-kind">
            <span className="lp-kind-head">
              <Icon aria-hidden />
              <span className="lp-kind-name display">{k.name}</span>
            </span>
            <span className="lp-kind-desc">{k.desc}</span>
          </li>
        );
      })}
    </ul>
  );
}
