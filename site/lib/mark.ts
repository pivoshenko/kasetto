/**
 * Geometry for the Kasetto mark: a compact cassette lying flat in isometric
 * view, extruded downward like a block.
 *
 * This is the single source for every surface the mark appears on. The DOM
 * mark (app/components/brand-mark.tsx) paints it with CSS tokens; the off-DOM
 * renderers (app/icon.tsx, app/apple-icon.tsx, app/opengraph-image.tsx and the
 * README assets) take the raw-hex SVG strings from `markSvg` / `markTileSvg`,
 * because next/og cannot read CSS variables.
 *
 * The cassette is drawn flat at 64 x 40 units (the 100.4 x 63.8 mm ratio of a
 * real compact cassette), then mapped onto the isometric floor by `ISO` inside a
 * 0 0 100 100 viewBox. Detail drops as pixels run out: the label, screws and
 * hubs go first, the bottom bevel next, and the 16/32px favicons keep only the
 * shell, the window and two reels.
 */
import { brandGradient, palette } from "@/lib/palette";

export const MARK_VIEWBOX = "0 0 100 100";

/** Flat cassette coordinates -> isometric floor, centred in the viewBox. */
export const ISO = "matrix(.866 .5 -.866 .5 39.6 18)";

/** Face roles; each surface maps a role to a fill. */
export type MarkRole = "top" | "label" | "side" | "hole";

export type MarkVariant = {
  /** 2 = full display mark, 1 = 64px, 0 = 32px and below. */
  detail: 0 | 1 | 2;
  /** Extrusion depth in viewBox units. */
  depth: number;
};

export const MARK_DISPLAY: MarkVariant = { detail: 2, depth: 9 };

/** Hand-fitted favicon variants; the block thickens as the size drops. */
export const MARK_FAVICONS = {
  16: { detail: 0, depth: 14 },
  32: { detail: 0, depth: 11 },
  64: { detail: 1, depth: 11 },
} as const satisfies Record<number, MarkVariant>;

export type MarkShape = {
  el: "rect" | "circle" | "path";
  role: MarkRole;
  attrs: Record<string, number | string>;
  /** Screen-space drop applied before `ISO`; only the extrusion layers use it. */
  drop?: number;
};

const BODY = { width: 64, height: 40, rx: 3 };

/** Shapes in paint order. Every shape sits in flat coordinates and is drawn through `ISO`. */
export function markShapes({ detail, depth }: MarkVariant): MarkShape[] {
  const shapes: MarkShape[] = [];
  // Stacked copies of the shell, half a unit apart, read as a solid extruded side
  for (let drop = depth; drop > 0; drop -= 0.5) {
    shapes.push({ el: "rect", role: "side", attrs: BODY, drop });
  }
  shapes.push({ el: "rect", role: "top", attrs: BODY });

  if (detail === 2) {
    shapes.push(
      { el: "rect", role: "label", attrs: { x: 4, y: 3.5, width: 56, height: 25, rx: 1.5 } },
      { el: "path", role: "side", attrs: { d: "M13 40L16 32.5H48L51 40Z" } },
      { el: "rect", role: "hole", attrs: { x: 17, y: 10, width: 30, height: 11, rx: 5.5 } },
      { el: "circle", role: "side", attrs: { cx: 24, cy: 15.5, r: 4.2 } },
      { el: "circle", role: "side", attrs: { cx: 40, cy: 15.5, r: 3 } },
      { el: "circle", role: "hole", attrs: { cx: 24, cy: 15.5, r: 1.6 } },
      { el: "circle", role: "hole", attrs: { cx: 40, cy: 15.5, r: 1.6 } },
      { el: "circle", role: "hole", attrs: { cx: 3.5, cy: 36.5, r: 0.9 } },
      { el: "circle", role: "hole", attrs: { cx: 60.5, cy: 36.5, r: 0.9 } },
      { el: "circle", role: "hole", attrs: { cx: 32, cy: 36.3, r: 0.9 } }
    );
    return shapes;
  }

  if (detail === 1) {
    shapes.push({ el: "path", role: "side", attrs: { d: "M13 40L16 32.5H48L51 40Z" } });
  }
  shapes.push(
    { el: "rect", role: "hole", attrs: { x: 11, y: 8, width: 42, height: 15, rx: 7.5 } },
    { el: "circle", role: "top", attrs: { cx: 22, cy: 15.5, r: 4.5 } },
    { el: "circle", role: "top", attrs: { cx: 42, cy: 15.5, r: 4.5 } }
  );
  return shapes;
}

export function shapeTransform(shape: MarkShape): string {
  return shape.drop ? `translate(0 ${shape.drop}) ${ISO}` : ISO;
}

export type MarkFills = Record<MarkRole, string>;

/** Flat fills for small sizes; `hole` is the surface the mark sits on. */
export function flatFills(hole: string = palette.bg): MarkFills {
  return { top: palette.accent, label: palette.accent2, side: palette.accent3, hole };
}

/** Gradient definitions for one mark instance, as SVG markup; `id` keeps instances apart. */
export function gradientDefs(id: string, stops: readonly string[] = brandGradient): string {
  const [a, b, c, d] = stops;
  return (
    `<linearGradient id="${id}-top" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient>` +
    `<linearGradient id="${id}-label" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${b}"/><stop offset="1" stop-color="${c}"/></linearGradient>` +
    `<linearGradient id="${id}-side" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${c}"/><stop offset="1" stop-color="${d}"/></linearGradient>`
  );
}

export function gradientFills(id: string, hole: string = palette.bg): MarkFills {
  return { top: `url(#${id}-top)`, label: `url(#${id}-label)`, side: `url(#${id}-side)`, hole };
}

/** The mark's shapes as SVG markup (no wrapper), painted with `fills`. */
export function markBody(variant: MarkVariant, fills: MarkFills): string {
  return markShapes(variant)
    .map((shape) => {
      const attrs = Object.entries(shape.attrs)
        .map(([k, v]) => `${k}="${v}"`)
        .join(" ");
      return `<${shape.el} ${attrs} fill="${fills[shape.role]}" transform="${shapeTransform(shape)}"/>`;
    })
    .join("");
}

/** A standalone SVG of the mark in raw hex. Gradient at display size, flat otherwise. */
export function markSvg(variant: MarkVariant, hole: string = palette.bg): string {
  const gradient = variant.detail === 2;
  const defs = gradient ? `<defs>${gradientDefs("k")}</defs>` : "";
  const fills = gradient ? gradientFills("k", hole) : flatFills(hole);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${MARK_VIEWBOX}">${defs}${markBody(variant, fills)}</svg>`;
}

/**
 * Tile geometry for the app icon: a raised dark squircle with the mark inset by
 * 20%. Below 32px the inset shrinks and the mark is drawn through `cropViewBox`,
 * because the full viewBox leaves a third of a 10px box empty and the cassette
 * reads as a blob in a browser tab.
 */
export const TILE = {
  radius: 0.24,
  inset: 0.2,
  smallInset: 0.12,
  cropViewBox: "4 15 92 72",
} as const;

/** The dark-squircle app icon at `px`, in raw hex; picks the variant that fits the size. */
export function markTileSvg(px: number): string {
  const variant: MarkVariant =
    px >= 128
      ? MARK_DISPLAY
      : px >= 64
        ? MARK_FAVICONS[64]
        : px >= 32
          ? MARK_FAVICONS[32]
          : MARK_FAVICONS[16];
  const small = px < 32;
  const inset = Math.round(px * (small ? TILE.smallInset : TILE.inset));
  const inner = px - inset * 2;
  const viewBox = small ? TILE.cropViewBox : MARK_VIEWBOX;
  const gradient = variant.detail === 2;
  const defs = gradient ? `<defs>${gradientDefs("k")}</defs>` : "";
  const fills = gradient ? gradientFills("k", palette.bgRaise) : flatFills(palette.bgRaise);
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${px}" height="${px}" viewBox="0 0 ${px} ${px}">${defs}` +
    `<rect x="0.5" y="0.5" width="${px - 1}" height="${px - 1}" rx="${px * TILE.radius}" fill="${palette.bgRaise}" stroke="${palette.border}"/>` +
    `<svg aria-hidden="true" x="${inset}" y="${inset}" width="${inner}" height="${inner}" viewBox="${viewBox}">${markBody(variant, fills)}</svg></svg>`
  );
}

export function svgDataUri(svg: string): string {
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
