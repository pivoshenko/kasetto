import type { MetadataRoute } from "next";
import { getOrderedDocsPages } from "@/lib/markdown";
import { SITE_URL } from "@/lib/site";

export const revalidate = false;

export default function sitemap(): MetadataRoute.Sitemap {
  const docs = getOrderedDocsPages().map((page) => ({
    url: `${SITE_URL}${page.url}`,
    changeFrequency: "weekly" as const,
    // The docs index outranks the individual pages it links to.
    priority: page.url === "/docs" ? 0.9 : 0.7,
  }));

  return [{ url: SITE_URL, changeFrequency: "weekly" as const, priority: 1 }, ...docs];
}
