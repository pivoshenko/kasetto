"use client";

import { useEffect, useRef } from "react";

// == Noise ==

type Noise3D = (x: number, y: number, z: number) => number;

/** 3D value noise with a deterministic permutation table, seeded by `seed`. */
function makeNoise(seed: number): Noise3D {
  const p = new Uint8Array(512);
  const perm: number[] = [];
  let s = seed | 0 || 7;
  for (let i = 0; i < 256; i++) perm[i] = i;
  for (let i = 255; i > 0; i--) {
    s = (s * 16807) % 2147483647;
    const j = s % (i + 1);
    const t = perm[i];
    perm[i] = perm[j];
    perm[j] = t;
  }
  for (let i = 0; i < 512; i++) p[i] = perm[i & 255];

  const hash = (x: number, y: number, z: number) => p[(p[(p[x & 255] + y) & 255] + z) & 255] / 255;
  const fade = (t: number) => t * t * (3 - 2 * t);
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

  return (x, y, z) => {
    const xi = Math.floor(x);
    const yi = Math.floor(y);
    const zi = Math.floor(z);
    const xf = fade(x - xi);
    const yf = fade(y - yi);
    const zf = fade(z - zi);
    const a = lerp(
      lerp(hash(xi, yi, zi), hash(xi + 1, yi, zi), xf),
      lerp(hash(xi, yi + 1, zi), hash(xi + 1, yi + 1, zi), xf),
      yf
    );
    const b = lerp(
      lerp(hash(xi, yi, zi + 1), hash(xi + 1, yi, zi + 1), xf),
      lerp(hash(xi, yi + 1, zi + 1), hash(xi + 1, yi + 1, zi + 1), xf),
      yf
    );
    return lerp(a, b, zf);
  };
}

// == Tuning ==

// sparse punctuation through to solid marks, so the field value reads straight off the glyph. The
// leading blank is the empty level and is never painted
const DENSITY_RAMP = " .:-=+*#%@";

// a monospace advance is around 0.6em, so a glyph sized this much larger than the column pitch
// fills its cell horizontally without colliding
const GLYPH_SCALE = 1.6;

/** Column pitch in CSS px; the whole look hangs off this. */
const CELL = 9;
/** Row pitch as a multiple of `CELL`, since a glyph box is taller than it is wide. */
const ROW_RATIO = 1.8;
/** Brightness steps the field is quantized into before it hits the ramp. */
const LEVELS = 7;
const SPEED = 0.35;
/** Field value above which a glyph takes the accent instead of the ink. */
const ACCENT_AT = 0.72;
const SEED = 7;
/** Noise frequency per cell; lower stretches the blobs wider. */
const SCALE = 0.07;
/** How far the cursor lifts the field under it, and its radius in px. */
const BUMP = 0.3;
const REACH = 180;

/**
 * A drifting field of ASCII glyphs behind the nav, ported from the pivoshenko.sites `Ascii` field
 * with the nav's tuning. Fills its nearest positioned ancestor and listens for the cursor on its
 * parent, so it must be a direct child of the band it decorates.
 */
export function AsciiField() {
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const el = wrap.current;
    const cv = canvas.current;
    if (!el || !cv) return;
    const ctx = cv.getContext("2d");
    if (!ctx) return;

    const noise = makeNoise(SEED);
    const mql = window.matchMedia("(prefers-reduced-motion: reduce)");
    let reduce = mql.matches;
    let raf = 0;
    let last = 0;
    let visible = true;
    let W = 0;
    let H = 0;
    let dpr = 1;
    const t0 = performance.now();

    const row = CELL * ROW_RATIO;
    const fontPx = CELL * GLYPH_SCALE;
    const top = DENSITY_RAMP.length - 1;

    // read once: the site is dark-only and nothing retints it after mount
    const cs = getComputedStyle(el);
    const ink = cs.getPropertyValue("--border-strong").trim();
    const hi = cs.getPropertyValue("--accent").trim();
    const family = cs.fontFamily.trim() || "ui-monospace, monospace";

    const size = () => {
      const r = el.getBoundingClientRect();
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      W = Math.max(1, r.width);
      H = Math.max(1, r.height);
      cv.width = Math.round(W * dpr);
      cv.height = Math.round(H * dpr);
    };

    // == Pointer ==

    // x/y is where the cursor sits over the field, a is how hard it presses in
    const ptr = { x: 0, y: 0, tx: 0, ty: 0, a: 0, ta: 0, seen: false };

    const onMove = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      ptr.tx = e.clientX - r.left;
      ptr.ty = e.clientY - r.top;
      ptr.ta = 1;
      if (!ptr.seen) {
        ptr.x = ptr.tx;
        ptr.y = ptr.ty;
        ptr.seen = true;
      }
    };

    const onLeave = () => {
      ptr.ta = 0;
    };

    // == Draw ==

    // quantizing before anything is painted is what makes this read as ASCII art rather than a
    // blurred gradient: the steps have to land on whole glyphs
    const quantize = (v: number) => Math.round(v * LEVELS) / LEVELS;

    const draw = (now: number) => {
      const z = reduce ? 0 : ((now - t0) / 1000) * 0.06 * SPEED;
      const drift = reduce ? 0 : ((now - t0) / 1000) * 2 * SPEED;
      const cols = Math.ceil(W / CELL);
      const rows = Math.ceil(H / row);
      const live = ptr.a > 0.004;
      const R2 = 2 * REACH * REACH;
      ptr.x += (ptr.tx - ptr.x) * 0.16;
      ptr.y += (ptr.ty - ptr.y) * 0.16;
      ptr.a += (ptr.ta - ptr.a) * 0.08;

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, W, H);
      // set once per frame rather than per glyph: assigning ctx.font reparses the shorthand
      ctx.font = `${fontPx}px ${family}`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";

      for (let y = 0; y < rows; y++) {
        for (let x = 0; x < cols; x++) {
          // the y step uses the row pitch, so a taller cell does not stretch the noise with it
          const nx = (x + drift / CELL) * SCALE * CELL * 0.5;
          const ny = y * SCALE * row * 0.5;
          let v = noise(nx, ny, z) * 0.65 + noise(nx * 2.1 + 5, ny * 2.1 + 9, z * 1.3) * 0.35;

          if (live) {
            const dx = x * CELL + CELL / 2 - ptr.x;
            const dy = y * row + row / 2 - ptr.y;
            v += ptr.a * BUMP * Math.exp(-(dx * dx + dy * dy) / R2);
          }

          const q = quantize(Math.min(1, Math.max(0, v)));
          const glyph = DENSITY_RAMP[Math.round(q * top)];
          if (glyph === " ") continue;

          const accent = q >= ACCENT_AT;
          // a floor under the quiet glyphs, so the field reads as a full grid rather than a
          // scatter of bright ones over empty ground
          ctx.globalAlpha = accent ? 0.5 + q * 0.4 : 0.3 + q * 0.6;
          ctx.fillStyle = accent ? hi : ink;
          ctx.fillText(glyph, x * CELL + CELL / 2, y * row + row / 2);
        }
      }
      ctx.globalAlpha = 1;
    };

    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      // ~24fps is plenty for a slow drift
      if (!visible || now - last < 42) return;
      last = now;
      draw(now);
    };

    // == Wiring ==

    const host = el.parentElement || el;
    let listening = false;

    const attach = () => {
      if (listening) return;
      host.addEventListener("pointermove", onMove, { passive: true });
      host.addEventListener("pointerleave", onLeave, { passive: true });
      listening = true;
    };

    const detach = () => {
      if (!listening) return;
      host.removeEventListener("pointermove", onMove);
      host.removeEventListener("pointerleave", onLeave);
      listening = false;
    };

    const applyMotion = () => {
      if (reduce) {
        cancelAnimationFrame(raf);
        raf = 0;
        detach();
        ptr.a = 0;
        ptr.ta = 0;
        ptr.seen = false;
        draw(performance.now());
        return;
      }
      attach();
      if (!raf) raf = requestAnimationFrame(loop);
    };

    const onMotionChange = (e: MediaQueryListEvent) => {
      reduce = e.matches;
      applyMotion();
    };

    size();
    draw(performance.now());
    applyMotion();
    mql.addEventListener("change", onMotionChange);

    const ro = new ResizeObserver(() => {
      size();
      draw(performance.now());
    });
    ro.observe(el);

    // a reduced-motion field is painted once, possibly before the web font lands, and nothing
    // else would repaint it in the right face
    document.fonts.ready.then(() => draw(performance.now()));

    const io = new IntersectionObserver((entries) => {
      visible = entries[0].isIntersecting;
    });
    io.observe(el);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
      mql.removeEventListener("change", onMotionChange);
      detach();
    };
  }, []);

  return (
    <div ref={wrap} className="ascii-field" aria-hidden="true">
      <canvas ref={canvas} />
    </div>
  );
}
