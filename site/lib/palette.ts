/**
 * Cassette palette in raw hex, for surfaces that cannot read CSS variables:
 * the next/og renderers behind app/icon.tsx, app/apple-icon.tsx and
 * app/opengraph-image.tsx. The DOM paints with the matching tokens in
 * app/styles/tokens.css, so a color change lands in both files.
 */
export const palette = {
  bg: "#1f1f1e",
  bgElevated: "#262625",
  bgRaise: "#2e2e2c",
  border: "#373634",
  borderStrong: "#46443f",
  fg: "#e4e2de",
  fgMuted: "#a8a195",
  fgSubtle: "#6e6759",
  /** Code comments: between subtle and muted, readable without competing with code. */
  comment: "#8b8477",
  accent: "#e8a94d",
  accent2: "#b6a6ef",
  accent3: "#7a68c2",
  accentDeep: "#3a2f6e",
  success: "#84c578",
  danger: "#e87e6c",
} as const;

/** The four stops the cassette is painted with: top face 0->1, label 1->2, sides 2->3. */
export const brandGradient = [
  palette.accent,
  palette.accent2,
  palette.accent3,
  palette.accentDeep,
] as const;
