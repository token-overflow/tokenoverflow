use clap::Parser;

use so_tag_sync::cli::Cli;

#[test]
fn default_mode_is_incremental() {
    let cli = Cli::parse_from(["so_tag_sync"]);
    assert!(!cli.full);
    assert!(!cli.from_file);
    assert!(!cli.dry_run);
}

#[test]
fn full_flag_sets_full_mode() {
    let cli = Cli::parse_from(["so_tag_sync", "--full"]);
    assert!(cli.full);
    assert!(!cli.from_file);
}

#[test]
fn from_file_flag_sets_from_file_mode() {
    let cli = Cli::parse_from(["so_tag_sync", "--from-file"]);
    assert!(cli.from_file);
    assert!(!cli.full);
}

#[test]
fn dry_run_flag() {
    let cli = Cli::parse_from(["so_tag_sync", "--full", "--dry-run"]);
    assert!(cli.full);
    assert!(cli.dry_run);
}

#[test]
fn default_tags_file_path() {
    let cli = Cli::parse_from(["so_tag_sync"]);
    assert_eq!(cli.tags_file.to_str().unwrap(), "stackoverflow_tags.json");
}

#[test]
fn default_synonyms_file_path() {
    let cli = Cli::parse_from(["so_tag_sync"]);
    assert_eq!(
        cli.synonyms_file.to_str().unwrap(),
        "stackoverflow_synonyms.json"
    );
}

#[test]
fn custom_tags_file_path() {
    let cli = Cli::parse_from(["so_tag_sync", "--tags-file", "/tmp/my-tags.json"]);
    assert_eq!(cli.tags_file.to_str().unwrap(), "/tmp/my-tags.json");
}

#[test]
fn custom_synonyms_file_path() {
    let cli = Cli::parse_from(["so_tag_sync", "--synonyms-file", "/tmp/my-synonyms.json"]);
    assert_eq!(cli.synonyms_file.to_str().unwrap(), "/tmp/my-synonyms.json");
}

#[test]
fn all_flags_combined() {
    let cli = Cli::parse_from([
        "so_tag_sync",
        "--full",
        "--dry-run",
        "--tags-file",
        "custom-tags.json",
        "--synonyms-file",
        "custom-synonyms.json",
    ]);
    assert!(cli.full);
    assert!(cli.dry_run);
    assert_eq!(cli.tags_file.to_str().unwrap(), "custom-tags.json");
    assert_eq!(cli.synonyms_file.to_str().unwrap(), "custom-synonyms.json");
}
