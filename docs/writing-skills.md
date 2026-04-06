# Writing Skills

A skill is a directory containing a `SKILL.md` file. This page explains how Kasetto discovers
skills and what format `SKILL.md` should follow.

## Directory Layout

Kasetto looks for skills in two locations within a source:

```
repo-root/
├── my-skill/
│   └── SKILL.md        ← discovered
├── skills/
│   ├── another-skill/
│   │   └── SKILL.md    ← discovered
│   └── third-skill/
│       └── SKILL.md    ← discovered
└── README.md           ← ignored (no SKILL.md)
```

Any subdirectory of the root or of a `skills/` folder that contains a `SKILL.md` file is
recognized as a skill. The directory name becomes the skill's identifier.

!!! important

    A `SKILL.md` file is required. Directories without one are silently skipped.

## SKILL.md Format

`SKILL.md` is a markdown file with optional YAML frontmatter:

```markdown
---
name: Code Reviewer
description: Reviews pull requests for common issues and style violations.
---

# Code Reviewer

Detailed instructions for the AI agent go here. This is the content that gets
installed into the agent's skill directory.
```

### Frontmatter Fields

| Field         | Required | Description                                        |
| ------------- | -------- | -------------------------------------------------- |
| `name`        | no       | Display name shown in `kst list` and `kst doctor`  |
| `description` | no       | Short description shown in the interactive browser |

Both fields are optional. If omitted, Kasetto falls back to parsing the markdown body:

- **Name:** First `#` heading in the document, or the directory name if no heading exists.
- **Description:** First non-empty, non-heading paragraph, or `"No description."` if none found.

### Minimal Example

A `SKILL.md` with no frontmatter works fine:

```markdown
# My Skill

You are an expert at doing X. When the user asks you to...
```

Kasetto will use `"My Skill"` as the display name and the first paragraph as the description.

## Referencing Skills in Config

Skills are referenced by their directory name in `kasetto.yaml`:

```yaml
skills:
  - source: https://github.com/org/skill-pack
    skills:
      - my-skill           # matches repo-root/my-skill/ or repo-root/skills/my-skill/
      - another-skill
```

Use `"*"` to install all discovered skills from a source:

```yaml
skills:
  - source: ~/Development/my-skills
    skills: "*"
```

## Custom Source Path

If a skill lives in a non-standard location within the repo, use the `path` field:

```yaml
skills:
  - source: https://github.com/acme/monorepo
    skills:
      - name: my-skill
        path: tools/ai-skills    # looks in tools/ai-skills/my-skill/SKILL.md
```
