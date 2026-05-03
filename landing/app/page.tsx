import { FaGithub, FaLinkedinIn } from "react-icons/fa";
import { GoStar } from "react-icons/go";
import { AgentsGrid } from "./components/agents-grid";
import { CopyButton } from "./components/copy-button";
import { ConfigExample, FeatureList } from "./components/feature-tabs";
import { ThemeToggle } from "./components/theme-toggle";

const INSTALL = [
  {
    label: "MACOS / LINUX",
    cmd: "curl -fsSL kasetto.dev/install | sh",
  },
  {
    label: "WINDOWS (POWERSHELL)",
    cmd: 'powershell -ExecutionPolicy Bypass -c "irm kasetto.dev/install.ps1 | iex"',
  },
  {
    label: "HOMEBREW",
    cmd: "brew install pivoshenko/tap/kasetto",
  },
  {
    label: "CARGO",
    cmd: "cargo install kasetto",
  },
];

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

export default async function Page() {
  const stars = await getStars();

  return (
    <div className="page-wrap">
      {/* ── Logo ── */}
      <div className="logo-wrap">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src="/logo.svg" alt="Kasetto" className="logo-img" />
      </div>

      {/* ── Hero ── */}
      <section className="section">
        <div className="grid-box">
          {/* Tagline */}
          <div className="hero-row">
            <p className="hero-tagline">DECLARATIVE AI AGENT ENVIRONMENT MANAGER</p>
          </div>

          {/* GET STARTED */}
          <div className="action-row">
            <span className="action-label">GET STARTED</span>
            <div className="install-right">
              <code className="install-cmd">curl -fsSL kasetto.dev/install | sh</code>
              <CopyButton text="curl -fsSL kasetto.dev/install | sh" />
            </div>
          </div>

          {/* GitHub */}
          <div className="action-row">
            <span className="action-label">
              GITHUB
              {stars && (
                <span className="accent-warm star-count">
                  <GoStar aria-hidden="true" />
                  {stars}
                </span>
              )}
            </span>
            <a
              href="https://github.com/pivoshenko/kasetto"
              className="action-link"
              target="_blank"
              rel="noopener noreferrer"
            >
              github.com/pivoshenko/kasetto <span className="arrow">↗</span>
            </a>
          </div>

          {/* Docs */}
          <div className="action-row">
            <span className="action-label">DOCS</span>
            <a
              href="https://docs.kasetto.dev"
              className="action-link"
              target="_blank"
              rel="noopener noreferrer"
            >
              docs.kasetto.dev <span className="arrow">↗</span>
            </a>
          </div>
        </div>
      </section>

      {/* ── Features ── */}
      <section className="section">
        <p className="section-label">FEATURES</p>
        <FeatureList />
      </section>

      {/* ── Supported Agents ── */}
      <section className="section">
        <p className="section-label">SUPPORTED AGENTS</p>
        <AgentsGrid />
      </section>

      {/* ── Config ── */}
      <section className="section">
        <p className="section-label">EXAMPLE</p>
        <ConfigExample />
      </section>

      {/* ── Install ── */}
      <section className="section">
        <p className="section-label">INSTALL</p>
        <div className="grid-box">
          {INSTALL.map((m) => (
            <div key={m.label} className="install-row">
              <span className="install-label">{m.label}</span>
              <div className="install-right">
                <code className="install-cmd">{m.cmd}</code>
                <CopyButton text={m.cmd} />
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* ── Footer ── */}
      <footer className="footer">
        <span className="footer-left">
          2026 Volodymyr Pivoshenko &lt;contact@pivoshenko.dev&gt;
        </span>
        <div className="footer-links">
          <ThemeToggle />
          <a
            href="https://github.com/pivoshenko"
            className="footer-icon"
            target="_blank"
            rel="noopener noreferrer"
            aria-label="GitHub"
          >
            <FaGithub />
          </a>
          <a
            href="https://linkedin.com/in/pivoshenko"
            className="footer-icon"
            target="_blank"
            rel="noopener noreferrer"
            aria-label="LinkedIn"
          >
            <FaLinkedinIn />
          </a>
        </div>
      </footer>
    </div>
  );
}
