import type { IconType } from "react-icons";
import { GoBook, GoHubot, GoNote, GoPackage, GoPlug, GoTerminal } from "react-icons/go";

const KINDS: { icon: IconType; name: string }[] = [
  { icon: GoBook, name: "Skills" },
  { icon: GoTerminal, name: "Commands" },
  { icon: GoHubot, name: "Subagents" },
  { icon: GoPlug, name: "MCPs" },
  { icon: GoNote, name: "Instructions" },
  { icon: GoPackage, name: "Plugins" },
];

export function AssetKinds() {
  return (
    <ul className="lp-wall lp-kinds">
      {KINDS.map((k) => {
        const Icon = k.icon;
        return (
          <li key={k.name} className="lp-wall-cell">
            <Icon className="lp-wall-icon" aria-hidden />
            <span className="lp-wall-name">{k.name}</span>
          </li>
        );
      })}
    </ul>
  );
}
