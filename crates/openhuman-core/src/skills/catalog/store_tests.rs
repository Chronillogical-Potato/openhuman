use super::*;

/// Write a cache file directly with a controlled `fetched_at_epoch`.
fn write_cache(dir: &std::path::Path, fetched_at_epoch: u64) {
    let cache = CatalogCache {
        entries: Vec::new(),
        fetched_at_epoch,
    };
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(CACHE_FILE), serde_json::to_string(&cache).unwrap()).unwrap();
}
