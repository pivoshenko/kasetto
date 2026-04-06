# Authentication

Kasetto supports pulling skills and MCP configs from private repositories and remote configs.
Authentication is configured via environment variables - no login command or credentials file needed.

## Supported git hosts

| Host                           | Example URL                            |
| ------------------------------ | -------------------------------------- |
| GitHub (including Enterprise)  | `https://github.com/org/repo`          |
| GitLab (including self-hosted) | `https://gitlab.com/group/project`     |
| Bitbucket Cloud                | `https://bitbucket.org/workspace/repo` |
| Codeberg                       | `https://codeberg.org/owner/repo`      |
| Gitea                          | `https://gitea.com/owner/repo`         |
| Forgejo                        | `https://forgejo.org/owner/repo`       |

GitHub Enterprise is auto-detected for any hostname with a standard `owner/repo` path layout.
GitLab self-hosted instances are detected when the hostname contains `gitlab`.

## Environment variables

### GitHub

| Variable       | Description                                 |
| -------------- | ------------------------------------------- |
| `GITHUB_TOKEN` | Personal access token or fine-grained token |
| `GH_TOKEN`     | Fallback if `GITHUB_TOKEN` is not set       |

Works for both `github.com` and GitHub Enterprise Server.

### GitLab

| Variable       | Description                                            |
| -------------- | ------------------------------------------------------ |
| `GITLAB_TOKEN` | Personal or project access token                       |
| `CI_JOB_TOKEN` | Fallback - automatically set in GitLab CI/CD pipelines |

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

## Remote configs

Authentication also applies when fetching configs via `--config <url>`. The token is selected
based on the URL's hostname, using the same variables listed above.

```console
$ export GITHUB_TOKEN=ghp_...
$ kst sync --config https://github.com/org/private-repo/raw/main/kasetto.yaml
```

If the remote config URL points to a private resource and no matching token is set, Kasetto
reports an HTTP error with a hint about which environment variable to set.

## Display variables

| Variable   | Effect                                                                    |
| ---------- | ------------------------------------------------------------------------- |
| `NO_TUI`   | Disables interactive screens (home menu, list browser). Set to any value. |
| `NO_COLOR` | Disables colored output. Set to any value.                                |
