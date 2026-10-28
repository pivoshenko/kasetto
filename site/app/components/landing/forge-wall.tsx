import type { IconType } from "react-icons";
import { FaFolderOpen, FaServer } from "react-icons/fa6";
import { SiBitbucket, SiCodeberg, SiForgejo, SiGitea, SiGithub, SiGitlab } from "react-icons/si";

// Mirrors the host classes in src/source/hosts.rs and the token hints in src/source/auth.rs;
// `token` is the env var a private repository reads, `note` stands in where there is none
const FORGES: { icon: IconType; name: string; token?: string; note?: string }[] = [
  { icon: SiGithub, name: "GitHub", token: "GITHUB_TOKEN" },
  { icon: SiGitlab, name: "GitLab", token: "GITLAB_TOKEN" },
  { icon: SiBitbucket, name: "Bitbucket", token: "BITBUCKET_TOKEN" },
  { icon: SiCodeberg, name: "Codeberg", token: "CODEBERG_TOKEN" },
  { icon: SiGitea, name: "Gitea", token: "GITEA_TOKEN" },
  { icon: SiForgejo, name: "Forgejo", token: "FORGEJO_TOKEN" },
  { icon: FaServer, name: "Self-hosted", note: "GitHub Enterprise, GitLab, Gitea" },
  { icon: FaFolderOpen, name: "Local path", note: "no network at all" },
];

export function ForgeWall() {
  return (
    <ul className="lp-wall lp-wall--forges">
      {FORGES.map((f) => {
        const Icon = f.icon;
        return (
          <li key={f.name} className="lp-wall-cell">
            <Icon className="lp-wall-icon" aria-hidden />
            <span className="lp-wall-name">{f.name}</span>
            <span className="lp-wall-note">
              {f.token ? (
                <>
                  {f.token} <span className="lp-wall-comment"># private repository</span>
                </>
              ) : (
                f.note
              )}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
