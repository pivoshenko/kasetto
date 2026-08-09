import { getOrderedDocsPages, getPageMarkdown } from "@/lib/markdown";

export const revalidate = false;

export function GET() {
  const body = getOrderedDocsPages().map(getPageMarkdown).join("\n\n---\n\n");

  return new Response(body, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
