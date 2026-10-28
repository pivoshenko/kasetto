import type { IconType } from "react-icons";
import { GoBook, GoNote, GoPlug, GoTerminal } from "react-icons/go";

// Discovery conventions mirror src/source/mod.rs
const KINDS: { icon: IconType; name: string; desc: string }[] = [
  { icon: GoBook, name: "Skills", desc: "A directory with a SKILL.md, copied whole" },
  { icon: GoTerminal, name: "Commands", desc: "Markdown, rewritten to each agent's format" },
  { icon: GoPlug, name: "MCPs", desc: "A server definition, merged into agent settings" },
  { icon: GoNote, name: "Instructions", desc: "Rules, as files or managed blocks in shared ones" },
];

export function AssetKinds() {
  return (
    <ul className="lp-wall lp-wall--kinds">
      {KINDS.map((k) => {
        const Icon = k.icon;
        return (
          <li key={k.name} className="lp-wall-cell">
            <Icon className="lp-wall-icon" aria-hidden />
            <span className="lp-wall-name">{k.name}</span>
            <span className="lp-wall-note">{k.desc}</span>
          </li>
        );
      })}
    </ul>
  );
}
