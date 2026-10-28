import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ImageResponse } from "next/og";
import {
  gradientDefs,
  gradientFills,
  MARK_DISPLAY,
  markBody,
  markSvg,
  svgDataUri,
} from "@/lib/mark";
import { palette } from "@/lib/palette";

export const alt = "Kasetto: your AI environment, under control";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

// Tight box around the display mark inside its 0 0 100 100 viewBox (shell plus
// extrusion), so the art can be sized and placed by its visible edges
const MARK_BOX = { x: 4, y: 17, w: 92, h: 63 };
const ART_W = 560;
const ART_H = Math.round((ART_W * MARK_BOX.h) / MARK_BOX.w);

// Satori has no soft radial fade, so the glow behind the art is its own SVG
const GLOW_W = 900;
const GLOW_H = 640;
const GLOW =
  `<svg xmlns="http://www.w3.org/2000/svg" width="${GLOW_W}" height="${GLOW_H}">` +
  `<defs><radialGradient id="glow" cx=".5" cy=".5" r=".5">` +
  `<stop offset="0" stop-color="${palette.accent}" stop-opacity=".15"/>` +
  `<stop offset="1" stop-color="${palette.accent}" stop-opacity="0"/></radialGradient></defs>` +
  `<ellipse cx="${GLOW_W / 2}" cy="${GLOW_H / 2}" rx="${GLOW_W / 2}" ry="${GLOW_H / 2}" fill="url(#glow)"/></svg>`;

// Film grain multiplied into the gradient faces; resvg rasterizes the filter
const GRAIN =
  '<filter id="grain" x="0" y="0" width="100%" height="100%">' +
  '<feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="2" stitchTiles="stitch" result="n"/>' +
  '<feColorMatrix in="n" type="saturate" values="0" result="g"/>' +
  '<feComponentTransfer in="g" result="a"><feFuncA type="linear" slope=".5"/></feComponentTransfer>' +
  '<feComposite in="a" in2="SourceGraphic" operator="in" result="m"/>' +
  '<feBlend in="SourceGraphic" in2="m" mode="multiply"/></filter>';

const ART =
  `<svg xmlns="http://www.w3.org/2000/svg" width="${ART_W}" height="${ART_H}" viewBox="${MARK_BOX.x} ${MARK_BOX.y} ${MARK_BOX.w} ${MARK_BOX.h}">` +
  `<defs>${gradientDefs("og")}${GRAIN}</defs>` +
  `<g filter="url(#grain)">${markBody(MARK_DISPLAY, gradientFills("og"))}</g></svg>`;

// Satori ignores font-stretch, so the expanded cut is its own family; the TTFs
// are static Google Fonts instances of Archivo (wdth 125 wght 500, wdth 100 wght
// 400). The JetBrains Mono file is a Latin subset; widen it before adding glyphs
// outside Basic Latin or they render as tofu
function loadFont(file: string) {
  return readFileSync(join(process.cwd(), "app/fonts", file));
}

const DISPLAY = "Archivo Expanded";
const TRACKING = "-0.02em";

export default function OpengraphImage() {
  const expanded = loadFont("archivo-expanded-500.ttf");
  const regular = loadFont("archivo-400.ttf");
  const mono = loadFont("jetbrains-mono-400.ttf");

  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        position: "relative",
        background: palette.bg,
        color: palette.fg,
        fontFamily: "Archivo",
      }}
    >
      <img
        src={svgDataUri(GLOW)}
        width={GLOW_W}
        height={GLOW_H}
        alt=""
        style={{
          position: "absolute",
          left: 1000 - GLOW_W / 2,
          top: (size.height - GLOW_H) / 2 + 20,
        }}
      />
      <img
        src={svgDataUri(ART)}
        width={ART_W}
        height={ART_H}
        alt=""
        style={{ position: "absolute", left: 724, top: (size.height - ART_H) / 2 + 20 }}
      />

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          padding: "64px 72px",
          width: "100%",
          height: "100%",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
          <img src={svgDataUri(markSvg(MARK_DISPLAY))} width={56} height={56} alt="" />
          <div
            style={{ fontFamily: DISPLAY, fontSize: 32, letterSpacing: TRACKING, marginTop: -4 }}
          >
            kasetto
          </div>
        </div>

        <div style={{ display: "flex", flexDirection: "column" }}>
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              fontFamily: DISPLAY,
              fontSize: 56,
              lineHeight: 1.04,
              letterSpacing: TRACKING,
            }}
          >
            <div>Your AI environment,</div>
            <div style={{ color: palette.accent }}>under control</div>
          </div>
          <div
            style={{
              marginTop: 26,
              fontSize: 26,
              lineHeight: 1.4,
              color: palette.fgMuted,
              maxWidth: 600,
            }}
          >
            Skills, commands, MCPs, and instructions declared in one file, pinned in one lock.
          </div>
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 14,
            fontFamily: "JetBrains Mono",
            fontSize: 22,
            color: palette.fgMuted,
          }}
        >
          <div style={{ width: 10, height: 10, borderRadius: 5, background: palette.accent }} />
          kasetto.dev
        </div>
      </div>
    </div>,
    {
      ...size,
      fonts: [
        { name: DISPLAY, data: expanded, weight: 500, style: "normal" },
        { name: "Archivo", data: regular, weight: 400, style: "normal" },
        { name: "JetBrains Mono", data: mono, weight: 400, style: "normal" },
      ],
    }
  );
}
