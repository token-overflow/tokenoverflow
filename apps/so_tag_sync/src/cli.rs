use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "so_tag_sync",
    about = "Sync Stack Overflow tags into TokenOverflow"
)]
pub struct Cli {
    /// Full sync: fetch ALL tags/synonyms from Stack Overflow, write to disk, load into DB.
    #[arg(long)]
    pub full: bool,

    /// Skip Stack Overflow API, load from previously saved files.
    #[arg(long)]
    pub from_file: bool,

    /// Path for tags data file.
    #[arg(long, default_value = "stackoverflow_tags.json")]
    pub tags_file: PathBuf,

    /// Path for synonyms data file.
    #[arg(long, default_value = "stackoverflow_synonyms.json")]
    pub synonyms_file: PathBuf,

    /// Fetch and save to disk but do not write to DB.
    #[arg(long)]
    pub dry_run: bool,
}
