import { palette } from "./palette";

/**
 * Cassette syntax theme for Shiki, used by the docs code blocks through
 * `source.config.ts`. Roles follow the landing page snippets: keys, keywords and
 * commands in amber, strings and arguments in violet, numbers and constants in
 * green, punctuation muted, comments subtle.
 */
export const cassetteTheme = {
  name: "cassette",
  type: "dark" as const,
  colors: {
    "editor.background": palette.bgElevated,
    "editor.foreground": palette.fg,
  },
  tokenColors: [
    { settings: { foreground: palette.fg, background: palette.bgElevated } },
    {
      scope: ["comment", "punctuation.definition.comment", "string.comment"],
      settings: { foreground: palette.comment },
    },
    {
      scope: [
        "keyword",
        "storage",
        "entity.name.tag",
        "entity.name.function",
        "support.function",
        "support.type.property-name",
        "meta.object-literal.key",
        "variable.other.property",
      ],
      settings: { foreground: palette.accent },
    },
    {
      scope: ["string", "string.unquoted", "markup.inline.raw", "markup.underline.link"],
      settings: { foreground: palette.accent2 },
    },
    {
      scope: ["constant.numeric", "constant.language", "constant.character", "support.constant"],
      settings: { foreground: palette.success },
    },
    {
      scope: ["punctuation", "keyword.operator", "meta.brace", "punctuation.definition.tag"],
      settings: { foreground: palette.fgMuted },
    },
    { scope: ["variable", "variable.parameter"], settings: { foreground: palette.fg } },
    {
      scope: ["markup.heading", "markup.bold"],
      settings: { foreground: palette.fg, fontStyle: "bold" },
    },
    { scope: ["markup.inserted"], settings: { foreground: palette.success } },
    { scope: ["markup.deleted", "invalid"], settings: { foreground: palette.danger } },
  ],
};
