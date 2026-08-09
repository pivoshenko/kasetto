import { getPageMarkdown } from "@/lib/markdown";
import { source } from "@/lib/source";

export async function GET(_req: Request, props: { params: Promise<{ slug?: string[] }> }) {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) return new Response("Not Found", { status: 404 });

  return new Response(getPageMarkdown(page), {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
}

export function generateStaticParams() {
  return source.generateParams();
}
