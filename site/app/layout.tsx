import { Analytics } from "@vercel/analytics/next";
import { SpeedInsights } from "@vercel/speed-insights/next";
import { RootProvider } from "fumadocs-ui/provider";
import type { Metadata } from "next";
import { JetBrains_Mono } from "next/font/google";
import { SITE_URL } from "@/lib/site";
import { TopNav } from "./components/top-nav";
import "./globals.css";

// Variable axis covers wght 100-800; see SKILL.md for the brand weight ladder.
const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-jetbrains-mono",
});

const DESCRIPTION =
  "Sync skills, MCP servers, slash-commands, and instructions from Git into Claude Code, Cursor, Codex, Copilot, and 18 more agents. One YAML file, lock-pinned, in Rust.";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    template: "%s - Kasetto",
    default: "Kasetto - Declarative AI Agent Environment Manager",
  },
  description: DESCRIPTION,
  alternates: {
    canonical: "/",
  },
  openGraph: {
    type: "website",
    url: SITE_URL,
    siteName: "Kasetto",
    title: "Kasetto - Declarative AI Agent Environment Manager",
    description: DESCRIPTION,
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: "Kasetto - Declarative AI Agent Environment Manager",
    description: DESCRIPTION,
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html
      lang="en"
      className={`dark ${jetbrainsMono.className}`}
      data-theme="dark"
      suppressHydrationWarning
    >
      <body>
        <RootProvider theme={{ enabled: false }}>
          <a href="#main" className="skip-link">
            Skip to main content
          </a>
          <TopNav />
          <div id="main" tabIndex={-1}>
            {children}
          </div>
        </RootProvider>
        <Analytics />
        <SpeedInsights />
      </body>
    </html>
  );
}
