import { ImageResponse } from "next/og";
import { markTileSvg, svgDataUri } from "@/lib/mark";

// One PNG per hand-fitted favicon variant, rasterized at its own pixel size, so a
// 16px tab gets the thick 16px geometry and a 2x tab picks the 32px file. An SVG
// favicon cannot swap geometry by size, and Safari ignores SVG favicons anyway
const SIZES = [16, 32, 64] as const;

export function generateImageMetadata() {
  return SIZES.map((px) => ({
    id: px,
    size: { width: px, height: px },
    contentType: "image/png",
  }));
}

export default function Icon({ id }: { id: number }) {
  const px = SIZES.find((size) => size === Number(id)) ?? 32;
  return new ImageResponse(
    <img src={svgDataUri(markTileSvg(px))} width={px} height={px} alt="" />,
    { width: px, height: px }
  );
}
