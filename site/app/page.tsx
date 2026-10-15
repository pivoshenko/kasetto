import { FaGithub, FaGlobe, FaLinkedinIn } from "react-icons/fa";
import { GoArrowRight, GoArrowUpRight, GoStar } from "react-icons/go";
import { softwareApplicationJsonLd } from "@/lib/structured-data";
import { AGENTS, AgentsGrid } from "./components/agents-grid";
import { BrandLogo, BrandMark } from "./components/brand-mark";
import { CopyButton } from "./components/copy-button";
import { ConfigExample, FeatureList } from "./components/feature-tabs";
import { HeroTerminal } from "./components/hero-terminal";
import { JsonLd } from "./components/json-ld";
import { AgentIcon } from "./components/landing/agent-icon";
import { HeroCassette } from "./components/landing/hero-cassette";
import { LockExample } from "./components/landing/lock-example";
import { SecretSources } from "./components/secret-sources";

const REPO = "https://github.com/pivoshenko/kasetto";
const CURL = "curl -fsSL kasetto.dev/install | sh";
const PLACEHOLDER = `\${kst_...}`;

const INSTALL = [
  { label: "macOS / Linux", cmd: CURL },
  {
    label: "Windows (PowerShell)",
    cmd: 'powershell -ExecutionPolicy Bypass -c "irm kasetto.dev/install.ps1 | iex"',
  },
  {
    label: "Homebrew",
    cmd: "brew tap pivoshenko/tap\nbrew trust pivoshenko/tap\nbrew install kasetto",
  },
  { label: "Cargo", cmd: "cargo install kasetto" },
];

const FOOTER = [
  {
    title: "Docs",
    links: [
      { label: "Quick start", href: "/docs" },
      { label: "Installation", href: "/docs/installation" },
      { label: "Configuration", href: "/docs/configuration" },
      { label: "Commands", href: "/docs/commands" },
      { label: "Cookbook", href: "/docs/cookbook" },
    ],
  },
  {
    title: "Project",
    links: [
      { label: "Repository", href: REPO },
      { label: "Releases", href: `${REPO}/releases` },
      { label: "Issues", href: `${REPO}/issues` },
      { label: "Contributing", href: `${REPO}/blob/main/CONTRIBUTING.md` },
    ],
  },
  {
    title: "More",
    links: [
      { label: "Supported agents", href: "/docs/agents" },
      { label: "FAQ", href: "/docs/faq" },
      { label: "Security", href: "/docs/security" },
      { label: "CI & automation", href: "/docs/ci" },
    ],
  },
];

async function getRepoData(): Promise<{ stars: string | null; version: string | null }> {
  try {
    const [repoRes, releaseRes] = await Promise.all([
      fetch("https://api.github.com/repos/pivoshenko/kasetto", {
        next: { revalidate: 3600 },
      }),
      fetch("https://api.github.com/repos/pivoshenko/kasetto/releases/latest", {
        next: { revalidate: 3600 },
      }),
    ]);
    const repo = repoRes.ok ? ((await repoRes.json()) as { stargazers_count: number }) : null;
    const release = releaseRes.ok ? ((await releaseRes.json()) as { tag_name: string }) : null;
    const n = repo?.stargazers_count ?? 0;
    return {
      stars: repo ? (n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n)) : null,
      version: release?.tag_name ?? null,
    };
  } catch {
    return { stars: null, version: null };
  }
}

function External({
  href,
  className,
  label,
  children,
}: {
  href: string;
  className?: string;
  label?: string;
  children: React.ReactNode;
}) {
  return (
    <a
      href={href}
      className={className}
      target="_blank"
      rel="noopener noreferrer"
      aria-label={label}
    >
      {children}
    </a>
  );
}

function Eyebrow({ index, children }: { index?: string; children: React.ReactNode }) {
  return (
    <p className="lp-eyebrow">
      {index ? <span className="lp-eyebrow-index">{index}</span> : null}
      {children}
    </p>
  );
}

function Feature({
  id,
  index,
  eyebrow,
  title,
  body,
  points,
  link,
  flip,
  children,
}: {
  id: string;
  index: string;
  eyebrow: string;
  title: React.ReactNode;
  body: React.ReactNode;
  points: React.ReactNode[];
  link: { href: string; label: string };
  flip?: boolean;
  children: React.ReactNode;
}) {
  return (
    <section
      id={id}
      className="lp-feature"
      data-flip={flip ? "" : undefined}
      aria-labelledby={`${id}-title`}
    >
      <div className="lp-feature-copy">
        <Eyebrow index={index}>{eyebrow}</Eyebrow>
        <h2 id={`${id}-title`} className="lp-h2 display">
          {title}
        </h2>
        <p className="lp-lede">{body}</p>
        <ul className="lp-points">
          {points.map((p, i) => (
            <li key={i}>{p}</li>
          ))}
        </ul>
        <a href={link.href} className="lp-textlink">
          {link.label} <GoArrowRight aria-hidden />
        </a>
      </div>
      <div className="lp-feature-art">{children}</div>
    </section>
  );
}

export default async function Page() {
  const { stars, version } = await getRepoData();

  return (
    <div className="lp">
      <JsonLd data={softwareApplicationJsonLd()} />

      {/* == Hero == */}
      <header className="lp-hero">
        <div className="lp-hero-copy">
          <Eyebrow>Declarative AI environment manager</Eyebrow>
          <h1 className="lp-h1 display">
            <span className="lp-h1-line">Your AI environment,</span>{" "}
            <span className="lp-h1-line">
              <em>under control</em>
            </span>
          </h1>
          <div className="lp-actions">
            <a href="/docs" className="lp-btn lp-btn--primary">
              Read the docs <GoArrowRight aria-hidden />
            </a>
            <External href={REPO} className="lp-btn">
              <FaGithub aria-hidden /> Star on GitHub
              {stars ? (
                <span className="lp-btn-count">
                  <GoStar aria-hidden /> {stars}
                </span>
              ) : null}
            </External>
          </div>
          <div className="lp-cmd">
            <span className="lp-cmd-prompt" aria-hidden="true">
              $
            </span>
            <code className="lp-cmd-text">{CURL}</code>
            <CopyButton text={CURL} label="Copy" />
          </div>
          <ul className="lp-hero-meta">
            {version ? <li>Latest {version}</li> : null}
            <li>macOS, Linux, Windows</li>
            <li>MIT / Apache 2.0</li>
          </ul>
        </div>
        <div className="lp-hero-art">
          <HeroCassette className="lp-cassette" />
        </div>
      </header>

      {/* == Agent Strip == */}
      <section className="lp-strip" aria-label="Supported agents">
        <div className="lp-marquee">
          {/* Two identical lists side by side; the track slides by exactly one list width, so the loop has no seam */}
          <div className="lp-marquee-track">
            {[false, true].map((copy) => (
              <ul key={String(copy)} className="lp-strip-list" aria-hidden={copy || undefined}>
                {AGENTS.map((a) => (
                  <li key={a.value}>
                    <AgentIcon preset={a.value} name={a.name} className="lp-strip-icon" />
                    <span className="lp-strip-name">{a.name}</span>
                  </li>
                ))}
              </ul>
            ))}
          </div>
        </div>
        <a href="#agents" className="lp-strip-more">
          See all <GoArrowRight aria-hidden />
        </a>
      </section>

      {/* == How It Works == */}
      <Feature
        id="declare"
        index="01"
        eyebrow="Declare"
        title={
          <>
            See everything in <em>one</em> file
          </>
        }
        body={
          <>
            <code>kasetto.yaml</code> is the full inventory of your AI environment: every skill,
            slash command, MCP server, and instruction, and the GitHub, GitLab, Bitbucket, or
            self-hosted repo it comes from.
          </>
        }
        points={[
          <>
            Four asset kinds, one list each: <code>skills</code>, <code>commands</code>,{" "}
            <code>mcps</code>, <code>instructions</code>
          </>,
          <>
            Pin a tag or commit with <code>ref:</code>, or follow a <code>branch:</code>
          </>,
          <>
            Share a team base with <code>extends:</code> and layer your own entries on top
          </>,
          <>
            <code>kst add</code> and <code>kst remove</code> edit the file and keep your comments
          </>,
        ]}
        link={{ href: "/docs/configuration", label: "Configuration reference" }}
      >
        <ConfigExample />
      </Feature>

      <Feature
        id="lock"
        index="02"
        eyebrow="Lock"
        flip
        title={
          <>
            Know exactly what is <em>installed</em>
          </>
        }
        body={
          <>
            Every sync records what it installed in <code>kasetto.lock</code>: the source, the
            resolved revision, where it landed, and a content hash. Commit it next to the config,
            and every change to your AI environment shows up in code review.
          </>
        }
        points={[
          <>
            <code>--locked</code> never touches the network and fails if the lock cannot satisfy the
            config
          </>,
          <>
            <code>--update</code> re-resolves moving branches, for every source or only the assets
            you name
          </>,
          <>Paths are stored relative to the project, so the lock is portable across machines</>,
        ]}
        link={{ href: "/docs/how-sync-works", label: "How sync works" }}
      >
        <LockExample />
      </Feature>

      <Feature
        id="sync"
        index="03"
        eyebrow="Sync"
        title={
          <>
            The same setup on <em>every</em> machine
          </>
        }
        body={
          <>
            <code>kst sync</code> makes a machine match the lock: it fetches each source, writes
            every asset where your agent expects it, and changes only what differs. New teammates
            run one command and get the environment the rest of the team already has.
          </>
        }
        points={[
          <>
            Skills, commands, MCP settings, and instructions land where each agent looks for them
          </>,
          <>Content is hashed and diffed against the lock, so unchanged assets are never touched</>,
          <>Anything you drop from the config is pruned on the next sync, so nothing lingers</>,
          <>
            Shared files such as <code>CLAUDE.md</code> get managed blocks, so your own edits
            survive
          </>,
        ]}
        link={{ href: "/docs/commands", label: "Command reference" }}
      >
        <HeroTerminal />
      </Feature>

      <Feature
        id="secrets"
        index="04"
        eyebrow="Secrets"
        flip
        title={
          <>
            Tokens never <em>touch</em> git
          </>
        }
        body={
          <>
            MCP servers need API keys. Write a <code>{PLACEHOLDER}</code> placeholder instead, and
            Kasetto resolves it at sync time from your environment, a credentials file, or the
            secret manager you already use.
          </>
        }
        points={[
          <>Resolved values are written only to the agent&apos;s config, never to the lock</>,
          <>Secret managers are read through the CLI session you are already signed in to</>,
          <>Private repos authenticate with tokens from env vars; Kasetto stores no credentials</>,
        ]}
        link={{ href: "/docs/secrets", label: "Secrets guide" }}
      >
        <SecretSources />
      </Feature>

      {/* == Feature Grid == */}
      <section className="lp-section" aria-labelledby="more-title">
        <div className="lp-section-head">
          <Eyebrow>Built for teams and CI</Eyebrow>
          <h2 id="more-title" className="lp-h2 display">
            The rest of the <em>spec</em> sheet
          </h2>
        </div>
        <FeatureList />
      </section>

      {/* == Agent Wall == */}
      <section id="agents" className="lp-section" aria-labelledby="agents-title">
        <div className="lp-section-head lp-section-head--split">
          <div>
            <Eyebrow>Agents</Eyebrow>
            <h2 id="agents-title" className="lp-h2 display">
              Works with the agent <em>you</em> use
            </h2>
          </div>
          <p className="lp-lede">
            Presets for {AGENTS.length} agents know where each keeps skills, commands, MCP servers,
            and instructions, globally and per project. Name yours under <code>agent:</code> and
            switch whenever you like: the environment comes with you.
          </p>
        </div>
        <AgentsGrid />
      </section>

      {/* == Install == */}
      <section id="install" className="lp-section lp-install" aria-labelledby="install-title">
        <div className="lp-install-copy">
          <Eyebrow>Install</Eyebrow>
          <h2 id="install-title" className="lp-h2 display">
            One binary. <em>Any</em> machine
          </h2>
          <p className="lp-lede">
            A single static binary for macOS, Linux, and Windows, with JSON output and real exit
            codes for CI. It installs as both <code>kasetto</code> and the short alias{" "}
            <code>kst</code>.
          </p>
          <div className="lp-install-links">
            <a href="/docs/installation" className="lp-textlink">
              Installation guide <GoArrowRight aria-hidden />
            </a>
            <External href={`${REPO}/releases`} className="lp-textlink">
              {version ? `Release ${version}` : "Releases"} <GoArrowUpRight aria-hidden />
            </External>
          </div>
        </div>
        <ul className="lp-install-list">
          {INSTALL.map((m) => (
            <li key={m.label} className="lp-install-row">
              <span className="lp-install-label">{m.label}</span>
              <pre className="lp-install-cmd">
                <code>{m.cmd}</code>
              </pre>
              <CopyButton text={m.cmd} label="Copy" />
            </li>
          ))}
        </ul>
      </section>

      {/* == Closing CTA == */}
      <section className="lp-cta" aria-labelledby="cta-title">
        <div className="lp-cta-inner">
          <BrandMark className="lp-cta-mark" />
          <div className="lp-cta-copy">
            <h2 id="cta-title" className="lp-h2 display">
              Put your AI environment <em>on tape</em>
            </h2>
            <p className="lp-lede">
              Declare it once, review every change, and run the same setup across your whole team.
            </p>
          </div>
          <div className="lp-actions lp-cta-actions">
            <a href="/docs" className="lp-btn lp-btn--primary">
              Get started <GoArrowRight aria-hidden />
            </a>
            <External href={REPO} className="lp-btn">
              <FaGithub aria-hidden /> View on GitHub
            </External>
          </div>
        </div>
      </section>

      {/* == Footer == */}
      <footer className="lp-footer">
        <div className="lp-footer-grid">
          <div className="lp-footer-brand">
            <span className="lp-footer-name display">
              <BrandLogo className="brand-logo" /> kasetto
            </span>
            <p className="lp-footer-tagline">
              Declarative AI agent environment manager. Written in Rust.
            </p>
            <div className="lp-footer-social">
              <External href="https://pivoshenko.dev" label="Author's blog">
                <FaGlobe aria-hidden />
              </External>
              <External href={REPO} label="GitHub">
                <FaGithub aria-hidden />
              </External>
              <External href="https://linkedin.com/in/pivoshenko" label="LinkedIn">
                <FaLinkedinIn aria-hidden />
              </External>
            </div>
            <a
              href="https://backlinklog.com/listing/kasetto.dev?utm_source=backlinklog&utm_medium=badge"
              className="lp-footer-badge"
            >
              <img
                src="https://backlinklog.com/badge/kasetto.dev.svg"
                alt="Listed on BacklinkLog"
                width={160}
                height={40}
              />
            </a>
          </div>

          {FOOTER.map((col) => (
            <nav key={col.title} className="lp-footer-col" aria-label={col.title}>
              <h3 className="lp-footer-title">{col.title}</h3>
              <ul>
                {col.links.map((l) => (
                  <li key={l.label}>
                    {l.href.startsWith("http") ? (
                      <External href={l.href}>{l.label}</External>
                    ) : (
                      <a href={l.href}>{l.label}</a>
                    )}
                  </li>
                ))}
              </ul>
            </nav>
          ))}
        </div>

        <div className="lp-footer-bottom">
          <span>
            2026 Volodymyr Pivoshenko{" "}
            <a href="mailto:contact@pivoshenko.dev">&lt;contact@pivoshenko.dev&gt;</a>
          </span>
          <span>MIT - Apache 2.0</span>
        </div>
      </footer>
    </div>
  );
}
