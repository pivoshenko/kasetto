import { getOrderedDocsPages } from "@/lib/markdown";

export const revalidate = false;

// apex 307-redirects to www, so link the canonical host directly
const SITE_URL = "https://www.kasetto.dev";

export function GET() {
  const links = getOrderedDocsPages()
    .map((page) => `- [${page.data.title}](${SITE_URL}${page.url}.md): ${page.data.description}`)
    .join("\n");

  const body = `# Kasetto

> A declarative AI agent environment manager. Syncs skills, MCP servers, slash-commands, and instructions from GitHub repos or local directories into 22 agent environments.

## Docs

${links}
`;

  return new Response(body, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
