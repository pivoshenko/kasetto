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

/** Every forge as a compact two-column list: the name and the env var a private one reads. */
export function ForgeList() {
  return (
    <ul className="lp-roster" data-cols="1">
      {FORGES.map((f) => {
        const Icon = f.icon;
        return (
          <li key={f.name}>
            <Icon aria-hidden />
            <span className="lp-roster-name">{f.name}</span>
            <code className="lp-roster-note">{f.note}</code>
          </li>
        );
      })}
    </ul>
  );
}
