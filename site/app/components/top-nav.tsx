import { FaGithub } from "react-icons/fa";
import { GoArrowUpRight, GoStarFill, GoTag } from "react-icons/go";
import { IoMenuOutline } from "react-icons/io5";
import { BrandLogo } from "./brand-mark";

const REPO_URL = "https://github.com/pivoshenko/kasetto";

async function getRepoMeta(): Promise<{ stars: string | null; version: string | null }> {
  const [stars, version] = await Promise.all([getStars(), getLatestRelease()]);
  return { stars, version };
}

async function getStars(): Promise<string | null> {
  try {
    const res = await fetch("https://api.github.com/repos/pivoshenko/kasetto", {
      next: { revalidate: 3600 },
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { stargazers_count: number };
    const n = data.stargazers_count;
    return n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n);
  } catch {
    return null;
  }
}

async function getLatestRelease(): Promise<string | null> {
  try {
    const res = await fetch("https://api.github.com/repos/pivoshenko/kasetto/releases/latest", {
      next: { revalidate: 3600 },
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { tag_name?: string };
    return data.tag_name ?? null;
  } catch {
    return null;
  }
}

export async function TopNav() {
  const { stars, version } = await getRepoMeta();
  const repoLabel = [
    "Kasetto on GitHub",
    stars && `${stars} stars`,
    version && `latest release ${version}`,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <header className="top-nav">
      <nav className="top-nav-inner" aria-label="Main">
        <a href="/" className="top-nav-brand" aria-label="Kasetto home">
          <BrandLogo className="brand-logo" />
          <span className="top-nav-wordmark">kasetto</span>
        </a>

        <ul className="top-nav-links">
          <li>
            <a href="/docs" className="top-nav-link">
              Docs
              <GoArrowUpRight className="top-nav-arrow" aria-hidden />
            </a>
          </li>
        </ul>

        <a
          href={REPO_URL}
          className="top-nav-repo"
          target="_blank"
          rel="noopener noreferrer"
          aria-label={repoLabel}
        >
          <FaGithub className="top-nav-repo-icon" aria-hidden />
          <span className="top-nav-repo-name">pivoshenko/kasetto</span>
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
        </a>

        {/* A native disclosure keeps the mobile menu usable without client JS */}
        <details className="top-nav-menu">
          <summary className="top-nav-menu-toggle" aria-label="Menu">
            <IoMenuOutline aria-hidden />
          </summary>
          <ul className="top-nav-menu-panel">
            <li>
              <a href="/docs">Docs</a>
            </li>
            <li>
              <a href={REPO_URL} target="_blank" rel="noopener noreferrer">
                GitHub
                {version && <span className="top-nav-menu-meta">{version}</span>}
              </a>
            </li>
          </ul>
        </details>
      </nav>
    </header>
  );
}
