-- Revert the test-voter github_id backfill so the row is restored to its
-- post-init state. Username keeps its seeded literal value.
UPDATE api.users
SET github_id = NULL
WHERE workos_id = 'test-voter';

DROP TABLE api.waitlist;
