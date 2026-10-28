import { DocsLayout } from "fumadocs-ui/layouts/docs";
import type { ReactNode } from "react";
import { FaGithub } from "react-icons/fa";
import { GoTag } from "react-icons/go";
import { BrandLogo } from "@/app/components/brand-mark";
import { source } from "@/lib/source";

const REPO_URL = "https://github.com/pivoshenko/kasetto";

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <DocsLayout
      tree={source.pageTree}
      // The title only shows in the mobile subnav: at md+ the site TopNav carries the brand and
      // docs.css hides the sidebar copy, so there is one brand and one nav bar at every width
      nav={{
        title: (
          <span className="docs-brand">
            <BrandLogo className="brand-logo" />
            <span className="docs-brand-wordmark">kasetto</span>
          </span>
        ),
        url: "/",
        transparentMode: "none",
      }}
      links={[
        {
          text: "Changelog",
          url: `${REPO_URL}/releases`,
          external: true,
          icon: <GoTag />,
        },
        { text: "GitHub", url: REPO_URL, external: true, icon: <FaGithub /> },
      ]}
      sidebar={{ collapsible: false }}
      disableThemeSwitch
    >
      {children}
    </DocsLayout>
  );
}
