# Writing Skills

Skills are just directories with a `SKILL.md` file inside. Here's how to structure yours and what Kasetto looks for.

## Directory Layout

Kasetto looks in two places within a source:

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

Any subdirectory at the root or inside `skills/` that contains a `SKILL.md` is picked up as a skill. The directory name is used as the skill's identifier.

!!! important

    `SKILL.md` is required. Directories without one are silently skipped.

## SKILL.md Format

`SKILL.md` is a markdown file. YAML frontmatter is optional but gives you control over how the skill appears in `kst list`:

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

Both are optional. If you skip them, Kasetto parses the markdown body instead:

- **Name:** First `#` heading in the document, or the directory name if no heading exists.
- **Description:** First non-empty, non-heading paragraph, or `"No description."` if none found.

### Minimal Example

No frontmatter? No problem:

```markdown
# My Skill

You are an expert at doing X. When the user asks you to...
```

Kasetto uses `"My Skill"` as the display name and the first paragraph as the description.

## Referencing Skills in Config

Reference skills by their directory name in `kasetto.yaml`:

```yaml
skills:
  - source: https://github.com/org/skill-pack
    skills:
      - my-skill           # matches repo-root/my-skill/ or repo-root/skills/my-skill/
      - another-skill
```

Want everything from a source? Use `"*"`:

```yaml
skills:
  - source: ~/Development/my-skills
    skills: "*"
```

## Custom Source Path

If a skill lives somewhere non-standard within the repo, point to it with the `path` field:

```yaml
skills:
  - source: https://github.com/acme/monorepo
    skills:
      - name: my-skill
        path: tools/ai-skills    # looks in tools/ai-skills/my-skill/SKILL.md
```
