import type { Metadata } from "next";
import { JetBrains_Mono } from "next/font/google";
import "./globals.css";

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
  display: "swap",
});

export const metadata: Metadata = {
  title: "Kasetto — Declarative AI Agent Environment Manager",
  description:
    "One YAML config. 21 agent presets. Sync AI skills from GitHub, GitLab, Bitbucket, and local directories. Written in Rust.",
  openGraph: {
    title: "Kasetto",
    description: "Declarative AI agent environment manager, written in Rust.",
    url: "https://kasetto.dev",
    siteName: "Kasetto",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "Kasetto",
    description: "Declarative AI agent environment manager, written in Rust.",
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={jetbrainsMono.className} suppressHydrationWarning>
      <head>
        <script
          dangerouslySetInnerHTML={{
            __html: `(function(){var s=localStorage.getItem('theme');var t=s==='light'||s==='dark'?s:window.matchMedia('(prefers-color-scheme: light)').matches?'light':'dark';document.documentElement.setAttribute('data-theme',t);})();`,
          }}
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
