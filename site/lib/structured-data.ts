import { getPageBody, type Page } from "@/lib/markdown";
import { SITE_URL } from "@/lib/site";

function toPlainText(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, "")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[`*_>]/g, "")
    .replace(/^\s*[-+]\s+/gm, "")
    .replace(/\s+/g, " ")
    .trim();
}

export function softwareApplicationJsonLd() {
  return {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "Kasetto",
    applicationCategory: "DeveloperApplication",
    operatingSystem: "macOS, Linux, Windows",
    url: SITE_URL,
    downloadUrl: `${SITE_URL}/install`,
    codeRepository: "https://github.com/pivoshenko/kasetto",
    programmingLanguage: "Rust",
    license: "https://github.com/pivoshenko/kasetto/blob/main/LICENSE-MIT",
    description:
      "A declarative AI agent environment manager. Syncs skills, MCP servers, slash-commands, and instructions from Git repositories into 23 AI coding agents.",
    author: {
      "@type": "Person",
      name: "Volodymyr Pivoshenko",
      url: "https://github.com/pivoshenko",
    },
    offers: {
      "@type": "Offer",
      price: "0",
      priceCurrency: "USD",
    },
  };
}

export function breadcrumbJsonLd(page: Page) {
  const trail = [
    { name: "Kasetto", url: SITE_URL },
    { name: "Documentation", url: `${SITE_URL}/docs` },
  ];
  if (page.url !== "/docs") {
    trail.push({ name: page.data.title, url: `${SITE_URL}${page.url}` });
  }

  return {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    itemListElement: trail.map((item, index) => ({
      "@type": "ListItem",
      position: index + 1,
      name: item.name,
      item: item.url,
    })),
  };
}

export function faqJsonLd(page: Page) {
  const entries = getPageBody(page)
    .split(/^## /m)
    .slice(1)
    .map((section) => {
      const newline = section.indexOf("\n");
      if (newline === -1) return null;
      const question = toPlainText(section.slice(0, newline));
      const answer = toPlainText(section.slice(newline));
      return question && answer ? { question, answer } : null;
    })
    .filter((entry) => entry !== null);

  if (entries.length === 0) return null;

  return {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    mainEntity: entries.map((entry) => ({
      "@type": "Question",
      name: entry.question,
      acceptedAnswer: { "@type": "Answer", text: entry.answer },
    })),
  };
}
