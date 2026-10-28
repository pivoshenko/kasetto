import type { IconType } from "react-icons";
import { FaFolderOpen, FaServer } from "react-icons/fa6";
import { SiBitbucket, SiCodeberg, SiForgejo, SiGitea, SiGithub, SiGitlab } from "react-icons/si";

// Mirrors the host classes in src/source/hosts.rs and the token hints in src/source/auth.rs;
// `note` is the env var a private instance reads
const FORGES: { icon: IconType; name: string; note: string }[] = [
  { icon: SiGithub, name: "GitHub", note: "GITHUB_TOKEN" },
  { icon: SiGitlab, name: "GitLab", note: "GITLAB_TOKEN" },
  { icon: SiBitbucket, name: "Bitbucket", note: "BITBUCKET_TOKEN" },
  { icon: SiCodeberg, name: "Codeberg", note: "CODEBERG_TOKEN" },
  { icon: SiGitea, name: "Gitea", note: "GITEA_TOKEN" },
  { icon: SiForgejo, name: "Forgejo", note: "FORGEJO_TOKEN" },
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
            <span className="lp-wall-note">{f.note}</span>
          </li>
        );
      })}
    </ul>
  );
}
