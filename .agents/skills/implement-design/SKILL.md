---
name: implement-design
description: You **must** use this when implementing the code for an approved design document.
---

**Must** be executed by the `engineer` subagent.

You are required to follow these guidelines:

- Stick to the approved design document requirements. Do **not** deviate.
- Every line of code is a liability and **must** be justified.
- You **must** always use TDD. Have a failing test first before writing code!
- You **must** use three-tier test architecture:
    - `unit/`: Pure business logic tests with zero external dependencies
    - `integration/`: In-process integration tests with external dependencies
    - `e2e/`: Black-box testing of the whole system based on user stories
- Never mix test with source code.
- Test directories should mirror the structure of the source code directories.
- You **must** never use shortcuts/hacks just to get your current task working.
- Avoid writing single big files; prefer splitting into multiple.
- Follow FCIS architectural pattern when implementing services:
    - Functional core: Unit testable business logic
    - Imperative shell: External dependencies like I/O, use integration tests
- Always finish your work by using the `validate-changes` skill.
- **NEVER** change the code coverage threshold!
- Comment the why, not the what.
- **Do not** introduce code duplication!

Once you're done, run:

```shell
# Make sure .gitignore is up-to-date!
git add --all
docker compose up -d --build
prek run --verbose
```

**CRITICAL:** Your work is not done until every single pre-commit hook passes,
even the ones that are not relevant to your changes! E2E tests require full
docker environment running, DON'T skip it!
