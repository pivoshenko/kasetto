"use client";

import { useEffect, useId, useRef, useState } from "react";

type MermaidProps = { chart: string };

export function Mermaid({ chart }: MermaidProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const id = useId().replace(/[^a-zA-Z0-9]/g, "");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function render() {
      const mermaid = (await import("mermaid")).default;
      const isDark =
        document.documentElement.getAttribute("data-theme") === "dark" ||
        document.documentElement.classList.contains("dark");

      mermaid.initialize({
        startOnLoad: false,
        theme: isDark ? "dark" : "default",
        securityLevel: "strict",
        fontFamily: "JetBrains Mono, Menlo, Consolas, monospace",
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

    const observer = new MutationObserver(() => {
      if (!cancelled) render();
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "class"],
    });

    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [chart, id]);

  if (error) {
    return (
      <pre className="text-sm text-red-500 whitespace-pre-wrap">Mermaid render error: {error}</pre>
    );
  }

  return <div ref={containerRef} className="my-4 flex justify-center" />;
}
