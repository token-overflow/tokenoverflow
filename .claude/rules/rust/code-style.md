---
paths:
    - "apps/api/src/**/*.rs"
---

# Rust Conventions

## Module Organization

- Use `mod.rs` **ONLY** for module organization and re-exports
- **NEVER** put business logic in `mod.rs` files
- Use absolute imports, avoid `super::` where possible

## Database Access

- ALWAYS prefer ORM methods over raw SQL queries
- Use query builder patterns for complex queries
