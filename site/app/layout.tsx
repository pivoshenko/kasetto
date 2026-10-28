import { Analytics } from "@vercel/analytics/next";
import { SpeedInsights } from "@vercel/speed-insights/next";
import { RootProvider } from "fumadocs-ui/provider";
import type { Metadata } from "next";
import { JetBrains_Mono } from "next/font/google";
import { SITE_URL } from "@/lib/site";
import { BackToTop } from "./components/back-to-top";
import { TopNav } from "./components/top-nav";
import "./globals.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/nav.css";
import "./styles/docs.css";
import "./styles/landing.css";

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-jetbrains-mono",
});

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    template: "%s - Kasetto",
    default: "Kasetto - Declarative AI Agent Environment Manager",
  },
  alternates: {
    canonical: "/",
  },
  openGraph: {
    type: "website",
    url: SITE_URL,
    siteName: "Kasetto",
    title: "Kasetto - Declarative AI Agent Environment Manager",
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: "Kasetto - Declarative AI Agent Environment Manager",
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html
      lang="en"
      className={`dark ${jetbrainsMono.variable}`}
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
          <BackToTop />
        </RootProvider>
        <Analytics />
        <SpeedInsights />
      </body>
    </html>
  );
}
