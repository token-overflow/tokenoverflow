use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::info;

use crate::types::{StackOverflowSynonym, StackOverflowTag, SynonymsFile, TagsFile};

pub fn write_tags_file(path: &Path, tags: &[StackOverflowTag]) -> Result<()> {
    let file = TagsFile {
        fetched_at: Utc::now(),
        tags: tags.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    fs::write(path, json).context("Failed to write tags file")?;
    info!("Wrote {} tags to {}", tags.len(), path.display());
    Ok(())
}

pub fn write_synonyms_file(path: &Path, synonyms: &[StackOverflowSynonym]) -> Result<()> {
    let file = SynonymsFile {
        fetched_at: Utc::now(),
        synonyms: synonyms.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    fs::write(path, json).context("Failed to write synonyms file")?;
    info!("Wrote {} synonyms to {}", synonyms.len(), path.display());
    Ok(())
}

pub fn read_tags_file(path: &Path) -> Result<Vec<StackOverflowTag>> {
    let json = fs::read_to_string(path).context("Failed to read tags file")?;
    let file: TagsFile = serde_json::from_str(&json).context("Failed to parse tags file")?;
    info!(
        "Read {} tags from {} (fetched at {})",
        file.tags.len(),
        path.display(),
        file.fetched_at
    );
    Ok(file.tags)
}

pub fn read_synonyms_file(path: &Path) -> Result<Vec<StackOverflowSynonym>> {
    let json = fs::read_to_string(path).context("Failed to read synonyms file")?;
    let file: SynonymsFile =
        serde_json::from_str(&json).context("Failed to parse synonyms file")?;
    info!(
        "Read {} synonyms from {} (fetched at {})",
        file.synonyms.len(),
        path.display(),
        file.fetched_at
    );
    Ok(file.synonyms)
}
