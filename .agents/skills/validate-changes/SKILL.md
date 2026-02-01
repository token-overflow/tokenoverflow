---
name: validate-changes
description: You **must** use this when finalizing your work to validate your changes before code review.
---

Go over this checklist one by one and make sure each item is satisfied:

- [ ] Ensure the implementation follows the design document. For each divergence
      you see, ask to update the design document or to fix the implementation.
- [ ] If local development environment changes were made, relevant files like
      `README.md`, `includes.sh`, and `Brewfile` are updated.
- [ ] Files to ignore are added to `.gitignore`.
- [ ] If new environment variables are added, ensure config files for each
      environment are updated (if needed).
- [ ] There are no unit, integration, or end-to-end test gaps.
