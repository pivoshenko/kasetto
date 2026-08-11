import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ImageResponse } from "next/og";

export const alt = "Kasetto: Declarative AI agent environment manager";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

// Brand tokens mirror the canonical palette in `src/colors.rs` (CLI is the
// source of truth) and the popil surfaces in `app/globals.css`. Update both
// when this changes.
const BG = "#1f1f1e"; // popil base
const FG = "#e4e2de"; // popil text
const MUTED = "#a8a195"; // CLI SECONDARY
const BORDER = "#2e2e2c"; // popil surface1
const ACCENT_WARM = "#e8a94d"; // CLI ATTENTION

function loadFont(file: string) {
  return readFileSync(join(process.cwd(), "app/fonts", file));
}

export default function OpengraphImage() {
  const regular = loadFont("jetbrains-mono-400.ttf");
  const semibold = loadFont("jetbrains-mono-600.ttf");
  const bold = loadFont("jetbrains-mono-700.ttf");
  const jp = loadFont("noto-sans-jp-700.ttf");

  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: BG,
        color: FG,
        fontFamily: "JetBrains Mono, monospace",
        padding: 56,
        position: "relative",
      }}
    >
      {/* J-card border */}
      <div
        style={{
          position: "absolute",
          inset: 32,
          border: `2px solid ${BORDER}`,
          display: "flex",
        }}
      />

      {/* Title block */}
      <div
        style={{
          marginTop: 48,
          display: "flex",
          flexDirection: "column",
          gap: 24,
        }}
      >
        <div
          style={{
            color: ACCENT_WARM,
            fontSize: 22,
            fontWeight: 700,
            letterSpacing: "0.32em",
            fontFamily: "Noto Sans JP",
          }}
        >
          カセット
        </div>
        <div
          style={{
            fontSize: 140,
            fontWeight: 700,
            lineHeight: 1,
            letterSpacing: "-0.02em",
            color: ACCENT_WARM,
          }}
        >
          Kasetto
        </div>
        <div
          style={{
            fontSize: 32,
            color: MUTED,
            letterSpacing: "0.02em",
            maxWidth: 1040,
            lineHeight: 1.3,
          }}
        >
          Declarative AI Agent Environment Manager written in Rust
        </div>
      </div>

      {/* Footer bar */}
      <div
        style={{
          position: "absolute",
          left: 56,
          right: 56,
          bottom: 56,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          fontSize: 20,
          letterSpacing: "0.24em",
          fontWeight: 600,
          color: ACCENT_WARM,
        }}
      >
        <span>KASETTO.DEV</span>
        <span style={{ color: MUTED }}>$ curl -fsSL kasetto.dev/install | sh</span>
      </div>
    </div>,
    {
      ...size,
      fonts: [
        { name: "JetBrains Mono", data: regular, weight: 400, style: "normal" },
        { name: "JetBrains Mono", data: semibold, weight: 600, style: "normal" },
        { name: "JetBrains Mono", data: bold, weight: 700, style: "normal" },
        { name: "Noto Sans JP", data: jp, weight: 700, style: "normal" },
      ],
    }
  );
}
