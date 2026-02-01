-- TokenOverflow Initial Schema
-- Extensions live in public. All application tables live in the api schema.

-- Enable pgvector extension (installed in public schema)
CREATE EXTENSION IF NOT EXISTS vector;

-- Application schema — keeps app tables separate from extensions and migrations
CREATE SCHEMA IF NOT EXISTS api;

-- Auto-update updated_at on row modification
CREATE OR REPLACE FUNCTION api.set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Users table (linked to WorkOS via GitHub OAuth)
CREATE TABLE api.users (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    workos_id VARCHAR(255) UNIQUE NOT NULL,
    github_id BIGINT UNIQUE,
    username VARCHAR(39) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- API keys for programmatic access
CREATE TABLE api.api_keys (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES api.users(id) ON DELETE CASCADE,
    key_hash VARCHAR(64) NOT NULL,
    key_prefix VARCHAR(16) NOT NULL,
    name VARCHAR(100) NOT NULL DEFAULT 'default',
    last_used TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Canonical tag names, seeded from Stack Overflow.
CREATE TABLE api.tags (
    id         UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    name       VARCHAR(35) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Synonym mappings from Stack Overflow (e.g., "js" -> "javascript").
CREATE TABLE api.tag_synonyms (
    id         UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    synonym    VARCHAR(35) UNIQUE NOT NULL,
    tag_id     UUID NOT NULL REFERENCES api.tags(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Questions with semantic embeddings
CREATE TABLE api.questions (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    title TEXT NOT NULL CONSTRAINT questions_title_length CHECK (char_length(title) BETWEEN 10 AND 150),
    body TEXT NOT NULL CONSTRAINT questions_body_length CHECK (char_length(body) BETWEEN 10 AND 1500),
    embedding vector(256) NOT NULL,
    submitted_by UUID NOT NULL REFERENCES api.users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Normalized join table linking questions to tags
CREATE TABLE api.question_tags (
    question_id UUID NOT NULL REFERENCES api.questions(id) ON DELETE CASCADE,
    tag_id      UUID NOT NULL REFERENCES api.tags(id) ON DELETE RESTRICT,
    PRIMARY KEY (question_id, tag_id)
);

-- Answers to questions
CREATE TABLE api.answers (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    question_id UUID NOT NULL REFERENCES api.questions(id) ON DELETE CASCADE,
    body TEXT NOT NULL CONSTRAINT answers_body_length CHECK (char_length(body) BETWEEN 10 AND 50000),
    submitted_by UUID NOT NULL REFERENCES api.users(id) ON DELETE RESTRICT,
    upvotes INTEGER NOT NULL DEFAULT 0,
    downvotes INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Votes on answers (one vote per user per answer)
CREATE TABLE api.votes (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    answer_id UUID NOT NULL REFERENCES api.answers(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES api.users(id) ON DELETE CASCADE,
    value INTEGER NOT NULL CONSTRAINT votes_value_check CHECK (value IN (-1, 1)),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT votes_answer_id_user_id_unique UNIQUE (answer_id, user_id)
);

-- Indexes for performance

-- users.id is already indexed via PRIMARY KEY constraint.

-- API key lookup by hash (for authentication)
CREATE INDEX api_keys_key_hash_idx ON api.api_keys (key_hash);

-- FK index: api_keys.user_id
CREATE INDEX api_keys_user_id_idx ON api.api_keys (user_id);

-- FK index: tag_synonyms.tag_id
CREATE INDEX tag_synonyms_tag_id_idx ON api.tag_synonyms (tag_id);

-- Vector similarity search using HNSW (works on empty tables, unlike IVFFlat)
CREATE INDEX questions_embedding_idx ON api.questions
    USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);

-- FK index: questions.submitted_by
CREATE INDEX questions_submitted_by_idx ON api.questions (submitted_by);

-- Fast lookup: all questions with a given tag
CREATE INDEX question_tags_tag_id_idx ON api.question_tags (tag_id);

-- Answer lookup by question
CREATE INDEX answers_question_id_idx ON api.answers (question_id);

-- FK index: answers.submitted_by
CREATE INDEX answers_submitted_by_idx ON api.answers (submitted_by);

-- FK index: votes.user_id (votes.answer_id is covered by the UNIQUE constraint)
CREATE INDEX votes_user_id_idx ON api.votes (user_id);

-- Defense-in-depth safety net: the primary self-vote check lives in
-- AnswerService::guard_self_vote. This trigger catches any bypass of the
-- service layer (direct SQL, future code paths, migrations).
CREATE OR REPLACE FUNCTION api.prevent_self_vote()
RETURNS TRIGGER AS $$
DECLARE
    answer_author UUID;
BEGIN
    SELECT submitted_by INTO answer_author
    FROM api.answers
    WHERE id = NEW.answer_id;

    IF answer_author = NEW.user_id THEN
        RAISE EXCEPTION 'Users cannot vote on their own answers'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers: auto-update updated_at on row modification
CREATE TRIGGER set_updated_at BEFORE UPDATE ON api.users        FOR EACH ROW EXECUTE FUNCTION api.set_updated_at();
CREATE TRIGGER set_updated_at BEFORE UPDATE ON api.tags         FOR EACH ROW EXECUTE FUNCTION api.set_updated_at();
CREATE TRIGGER set_updated_at BEFORE UPDATE ON api.tag_synonyms FOR EACH ROW EXECUTE FUNCTION api.set_updated_at();
CREATE TRIGGER set_updated_at BEFORE UPDATE ON api.questions    FOR EACH ROW EXECUTE FUNCTION api.set_updated_at();
CREATE TRIGGER set_updated_at BEFORE UPDATE ON api.answers      FOR EACH ROW EXECUTE FUNCTION api.set_updated_at();

-- Prevent users from voting on their own answers
CREATE TRIGGER prevent_self_vote
    BEFORE INSERT OR UPDATE ON api.votes
    FOR EACH ROW
    EXECUTE FUNCTION api.prevent_self_vote();

-- System user for API operations (used when no authenticated user)
INSERT INTO api.users (id, workos_id, username)
VALUES ('00000000-0000-0000-0000-000000000001', 'system', 'system');

-- Test voter for e2e vote tests (distinct from system user to avoid self-vote guard)
INSERT INTO api.users (id, workos_id, username)
VALUES ('00000000-0000-0000-0000-000000000002', 'test-voter', 'test-voter');

-- Seed top 100 Stack Overflow tags by question count
INSERT INTO api.tags (name) VALUES
    ('javascript'), ('python'), ('java'), ('c#'), ('php'),
    ('android'), ('html'), ('jquery'), ('c++'), ('css'),
    ('ios'), ('mysql'), ('sql'), ('r'), ('node.js'),
    ('reactjs'), ('arrays'), ('c'), ('asp.net'), ('json'),
    ('ruby-on-rails'), ('.net'), ('sql-server'), ('swift'),
    ('python-3.x'), ('objective-c'), ('django'), ('angular'),
    ('excel'), ('regex'), ('pandas'), ('ruby'), ('linux'),
    ('ajax'), ('typescript'), ('xml'), ('vb.net'), ('spring'),
    ('database'), ('wordpress'), ('string'), ('mongodb'),
    ('postgresql'), ('windows'), ('git'), ('bash'), ('firebase'),
    ('algorithm'), ('docker'), ('list'), ('amazon-web-services'),
    ('azure'), ('spring-boot'), ('vue.js'), ('dataframe'),
    ('multithreading'), ('flutter'), ('api'), ('function'),
    ('image'), ('tensorflow'), ('numpy'), ('kotlin'),
    ('rest'), ('google-chrome'), ('maven'), ('selenium'),
    ('react-native'), ('eclipse'), ('performance'), ('macos'),
    ('powershell'), ('matplotlib'), ('dictionary'), ('unit-testing'),
    ('go'), ('scala'), ('class'), ('dart'), ('perl'),
    ('apache'), ('visual-studio'), ('nginx'), ('laravel'),
    ('express'), ('machine-learning'), ('css-selectors'), ('xcode'),
    ('google-maps'), ('rust'), ('graphql'), ('redis'),
    ('hadoop'), ('webpack'), ('xaml'), ('svelte'), ('next.js'),
    ('flask'), ('fastapi'), ('tailwindcss'), ('kubernetes'),
    ('github-actions'), ('terraform'), ('elasticsearch')
ON CONFLICT (name) DO NOTHING;

-- Seed the most common synonyms for the top 100 tags
INSERT INTO api.tag_synonyms (synonym, tag_id) VALUES
    ('js',          (SELECT id FROM api.tags WHERE name = 'javascript')),
    ('ecmascript',  (SELECT id FROM api.tags WHERE name = 'javascript')),
    ('vanillajs',   (SELECT id FROM api.tags WHERE name = 'javascript')),
    ('py',          (SELECT id FROM api.tags WHERE name = 'python')),
    ('python3',     (SELECT id FROM api.tags WHERE name = 'python')),
    ('ts',          (SELECT id FROM api.tags WHERE name = 'typescript')),
    ('golang',      (SELECT id FROM api.tags WHERE name = 'go')),
    ('k8s',         (SELECT id FROM api.tags WHERE name = 'kubernetes')),
    ('postgres',    (SELECT id FROM api.tags WHERE name = 'postgresql')),
    ('node',        (SELECT id FROM api.tags WHERE name = 'node.js')),
    ('nodejs',      (SELECT id FROM api.tags WHERE name = 'node.js')),
    ('react',       (SELECT id FROM api.tags WHERE name = 'reactjs')),
    ('nextjs',      (SELECT id FROM api.tags WHERE name = 'next.js')),
    ('vuejs',       (SELECT id FROM api.tags WHERE name = 'vue.js')),
    ('vue',         (SELECT id FROM api.tags WHERE name = 'vue.js'))
ON CONFLICT (synonym) DO NOTHING;

-- Grant pg_shadow read access for PgBouncer auth_query (SCRAM-SHA-256 support)
GRANT pg_read_all_data TO tokenoverflow;
