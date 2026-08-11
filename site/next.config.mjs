import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

const DOC_SLUGS = [
  "agents",
  "authentication",
  "ci",
  "commands",
  "configuration",
  "cookbook",
  "faq",
  "how-sync-works",
  "installation",
  "security",
  "sharing-instructions",
  "sync-flow",
  "vs-alternatives",
  "writing-skills",
];

const docsHost = [{ type: "host", value: "docs.kasetto.dev" }];

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "X-Frame-Options", value: "DENY" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          {
            key: "Permissions-Policy",
            value: "camera=(), microphone=(), geolocation=()",
          },
        ],
      },
      // The raw-Markdown mirrors duplicate every docs page verbatim. They exist
      // for humans and LLM agents that fetch them directly, so keep them
      // reachable but out of the index - otherwise each page competes with its
      // own HTML twin.
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
      // Everything else on the docs host, which otherwise serves the whole site
      // a second time: /docs/<path>, /llms.txt, /sitemap.xml. Last, so the
      // legacy bare-slug rules above still win.
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
