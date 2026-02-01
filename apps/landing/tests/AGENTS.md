# Testing

All tests live under `apps/landing/tests/`. `src/` holds production code
only; tests never sit next to the code they exercise. Tests mirror the
source code directory structure.

```
tests/
├── common/        # shared fixtures, helpers, mock data
├── unit/          # Vitest: future src/lib/ logic
├── integration/   # cross-module tests (future)
└── e2e/           # Playwright + axe specs
```

Filename conventions:

- `.test.ts` for Vitest (unit + integration)
- `.spec.ts` for Playwright (e2e)
