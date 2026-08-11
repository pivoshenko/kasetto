import { DocsBody, DocsDescription, DocsPage, DocsTitle } from "fumadocs-ui/page";
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { CopyButton } from "@/app/components/copy-button";
import { JsonLd } from "@/app/components/json-ld";
import { getPageMarkdown } from "@/lib/markdown";
import { source } from "@/lib/source";
import { breadcrumbJsonLd, faqJsonLd } from "@/lib/structured-data";
import { getMDXComponents } from "@/mdx-components";

const OG_IMAGE = {
  url: "/opengraph-image",
  width: 1200,
  height: 630,
  alt: "Kasetto: Declarative AI agent environment manager",
};

export default async function Page(props: { params: Promise<{ slug?: string[] }> }) {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  const MDX = page.data.body;
  const faq = page.url === "/docs/faq" ? faqJsonLd(page) : null;

  return (
    <DocsPage toc={page.data.toc} full={page.data.full}>
      <JsonLd data={breadcrumbJsonLd(page)} />
      {faq && <JsonLd data={faq} />}
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription>{page.data.description}</DocsDescription>
      <div className="docs-page-actions">
        <CopyButton text={getPageMarkdown(page)} label="Copy page" />
        <a className="action-link" href={`${page.url}.md`}>
          View as Markdown
        </a>
      </div>
      <DocsBody>
        <MDX components={getMDXComponents()} />
      </DocsBody>
    </DocsPage>
  );
}

export function generateStaticParams() {
  return source.generateParams();
}

export async function generateMetadata(props: {
  params: Promise<{ slug?: string[] }>;
}): Promise<Metadata> {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  const title = `${page.data.title} - Kasetto`;

  return {
    title: page.data.title,
    description: page.data.description,
    alternates: {
      canonical: page.url,
    },
    openGraph: {
      type: "article",
      url: page.url,
      siteName: "Kasetto",
      locale: "en_US",
      title,
      description: page.data.description,
      images: OG_IMAGE,
    },
    twitter: {
      card: "summary_large_image",
      title,
      description: page.data.description,
      images: OG_IMAGE,
    },
  };
}
