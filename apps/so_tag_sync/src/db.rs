use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Array, Nullable, Timestamptz, VarChar};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use tracing::{info, warn};

use crate::types::{StackOverflowSynonym, StackOverflowTag};

pub async fn create_pool(database_url: &str) -> Result<Pool<AsyncPgConnection>> {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    Pool::builder()
        .max_size(2)
        .build(config)
        .await
        .context("Failed to create database connection pool")
}

pub async fn upsert_tags(
    pool: &Pool<AsyncPgConnection>,
    tags: &[StackOverflowTag],
) -> Result<usize> {
    let mut conn = pool.get().await.context("Failed to get DB connection")?;
    let mut count = 0;

    // Deduplicate — the SO API can return the same tag on multiple pages
    let mut seen = HashSet::with_capacity(tags.len());
    let unique_names: Vec<&str> = tags
        .iter()
        .filter(|t| seen.insert(t.name.as_str()))
        .map(|t| t.name.as_str())
        .collect();

    if unique_names.len() < tags.len() {
        info!(
            "Deduplicated {} -> {} unique tags",
            tags.len(),
            unique_names.len()
        );
    }

    for chunk in unique_names.chunks(1000) {
        let names: Vec<String> = chunk.iter().map(|n| (*n).to_owned()).collect();

        let result = diesel::sql_query(
            "INSERT INTO api.tags (name) \
             SELECT unnest($1::varchar[]) \
             ON CONFLICT (name) DO UPDATE SET updated_at = NOW()",
        )
        .bind::<Array<VarChar>, _>(&names)
        .execute(&mut *conn)
        .await
        .context("Failed to upsert tags")?;

        count += result;
    }

    Ok(count)
}

#[derive(QueryableByName)]
struct TagIdRow {
    #[diesel(sql_type = VarChar)]
    name: String,
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    id: uuid::Uuid,
}

pub async fn upsert_synonyms(
    pool: &Pool<AsyncPgConnection>,
    synonyms: &[StackOverflowSynonym],
) -> Result<usize> {
    let mut conn = pool.get().await.context("Failed to get DB connection")?;
    let mut count = 0;

    // Deduplicate by synonym (the "from" field) — keep first occurrence
    let mut seen = HashSet::with_capacity(synonyms.len());
    let unique: Vec<&StackOverflowSynonym> = synonyms
        .iter()
        .filter(|s| seen.insert(s.from.as_str()))
        .collect();

    if unique.len() < synonyms.len() {
        info!(
            "Deduplicated {} -> {} unique synonyms",
            synonyms.len(),
            unique.len()
        );
    }

    // Pre-fetch existing tag name -> id mapping so we can filter out synonyms
    // whose target tag is missing and resolve tag IDs for insertion.
    let tag_map: HashMap<String, uuid::Uuid> = {
        let rows: Vec<TagIdRow> = diesel::sql_query("SELECT name, id FROM api.tags")
            .load(&mut *conn)
            .await
            .context("Failed to query existing tags")?;
        rows.into_iter().map(|r| (r.name, r.id)).collect()
    };

    let mut skipped: Vec<String> = Vec::new();
    let mut synonym_names: Vec<String> = Vec::with_capacity(unique.len());
    let mut tag_ids: Vec<uuid::Uuid> = Vec::with_capacity(unique.len());

    for s in &unique {
        if let Some(&resolved_tag_id) = tag_map.get(s.to.as_str()) {
            synonym_names.push(s.from.clone());
            tag_ids.push(resolved_tag_id);
        } else {
            skipped.push(format!("{} -> {}", s.from, s.to));
        }
    }

    if !skipped.is_empty() {
        warn!(
            "Skipped {} synonyms whose target tags don't exist (e.g. {})",
            skipped.len(),
            skipped
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    for (names_chunk, ids_chunk) in synonym_names.chunks(500).zip(tag_ids.chunks(500)) {
        let result = diesel::sql_query(
            "INSERT INTO api.tag_synonyms (synonym, tag_id) \
             SELECT * FROM unnest($1::varchar[], $2::uuid[]) \
             ON CONFLICT (synonym) DO UPDATE SET updated_at = NOW()",
        )
        .bind::<Array<VarChar>, _>(names_chunk)
        .bind::<Array<diesel::sql_types::Uuid>, _>(ids_chunk)
        .execute(&mut *conn)
        .await
        .context("Failed to upsert synonyms")?;

        count += result;
    }

    Ok(count)
}

#[derive(QueryableByName)]
struct LastSyncRow {
    #[diesel(sql_type = Nullable<Timestamptz>)]
    max_date: Option<DateTime<Utc>>,
}

pub async fn get_last_sync_date(pool: &Pool<AsyncPgConnection>) -> Result<Option<DateTime<Utc>>> {
    let mut conn = pool.get().await.context("Failed to get DB connection")?;

    let row: LastSyncRow = diesel::sql_query("SELECT MAX(created_at) AS max_date FROM api.tags")
        .get_result(&mut *conn)
        .await
        .context("Failed to query last sync date")?;

    Ok(row.max_date)
}
