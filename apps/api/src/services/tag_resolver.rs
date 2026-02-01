use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use strsim::jaro_winkler;

use crate::error::AppError;
use crate::services::repository::TagRepository;
use crate::services::tags::normalize_tags;

const SIMILARITY_THRESHOLD: f64 = 0.85;

/// Three-layer in-memory tag resolver.
///
/// Layer 1: Synonym HashMap (O(1)) -- resolves abbreviations like "js" -> "javascript"
/// Layer 2: Canonical HashSet (O(1)) -- verifies exact canonical matches
/// Layer 3: Jaro-Winkler similarity (O(n)) -- catches typos, rare path
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
    pub async fn new<Conn: Send>(
        repo: &(dyn TagRepository<Conn> + Sync),
        conn: &mut Conn,
    ) -> Result<Self, AppError> {
        let synonyms = repo.load_synonyms(conn).await?;
        let canonicals = repo.load_canonicals(conn).await?;
        let canonical_set: HashSet<String> = canonicals.iter().cloned().collect();
        Ok(Self {
            synonyms: RwLock::new(synonyms),
            canonical_set: RwLock::new(canonical_set),
            canonical_list: RwLock::new(canonicals),
        })
    }

    /// Construct from raw data (for unit tests, no DB needed).
    pub fn from_data(synonyms: HashMap<String, String>, canonicals: Vec<String>) -> Self {
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

        if let Some((canonical, _)) = best.filter(|(_, s)| *s >= SIMILARITY_THRESHOLD) {
            return Some(canonical.clone());
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
            if let Some(resolved) = self.resolve(&tag)
                && seen.insert(resolved.clone())
            {
                result.push(resolved);
            }
        }
        result
    }

    /// Reload all data from the database.
    pub async fn refresh<Conn: Send>(
        &self,
        repo: &(dyn TagRepository<Conn> + Sync),
        conn: &mut Conn,
    ) -> Result<(), AppError> {
        let synonyms = repo.load_synonyms(conn).await?;
        let canonicals = repo.load_canonicals(conn).await?;
        let canonical_set: HashSet<String> = canonicals.iter().cloned().collect();

        *self.synonyms.write().expect("RwLock poisoned") = synonyms;
        *self.canonical_set.write().expect("RwLock poisoned") = canonical_set;
        *self.canonical_list.write().expect("RwLock poisoned") = canonicals;
        Ok(())
    }
}
