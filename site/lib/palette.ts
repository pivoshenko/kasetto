/**
 * Lime Ice palette in raw hex, for surfaces that cannot read CSS variables:
 * the next/og renderers behind app/icon.tsx, app/apple-icon.tsx and
 * app/opengraph-image.tsx. The DOM paints with the matching tokens in
 * app/styles/tokens.css, so a color change lands in both files.
 */
export const palette = {
  bg: "#08090b",
  bgElevated: "#111317",
  bgRaise: "#171a1f",
  border: "#22262d",
  borderStrong: "#2e333b",
  fg: "#e7ebef",
  fgMuted: "#8a919b",
  fgSubtle: "#596069",
  /** Code comments: between subtle and muted, readable without competing with code. */
  comment: "#6d747e",
  accent: "#c6ff3d",
  accent2: "#3fd7ff",
  accent3: "#2a7fd4",
  accentDeep: "#173a7a",
  success: "#5fe3a1",
  danger: "#ff6b6b",
} as const;

/** The four stops the cassette is painted with: top face 0->1, label 1->2, sides 2->3. */
export const brandGradient = [
  palette.accent,
  palette.accent2,
  palette.accent3,
  palette.accentDeep,
] as const;
