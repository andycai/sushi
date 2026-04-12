# Sushi Reusable Skills

This directory contains project-specific reusable skills for Codex/agent workflows.

## Included Skills

- `sushi-admin-feature-delivery`: Admin page + partial + JS delivery checklist.
- `sushi-api-route-contract`: API route contract and status/error consistency guardrails.
- `sushi-cli-command-delivery`: CLI subcommand implementation checklist.
- `sushi-lua-plugin-delivery`: Lua plugin scaffolding and parity checklist.
- `sushi-release-verification`: Pre-commit and pre-merge verification gate.

## Suggested Installation

Copy (or symlink) this `.agents/skills/sushi/` directory into your agent skill search path.

Example:

```bash
# Example path; adjust for your local agent setup
ln -s /path/to/sushi/.agents/skills/sushi ~/.agents/skills/sushi
```

## Maintenance Rule

When architecture changes (route layout, template/static conventions, plugin permissions), update these skills in the same PR.
