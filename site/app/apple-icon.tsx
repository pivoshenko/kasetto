import { ImageResponse } from "next/og";
import { markTileSvg, svgDataUri } from "@/lib/mark";
import { palette } from "@/lib/palette";

const PX = 180;

export const size = { width: PX, height: PX };
export const contentType = "image/png";

export default function AppleIcon() {
  return new ImageResponse(
    // iOS composites the icon onto black before applying its own mask, so the tile
    // sits on its own surface to keep the squircle corners from showing black
    <div style={{ display: "flex", width: PX, height: PX, background: palette.bgRaise }}>
      <img src={svgDataUri(markTileSvg(PX))} width={PX} height={PX} alt="" />
    </div>,
    size
  );
}
