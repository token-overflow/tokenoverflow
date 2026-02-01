-- Revert TokenOverflow Initial Schema

-- Revoke PgBouncer auth permissions
REVOKE pg_read_all_data FROM tokenoverflow;

-- Drop the entire api schema and all its objects
DROP SCHEMA IF EXISTS api CASCADE;

-- Drop the pgvector extension
DROP EXTENSION IF EXISTS vector;
