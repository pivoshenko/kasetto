import { getOrderedDocsPages } from "@/lib/markdown";
import { SITE_URL } from "@/lib/site";

export const revalidate = false;

export function GET() {
  const links = getOrderedDocsPages()
    .map((page) => `- [${page.data.title}](${SITE_URL}${page.url}.md): ${page.data.description}`)
    .join("\n");

  const body = `# Kasetto

> A declarative AI agent environment manager. Syncs skills, MCPs, commands, instructions, and subagents from GitHub repositories or local directories into 23 agent environments, and enables Claude Code plugins in Claude Code and Copilot CLI.

## Docs

${links}
`;

  return new Response(body, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
