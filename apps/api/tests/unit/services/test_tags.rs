use tokenoverflow::services::tags::{normalize_tag, normalize_tags};

// --- normalize_tag ---

#[test]
fn lowercases() {
    assert_eq!(normalize_tag("JavaScript"), "javascript");
}

#[test]
fn trims_whitespace() {
    assert_eq!(normalize_tag("  react  "), "react");
}

#[test]
fn space_to_hyphen() {
    assert_eq!(normalize_tag("React Native"), "react-native");
}

#[test]
fn underscore_to_hyphen() {
    assert_eq!(normalize_tag("shared_ptr"), "shared-ptr");
}

#[test]
fn preserves_dot() {
    assert_eq!(normalize_tag("node.js"), "node.js");
}

#[test]
fn preserves_plus() {
    assert_eq!(normalize_tag("c++"), "c++");
}

#[test]
fn preserves_hash() {
    assert_eq!(normalize_tag("c#"), "c#");
}

#[test]
fn preserves_leading_dot() {
    assert_eq!(normalize_tag(".NET"), ".net");
}

#[test]
fn strips_invalid_chars() {
    assert_eq!(normalize_tag("foo@bar!"), "foobar");
}

#[test]
fn strips_slash() {
    assert_eq!(normalize_tag("rust/wasm"), "rustwasm");
}

#[test]
fn strips_non_ascii() {
    assert_eq!(normalize_tag("über-cool"), "ber-cool");
}

#[test]
fn collapses_hyphens() {
    assert_eq!(normalize_tag("a--b---c"), "a-b-c");
}

#[test]
fn collapses_mixed_separators() {
    assert_eq!(normalize_tag("one _ two  three"), "one-two-three");
}

#[test]
fn trims_leading_trailing_hyphens() {
    assert_eq!(normalize_tag("--react--"), "react");
}

#[test]
fn only_hyphens_empty() {
    assert_eq!(normalize_tag("---"), "");
}

#[test]
fn only_invalid_chars_empty() {
    assert_eq!(normalize_tag("@!$%"), "");
}

#[test]
fn empty_input() {
    assert_eq!(normalize_tag(""), "");
}

#[test]
fn single_char() {
    assert_eq!(normalize_tag("r"), "r");
}

#[test]
fn complex_so_tag() {
    assert_eq!(normalize_tag("c#-10.0"), "c#-10.0");
}

#[test]
fn plus_with_digits() {
    assert_eq!(normalize_tag("c++20"), "c++20");
}

#[test]
fn hash_in_middle() {
    assert_eq!(normalize_tag("pkcs#11"), "pkcs#11");
}

#[test]
fn asp_net_core() {
    assert_eq!(normalize_tag("ASP.NET Core"), "asp.net-core");
}

#[test]
fn python_version() {
    assert_eq!(normalize_tag("Python 3.6"), "python-3.6");
}

#[test]
fn idempotent() {
    assert_eq!(normalize_tag("already-normalized"), "already-normalized");
}

#[test]
fn mixed_separators_tag() {
    assert_eq!(normalize_tag("React_Native App"), "react-native-app");
}

// --- normalize_tags ---

#[test]
fn normalize_tags_deduplicates() {
    let tags = vec!["js".to_string(), "JS".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["js"]);
}

#[test]
fn normalize_tags_filters_empty() {
    let tags = vec!["react".to_string(), "---".to_string(), "rust".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["react", "rust"]);
}

#[test]
fn normalize_tags_preserves_order() {
    let tags = vec!["rust".to_string(), "react".to_string(), "go".to_string()];
    assert_eq!(normalize_tags(&tags), vec!["rust", "react", "go"]);
}

#[test]
fn normalize_tags_empty_input() {
    let tags: Vec<String> = vec![];
    let result: Vec<String> = vec![];
    assert_eq!(normalize_tags(&tags), result);
}
