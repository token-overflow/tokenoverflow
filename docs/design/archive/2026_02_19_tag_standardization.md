# Design: Tag Standardization

## Problem Statement

Tags are currently free-form text stored as `TEXT[]` on the `questions` table.
The only guardrail is the `normalize_tag` function in
`apps/api/src/services/tags.rs`, which lowercases, replaces spaces/underscores
with hyphens, and collapses consecutive hyphens. This prevents trivial
formatting differences (`Rust` vs `rust`) but does nothing about semantic
duplicates: `js`, `javascript`, `java-script`, `ecmascript` all survive
normalization as four distinct tags.

Because agents generate tags autonomously and at high frequency (10-100+ calls
per coding session per the product brief), the tag space will grow unbounded.
This causes three concrete problems:

1. **Search misses** -- a question tagged `js` will not be found when an agent
   searches with the tag `javascript`, even though they mean the same thing.
2. **GIN index bloat** -- the `questions_tags_idx` GIN index grows with every
   unique tag, degrading insert and query performance.
3. **Analytics noise** -- tag-based metrics become meaningless when the same
   concept is spread across dozens of variations.

The goal is to standardize tags so that semantic duplicates are collapsed into a
single canonical form, without degrading UX (agents should not need to memorize
an exact list) or performance (tag resolution should not add measurable
latency).

---

## Architecture Overview

The solution adds a **tag registry** seeded from Stack Overflow's complete tag
and synonym datasets (~66K canonical tags). Tags are stored in a normalized
`question_tags` join table. When a question is submitted or searched, each tag
passes through a three-layer resolution pipeline. Tags that cannot be resolved
to a known canonical form are **silently dropped** -- with 66K Stack
Overflow tags covering virtually all programming topics, unresolvable
tags are almost certainly garbage.

### Resolution Pipeline

```text
Agent submits tags:          ["JS", "javascrip", "xyzgarbage", "react"]
         |
         v
  normalize_tag()            ["js", "javascrip", "xyzgarbage", "react"]
         |
         v
  Layer 1: Synonym lookup    "js" -> "javascript"  (HashMap, O(1))
  Layer 2: Canonical lookup   "react" -> hit        (HashSet, O(1))
  Layer 3: Jaro-Winkler      "javascrip" -> "javascript" (0.98 > 0.85)
  No match:                   "xyzgarbage" -> DROPPED
         |
         v
  Deduplicate                ["javascript", "react"]
         |
         v
  Store via question_tags join table
```

### Component Diagram

```text
┌────────────────────────────────────────────────────────────────────────┐
│                         Tag Standardization                            │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌────────────┐     ┌───────────────┐     ┌─────────────────────────┐ │
│  │ MCP / REST │────>│ TagResolver   │────>│ TagRepository (trait)   │ │
│  │ (submit,   │     │               │     │                         │ │
│  │  search)   │     │ 1. synonyms   │     │ - load_synonyms()       │ │
│  └────────────┘     │    (HashMap)  │     │ - load_canonicals()     │ │
│                     │ 2. canonicals │     │ - find_tag_ids()        │ │
│                     │    (HashSet)  │     └────────────┬────────────┘ │
│                     │ 3. jaro-      │                  │              │
│                     │    winkler    │                  v              │
│                     │    (Vec)      │     ┌─────────────────────────┐ │
│                     └───────────────┘     │ PostgreSQL              │ │
│                                           │                         │ │
│  ┌────────────┐                           │ tags                    │ │
│  │ so-tag-    │── fetch ──> disk ──> DB──>│ tag_synonyms            │ │
│  │ sync CLI   │                           │ question_tags           │ │
│  └────────────┘                           └─────────────────────────┘ │
│                                                                        │
│  ┌────────────┐                                                        │
│  │ Claude     │                                                        │
│  │ Plugin     │── tags.md (flat file, one tag per line)                │
│  └────────────┘                                                        │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

**Why Stack Overflow as the tag source?**

Stack Overflow maintains ~66K canonical programming tags with community-curated
synonym mappings (e.g., `js` -> `javascript`, `k8s` -> `kubernetes`). This is
the most comprehensive, battle-tested tag taxonomy for programming topics. Using
it avoids reinventing the wheel and provides immediate coverage of virtually all
programming languages, frameworks, libraries, and tools.

**Why a join table instead of `TEXT[]`?**

A `question_tags(question_id, tag_id)` join table provides referential integrity
-- every tag on a question must exist in the `tags` table. This prevents garbage
accumulation by construction. It also enables efficient tag-centric queries (all
questions with tag X) via B-tree index scans. With max 5 tags per question, the
join table overhead is negligible.

**Why drop unknown tags instead of auto-inserting?**

With 66K Stack Overflow tags covering the programming universe, an
unresolvable tag is almost certainly garbage from an agent (typo that
doesn't match anything, random
text, etc.). Auto-inserting unknown tags would lead to millions of junk rows
that are never cleaned up, never mapped to synonyms, and pollute analytics. The
cost of silently dropping a rare legitimate tag is low (question still saves,
just missing one tag) and temporary (add it in the next Stack Overflow sync).

**Why synonym map + Jaro-Winkler instead of just similarity?**

Semantic abbreviations (`js`, `py`, `k8s`, `ts`) have very low string
similarity scores against their canonical forms (Jaro-Winkler: `js` vs
`javascript` = ~0.51). These are the most common agent inputs and string
similarity will never resolve them. The synonym map from Stack Overflow
handles these O(1).
Jaro-Winkler then catches typos (`"javascrip"` -> `"javascript"`, score 0.98)
as a fallback for the rare case where a tag isn't an exact synonym or canonical
match.

**Why custom normalization instead of a slug library?**

No existing Rust slug crate (`slug`, `slugify`, `str_slug`) preserves `+`, `#`,
and `.` characters -- they all strip non-alphanumeric characters. Stack Overflow
tags require these characters (e.g., `c++`, `c#`, `.net`, `node.js`). The
custom `normalize_tag` function is ~20 lines and must be thoroughly unit tested
to ensure it matches Stack Overflow's format exactly.

**Why an in-memory cache?**

The full tag set (~66K names + ~5K synonyms) fits in ~2-3MB of memory. Loading
at startup avoids a database round-trip on every request. The 99% path (synonym
or canonical hit) adds < 1 microsecond of latency. The Jaro-Winkler fallback
(~13ms for 66K comparisons) only fires for truly unknown tags, which is rare.

---

## Alternatives Considered

### Alternative A: Enhanced Normalization Only (No Registry)

Extend the existing `normalize_tag` function with a hardcoded synonym map
embedded directly in Rust code.

```rust
fn normalize_tag(tag: &str) -> String {
    let lowered = tag.trim().to_lowercase();
    // ... existing normalization ...
    match normalized.as_str() {
        "js" | "java-script" | "ecmascript" => "javascript".to_string(),
        "ts" | "type-script" => "typescript".to_string(),
        "py" => "python".to_string(),
        _ => normalized,
    }
}
```

### Alternative B: Stack Overflow-Seeded Registry with Drop (Recommended)

All ~66K Stack Overflow tags and their synonyms are loaded into a `tags` and
`tag_synonyms` table. Tags are resolved via a three-layer in-memory pipeline
(synonym lookup, canonical lookup, Jaro-Winkler similarity). Unresolvable tags
are silently dropped. Tags are stored via a normalized `question_tags` join
table with referential integrity.

### Alternative C: Stack Overflow-Seeded Open Registry (Auto-Insert Unknown)

Same as Alternative B, but unknown tags are auto-inserted into the `tags` table
instead of being dropped. This prevents data loss but leads to unbounded table
growth from agent-generated garbage.

### Alternative D: LLM-Based Tag Resolution

Use the LLM to normalize agent-submitted tags at submission time. Prompt:
"Given these tags, return their canonical forms."

### Comparison

| Criterion                  | A: Hardcoded Map   | B: Stack Overflow + Drop        | C: Stack Overflow + Auto-Insert | D: LLM Resolution      |
|----------------------------|--------------------|---------------------|---------------------|-------------------------|
| Solves synonym problem     | Partially          | Yes                 | Yes                 | Yes                     |
| Prevents tag table sprawl  | N/A                | Yes                 | No                  | Partially               |
| Requires redeploy to add   | Yes                | No (sync tool)      | No (sync tool)      | No                      |
| Agent UX impact            | None               | None (silent drop)  | None                | None                    |
| Latency impact             | None               | Negligible (cache)  | Negligible (cache)  | +100-500ms per call     |
| Cost impact                | None               | None                | None                | LLM API cost            |
| Complexity                 | Very low           | Medium              | Medium              | Medium                  |
| Handles typos              | No                 | Yes (Jaro-Winkler)  | Yes (Jaro-Winkler)  | Unpredictable           |
| Tag data quality           | N/A                | High (curated)      | Low (garbage)       | Unpredictable           |
| Industry precedent         | Small projects     | Stack Overflow      | --                  | None at scale           |

### Recommendation: Alternative B (Stack Overflow-Seeded Registry with Drop)

Alternative B provides the best balance of data quality and agent UX. The 66K
Stack Overflow tags cover virtually all programming topics. Synonyms handle semantic
abbreviations. Jaro-Winkler catches typos. Unresolvable tags are silently
dropped, keeping the registry clean. The question still saves successfully --
the agent's workflow is never interrupted.

Alternative C was rejected because auto-inserting unknown tags leads to
unbounded table growth that is never cleaned up. The cost of dropping a rare
legitimate tag is much lower than the cost of accumulating millions of junk
rows.

Alternative D is ruled out because it adds latency, cost, and nondeterminism to
every request.

---

## Interfaces

### Database Schema

#### `tags` table

Stores all canonical tag names, seeded from Stack Overflow.

```sql
CREATE TABLE tags (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       VARCHAR(35) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- `name` is the canonical form (e.g., `javascript`, `react`, `next.js`).
- `UNIQUE` on `name` prevents duplicates and enables fast lookups.
- `created_at` doubles as the sync watermark for incremental Stack Overflow fetches:
  `SELECT MAX(created_at) FROM tags` gives the timestamp of the last sync.
- `updated_at` tracks when a tag was last refreshed by the sync tool.
- Max 35 chars matches Stack Overflow's tag length limit.

#### `tag_synonyms` table

Stores synonym mappings from Stack Overflow (e.g., `js` -> `javascript`).

```sql
CREATE TABLE tag_synonyms (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    synonym    VARCHAR(35) UNIQUE NOT NULL,
    tag_id     BIGINT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX tag_synonyms_tag_id_idx ON tag_synonyms (tag_id);
```

- `synonym` is the non-canonical form (e.g., `js`, `k8s`, `py`).
- `UNIQUE` on `synonym` ensures each synonym maps to exactly one canonical tag.
- `tag_id` references the canonical tag it maps to.
- `created_at` serves as sync watermark for incremental synonym fetches.
- A synonym must not collide with any canonical tag name (enforced at the
  application layer in the sync tool).

#### `question_tags` join table

Replaces the `questions.tags TEXT[]` column with a normalized join table.

```sql
CREATE TABLE question_tags (
    question_id BIGINT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    tag_id      BIGINT NOT NULL REFERENCES tags(id) ON DELETE RESTRICT,
    PRIMARY KEY (question_id, tag_id)
);

-- Fast lookup: all questions with a given tag
CREATE INDEX question_tags_tag_id_idx ON question_tags (tag_id);
```

- Composite primary key prevents duplicate tags on a question.
- `ON DELETE RESTRICT` on `tag_id` prevents accidentally deleting a tag that is
  still in use. Tags must be unmapped from all questions before deletion.
- `ON DELETE CASCADE` on `question_id` cleans up tag links when a question is
  deleted.
- B-tree index on `tag_id` enables efficient "all questions with tag X" queries.

#### Migration: data migration from `TEXT[]` to join table

The migration converts existing `questions.tags` array data into `question_tags`
rows, then drops the `tags` column and its GIN index.

```sql
-- Migrate existing data (only tags that exist in the registry)
INSERT INTO question_tags (question_id, tag_id)
SELECT DISTINCT q.id, t.id
FROM questions q,
     unnest(q.tags) AS raw_tag
JOIN tags t ON t.name = raw_tag
ON CONFLICT DO NOTHING;

-- Also resolve known synonyms in existing data
INSERT INTO question_tags (question_id, tag_id)
SELECT DISTINCT q.id, ts.tag_id
FROM questions q,
     unnest(q.tags) AS raw_tag
JOIN tag_synonyms ts ON ts.synonym = raw_tag
ON CONFLICT DO NOTHING;

-- Drop the old column and index
DROP INDEX questions_tags_idx;
ALTER TABLE questions DROP COLUMN tags;
```

#### Seed Data (migration)

The migration seeds the **top 100 most common programming tags** so that all
environments (unit test, local, development, production) have a baseline tag set
without requiring the Stack Overflow sync tool. These are the 100 most
popular tags on Stack Overflow by question count.

```sql
INSERT INTO tags (name) VALUES
    -- Top 100 Stack Overflow tags by question count (approximate order)
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
INSERT INTO tag_synonyms (synonym, tag_id) VALUES
    ('js',          (SELECT id FROM tags WHERE name = 'javascript')),
    ('ecmascript',  (SELECT id FROM tags WHERE name = 'javascript')),
    ('vanillajs',   (SELECT id FROM tags WHERE name = 'javascript')),
    ('py',          (SELECT id FROM tags WHERE name = 'python')),
    ('python3',     (SELECT id FROM tags WHERE name = 'python')),
    ('ts',          (SELECT id FROM tags WHERE name = 'typescript')),
    ('golang',      (SELECT id FROM tags WHERE name = 'go')),
    ('k8s',         (SELECT id FROM tags WHERE name = 'kubernetes')),
    ('postgres',    (SELECT id FROM tags WHERE name = 'postgresql')),
    ('node',        (SELECT id FROM tags WHERE name = 'node.js')),
    ('nodejs',      (SELECT id FROM tags WHERE name = 'node.js')),
    ('react',       (SELECT id FROM tags WHERE name = 'reactjs')),
    ('nextjs',      (SELECT id FROM tags WHERE name = 'next.js')),
    ('vuejs',       (SELECT id FROM tags WHERE name = 'vue.js')),
    ('vue',         (SELECT id FROM tags WHERE name = 'vue.js'))
ON CONFLICT (synonym) DO NOTHING;
```

Note: The full Stack Overflow dataset (~66K tags + all synonyms) is loaded by the
`so-tag-sync` tool after initial deployment. The migration seed ensures a
working baseline for development and testing.

### Normalization

The existing `normalize_tag` function is updated to strip characters outside
Stack Overflow's allowed set: `a-z`, `0-9`, `+`, `#`, `.`, `-`. No existing
Rust slug crate (`slug`, `slugify`, `str_slug`) preserves `+`, `#`, and `.`,
so a custom implementation is required.

```text
Input -> trim -> lowercase -> spaces/underscores to hyphens
      -> strip invalid chars -> collapse hyphens -> trim hyphens
```

Updated function:

```rust
/// Normalize a tag to Stack Overflow-compatible format.
///
/// Allowed characters: a-z, 0-9, +, #, ., -
/// Preserves dots (e.g., `next.js`), plus signs (e.g., `c++`),
/// and hash (e.g., `c#`).
pub fn normalize_tag(tag: &str) -> String {
    let lowered = tag.trim().to_lowercase();

    let replaced: String = lowered
        .chars()
        .map(|c| match c {
            ' ' | '_' => '-',
            _ => c,
        })
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '.' | '-'))
        .collect();

    // Collapse consecutive hyphens
    let mut collapsed = String::with_capacity(replaced.len());
    let mut prev_hyphen = false;
    for c in replaced.chars() {
        if c == '-' {
            if !prev_hyphen {
                collapsed.push(c);
            }
            prev_hyphen = true;
        } else {
            collapsed.push(c);
            prev_hyphen = false;
        }
    }

    // Strip leading/trailing hyphens
    collapsed.trim_matches('-').to_string()
}
```

### Rust Interfaces

#### TagRepository Trait

New file: `apps/api/src/services/repository/tag.rs`

```rust
#[async_trait]
pub trait TagRepository: Send + Sync {
    /// Load all synonym mappings: synonym -> canonical_name.
    async fn load_synonyms(&self) -> Result<HashMap<String, String>, AppError>;

    /// Load all canonical tag names.
    async fn load_canonicals(&self) -> Result<Vec<String>, AppError>;

    /// Find tag IDs for a list of canonical names.
    /// Returns only the names that exist in the tags table.
    async fn find_tag_ids(&self, names: &[String]) -> Result<Vec<(String, i64)>, AppError>;

    /// Insert question_tags rows for a given question.
    async fn link_tags_to_question(
        &self,
        question_id: i64,
        tag_ids: &[i64],
    ) -> Result<(), AppError>;

    /// Get tag names for a question via the join table.
    async fn get_question_tags(&self, question_id: i64) -> Result<Vec<String>, AppError>;
}
```

#### TagResolver (In-Memory Cache)

New file: `apps/api/src/services/tag_resolver.rs`

```rust
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use strsim::jaro_winkler;

const SIMILARITY_THRESHOLD: f64 = 0.85;

pub struct TagResolver {
    /// synonym -> canonical_name (e.g., "js" -> "javascript")
    synonyms: RwLock<HashMap<String, String>>,

    /// Set of all canonical names for O(1) membership check
    canonical_set: RwLock<HashSet<String>>,

    /// All canonical names as Vec for Jaro-Winkler iteration
    canonical_list: RwLock<Vec<String>>,
}

impl TagResolver {
    /// Build the resolver by loading all data from the repository.
    pub async fn new(repo: &dyn TagRepository) -> Result<Self, AppError> {
        let synonyms = repo.load_synonyms().await?;
        let canonicals = repo.load_canonicals().await?;
        let canonical_set: HashSet<String> = canonicals.iter().cloned().collect();
        Ok(Self {
            synonyms: RwLock::new(synonyms),
            canonical_set: RwLock::new(canonical_set),
            canonical_list: RwLock::new(canonicals),
        })
    }

    /// Construct from raw data (for unit tests, no DB needed).
    pub fn from_data(
        synonyms: HashMap<String, String>,
        canonicals: Vec<String>,
    ) -> Self {
        let canonical_set: HashSet<String> = canonicals.iter().cloned().collect();
        Self {
            synonyms: RwLock::new(synonyms),
            canonical_set: RwLock::new(canonical_set),
            canonical_list: RwLock::new(canonicals),
        }
    }

    /// Resolve a single normalized tag to its canonical form.
    /// Returns None if the tag cannot be resolved (will be dropped).
    pub fn resolve(&self, tag: &str) -> Option<String> {
        let synonyms = self.synonyms.read().expect("RwLock poisoned");
        let canonical_set = self.canonical_set.read().expect("RwLock poisoned");
        let canonical_list = self.canonical_list.read().expect("RwLock poisoned");

        // Layer 1: synonym lookup (O(1))
        if let Some(canonical) = synonyms.get(tag) {
            return Some(canonical.clone());
        }

        // Layer 2: canonical set lookup (O(1))
        if canonical_set.contains(tag) {
            return Some(tag.to_string());
        }

        // Layer 3: Jaro-Winkler similarity (O(n), rare path)
        let best = canonical_list
            .iter()
            .map(|c| (c, jaro_winkler(tag, c)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if let Some((canonical, score)) = best {
            if score >= SIMILARITY_THRESHOLD {
                return Some(canonical.clone());
            }
        }

        // No match -- tag will be dropped
        None
    }

    /// Resolve a list of tags: normalize, resolve, deduplicate, drop unknowns.
    pub fn resolve_tags(&self, tags: &[String]) -> Vec<String> {
        let normalized = normalize_tags(tags);
        let mut seen = HashSet::new();
        let mut result = Vec::with_capacity(normalized.len());
        for tag in normalized {
            if let Some(resolved) = self.resolve(&tag) {
                if seen.insert(resolved.clone()) {
                    result.push(resolved);
                }
            }
            // else: tag is dropped silently
        }
        result
    }

    /// Reload all data from the database.
    pub async fn refresh(&self, repo: &dyn TagRepository) -> Result<(), AppError> {
        let synonyms = repo.load_synonyms().await?;
        let canonicals = repo.load_canonicals().await?;
        let canonical_set: HashSet<String> = canonicals.iter().cloned().collect();

        *self.synonyms.write().expect("RwLock poisoned") = synonyms;
        *self.canonical_set.write().expect("RwLock poisoned") = canonical_set;
        *self.canonical_list.write().expect("RwLock poisoned") = canonicals;
        Ok(())
    }
}
```

#### Integration into AppState

```rust
pub struct AppState {
    // ... existing fields ...
    pub tag_resolver: Arc<TagResolver>,
}
```

### REST/MCP Interface Changes

**No changes to the external API contracts.** Agents continue to send free-form
tags exactly as they do today. The resolution happens transparently in the
service layer. Responses will contain only canonical tag names resolved from
the registry.

The response format for tags changes from inline array to a lookup from the
join table, but the JSON shape remains the same:
`"tags": ["javascript", "react"]`.

### `so-tag-sync` CLI Tool

A workspace member binary that fetches Stack Overflow tags and synonyms and
loads them into the database.

#### Project Structure

```text
apps/so-tag-sync/
├── Cargo.toml          # workspace member binary
└── src/
    └── main.rs
```

#### Dependencies

- `reqwest` -- HTTP client for Stack Overflow API
- `serde` / `serde_json` -- JSON deserialization
- `diesel` -- database operations (shared with API)
- `clap` -- CLI argument parsing
- `tracing` -- structured logging
- `tokio` -- async runtime

#### CLI Interface

```text
so-tag-sync [OPTIONS]

Options:
    --full              Full sync: fetch ALL tags/synonyms from Stack Overflow API,
                        write to disk, then load into DB
    --from-file         Skip Stack Overflow API, load from previously saved files
    --tags-file <PATH>  Path for tags data [default: stackoverflow-tags.json]
    --synonyms-file <PATH>  Path for synonyms data [default: stackoverflow-synonyms.json]
    --dry-run           Fetch and save to disk but do not write to DB

Environment:
    TOKENOVERFLOW_STACKOVERFLOW_API_KEY   Stack Exchange API key (required for full sync)
    TOKENOVERFLOW_ENV          Config environment (local, development, production)
```

Default mode (no flags) is **incremental sync**: fetch only tags/synonyms
added to Stack Overflow since the last sync, and upsert them into the database.

Note: The Stack Exchange API enforces a hard maximum of **100 results per page**
(`pagesize=100`). This cannot be increased. A full sync of ~66K tags requires
~660 paginated API requests.

#### Modes of Operation

#### Mode 1: Incremental Sync (default)

The standard way to keep tags up to date. Designed to be run as a scheduled
GitHub Action in the future.

```text
1. Connect to DB
2. SELECT MAX(created_at) FROM tags → last_sync_date
   (if NULL, error: run --full first)
3. GET /tags?sort=activity&min={last_sync_unix}&pagesize=100&page=1..N
4. Upsert new tags: INSERT INTO tags (name) ... ON CONFLICT (name) DO NOTHING
5. GET /tags/synonyms?sort=creation&min={last_sync_unix}&pagesize=100&page=1..N
6. Upsert new synonyms: INSERT INTO tag_synonyms ... ON CONFLICT DO NOTHING
7. Log summary: X new tags, Y new synonyms
```

#### Mode 2: Full Sync (`--full`)

For initial population. Fetches everything from Stack Overflow and writes
each dataset to
disk as it completes (so the expensive API work is preserved even if a later
step fails), then loads both into DB in a single transaction.

```text
1. Paginate GET /tags?sort=popular&pagesize=100&page=1..~660
   → collect all ~66K tags
2. Write tags to disk (--tags-file, default: stackoverflow-tags.json)
3. Paginate GET /tags/synonyms?sort=creation&pagesize=100&page=1..N
   → collect all synonym pairs
4. Write synonyms to disk (--synonyms-file, default: stackoverflow-synonyms.json)
5. Read both files from disk
6. BEGIN transaction
7. Batch upsert all tags into `tags` table
8. Batch upsert all synonyms into `tag_synonyms` table
9. COMMIT
10. Log summary: X tags, Y synonyms loaded
```

#### Mode 3: From File (`--from-file`)

Skip the Stack Overflow API entirely, load from previously saved JSON
files. Useful for
loading the same dataset into a different database (e.g., production) without
re-fetching from Stack Overflow.

```text
1. Read tags from --tags-file path
2. Read synonyms from --synonyms-file path
3. Connect to DB
4. BEGIN transaction
5. Batch upsert all tags
6. Batch upsert all synonyms
7. COMMIT
```

#### Disk File Formats

`stackoverflow-tags.json`:

```json
{
  "fetched_at": "2026-02-19T12:00:00Z",
  "tags": [
    { "name": "javascript", "count": 2533073 },
    { "name": "python", "count": 2221821 }
  ]
}
```

`stackoverflow-synonyms.json`:

```json
{
  "fetched_at": "2026-02-19T12:30:00Z",
  "synonyms": [
    { "from": "js", "to": "javascript" },
    { "from": "py", "to": "python" }
  ]
}
```

#### Operational Details

| Concern | Approach |
|---|---|
| Rate limiting | 2 req/sec with exponential backoff on 429s |
| API key | `TOKENOVERFLOW_STACKOVERFLOW_API_KEY` env var (10K req/day with key) |
| Max page size | 100 (hard limit from Stack Overflow API) |
| Retries | 3 retries per request with backoff |
| Idempotency | All upserts use `ON CONFLICT DO NOTHING` |
| DB connection | Reads from app config via `TOKENOVERFLOW_ENV` |
| Disk write | Atomic write (write to temp file, rename) |
| Logging | `tracing` with structured output |
| Full sync time | ~660 API calls at 2/sec = ~5.5 minutes for tags, plus synonyms |

#### Usage

```bash
# Initial full sync: fetch from Stack Overflow, save to disk, load into local DB
TOKENOVERFLOW_STACKOVERFLOW_API_KEY=xxx cargo run -p so-tag-sync -- --full

# Load same data into production DB (no Stack Overflow API calls)
TOKENOVERFLOW_ENV=production cargo run -p so-tag-sync -- --from-file

# Incremental sync (default, for scheduled runs)
TOKENOVERFLOW_STACKOVERFLOW_API_KEY=xxx cargo run -p so-tag-sync

# Fetch only, don't write to DB
TOKENOVERFLOW_STACKOVERFLOW_API_KEY=xxx cargo run -p so-tag-sync -- --full --dry-run
```

### Claude Plugin: `tags.md`

A flat file listing the full Stack Overflow tag universe, placed in the Claude
plugin for agent reference. One tag per line, nothing else.

New file: `apps/claude/tags.md`

```text
javascript
python
java
c#
php
android
html
...
```

Referenced in `apps/claude/skills/submit-to-tokenoverflow/SKILL.md`:

```text
Before submitting, check apps/claude/tags.md for canonical tag names.
Use the canonical form when possible. Max 5 tags, lowercase kebab-case.
```

Also referenced in `apps/claude/instructions.md` rule 4.

This file is manually populated from the Stack Overflow dataset after
running the full sync. Update it when running a new full Stack Overflow
sync.

---

## Logic

### Tag Resolution Flow (Submit Path)

```text
Agent submits: ["JS", "React_Native", "typescipt", "xyzgarbage", "react"]
         |
         v
  normalize_tags()         -> ["js", "react-native", "typescipt", "xyzgarbage", "react"]
         |
         v
  For each tag:
    "js"          -> synonym lookup HIT  -> "javascript"           kept
    "react-native"-> synonym lookup MISS
                  -> canonical lookup HIT -> "react-native"        kept
    "typescipt"   -> synonym lookup MISS
                  -> canonical lookup MISS
                  -> jaro-winkler: "typescript" = 0.97 > 0.85     kept
    "xyzgarbage"  -> synonym lookup MISS
                  -> canonical lookup MISS
                  -> jaro-winkler: best = 0.42 < 0.85             DROPPED
    "react"       -> synonym or canonical hit (depending on Stack Overflow data)
         |
         v
  Deduplicate              -> ["javascript", "react-native", "typescript", "react"]
         |
         v
  Look up tag_ids          -> find_tag_ids(["javascript", ...])
         |
         v
  Insert question_tags rows
```

### Tag Resolution Flow (Search Path)

Identical resolution pipeline. The resolved canonical names are used to query
the `question_tags` join table instead of the old `@> ARRAY[...]` operator.

```sql
-- Old: GIN containment on TEXT[]
SELECT ... FROM questions q
WHERE q.tags @> $1::text[];

-- New: JOIN through question_tags
SELECT DISTINCT q.*
FROM questions q
JOIN question_tags qt ON qt.question_id = q.id
JOIN tags t ON t.id = qt.tag_id
WHERE t.name = ANY($1::text[]);
```

### Integration Points

**1. `apps/api/src/services/question.rs` -- QuestionService::create**

Replace `normalize_tags` call with `tag_resolver.resolve_tags`, then look up
`tag_id`s and insert `question_tags` rows.

```rust
pub async fn create(
    repo: &dyn QuestionRepository,
    tag_repo: &dyn TagRepository,
    embedding: &dyn EmbeddingService,
    tag_resolver: &TagResolver,
    title: &str,
    body: &str,
    answer: &str,
    tags: Option<&[String]>,
) -> Result<CreateQuestionResponse, AppError> {
    let resolved = tags
        .map(|t| tag_resolver.resolve_tags(t))
        .unwrap_or_default();
    let tag_pairs = tag_repo.find_tag_ids(&resolved).await?;
    let tag_ids: Vec<i64> = tag_pairs.into_iter().map(|(_, id)| id).collect();
    // ... create question ...
    tag_repo.link_tags_to_question(question_id, &tag_ids).await?;
}
```

**2. `apps/api/src/services/search.rs` -- SearchService::search**

Same pattern -- resolve tags, then query via join table.

### Cache Lifecycle

1. **Startup**: `TagResolver::new(repo)` loads synonyms + canonicals into
   memory.
2. **Steady state**: All tag resolution uses in-memory data structures. No DB
   calls during request processing.
3. **Refresh**: After running `so-tag-sync`, restart the API (or call a future
   admin refresh endpoint). Tag sync is infrequent (weekly/monthly).

---

## Edge Cases & Constraints

### Normalization Edge Cases

| Input | Output | Notes |
|---|---|---|
| `"Python 3.6"` | `"python-3.6"` | Space to hyphen, dot preserved |
| `"Node.js"` | `"node.js"` | Dot preserved |
| `"C++"` | `"c++"` | Plus preserved |
| `"C#"` | `"c#"` | Hash preserved |
| `".NET"` | `".net"` | Leading dot preserved |
| `"ASP.NET Core"` | `"asp.net-core"` | Dot + space to hyphen |
| `"shared_ptr"` | `"shared-ptr"` | Underscore to hyphen |
| `"foo@bar!"` | `"foobar"` | Invalid chars stripped |
| `"rust/wasm"` | `"rustwasm"` | Slash stripped |
| `"  React  "` | `"react"` | Whitespace trimmed |
| `"UPPER_CASE"` | `"upper-case"` | Lowercased + underscore to hyphen |
| `"---"` | `""` | Only hyphens, empty after trim |
| `"@!$%"` | `""` | Only invalid chars, empty |
| `""` | `""` | Empty input |
| `"a"` | `"a"` | Single char tag |
| `"c++20"` | `"c++20"` | Plus + digits preserved |
| `"c#-10.0"` | `"c#-10.0"` | Hash + dot + hyphen preserved |
| `"pkcs#11"` | `"pkcs#11"` | Hash in middle |
| `"  hello  world  "` | `"hello-world"` | Multiple spaces collapsed |
| `"one__two"` | `"one-two"` | Multiple underscores collapsed |
| `"already-normalized"` | `"already-normalized"` | Idempotent |
| `"React_Native App"` | `"react-native-app"` | Mixed separators |
| `"über-cool"` | `"ber-cool"` | Non-ASCII stripped |

### Tag Resolution Edge Cases

| Scenario | Behavior | Example |
|---|---|---|
| Tag is a known synonym | Resolves to canonical | `js` -> `javascript` |
| Tag is already canonical | Returns unchanged | `javascript` -> `javascript` |
| Tag is a typo of a canonical | Jaro-Winkler resolves it | `"javascrip"` -> `"javascript"` |
| Tag is unresolvable | Silently dropped | `"xyzgarbage"` -> dropped |
| Synonym + canonical submitted together | Deduplicates | `["js", "javascript"]` -> `["javascript"]` |
| Two synonyms of the same canonical | Deduplicates | `["js", "ecmascript"]` -> `["javascript"]` |
| All tags unresolvable | Question saves with no tags | `["xyzgarbage"]` -> `[]` |
| Empty tag after normalization | Filtered by `normalize_tags` | `"---"` -> filtered |
| Invalid characters | Stripped by normalization | `"foo@bar"` -> `"foobar"` |
| Synonym = canonical name | Prevented by sync tool | Application-enforced |
| Jaro-Winkler score exactly 0.85 | Accepted (threshold is >=) | Boundary case |
| Very short tag (1-2 chars) | Low JW scores, likely synonym or drop | `"r"` canonical hit, `"ab"` likely dropped |
| Typo resolves to different word | Prevented by 0.85 threshold | `"react"` and `"redux"` = ~0.67, no false match |

### Constraints

| Constraint | Value | Rationale |
|---|---|---|
| Max tags per question | 5 | Existing limit, unchanged |
| Max tag length | 35 chars | Matches Stack Overflow limit |
| Tag character set | `a-z 0-9 + # . -` | Matches Stack Overflow format |
| Canonical tags in registry | ~66K (from Stack Overflow) + top 100 seed | Stack Overflow's full dataset |
| Synonyms in registry | All Stack Overflow synonyms | Stack Overflow's curated mappings |
| Jaro-Winkler threshold | 0.85 | High enough to avoid false matches |
| Stack Overflow API max page size | 100 | Hard limit from Stack Exchange API |
| Cache refresh | On restart | Sync runs are infrequent |
| Unknown tag handling | Silently dropped | Prevents registry pollution |

### Data Integrity Rules

1. A canonical tag name must not appear in the `tag_synonyms` table.
2. A synonym must not appear in the `tags` table as a canonical name.
3. Both constraints are enforced by the `so-tag-sync` tool during import.
4. The `UNIQUE` constraint on `tag_synonyms.synonym` ensures each synonym maps
   to exactly one canonical tag.
5. `question_tags.tag_id` references `tags(id) ON DELETE RESTRICT` -- a tag
   cannot be deleted while questions reference it.

### Performance Impact

| Operation | Before | After | Delta |
|---|---|---|---|
| Tag resolution (99% path) | `normalize_tags` (in-memory) | + HashMap/HashSet lookup | < 1us |
| Tag resolution (rare path) | N/A | + Jaro-Winkler vs 66K tags | ~13ms |
| Question submission | 1 INSERT | 1 INSERT + up to 5 `question_tags` INSERTs | Negligible |
| Search with tags | GIN `@>` on TEXT[] | JOIN through `question_tags` | Comparable |
| Startup | No-op | 2 DB queries to load cache | ~10ms one-time |
| Memory | None | ~66K strings + HashMap + HashSet | ~2-3MB |

### Backward Compatibility

Existing questions have tags in the `TEXT[]` column. The migration converts
them to `question_tags` rows (resolving synonyms in the process), then drops
the column. This is a one-way migration.

---

## Test Plan

### Unit Tests: `normalize_tag` / `normalize_tags`

Located in `apps/api/tests/unit/`. Run with
`cargo test -p tokenoverflow --test unit`.

| Test | Input | Expected Output |
|---|---|---|
| `lowercases` | `"JavaScript"` | `"javascript"` |
| `trims_whitespace` | `"  react  "` | `"react"` |
| `space_to_hyphen` | `"React Native"` | `"react-native"` |
| `underscore_to_hyphen` | `"shared_ptr"` | `"shared-ptr"` |
| `preserves_dot` | `"node.js"` | `"node.js"` |
| `preserves_plus` | `"c++"` | `"c++"` |
| `preserves_hash` | `"c#"` | `"c#"` |
| `preserves_leading_dot` | `".NET"` | `".net"` |
| `strips_invalid_chars` | `"foo@bar!"` | `"foobar"` |
| `strips_slash` | `"rust/wasm"` | `"rustwasm"` |
| `strips_non_ascii` | `"über-cool"` | `"ber-cool"` |
| `collapses_hyphens` | `"a--b---c"` | `"a-b-c"` |
| `collapses_mixed_separators` | `"one _ two  three"` | `"one-two-three"` |
| `trims_leading_trailing_hyphens` | `"--react--"` | `"react"` |
| `only_hyphens_empty` | `"---"` | `""` |
| `only_invalid_chars_empty` | `"@!$%"` | `""` |
| `empty_input` | `""` | `""` |
| `single_char` | `"r"` | `"r"` |
| `complex_so_tag` | `"c#-10.0"` | `"c#-10.0"` |
| `plus_with_digits` | `"c++20"` | `"c++20"` |
| `hash_in_middle` | `"pkcs#11"` | `"pkcs#11"` |
| `asp_net_core` | `"ASP.NET Core"` | `"asp.net-core"` |
| `python_version` | `"Python 3.6"` | `"python-3.6"` |
| `idempotent` | `"already-normalized"` | `"already-normalized"` |
| `normalize_tags_deduplicates` | `["js", "JS"]` | `["js"]` |
| `normalize_tags_filters_empty` | `["react", "---", "rust"]` | `["react", "rust"]` |
| `normalize_tags_preserves_order` | `["rust", "react", "go"]` | `["rust", "react", "go"]` |
| `normalize_tags_empty_input` | `[]` | `[]` |

### Unit Tests: `TagResolver`

| Test | Description |
|---|---|
| `resolves_known_synonym` | `"js"` -> `Some("javascript")` |
| `resolves_canonical` | `"javascript"` -> `Some("javascript")` |
| `resolves_typo_via_similarity` | `"javascrip"` -> `Some("javascript")` |
| `drops_unknown` | `"xyzgarbage"` -> `None` |
| `deduplicates_synonym_and_canonical` | `["js", "javascript"]` -> `["javascript"]` |
| `deduplicates_multiple_synonyms` | `["js", "ecmascript"]` -> `["javascript"]` |
| `normalizes_before_resolving` | `["JS", "Java_Script"]` -> `["javascript"]` |
| `drops_all_unknown` | `["xyzgarbage", "asdfqwer"]` -> `[]` |
| `preserves_order` | `["react", "js"]` -> `["react", "javascript"]` |
| `mixed_resolve_and_drop` | `["js", "xyzgarbage", "react"]` -> `["javascript", "react"]` |
| `empty_input` | `[]` -> `[]` |
| `similarity_threshold_boundary_above` | score = 0.86 -> resolves |
| `similarity_threshold_boundary_below` | score = 0.84 -> drops |
| `short_canonical_exact_match` | `"r"` -> `Some("r")` (canonical hit, no JW) |
| `no_false_positive_similar_tags` | `"react"` does not resolve to `"redux"` |
| `synonym_takes_priority_over_jw` | synonym hit short-circuits, no JW check |
| `canonical_takes_priority_over_jw` | canonical hit short-circuits, no JW check |
| `resolve_tags_with_invalid_chars` | `["foo@bar"]` -> normalized first, then resolved |

### Unit Tests: `so-tag-sync`

Located in `apps/so-tag-sync/tests/unit/`. Run with
`cargo test -p so-tag-sync --test unit`.

| Test | Description |
|---|---|
| `incremental_is_default_mode` | No CLI flags -> incremental mode |
| `incremental_fetches_from_watermark` | API called with `min=last_sync_date` |
| `incremental_errors_when_no_watermark` | No tags in DB -> error (run --full first) |
| `full_writes_tags_to_disk_then_synonyms` | Tags file written before synonym fetch starts |
| `full_writes_synonyms_to_disk` | Synonyms file written after synonym fetch |
| `full_loads_both_files_into_db` | DB populated from disk files |
| `from_file_skips_api` | `--from-file` mode makes zero API calls |
| `from_file_reads_both_files` | Both tags and synonyms files read |
| `upserts_are_idempotent` | Running twice produces same result |
| `dry_run_does_not_write_db` | `--dry-run` writes files but not DB |
| `handles_empty_api_response` | No new tags -> no errors, log "0 new" |
| `respects_rate_limit` | Requests spaced at 2/sec |
| `retries_on_transient_error` | 429/5xx retried up to 3 times |
| `synonym_skipped_when_canonical_missing` | Synonym pointing to unknown tag is skipped |

### Integration Tests

Located in `apps/api/tests/integration/`. Run with
`cargo test -p tokenoverflow --test integration`.

| Test | Description |
|---|---|
| `tag_repository::loads_synonyms` | `PgTagRepository` loads synonyms from seeded DB |
| `tag_repository::loads_canonicals` | `PgTagRepository` loads canonical names |
| `tag_repository::find_tag_ids_existing` | Returns IDs for tags that exist |
| `tag_repository::find_tag_ids_missing` | Returns empty for tags that don't exist |
| `tag_repository::find_tag_ids_partial` | Returns only existing subset |
| `tag_repository::links_tags_to_question` | Creates `question_tags` rows |
| `tag_repository::links_tags_idempotent` | Linking same tags twice doesn't error |
| `tag_repository::get_question_tags` | Returns tag names for a question |
| `tag_repository::get_question_tags_empty` | Returns empty for question with no tags |
| `tag_resolver::resolves_from_db` | Seed DB -> load resolver -> resolve tags |
| `question_create::resolves_tags_on_submit` | Submit with `["js"]`, verify `question_tags` has `javascript` |
| `question_create::drops_unknown_tags` | Submit with `["xyzgarbage"]`, verify no `question_tags` rows |
| `question_create::mixed_known_unknown` | Submit with `["js", "xyzgarbage"]`, only `javascript` stored |
| `question_create::typo_resolved` | Submit with `["typescipt"]`, stored as `typescript` |
| `search::resolves_tags_on_search` | Search with `["js"]`, matches question tagged `javascript` |
| `search::queries_via_join_table` | Verify search uses `question_tags` JOIN |
| `search::no_tags_still_works` | Search without tags returns results |

### E2E Tests

Located in `apps/api/tests/e2e/`. Run with
`cargo test -p tokenoverflow --test e2e`.

| Test | Description |
|---|---|
| `submit_resolves_synonym_tags` | POST submit with `["js", "React"]`, verify response has canonical tags |
| `submit_drops_unknown_tags` | POST submit with `["js", "xyzgarbage"]`, verify only canonical in response |
| `submit_resolves_typo_tags` | POST submit with `["typescipt"]`, verify stored as `typescript` |
| `submit_all_unknown_drops_all` | POST submit with `["xyzgarbage"]`, verify empty tags |
| `search_resolves_synonym_tags` | Submit with canonical, search with synonym, finds the question |
| `search_no_results_for_garbage` | Search with `["xyzgarbage"]` tag filter, returns nothing |
| `mcp_submit_resolves_tags` | MCP submit with synonyms, verify canonical tags in response |
| `mcp_submit_drops_unknown` | MCP submit with unknown tags, verify dropped |
| `mcp_search_resolves_tags` | MCP search with synonyms, verify results match canonical |
| `round_trip_synonym_search` | Submit with `["js"]`, search with `["ecmascript"]`, finds it (both are synonyms of `javascript`) |

---

## Documentation Changes

### MCP Server Instructions

Update rule 4 in `apps/api/src/mcp/server.rs`:

```
4. USE TAGS -- Always include language, framework, and library tags when
   searching or submitting (e.g., ["rust", "axum", "tower-http"]).
   Use lowercase kebab-case. Max 5 tags. Common abbreviations like "js",
   "ts", "py" are automatically resolved to their canonical forms.
   Unrecognized tags are silently ignored.
```

### Claude Plugin Instructions

Update rule 4 in `apps/claude/instructions.md` with the same note. Add
reference to `tags.md`:

```
4. USE TAGS -- Always include tags (max 5, lowercase kebab-case). Check
   tags.md for canonical tag names when possible. Common abbreviations
   are automatically resolved. Unrecognized tags are silently ignored.
```

---

## Development Environment Changes

### New Workspace Member

Add `apps/so-tag-sync` to the workspace `Cargo.toml`:

```toml
members = ["apps/api", "apps/embedding-service", "apps/so-tag-sync"]
```

### New Environment Variable

`TOKENOVERFLOW_STACKOVERFLOW_API_KEY` -- Stack Exchange API key for the
sync tool. Only needed when running `so-tag-sync` with `--full` or
incremental mode. Not
needed for `--from-file` mode or for the API itself.

### New Dependency (API crate)

`strsim = "0.11"` -- string similarity for Jaro-Winkler in `TagResolver`.
Zero runtime dependencies, pure safe Rust, 37M monthly downloads.

---

## Tasks

### Task 1: Database Migration

**Scope:** Create the Diesel migration that adds `tags`, `tag_synonyms`, and
`question_tags` tables, seeds the top 100 tags, migrates existing data from
`TEXT[]`, and drops the old column.

**Files:**
- `apps/api/migrations/<timestamp>_tag_standardization/up.sql`
- `apps/api/migrations/<timestamp>_tag_standardization/down.sql`

**Requirements:**
- `tags` table with `id`, `name` (unique, varchar(35)), `created_at`,
  `updated_at`
- `tag_synonyms` table with `id`, `synonym` (unique), `tag_id` (FK),
  `created_at`, `updated_at`
- `question_tags` join table with composite PK, indexes, FK constraints
- Seed top 100 Stack Overflow tags + most common synonyms
- Migrate existing `questions.tags` data to `question_tags`
- Drop `questions.tags` column and GIN index
- `down.sql` reverses the migration

**Success criteria:**
- `diesel migration run` succeeds
- `diesel migration redo` succeeds
- `diesel print-schema` reflects new tables

### Task 2: Update Normalization

**Scope:** Update `normalize_tag` to strip characters outside Stack
Overflow's allowed set.

**Files:**
- `apps/api/src/services/tags.rs` (update)
- Unit tests for new normalization behavior (28 tests)

**Requirements:**
- Strip chars not in `a-z`, `0-9`, `+`, `#`, `.`, `-`
- All tests from the normalization test plan pass
- Idempotent: normalizing an already-normalized tag returns it unchanged

**Success criteria:**
- All 28 normalization unit tests pass

### Task 3: TagRepository Trait and PgTagRepository

**Scope:** Define the `TagRepository` trait and implement `PgTagRepository`.

**Files:**
- `apps/api/src/services/repository/tag.rs` (new -- trait)
- `apps/api/src/services/repository/pg_tag.rs` (new -- implementation)
- `apps/api/src/services/repository/mod.rs` (update)
- `apps/api/src/db/schema.rs` (auto-generated)

**Requirements:**
- `load_synonyms()` returns `HashMap<String, String>`
- `load_canonicals()` returns `Vec<String>`
- `find_tag_ids()` maps names to IDs
- `link_tags_to_question()` inserts `question_tags` rows
- `get_question_tags()` retrieves tag names for a question

**Success criteria:**
- All 9 tag repository integration tests pass

### Task 4: TagResolver

**Scope:** Implement the three-layer in-memory tag resolver.

**Files:**
- `apps/api/src/services/tag_resolver.rs` (new)
- `apps/api/src/services/mod.rs` (update)
- `apps/api/Cargo.toml` -- add `strsim = "0.11"` dependency

**Requirements:**
- Three-layer resolution: synonym -> canonical -> Jaro-Winkler
- `resolve()` returns `Option<String>` (None = drop)
- `resolve_tags()` normalizes, resolves, deduplicates
- `from_data()` constructor for unit tests
- `refresh()` for cache reload
- All 18 resolver unit tests pass

### Task 5: Service Layer Integration

**Scope:** Wire `TagResolver` and `TagRepository` into the service layer,
replacing `normalize_tags` calls and `TEXT[]` storage.

**Files:**
- `apps/api/src/services/question.rs` (update)
- `apps/api/src/services/search.rs` (update)
- `apps/api/src/services/repository/pg_search.rs` (update -- JOIN query)
- `apps/api/src/api/state.rs` (update)
- `apps/api/src/api/routes/questions.rs` (update)
- `apps/api/src/api/routes/search.rs` (update)
- `apps/api/src/mcp/tools/submit.rs` (update)
- `apps/api/src/mcp/tools/search_questions.rs` (update)
- `apps/api/src/main.rs` (update -- initialize TagResolver)
- `apps/api/src/db/models/question.rs` (update -- remove tags field)

**Requirements:**
- `TagResolver` in `AppState` as `Arc<TagResolver>`
- Submit path: resolve tags -> find IDs -> insert `question_tags` rows
- Search path: resolve tags -> JOIN query through `question_tags`
- Response: retrieve tags via `get_question_tags()`
- All existing + new integration tests pass
- All E2E tests pass

### Task 6: `so-tag-sync` CLI Tool

**Scope:** Build the Stack Overflow tag sync binary with full,
incremental, and from-file modes.

**Files:**
- `apps/so-tag-sync/Cargo.toml` (new)
- `apps/so-tag-sync/src/main.rs` (new)
- `Cargo.toml` (update -- add workspace member)

**Requirements:**
- Incremental mode (default): fetch from `MAX(created_at)` watermark
- Full mode: fetch tags -> write to disk, fetch synonyms -> write to disk,
  then load both into DB in one transaction
- From-file mode: read both files -> load into DB
- Rate limiting, retries, structured logging
- All 14 sync unit tests pass

### Task 7: Claude Plugin `tags.md` + Documentation

**Scope:** Create `tags.md` and update instructions.

**Files:**
- `apps/claude/tags.md` (new -- flat file, one tag per line)
- `apps/claude/instructions.md` (update rule 4)
- `apps/claude/skills/submit-to-tokenoverflow/SKILL.md` (update)
- `apps/api/src/mcp/server.rs` (update rule 4)

**Requirements:**
- `tags.md` lists all canonical tags, one per line, nothing else
- Instructions reference `tags.md` and mention automatic resolution
- Instructions note that unrecognized tags are silently ignored
