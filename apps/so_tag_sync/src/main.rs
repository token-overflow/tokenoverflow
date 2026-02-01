#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use anyhow::{Result, bail};
use clap::Parser;
use tracing::info;

use so_tag_sync::api_client::StackOverflowClient;
use so_tag_sync::cli::Cli;
use so_tag_sync::{config, db, file_io};

// Thin CLI entry point — orchestrates individually tested modules
// (api_client, db, file_io, config). Each module has unit + integration tests.
#[cfg_attr(coverage_nightly, coverage(off))]
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let api_key = std::env::var("TOKENOVERFLOW_STACKOVERFLOW_API_KEY").ok();
    let access_token = std::env::var("TOKENOVERFLOW_STACKOVERFLOW_ACCESS_TOKEN").ok();

    if cli.from_file {
        run_from_file(&cli).await
    } else if cli.full {
        run_full_sync(&cli, api_key, access_token).await
    } else {
        run_incremental_sync(api_key, access_token).await
    }
}

/// Mode 3: Load tags and synonyms from previously saved JSON files.
// Orchestration-only — file_io, config, db each have their own tests
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_from_file(cli: &Cli) -> Result<()> {
    info!("Mode: from-file");
    let tags = file_io::read_tags_file(&cli.tags_file)?;
    let synonyms = file_io::read_synonyms_file(&cli.synonyms_file)?;

    if !cli.dry_run {
        let db_url = config::get_database_url()?;
        let pool = db::create_pool(&db_url).await?;
        let tag_count = db::upsert_tags(&pool, &tags).await?;
        let syn_count = db::upsert_synonyms(&pool, &synonyms).await?;
        info!(
            "Loaded {} tags and {} synonyms from files into DB",
            tag_count, syn_count
        );
    } else {
        info!("Dry run: skipping DB writes");
    }

    Ok(())
}

/// Mode 2: Fetch ALL tags/synonyms from Stack Overflow, write to disk, then load into DB.
// Orchestration-only — api_client, file_io, config, db each have their own tests
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_full_sync(
    cli: &Cli,
    api_key: Option<String>,
    access_token: Option<String>,
) -> Result<()> {
    info!("Mode: full sync");
    let client = StackOverflowClient::new(api_key, access_token);

    let tags = client.fetch_all_tags().await?;
    file_io::write_tags_file(&cli.tags_file, &tags)?;

    let synonyms = client.fetch_all_synonyms().await?;
    file_io::write_synonyms_file(&cli.synonyms_file, &synonyms)?;

    if !cli.dry_run {
        let db_url = config::get_database_url()?;
        let pool = db::create_pool(&db_url).await?;
        let tag_count = db::upsert_tags(&pool, &tags).await?;
        let syn_count = db::upsert_synonyms(&pool, &synonyms).await?;
        info!(
            "Full sync complete: {} tags, {} synonyms",
            tag_count, syn_count
        );
    } else {
        info!("Dry run: files written, DB writes skipped");
    }

    Ok(())
}

/// Mode 1 (default): Incremental sync — only fetch tags/synonyms newer than the last sync.
// Orchestration-only — config, db, api_client each have their own tests
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_incremental_sync(api_key: Option<String>, access_token: Option<String>) -> Result<()> {
    info!("Mode: incremental sync");
    let db_url = config::get_database_url()?;
    let pool = db::create_pool(&db_url).await?;

    let last_sync = db::get_last_sync_date(&pool).await?;
    let last_sync = match last_sync {
        Some(date) => date,
        None => bail!("No tags in database. Run with --full first."),
    };

    info!("Last sync: {}", last_sync);

    let client = StackOverflowClient::new(api_key, access_token);
    let min_timestamp = last_sync.timestamp();

    let tags = client.fetch_tags_since(min_timestamp).await?;
    if !tags.is_empty() {
        let tag_count = db::upsert_tags(&pool, &tags).await?;
        info!("{} tags upserted", tag_count);
    } else {
        info!("No new tags since last sync");
    }

    let synonyms = client.fetch_synonyms_since(min_timestamp).await?;
    if !synonyms.is_empty() {
        let syn_count = db::upsert_synonyms(&pool, &synonyms).await?;
        info!("{} synonyms upserted", syn_count);
    } else {
        info!("No new synonyms since last sync");
    }

    Ok(())
}
