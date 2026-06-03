import { FaGithub } from "react-icons/fa";
import { GoStar, GoTag } from "react-icons/go";

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

  return (
    <nav className="top-nav">
      <div className="top-nav-inner">
        <a href="/" className="top-nav-brand" aria-label="Kasetto home">
          <img src="/favicon.svg" alt="" className="top-nav-logo-img" aria-hidden />
          <span className="top-nav-name">KASETTO</span>
        </a>
        <div className="top-nav-links">
          <a href="/docs" className="top-nav-link">
            DOCS{" "}
            <span className="top-nav-arrow" aria-hidden>
              ↗
            </span>
          </a>
          <a
            href="https://github.com/pivoshenko/kasetto"
            className="top-nav-repo"
            target="_blank"
            rel="noopener noreferrer"
            aria-label="GitHub repository"
          >
            <FaGithub className="top-nav-repo-icon" aria-hidden />
            <span className="top-nav-repo-name">pivoshenko/kasetto</span>
            {stars && (
              <span className="top-nav-stars">
                <GoStar aria-hidden /> {stars}
              </span>
            )}
            {version && (
              <span className="top-nav-version">
                <GoTag aria-hidden /> {version}
              </span>
            )}
          </a>
        </div>
      </div>
    </nav>
  );
}
