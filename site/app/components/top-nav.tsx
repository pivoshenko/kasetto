import Link from "next/link";
import { Suspense } from "react";
import { FaGithub } from "react-icons/fa";
import { GoArrowUpRight, GoStarFill, GoTag } from "react-icons/go";
import { IoMenuOutline } from "react-icons/io5";
import { getLatestRelease, getStars } from "@/lib/github";
import { REPO_URL } from "@/lib/site";
import { BrandLogo } from "./brand-mark";

/** The star count and release tag beside the repository name; streams in after the shell. */
async function RepoStats() {
  const [stars, version] = await Promise.all([getStars(), getLatestRelease()]);
  return (
    <>
      {stars && (
        <span className="top-nav-repo-stat">
          <GoStarFill className="top-nav-star" aria-hidden />
          {stars}
        </span>
      )}
      {version && (
        <span className="top-nav-repo-stat top-nav-version">
          <GoTag className="top-nav-tag" aria-hidden />
          {version}
        </span>
      )}
    </>
  );
}

async function MenuVersion() {
  const version = await getLatestRelease();
  return version ? <span className="top-nav-menu-meta">{version}</span> : null;
}

/**
 * The nav renders synchronously; the GitHub numbers are the only async part and sit in their
 * own Suspense boundaries, so a cache miss there never delays the first byte of a page.
 */
export function TopNav() {
  return (
    <header className="top-nav">
      <nav className="top-nav-inner" aria-label="Main">
        <Link href="/" className="top-nav-brand" aria-label="Kasetto home">
          <BrandLogo className="brand-logo" />
          <span className="top-nav-wordmark">kasetto</span>
        </Link>

        <ul className="top-nav-links">
          <li>
            <Link href="/docs" className="top-nav-link">
              Docs
              <GoArrowUpRight className="top-nav-arrow" aria-hidden />
            </Link>
          </li>
        </ul>

        <a
          href={REPO_URL}
          className="top-nav-repo"
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Kasetto on GitHub"
        >
          <FaGithub className="top-nav-repo-icon" aria-hidden />
          <span className="top-nav-repo-name">pivoshenko/kasetto</span>
          <Suspense>
            <RepoStats />
          </Suspense>
        </a>

        {/* A native disclosure keeps the mobile menu usable without client JS */}
        <details className="top-nav-menu">
          <summary className="top-nav-menu-toggle" aria-label="Menu">
            <IoMenuOutline aria-hidden />
          </summary>
          <ul className="top-nav-menu-panel">
            <li>
              <Link href="/docs">Docs</Link>
            </li>
            <li>
              <a href={REPO_URL} target="_blank" rel="noopener noreferrer">
                GitHub
                <Suspense>
                  <MenuVersion />
                </Suspense>
              </a>
            </li>
          </ul>
        </details>
      </nav>
    </header>
  );
}
