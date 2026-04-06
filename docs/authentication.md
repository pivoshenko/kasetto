# Authentication

Kasetto supports pulling skills and MCP configs from private repositories and remote configs.
Authentication is configured via environment variables - no login command or credentials file needed.

## Supported Git Hosts

| Host               | Example URL                                         |
| ------------------ | --------------------------------------------------- |
| GitHub             | `https://github.com/org/repo`                       |
| GitHub Enterprise  | `https://ghe.example.com/org/repo`                  |
| GitLab             | `https://gitlab.com/group/project`                  |
| GitLab self-hosted | `https://gitlab.example.com/group/subgroup/project` |
| Bitbucket Cloud    | `https://bitbucket.org/workspace/repo`              |
| Codeberg           | `https://codeberg.org/owner/repo`                   |
| Gitea              | `https://gitea.com/owner/repo`                      |
| Forgejo            | `https://forgejo.org/owner/repo`                    |

### Host Detection Rules

Kasetto identifies the git host from the URL's hostname:

- **GitLab** - `gitlab.com`, any subdomain of `gitlab.com` (e.g., `sub.gitlab.com`), or any
  hostname starting with `gitlab.` (e.g., `gitlab.example.com`).
- **Bitbucket** - `bitbucket.org` or `www.bitbucket.org`.
- **Codeberg / Gitea / Forgejo** - `codeberg.org`, `gitea.com`, `forgejo.org` (and their `www.`
  variants).
- **GitHub** - `github.com` and any other hostname not matching the rules above.

!!! note

    Any unrecognized hostname with an `owner/repo` path (2 segments) is treated as **GitHub
    Enterprise**. This means `ghe.example.com/acme/skills` works automatically with
    `GITHUB_TOKEN`. Unrecognized hostnames with 3+ path segments (e.g.,
    `git.example.com/group/sub/repo`) are treated as **GitLab-style**.

!!! warning

    Self-hosted Gitea or Forgejo instances (e.g., `gitea.mycompany.com`) are **not**
    auto-detected. They will be treated as GitHub Enterprise. If your self-hosted instance
    uses the GitHub-compatible API layout this may still work, but the `GITEA_TOKEN` variable
    will not be applied. Use `GITHUB_TOKEN` instead, or open an issue if you need explicit
    support for a custom Gitea domain.

## Environment Variables

### GitHub

| Variable       | Description                                 |
| -------------- | ------------------------------------------- |
| `GITHUB_TOKEN` | Personal access token or fine-grained token |
| `GH_TOKEN`     | Fallback if `GITHUB_TOKEN` is not set       |

Works for both `github.com` and GitHub Enterprise Server.

```console
$ export GITHUB_TOKEN=ghp_...
$ kst sync --config kasetto.yaml
```

### GitLab

| Variable       | Description                                            |
| -------------- | ------------------------------------------------------ |
| `GITLAB_TOKEN` | Personal or project access token                       |
| `CI_JOB_TOKEN` | Fallback - automatically set in GitLab CI/CD pipelines |

Works for `gitlab.com` and self-hosted instances whose hostname starts with `gitlab.`.

```console
$ export GITLAB_TOKEN=glpat-...
$ kst sync --config kasetto.yaml
```

### Bitbucket Cloud

Bitbucket supports two authentication methods:

**Method 1 - API token:**

| Variable          | Description           |
| ----------------- | --------------------- |
| `BITBUCKET_EMAIL` | Account email address |
| `BITBUCKET_TOKEN` | Atlassian API token   |

**Method 2 - App password:**

| Variable                 | Description                                                 |
| ------------------------ | ----------------------------------------------------------- |
| `BITBUCKET_USERNAME`     | Bitbucket username                                          |
| `BITBUCKET_APP_PASSWORD` | App password (create at Bitbucket Settings > App passwords) |

Method 1 is tried first. If those variables are not set, method 2 is used.

### Codeberg / Gitea / Forgejo

| Variable         | Description                             |
| ---------------- | --------------------------------------- |
| `GITEA_TOKEN`    | Personal access token                   |
| `CODEBERG_TOKEN` | Fallback if `GITEA_TOKEN` is not set    |
| `FORGEJO_TOKEN`  | Fallback if neither of the above is set |

All three variables are checked in order. The first one found is used for any Gitea-family host.

## Remote Configs

Authentication also applies when fetching configs via `--config <url>`. The token is selected
based on the URL's hostname, using the same detection rules above.

```console
$ export GITHUB_TOKEN=ghp_...
$ kst sync --config https://github.com/org/private-repo/raw/main/kasetto.yaml
```

If the remote config URL points to a private resource and no matching token is set, Kasetto
reports an HTTP error with a hint about which environment variable to set.

## Display Variables

| Variable   | Effect                                                                    |
| ---------- | ------------------------------------------------------------------------- |
| `NO_TUI`   | Disables interactive screens (home menu, list browser). Set to any value. |
| `NO_COLOR` | Disables colored output. Set to any value.                                |
