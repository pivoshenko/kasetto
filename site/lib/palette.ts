/**
 * Cassette palette (pivoshenko slots) in raw hex, for surfaces that cannot read CSS variables:
 * the next/og renderers behind app/icon.tsx, app/apple-icon.tsx and
 * app/opengraph-image.tsx. The DOM paints with the matching tokens in
 * app/styles/tokens.css, so a color change lands in both files.
 */
export const palette = {
  bg: "#0d0d0d",
  bgElevated: "#181818",
  bgRaise: "#212121",
  border: "#2a2a2a",
  borderStrong: "#57534e",
  fg: "#e7e5e4",
  fgMuted: "#a8a29e",
  fgSubtle: "#78716c",
  /** Code comments: between subtle and muted, readable without competing with code. */
  comment: "#908a85",
  accent: "#d4b070",
  accent2: "#babcd8",
  accent3: "#aa89c2",
  accentDeep: "#4a3b5c",
  success: "#8aaf86",
  danger: "#df7c7b",
} as const;

/**
 * The CLI's color roles from src/colors.rs, for mockups of real terminal output; the DOM reads
 * the matching `--cli-*` tokens.
 */
export const cli = {
  attention: "#e8a94d",
  success: "#84c578",
  error: "#e87e6c",
  info: "#6cbfd3",
  secondary: "#a8a195",
  infra: "#6e6759",
} as const;

/** The four stops the cassette is painted with: top face 0->1, label 1->2, sides 2->3. */
export const brandGradient = [
  palette.accent,
  palette.accent2,
  palette.accent3,
  palette.accentDeep,
] as const;
