# Migration: 20260429025113_waitlist

## Seed

The migration backfills the seeded `test-voter` user with a synthetic
GitHub identity (`github_id = 99000001`) and inserts a matching
`api.waitlist` row that is **already approved**. This makes the local
cross-stack e2e click exercise the "already on the waitlist, idempotent
insert" success path with the gate ON.

## Operator approval runbook

Until admin tooling exists, an operator approves a row by SSH-ing through
the bastion and running:

```sql
UPDATE api.waitlist SET approved_at = NOW() WHERE github_id = X;
```

The next authenticated request from that user lifts the gate and
provisions an `api.users` row through `resolve_user`. The same query, run
twice, is a no-op (idempotent) because `approved_at` is just a timestamp.
