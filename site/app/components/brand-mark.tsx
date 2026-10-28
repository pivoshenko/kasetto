import { useId } from "react";
import {
  MARK_DISPLAY,
  MARK_FAVICONS,
  MARK_VIEWBOX,
  type MarkRole,
  type MarkVariant,
  markShapes,
  shapeTransform,
  TILE,
} from "@/lib/mark";

const STOPS = ["var(--accent)", "var(--accent-2)", "var(--accent-3)", "var(--accent-deep)"];

const FLAT: Record<MarkRole, string> = {
  top: "var(--accent)",
  label: "var(--accent-2)",
  side: "var(--accent-3)",
  hole: "var(--bg)",
};

/**
 * The cassette mark in the DOM, painted with tokens. The display variant takes
 * the brand gradient; smaller variants are flat. `hole` is the surface behind
 * the mark, which shows through the window and reel hubs.
 */
export function BrandMark({
  variant = MARK_DISPLAY,
  hole = "var(--bg)",
  viewBox = MARK_VIEWBOX,
  className,
}: {
  variant?: MarkVariant;
  hole?: string;
  /** Pass `TILE.cropViewBox` to draw the mark edge to edge in a small tile. */
  viewBox?: string;
  className?: string;
}) {
  const id = useId().replace(/:/g, "");
  const gradient = variant.detail === 2;
  const fill = (role: MarkRole) =>
    role === "hole" ? hole : gradient ? `url(#${id}-${role})` : FLAT[role];

  return (
    <svg viewBox={viewBox} className={className} aria-hidden="true" focusable="false">
      {gradient ? (
        <defs>
          <linearGradient id={`${id}-top`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor={STOPS[0]} />
            <stop offset="1" stopColor={STOPS[1]} />
          </linearGradient>
          <linearGradient id={`${id}-label`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor={STOPS[1]} />
            <stop offset="1" stopColor={STOPS[2]} />
          </linearGradient>
          <linearGradient id={`${id}-side`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={STOPS[2]} />
            <stop offset="1" stopColor={STOPS[3]} />
          </linearGradient>
        </defs>
      ) : null}
      {markShapes(variant).map((shape, i) => {
        const El = shape.el;
        return (
          <El key={i} {...shape.attrs} fill={fill(shape.role)} transform={shapeTransform(shape)} />
        );
      })}
    </svg>
  );
}

/** The dark-squircle app tile, drawn the same way as `markTileSvg` draws the favicon. */
export function BrandTile({ size, className }: { size: number; className?: string }) {
  const variant = size >= 64 ? MARK_FAVICONS[64] : MARK_FAVICONS[32];
  const small = size < 32;
  const inset = Math.round(size * (small ? TILE.smallInset : TILE.inset));
  return (
    <span
      className={className}
      style={{
        display: "inline-grid",
        placeItems: "center",
        width: size,
        height: size,
        flex: "none",
        borderRadius: size * TILE.radius,
        background: "var(--bg-raise)",
        border: "1px solid var(--border)",
        padding: inset - 1,
      }}
      aria-hidden="true"
    >
      <BrandMark
        variant={variant}
        hole="var(--bg-raise)"
        viewBox={small ? TILE.cropViewBox : MARK_VIEWBOX}
        className="brand-tile-mark"
      />
    </span>
  );
}

/**
 * The nav logo: the bare mark cropped to its own bounds, so it reads at nav
 * height instead of shrinking inside a tile. Uses the bolder 64px geometry.
 */
export function BrandLogo({ className }: { className?: string }) {
  return <BrandMark variant={MARK_FAVICONS[64]} viewBox={TILE.cropViewBox} className={className} />;
}
