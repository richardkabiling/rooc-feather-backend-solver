# Copilot Instructions

## Commit messages

Always use **Conventional Commits** format:

```
<type>[!]: <description>
```

The CI auto-tag workflow reads these to determine the semver bump on every merge to `main`:

| Type | Bump | Use when |
|------|------|----------|
| `fix` | patch | Correcting a bug |
| `chore` | patch | Maintenance, deps, tooling, CI |
| `refactor` | patch | Code restructure with no behaviour change |
| `perf` | patch | Performance improvement |
| `docs` | patch | Documentation only |
| `test` | patch | Tests only |
| `feat` | **minor** | New backwards-compatible feature |
| any type + `!` suffix | **major** | Breaking change |

A `BREAKING CHANGE:` footer in the commit body also triggers a major bump.

Examples:
```
fix: correct t1_cost double-count in inventory loader
feat: add cross-chain solution sharing via shared best pool
feat!: change JSON output schema — statues now sorted by score
chore: update actions to Node 22-compatible versions
docs: add semver commit conventions to CLAUDE.md
```

Keep the description lowercase, imperative mood, no trailing period.
