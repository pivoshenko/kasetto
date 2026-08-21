import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ImageResponse } from "next/og";
import type { CSSProperties } from "react";

export const alt = "Kasetto: Declarative AI agent environment manager";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

// Brand tokens mirror the canonical palette in `src/colors.rs` (CLI is the
// source of truth) and the popil surfaces in `app/globals.css`. Update both
// when this changes.
const BG = "#1f1f1e"; // popil base
const FG = "#e4e2de"; // popil text
const MUTED = "#a8a195"; // CLI SECONDARY
const LAVENDER = "#b89cdc"; // CLI BRAND
const ACCENT_WARM = "#e8a94d"; // CLI ATTENTION
const ADDED = "#84c578"; // CLI SUCCESS
const REMOVED = "#e87e6c"; // CLI ERROR
const DIM = "#6e6759"; // CLI INFRA
const JP_YELLOW = "#d4b070"; // popil yellow

// Same six lines as `assets/social-preview-dark.svg`, so the shared card and the
// GitHub social preview stay in sync. 58 columns; at 28px in JetBrains Mono
// (0.6em advance) that is 974px, which clears the 1120px frame interior.
const WORDMARK = [
  "██╗  ██╗ █████╗ ███████╗███████╗████████╗████████╗ ██████╗",
  "██║ ██╔╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝╚══██╔══╝██╔═══██╗",
  "█████╔╝ ███████║███████╗█████╗     ██║      ██║   ██║   ██║",
  "██╔═██╗ ██╔══██║╚════██║██╔══╝     ██║      ██║   ██║   ██║",
  "██║  ██╗██║  ██║███████║███████╗   ██║      ██║   ╚██████╔╝",
  "╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝      ╚═╝    ╚═════╝",
];

// The wordmark draws as two layers over one grid so the letters read as stacked
// tiles rather than a solid slab, matching `assets/social-preview-dark.svg`: the
// box-drawing rules paint over the blocks, keeping the shadow outline
// continuous, while BG-colored bars cut a 3px gap into every row seam.
const BLOCK = "\u2588";
const splitLayer = (keepBlocks: boolean) =>
  WORDMARK.map((line) =>
    Array.from(line, (ch) => ((ch === BLOCK) === keepBlocks ? ch : " ")).join("")
  );
const WORDMARK_BLOCKS = splitLayer(true);
const WORDMARK_RULES = splitLayer(false);

// U+2588 renders exactly 40px tall at font-size 28, so a 40px line box butts the
// rows together with no overlap and the seams land on clean 40px multiples.
const WORDMARK_FONT_SIZE = 28;
const WORDMARK_LINE_H = 40;
const WORDMARK_SEAM_W = 3;
const WORDMARK_SEAMS = WORDMARK.slice(1).map(
  (_, i) => WORDMARK_LINE_H * (i + 1) - WORDMARK_SEAM_W / 2
);
// Satori resolves a percentage width on the seam bars against more than the
// wordmark, and the overhang notches the J-card border - so pin the bars to the
// longest line at the 0.6em advance instead.
const WORDMARK_W = Math.max(...WORDMARK.map((line) => line.length)) * WORDMARK_FONT_SIZE * 0.6;
const WORDMARK_ROW: CSSProperties = {
  fontSize: WORDMARK_FONT_SIZE,
  fontWeight: 700,
  lineHeight: `${WORDMARK_LINE_H}px`,
  whiteSpace: "pre",
};

const CHIPS = [
  { dot: ACCENT_WARM, count: "4", label: "updated", labelColor: ACCENT_WARM },
  { dot: ADDED, count: "2", label: "added", labelColor: ADDED },
  { dot: REMOVED, count: "1", label: "removed", labelColor: REMOVED },
  { dot: DIM, count: "11", label: "unchanged", labelColor: MUTED },
];

// The bundled fonts are subsets, unhinted and stripped of layout tables:
// JetBrains Mono 2.304 (Latin + U+25CF/U+276F, plus U+2500-257F box drawing and
// U+2580-259F block elements at 700 for the wordmark) and Noto Sans JP,
// instanced at wght 700 and cut to the kana in the subtitle. Widen the ranges
// before adding glyphs the subsets do not carry, or they render as tofu.
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
        alignItems: "center",
        justifyContent: "center",
        background: BG,
        color: FG,
        fontFamily: "JetBrains Mono, monospace",
        position: "relative",
      }}
    >
      {/* J-card border - Satori ignores the `inset` shorthand, so size it explicitly */}
      <div
        style={{
          position: "absolute",
          top: 38,
          left: 38,
          width: size.width - 76,
          height: size.height - 76,
          border: `4px solid ${LAVENDER}`,
          display: "flex",
        }}
      />

      {/* ASCII wordmark */}
      <div style={{ display: "flex", position: "relative", marginTop: 24, color: LAVENDER }}>
        <div style={{ display: "flex", flexDirection: "column" }}>
          {WORDMARK_BLOCKS.map((line, i) => (
            <div key={`block-${i}`} style={WORDMARK_ROW}>
              {line}
            </div>
          ))}
        </div>

        {WORDMARK_SEAMS.map((top) => (
          <div
            key={top}
            style={{
              position: "absolute",
              display: "flex",
              left: 0,
              top,
              width: WORDMARK_W,
              height: WORDMARK_SEAM_W,
              background: BG,
            }}
          />
        ))}

        <div
          style={{
            display: "flex",
            flexDirection: "column",
            position: "absolute",
            top: 0,
            left: 0,
          }}
        >
          {WORDMARK_RULES.map((line, i) => (
            <div key={`rule-${i}`} style={WORDMARK_ROW}>
              {line}
            </div>
          ))}
        </div>
      </div>

      {/* Tagline */}
      <div style={{ display: "flex", fontSize: 24, marginTop: 46, whiteSpace: "pre" }}>
        <span style={{ color: MUTED }}>A declarative </span>
        <span style={{ color: FG }}>AI agent environment manager</span>
        <span style={{ color: MUTED }}>, written in </span>
        <span style={{ color: ACCENT_WARM }}>Rust</span>
        <span style={{ color: MUTED }}>.</span>
      </div>

      {/* Prompt + command */}
      <div style={{ display: "flex", fontSize: 22, marginTop: 20, whiteSpace: "pre" }}>
        <span style={{ color: ACCENT_WARM }}>❯</span>
        <span style={{ color: FG }}> kst </span>
        <span style={{ color: ACCENT_WARM }}>sync</span>
      </div>

      {/* Chip strip totals */}
      <div style={{ display: "flex", fontSize: 19, marginTop: 18, gap: 34, whiteSpace: "pre" }}>
        {CHIPS.map((chip) => (
          <div key={chip.label} style={{ display: "flex" }}>
            <span style={{ color: chip.dot }}>●</span>
            <span style={{ color: FG }}>{` ${chip.count} `}</span>
            <span style={{ color: chip.labelColor }}>{chip.label}</span>
          </div>
        ))}
      </div>

      {/* Japanese subtitle */}
      <div
        style={{
          display: "flex",
          marginTop: 24,
          fontSize: 13,
          fontWeight: 700,
          letterSpacing: 5.2,
          color: JP_YELLOW,
          fontFamily: "Noto Sans JP",
        }}
      >
        スキル・パッケージ・マネージャー
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
