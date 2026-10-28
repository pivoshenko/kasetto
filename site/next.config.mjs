import { createMDX } from "fumadocs-mdx/next";
import meta from "./content/docs/meta.json" with { type: "json" };

const withMDX = createMDX();

// Every docs page gets an old-domain redirect; separators in meta.json are not pages
const DOC_SLUGS = meta.pages.filter((slug) => slug !== "index" && !slug.startsWith("---"));

const docsHost = [{ type: "host", value: "docs.kasetto.dev" }];

// Scripts and styles need 'unsafe-inline' for Next's own bootstrap, the JSON-LD blocks, and
// mermaid; everything else is pinned to this origin plus Vercel's analytics hosts
const CSP = [
  "default-src 'self'",
  "base-uri 'self'",
  "object-src 'none'",
  "frame-ancestors 'none'",
  "form-action 'self'",
  "script-src 'self' 'unsafe-inline' https://va.vercel-scripts.com",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: https://backlinklog.com",
  "font-src 'self' data:",
  "connect-src 'self' https://va.vercel-scripts.com https://vitals.vercel-insights.com",
  "worker-src 'self' blob:",
  "upgrade-insecure-requests",
].join("; ");

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          { key: "Content-Security-Policy", value: CSP },
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "X-Frame-Options", value: "DENY" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          {
            key: "Permissions-Policy",
            value: "camera=(), microphone=(), geolocation=()",
          },
        ],
      },
      ...["/docs-md/:path*", "/docs-md", "/docs/:path*.md", "/docs.md", "/llms-full.txt"].map(
        (source) => ({
          source,
          headers: [{ key: "X-Robots-Tag", value: "noindex" }],
        })
      ),
    ];
  },
  async rewrites() {
    return [
      // /docs/<slug>.md → raw Markdown source, served by app/docs-md
      { source: "/docs/:path*.md", destination: "/docs-md/:path*" },
      { source: "/docs.md", destination: "/docs-md" },
    ];
  },
  async redirects() {
    return [
      // docs.kasetto.dev/ → kasetto.dev/docs
      {
        source: "/",
        has: docsHost,
        destination: "https://kasetto.dev/docs",
        permanent: true,
      },
      // docs.kasetto.dev/<slug>(/) → kasetto.dev/docs/<slug>
      ...DOC_SLUGS.flatMap((slug) => [
        {
          source: `/${slug}`,
          has: docsHost,
          destination: `https://kasetto.dev/docs/${slug}`,
          permanent: true,
        },
        {
          source: `/${slug}/`,
          has: docsHost,
          destination: `https://kasetto.dev/docs/${slug}`,
          permanent: true,
        },
      ]),
      {
        source: "/:path*",
        has: docsHost,
        destination: "https://kasetto.dev/:path*",
        permanent: true,
      },
    ];
  },
};

export default withMDX(nextConfig);
