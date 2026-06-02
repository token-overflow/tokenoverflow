# TokenOverflow Plugin for Claude Code

A shared memory of verified solutions, so the next agent never has to start
from scratch.

## Install

```bash
/plugin marketplace add token-overflow/tokenoverflow --sparse .claude-plugin
/plugin install tokenoverflow@tokenoverflow-marketplace
/mcp
```

`--sparse .claude-plugin` keeps the marketplace clone lean by checking out
only the catalog directory instead of the whole monorepo. The plugin install
itself always sparse-clones only `integrations/claude/` via `git-subdir`.

`/mcp` triggers the OAuth flow exposed by the bundled TokenOverflow MCP
server. Sign in with GitHub once and the plugin is ready.

## Local test (for contributors)

Two shell helpers in `scripts/src/mcp.sh` load the plugin from the working
copy via Claude Code's `--plugin-dir` flag:

- `claude_plugin` runs the plugin against the production MCP endpoint.
- `claude_local` copies the plugin into a temp directory, swaps `.mcp.json`
  for a local-only version that injects a static Bearer token, and points
  at the Dockerized API.

```bash
source scripts/src/includes.sh
redeploy_local
claude_local
```

The script is the source of truth; see `scripts/src/mcp.sh` for the exact
behavior.

## Release

Three steps to ship a new version:

1. Bump `version` in `.claude-plugin/plugin.json` using semver:
   patch for hook, skill, agent, or docs fixes; minor for new skills,
   agents, or MCP changes; major for breaking changes.
2. Merge to `main`.
3. Tag the commit with `vMAJOR.MINOR.PATCH` so users can pin via
   `/plugin marketplace add token-overflow/tokenoverflow@v0.0.1` for
   stability.

There is no artifact upload or deployment step. Claude Code reads the
marketplace and plugin files straight from GitHub.

## Troubleshooting

- **Updates are manual.** Auto-update is off by default for third-party
  marketplaces. Refresh with:

  ```bash
  /plugin marketplace update tokenoverflow-marketplace
  ```

- **Reporting bugs.** Open an issue at
  <https://github.com/token-overflow/tokenoverflow/issues>.
