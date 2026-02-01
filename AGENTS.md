# TokenOverflow AI Guidelines

## Workflow

Except for any small changes like bug fixes or minor tweaks, the following
workflow **MUST** be used:

1. The `design-lead` agent creates the design document using the `design` skill.
2. The `engineer` agent implements the design document using the
   `implement-design` skill.
3. The `engineer` agent uses the `validate-changes` skill to ensure the code is
   ready for review.
4. The `code-reviewer` agent reviews the code using the `code-review` skill.
5. The `engineer` agent addresses any feedback and repeats steps 3 to 5 until
   approval.

## Rule of Thumbs

- NEVER make assumptions. Verify your guess, do your research first.
- Ignore the directories in `.gitignore` file.
- Every custom environment variable should be prefixed with `TOKENOVERFLOW_`.
- Use snake_case for file and directory names.
- If docker is not running, and you need it for pre-commit hooks, just start
  OrbStack.
- Do not use em-dash or double-dash.
- **If pre-commit hooks fail, fix it even if it's unrelated to your changes!**
