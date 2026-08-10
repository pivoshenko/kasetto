import type { MetadataRoute } from "next";
import { SITE_URL } from "@/lib/site";

export const revalidate = false;

export default function robots(): MetadataRoute.Robots {
  return {
    rules: [
      {
        userAgent: "*",
        allow: "/",
        // Raw-Markdown mirrors of the docs; noindex-ed at the header level too.
        disallow: ["/docs-md/", "/api/"],
      },
    ],
    sitemap: `${SITE_URL}/sitemap.xml`,
    host: SITE_URL,
  };
}
