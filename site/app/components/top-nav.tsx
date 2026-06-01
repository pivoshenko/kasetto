import { FaGithub } from "react-icons/fa";
import { GoStar } from "react-icons/go";

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

export async function TopNav() {
  const stars = await getStars();

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
          </a>
        </div>
      </div>
    </nav>
  );
}
