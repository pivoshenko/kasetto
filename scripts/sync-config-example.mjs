#!/usr/bin/env node
// Sync the canonical kasetto.example.yaml into the README, docs, and homepage hero.
// Run after editing kasetto.example.yaml:        just sync-config
// Check for drift (non-zero exit if out of date):  node scripts/sync-config-example.mjs --check
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const example = readFileSync(join(root, "kasetto.example.yaml"), "utf8").replace(/\n+$/, "");

// ── Markdown embed (README + docs): verbatim fenced block ──────────────
const mdBlock = "```yaml\n" + example + "\n```";

// ── Hero tokens (feature-tabs.tsx): tokenize YAML for syntax coloring ──
function jsStr(s) {
  return s.includes('"') ? `'${s.replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'` : `"${s}"`;
}
function tokenize(text) {
  const out = [];
  const push = (t, v) => out.push(v === undefined ? `  { t: "${t}" }` : `  { t: "${t}", v: ${jsStr(v)} }`);
  for (const raw of text.split("\n")) {
    if (raw.trim() === "") { push("nl"); continue; }
    const indent = raw.length - raw.trimStart().length;
    let rest = raw.slice(indent);
    if (indent > 0) push("dash", " ".repeat(indent));
    if (rest.startsWith("- ")) { push("punct", "- "); rest = rest.slice(2); }
    if (rest.startsWith("#")) { push("cmt", rest); push("nl"); continue; }
    const ci = rest.indexOf(":");
    if (ci === -1) { push("str", rest); push("nl"); continue; } // scalar list value
    const key = rest.slice(0, ci);
    const after = rest.slice(ci + 1);
    push("key", key);
    if (after.trim() === "") { push("punct", ":"); push("nl"); continue; }
    push("punct", ": ");
    const val = after.replace(/^ /, "");
    const hi = val.indexOf("#");
    const valuePart = hi === -1 ? val : val.slice(0, hi);
    const comment = hi === -1 ? "" : val.slice(hi);
    const trimmed = valuePart.trimEnd();
    const trailing = valuePart.slice(trimmed.length);
    if (/^https?:\/\//.test(trimmed)) push("url", trimmed.replace(/^https?:\/\//, ""));
    else push("str", trimmed);
    if (comment) push("cmt", trailing + comment);
    push("nl");
  }
  return "const CONFIG_LINES: Token[] = [\n" + out.map((l) => l + ",").join("\n") + "\n];";
}
const heroBlock = tokenize(example);

const targets = [
  { file: "README.md", start: "<!-- kasetto-config:start -->", end: "<!-- kasetto-config:end -->", body: mdBlock },
  { file: "site/content/docs/configuration.mdx", start: "{/* kasetto-config:start */}", end: "{/* kasetto-config:end */}", body: mdBlock },
  { file: "site/app/components/feature-tabs.tsx", startsWith: "// kasetto-config:start", end: "// kasetto-config:end", body: heroBlock },
];

let drift = false;
for (const t of targets) {
  if (check && t.file.endsWith(".tsx")) continue; // hero formatting handled by `just sync-config`
  const path = join(root, t.file);
  const src = readFileSync(path, "utf8");
  // start marker may be a fixed string or a line prefix (tsx start carries a trailing comment)
  let i, startLen;
  if (t.start) { i = src.indexOf(t.start); startLen = t.start.length; }
  else {
    i = src.indexOf(t.startsWith);
    const eol = src.indexOf("\n", i);
    startLen = (eol === -1 ? src.length : eol) - i;
  }
  const j = src.indexOf(t.end);
  if (i === -1 || j === -1 || j < i) {
    console.error(`✗ ${t.file}: markers not found`);
    process.exit(2);
  }
  const next = src.slice(0, i + startLen) + "\n" + t.body + "\n" + src.slice(j);
  if (next === src) { console.log(`✓ ${t.file}: up to date`); continue; }
  drift = true;
  if (check) console.error(`✗ ${t.file}: out of date — run \`just sync-config\``);
  else { writeFileSync(path, next); console.log(`✎ ${t.file}: updated`); }
}
if (check && drift) process.exit(1);
