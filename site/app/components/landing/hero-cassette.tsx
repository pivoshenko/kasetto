import { useId } from "react";
import { ISO, type MarkRole, type MarkVariant, markShapes, shapeTransform } from "@/lib/mark";

const VARIANT: MarkVariant = { detail: 2, depth: 7 };

// Hub centres in the flat 64 x 40 cassette space, matching markShapes
const REELS = [
  { cx: 24, cy: 15.5 },
  { cx: 40, cy: 15.5 },
];

/**
 * The hero art: the Iso tape mark at display scale, painted with the brand
 * gradient, dusted with film grain and lit by a soft amber glow. Label copy is
 * set in the cassette's flat coordinates and pushed through the same `ISO`
 * transform, so it lies on the label like print.
 */
export function HeroCassette({ className }: { className?: string }) {
  const id = useId().replace(/:/g, "");
  const fill = (role: MarkRole) => (role === "hole" ? "var(--bg)" : `url(#${id}-${role})`);

  return (
    <svg
      viewBox="-8 4 116 92"
      className={className}
      role="img"
      aria-label="An isometric cassette labelled kasetto.yaml, side A: skills, commands, MCPs, instructions"
    >
      <defs>
        <linearGradient id={`${id}-top`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" style={{ stopColor: "var(--accent)" }} />
          <stop offset="1" style={{ stopColor: "var(--accent-2)" }} />
        </linearGradient>
        <linearGradient id={`${id}-label`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" style={{ stopColor: "var(--accent-2)" }} />
          <stop offset="1" style={{ stopColor: "var(--accent-3)" }} />
        </linearGradient>
        <linearGradient id={`${id}-side`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" style={{ stopColor: "var(--accent-3)" }} />
          <stop offset="1" style={{ stopColor: "var(--accent-deep)" }} />
        </linearGradient>
        <radialGradient id={`${id}-glow`} cx=".5" cy=".5" r=".5">
          <stop offset="0" style={{ stopColor: "var(--accent)", stopOpacity: 0.2 }} />
          <stop offset=".55" style={{ stopColor: "var(--accent-2)", stopOpacity: 0.06 }} />
          <stop offset="1" style={{ stopColor: "var(--accent-2)", stopOpacity: 0 }} />
        </radialGradient>
        <filter id={`${id}-grain`} x="0" y="0" width="100%" height="100%">
          <feTurbulence
            type="fractalNoise"
            baseFrequency="2.4"
            numOctaves="2"
            stitchTiles="stitch"
            result="noise"
          />
          <feColorMatrix in="noise" type="saturate" values="0" result="mono" />
          <feComponentTransfer in="mono" result="faint">
            <feFuncA type="linear" slope=".5" />
          </feComponentTransfer>
          <feComposite in="faint" in2="SourceGraphic" operator="in" result="masked" />
          <feBlend in="SourceGraphic" in2="masked" mode="multiply" />
        </filter>
        <filter id={`${id}-soft`} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="3.5" />
        </filter>
      </defs>

      <ellipse cx="50" cy="50" rx="58" ry="44" fill={`url(#${id}-glow)`} />

      <g className="lp-cassette-float">
        <rect
          width="64"
          height="40"
          rx="3"
          transform={`translate(0 10) ${ISO}`}
          fill="var(--accent-3)"
          opacity=".3"
          filter={`url(#${id}-soft)`}
          className="lp-cassette-shadow"
        />
        <g filter={`url(#${id}-grain)`}>
          {markShapes(VARIANT).map((shape, i) => {
            const El = shape.el;
            return (
              <El
                key={i}
                {...shape.attrs}
                style={{ fill: fill(shape.role) }}
                transform={shapeTransform(shape)}
              />
            );
          })}
          <g transform={ISO} className="lp-cassette-print">
            <rect x="7" y="5.6" width="5.6" height="5.6" rx=".4" fill="none" strokeWidth=".45" />
            <text x="8.25" y="10.05" fontSize="4.2" fontWeight="600">
              A
            </text>
            {/* textLength pins each run in label units, so print never leaves the label (x 4..60) whatever font loads */}
            <text
              x="15.5"
              y="8.6"
              fontSize="3.1"
              fontWeight="500"
              textLength="18"
              lengthAdjust="spacingAndGlyphs"
            >
              kasetto.yaml
            </text>
            <text
              x="50"
              y="8.6"
              fontSize="2.2"
              opacity=".75"
              textLength="7.6"
              lengthAdjust="spacingAndGlyphs"
            >
              LOCKED
            </text>
            <text
              x="7"
              y="26.2"
              fontSize="2.15"
              opacity=".8"
              textLength="51.9"
              lengthAdjust="spacingAndGlyphs"
            >
              SKILLS / COMMANDS / MCPs / INSTRUCTIONS
            </text>
            {REELS.map((r) => (
              <g key={r.cx} className="lp-reel">
                {[0, 60, 120].map((a) => (
                  <rect
                    key={a}
                    x={r.cx - 0.22}
                    y={r.cy - 1.5}
                    width=".44"
                    height="3"
                    rx=".2"
                    transform={`rotate(${a} ${r.cx} ${r.cy})`}
                    style={{ fill: "var(--accent-3)" }}
                  />
                ))}
              </g>
            ))}
          </g>
        </g>
      </g>
    </svg>
  );
}
