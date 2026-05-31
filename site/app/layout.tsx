import { RootProvider } from "fumadocs-ui/provider";
import type { Metadata } from "next";
import { JetBrains_Mono } from "next/font/google";
import { TopNav } from "./components/top-nav";
import "./globals.css";

// Variable axis covers wght 100–800 — see SKILL.md for the brand weight ladder.
const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-jetbrains-mono",
});

export const metadata: Metadata = {
  metadataBase: new URL("https://kasetto.dev"),
  title: {
    template: "%s — Kasetto",
    default: "Kasetto",
  },
  description: "Declarative AI Agent Environment Manager written in Rust",
  openGraph: {
    type: "website",
    url: "https://kasetto.dev",
    siteName: "Kasetto",
    title: "Kasetto",
    description: "Declarative AI Agent Environment Manager written in Rust",
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: "Kasetto",
    description: "Declarative AI Agent Environment Manager written in Rust",
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={jetbrainsMono.className} suppressHydrationWarning>
      <body>
        <RootProvider theme={{ enabled: true, defaultTheme: "dark", enableSystem: false }}>
          <a href="#main" className="skip-link">
            Skip to main content
          </a>
          <TopNav />
          <div id="main" tabIndex={-1}>
            {children}
          </div>
        </RootProvider>
      </body>
    </html>
  );
}
