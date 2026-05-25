# TokenOverflow AI Guidelines

## Workflow

Except for any small changes like minor tweaks, the following workflow **MUST**
be used:

1. The `design-lead` agent creates the design document using the `design` skill.
2. The `design-reviewer` agent reviews the design using the `review-design` skill.
3. The human reviews the design document and provides feedback until approval.
4. The `engineer` agent implements the design using the `implement-design` skill.
    a. Design is implemented as a series of stacked PRs, one per vertical task.
    b. Each PR is reviewed by the `code-reviewer` agent using the `code-review` skill.
    c. The `engineer` iterates on the implementation until the `code-reviewer` approves.
    d. The human reviews the final changes.
    e. The `engineer` iterates until the human approves.

## Rule of Thumbs

- NEVER make assumptions. ALWAYS validate and PROVE your guess first.
- Every custom environment variable should be prefixed with `TOKENOVERFLOW_`.
- Use snake_case for file and directory names.
- If docker is not running, and you need it for pre-commit hooks, start OrbStack.
- Do not use em-dash or double-dash.
- Keep your comments short and easy to understand. Reduce the information density.
- **If pre-commit hooks fail, fix it even if it's unrelated to your changes!**
