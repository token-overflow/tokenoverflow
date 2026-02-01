# Repository Pattern

## Repository Structure

```text
repository/
  interface/     Trait definitions with <Conn: Send = AsyncPgConnection>
    answer.rs
    question.rs
    search.rs
    tag.rs
    user.rs
  postgres/      PostgreSQL implementations (impl for AsyncPgConnection)
    answer.rs
    question.rs
    search.rs
    tag.rs
    user.rs
```

## Generic Connection Type

Repository traits are generic over a connection type: `<Conn: Send = AsyncPgConnection>`.
This exists for testability:

- **Production**: `Conn = AsyncPgConnection`. Handlers check out a real
  connection, pass it through services to Pg repository implementations.
- **Unit tests**: `Conn = NoopConn` (a zero-cost unit struct). Services are
  tested with mock repos and no external dependencies.
- **Integration tests**: `Conn = AsyncPgConnection`. Handlers are tested
  with a real testcontainers pool and mock repos (via blanket impl).

Mock repositories use a blanket impl `impl<Conn: Send + 'static>` so the
same mock type works for both `NoopConn` and `AsyncPgConnection`.

To add a new repository, follow this pattern:

1. Define a trait in `repository/interface/` with `<Conn: Send = AsyncPgConnection>`.
2. Implement it for `AsyncPgConnection` in `repository/postgres/`.
3. Add a blanket mock impl in `tests/common/mock_repository.rs`.
4. Service methods that use the repo gain `<Conn: Send>` on the method and
   `conn: &mut Conn` as the first parameter.
