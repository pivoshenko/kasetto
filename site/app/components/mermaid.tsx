"use client";

import { useEffect, useId, useRef, useState } from "react";
import { palette } from "@/lib/palette";

type MermaidProps = { chart: string };

// next/font registers hashed family names, so read the resolved stacks instead of hardcoding them
function fontStack(variable: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(variable).trim();
  return value || fallback;
}

export function Mermaid({ chart }: MermaidProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const id = useId().replace(/[^a-zA-Z0-9]/g, "");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function render() {
      const mermaid = (await import("mermaid")).default;
      const sans = fontStack("--font-sans", "Arial, sans-serif");

      mermaid.initialize({
        startOnLoad: false,
        theme: "base",
        securityLevel: "strict",
        fontFamily: sans,
        themeVariables: {
          darkMode: true,
          fontFamily: sans,
          fontSize: "14px",
          background: palette.bgElevated,
          primaryColor: palette.bgRaise,
          primaryTextColor: palette.fg,
          primaryBorderColor: palette.borderStrong,
          secondaryColor: palette.bgElevated,
          secondaryTextColor: palette.fg,
          secondaryBorderColor: palette.border,
          tertiaryColor: palette.bgElevated,
          tertiaryTextColor: palette.fg,
          tertiaryBorderColor: palette.border,
          lineColor: palette.fgMuted,
          textColor: palette.fg,
          mainBkg: palette.bgRaise,
          nodeBorder: palette.borderStrong,
          clusterBkg: palette.bg,
          clusterBorder: palette.border,
          titleColor: palette.fg,
          edgeLabelBackground: palette.bgElevated,
        },
        flowchart: { curve: "basis", padding: 14 },
      });

      try {
        const { svg } = await mermaid.render(`mermaid-${id}`, chart);
        if (!cancelled && containerRef.current) {
          containerRef.current.innerHTML = svg;
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    }

    render();

    return () => {
      cancelled = true;
    };
  }, [chart, id]);

  if (error) {
    return <pre className="kasetto-mermaid-error">Mermaid render error: {error}</pre>;
  }

  return <div ref={containerRef} className="kasetto-mermaid not-prose" />;
}
