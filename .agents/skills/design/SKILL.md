---
name: design
description: You **MUST** use this before implementing any new feature or making significant changes to the codebase. Not needed for small refactors, bug fixes, or minor tweaks.
---

Should be executed by the `design-lead` subagent.

You **must** follow the steps below:

1. ALWAYS make sure the git stage is clean! If not, ask the user to clean up.
2. Once the stage is clean, create a new branch from the latest `origin/main`:
    - Run: `git fetch origin main && git checkout -b <branch_name> origin/main`
    - Branch naming: `feature/...`, `bugfix/...`, `hotfix/...`, `chore/...`,
      `refactor/...`, `docs/...`
3. Read `docs/brief/2026_01_31_tokenoverflow.md` and `README.md`.
4. If provided in the user's message, read the PRD carefully.
5. Run `source scripts/src/includes.sh` and
   `create_doc design <feature_name>` to create the design document.
6. Read the generated template under `docs/design/` and grok the structure.
7. Ask clarifying questions about the design if you are not sure of anything.
8. Use subagents to do deep research online.
    - Keep track of all research learnings, make sure they are never lost.
9. Ask clarifying questions with your new learnings if needed.
10. Write one section at a time. For each section:
    - Provide multiple alternatives with trade-offs in a table.
    - Give examples of what each option would look like in practice.
    - Let the user decide which option to pick.
    - Ask for approval before moving to the next section.
11. If a PRD was provided, make sure all its requirements are now satisfied.
12. Once done, ask for a review and keep iterating until you get approval.

List of guidelines you **must** follow:

- Prevent scope creep by sticking to the original requirements.
- Never try to re-invent the wheel. Research best practices using subagents.
- When introducing new libraries, always check if they are well-maintained.
- When iterating on the design, do not reference previous iterations. The
  document should always read as the target state, not as a changelog.
- Ensure every design follows industry best practices without taking any
  shortcuts or reaching for hacks.
- Ensure the design respects the current architecture and coding standards.
- Use the latest version of dependencies unless there is a strong reason not to.
- Do not edit historical design documents.

**CRITICAL:** Your work is not complete until you completely fill the design
document on disk and save your changes. Do not leave an empty template. Do not
make any git commits.
