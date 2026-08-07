import { createPreset } from "fumadocs-ui/tailwind-plugin";
import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./app/**/*.{ts,tsx}",
    "./content/**/*.{md,mdx}",
    "./mdx-components.tsx",
    "./node_modules/fumadocs-ui/dist/**/*.js",
  ],
  presets: [createPreset()],
  theme: {
    extend: {
      fontFamily: {
        mono: ["JetBrains Mono", "Menlo", "Consolas", "monospace"],
        sans: ["DM Sans", "system-ui", "sans-serif"],
      },
    },
  },
};

export default config;
