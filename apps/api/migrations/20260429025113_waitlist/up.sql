CREATE TABLE api.waitlist (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    github_id BIGINT UNIQUE NOT NULL,
    github_username VARCHAR(39) NOT NULL,
    email VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    approved_at TIMESTAMPTZ NULL,
    user_id UUID NULL REFERENCES api.users(id) ON DELETE SET NULL
);

-- Backfill the seeded `test-voter` user with a synthetic GitHub identity so
-- the local AuthKit-bypass path (apps/web/src/lib/local_auth_stub.ts) drives
-- the waitlist insert end to end without calling the real WorkOS Identities
-- API.
UPDATE api.users
SET github_id = 99000001, username = 'test-voter'
WHERE workos_id = 'test-voter';

-- Seed the `test-voter` waitlist application as already approved so the
-- gate ON path is exercised end to end locally.
INSERT INTO api.waitlist (github_id, github_username, email, approved_at, user_id)
VALUES (
    99000001,
    'test-voter',
    'test-voter@example.test',
    NOW(),
    '00000000-0000-0000-0000-000000000002'
)
ON CONFLICT (github_id) DO NOTHING;
