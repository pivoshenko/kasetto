import meta from "@/content/docs/meta.json";
import { source } from "@/lib/source";

type Page = NonNullable<ReturnType<typeof source.getPage>>;

const FRONTMATTER = /^---\n[\s\S]*?\n---\n/;
const LEADING_MDX_IMPORTS = /^(import .+\n)+/;

/** Raw MDX source for a docs page, as plain Markdown with frontmatter swapped for a heading. */
export function getPageMarkdown(page: Page): string {
  const body = page.data.content
    .replace(FRONTMATTER, "")
    .trimStart()
    .replace(LEADING_MDX_IMPORTS, "")
    .trim();
  const heading = page.data.description
    ? `# ${page.data.title}\n\n${page.data.description}`
    : `# ${page.data.title}`;

  return `${heading}\n\n${body}\n`;
}

/** All docs pages in the sidebar order defined by `content/docs/meta.json`. */
export function getOrderedDocsPages(): Page[] {
  const order = meta.pages;

  return [...source.getPages()].sort((a, b) => {
    const slugA = a.slugs.length ? a.slugs.join("/") : "index";
    const slugB = b.slugs.length ? b.slugs.join("/") : "index";
    return order.indexOf(slugA) - order.indexOf(slugB);
  });
}
