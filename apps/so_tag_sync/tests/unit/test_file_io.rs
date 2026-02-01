use so_tag_sync::file_io;
use so_tag_sync::types::{StackOverflowSynonym, StackOverflowTag};

#[test]
fn write_and_read_tags_file_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.json");

    let tags = vec![
        StackOverflowTag {
            name: "javascript".to_string(),
            count: 2533073,
        },
        StackOverflowTag {
            name: "python".to_string(),
            count: 2221821,
        },
        StackOverflowTag {
            name: "c++".to_string(),
            count: 801234,
        },
    ];

    file_io::write_tags_file(&path, &tags).unwrap();
    let read_back = file_io::read_tags_file(&path).unwrap();

    assert_eq!(read_back.len(), 3);
    assert_eq!(read_back[0].name, "javascript");
    assert_eq!(read_back[0].count, 2533073);
    assert_eq!(read_back[1].name, "python");
    assert_eq!(read_back[2].name, "c++");
}

#[test]
fn write_and_read_synonyms_file_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synonyms.json");

    let synonyms = vec![
        StackOverflowSynonym {
            from: "js".to_string(),
            to: "javascript".to_string(),
        },
        StackOverflowSynonym {
            from: "py".to_string(),
            to: "python".to_string(),
        },
    ];

    file_io::write_synonyms_file(&path, &synonyms).unwrap();
    let read_back = file_io::read_synonyms_file(&path).unwrap();

    assert_eq!(read_back.len(), 2);
    assert_eq!(read_back[0].from, "js");
    assert_eq!(read_back[0].to, "javascript");
    assert_eq!(read_back[1].from, "py");
    assert_eq!(read_back[1].to, "python");
}

#[test]
fn write_tags_file_creates_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.json");

    let tags = vec![StackOverflowTag {
        name: "rust".to_string(),
        count: 50000,
    }];

    file_io::write_tags_file(&path, &tags).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(parsed.get("fetched_at").is_some());
    assert!(parsed.get("tags").unwrap().is_array());
    assert_eq!(parsed["tags"][0]["name"], "rust");
    assert_eq!(parsed["tags"][0]["count"], 50000);
}

#[test]
fn write_synonyms_file_creates_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synonyms.json");

    let synonyms = vec![StackOverflowSynonym {
        from: "k8s".to_string(),
        to: "kubernetes".to_string(),
    }];

    file_io::write_synonyms_file(&path, &synonyms).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(parsed.get("fetched_at").is_some());
    assert!(parsed.get("synonyms").unwrap().is_array());
    assert_eq!(parsed["synonyms"][0]["from"], "k8s");
    assert_eq!(parsed["synonyms"][0]["to"], "kubernetes");
}

#[test]
fn write_empty_tags_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.json");

    file_io::write_tags_file(&path, &[]).unwrap();
    let read_back = file_io::read_tags_file(&path).unwrap();

    assert!(read_back.is_empty());
}

#[test]
fn write_empty_synonyms_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synonyms.json");

    file_io::write_synonyms_file(&path, &[]).unwrap();
    let read_back = file_io::read_synonyms_file(&path).unwrap();

    assert!(read_back.is_empty());
}

#[test]
fn read_tags_file_nonexistent_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");

    let result = file_io::read_tags_file(&path);
    assert!(result.is_err());
}

#[test]
fn read_synonyms_file_nonexistent_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");

    let result = file_io::read_synonyms_file(&path);
    assert!(result.is_err());
}

#[test]
fn read_tags_file_invalid_json_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.json");
    std::fs::write(&path, "not valid json").unwrap();

    let result = file_io::read_tags_file(&path);
    assert!(result.is_err());
}

#[test]
fn read_synonyms_file_invalid_json_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.json");
    std::fs::write(&path, "not valid json").unwrap();

    let result = file_io::read_synonyms_file(&path);
    assert!(result.is_err());
}

#[test]
fn tags_file_preserves_special_characters() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.json");

    let tags = vec![
        StackOverflowTag {
            name: "c#".to_string(),
            count: 1000,
        },
        StackOverflowTag {
            name: "c++".to_string(),
            count: 2000,
        },
        StackOverflowTag {
            name: "node.js".to_string(),
            count: 3000,
        },
        StackOverflowTag {
            name: ".net".to_string(),
            count: 4000,
        },
    ];

    file_io::write_tags_file(&path, &tags).unwrap();
    let read_back = file_io::read_tags_file(&path).unwrap();

    assert_eq!(read_back[0].name, "c#");
    assert_eq!(read_back[1].name, "c++");
    assert_eq!(read_back[2].name, "node.js");
    assert_eq!(read_back[3].name, ".net");
}

#[test]
fn tags_file_overwrite_replaces_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.json");

    let first = vec![StackOverflowTag {
        name: "old".to_string(),
        count: 1,
    }];
    file_io::write_tags_file(&path, &first).unwrap();

    let second = vec![StackOverflowTag {
        name: "new".to_string(),
        count: 2,
    }];
    file_io::write_tags_file(&path, &second).unwrap();

    let read_back = file_io::read_tags_file(&path).unwrap();
    assert_eq!(read_back.len(), 1);
    assert_eq!(read_back[0].name, "new");
}

#[test]
fn large_tags_file_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.json");

    let tags: Vec<StackOverflowTag> = (0..10_000)
        .map(|i| StackOverflowTag {
            name: format!("tag-{}", i),
            count: i,
        })
        .collect();

    file_io::write_tags_file(&path, &tags).unwrap();
    let read_back = file_io::read_tags_file(&path).unwrap();

    assert_eq!(read_back.len(), 10_000);
    assert_eq!(read_back[0].name, "tag-0");
    assert_eq!(read_back[9999].name, "tag-9999");
}
