use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
};

use skia_safe::{
    Data, FontMgr, Typeface,
    resources::{ResourceProvider, UReqResourceProvider},
};

/// A Skia [`ResourceProvider`] that:
/// - Caches loaded resources (per path + name) in a shared `HashMap`.
/// - Probes local filesystem directories before falling back to HTTP(S).
/// - Is cheaply cloneable for sharing state with Skia's native provider wrapper.
#[derive(Debug, Clone)]
pub(crate) struct CachedResourceProvider {
    // HTTP(S) fetcher backed by `ureq`; used as fallback.
    inner: Rc<UReqResourceProvider>,
    // Shared cache keyed by local path or `"{resource_path}\0{resource_name}"` for fallback.
    cache: Arc<Mutex<HashMap<String, Data>>>,
    // Ordered list of local directories to search.
    search_dirs: Arc<Mutex<Vec<PathBuf>>>,
}

impl CachedResourceProvider {
    pub(crate) fn new(font_mgr: impl Into<FontMgr>) -> Self {
        Self {
            inner: Rc::new(UReqResourceProvider::new(font_mgr)),
            cache: Arc::new(Mutex::new(HashMap::new())),
            search_dirs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn add_search_dir(&self, dir: impl Into<PathBuf>) {
        self.search_dirs
            .lock()
            .expect("resource search dir mutex poisoned")
            .push(dir.into());
    }

    pub(crate) fn set_search_dirs<I, P>(&self, dirs: I)
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let mut search_dirs = self
            .search_dirs
            .lock()
            .expect("resource search dir mutex poisoned");
        search_dirs.clear();
        search_dirs.extend(dirs.into_iter().map(Into::into));
    }

    /// Composite key so `("fonts", "Roboto.ttf")` and `("images", "logo.svg")`
    /// don't collide even if both refer to a file named identically.
    fn cache_key(resource_path: &str, resource_name: &str) -> String {
        if resource_path.is_empty() {
            resource_name.to_owned()
        } else {
            format!("{resource_path}\0{resource_name}")
        }
    }

    /// Tries to load a resource from the local filesystem.
    fn load_local(&self, resource_path: &str, resource_name: &str) -> Option<(String, Data)> {
        for path in self.local_candidates(resource_path, resource_name) {
            if let Ok(bytes) = fs::read(&path) {
                let key = format!("local\0{}", path.to_string_lossy());
                return Some((key, Data::new_copy(&bytes)));
            }
        }
        None
    }

    /// Builds candidate paths for a local resource lookup.
    ///
    /// Skips URL-like and data URIs. Tries:
    /// 1. `{resource_path}/{resource_name}` (if resource_path is a local dir)
    /// 2. `{search_dir}/{resource_name}` for each registered search directory.
    fn local_candidates(&self, resource_path: &str, resource_name: &str) -> Vec<PathBuf> {
        let resource_name = resource_name.trim();
        if resource_name.is_empty()
            || is_url_like(resource_name)
            || resource_name.starts_with("data:")
        {
            return Vec::new();
        }

        let resource_name_path = Path::new(resource_name);
        if resource_name_path.is_absolute() {
            return vec![resource_name_path.to_path_buf()];
        }

        let mut candidates = Vec::new();
        if !resource_path.is_empty() && !is_url_like(resource_path) {
            candidates.push(Path::new(resource_path).join(resource_name_path));
        }

        if let Ok(search_dirs) = self.search_dirs.lock() {
            candidates.extend(search_dirs.iter().map(|dir| dir.join(resource_name_path)));
        }

        candidates
    }
}

/// Returns true if the string looks like a URL (http, https, file) that
/// should be fetched by the upstream `UReqResourceProvider` instead of
/// the local filesystem.
fn is_url_like(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("file://")
}

impl ResourceProvider for CachedResourceProvider {
    fn load(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        let key = Self::cache_key(resource_path, resource_name);

        // Check the in-memory cache first.
        if let Some(data) = self.cache.lock().ok()?.get(&key).cloned() {
            return Some(data);
        }

        if let Some((local_key, data)) = self.load_local(resource_path, resource_name) {
            self.cache.lock().ok()?.insert(local_key, data.clone());
            return Some(data);
        }

        // Fall back to HTTP(S) via ureq.
        let data = self.inner.load(resource_path, resource_name)?;
        self.cache.lock().ok()?.insert(key, data.clone());
        Some(data)
    }

    fn load_typeface(&self, name: &str, url: &str) -> Option<Typeface> {
        self.inner.load_typeface(name, url)
    }

    fn font_mgr(&self) -> FontMgr {
        self.inner.font_mgr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloned_resource_provider_shares_cache() {
        let provider = CachedResourceProvider::new(FontMgr::default());
        let cloned = provider.clone();

        assert!(Arc::ptr_eq(&provider.cache, &cloned.cache));
    }

    #[test]
    fn cloned_resource_provider_shares_search_dirs() {
        let provider = CachedResourceProvider::new(FontMgr::default());
        let cloned = provider.clone();

        provider.add_search_dir("assets");

        assert_eq!(
            cloned.local_candidates("", "image.png"),
            vec![PathBuf::from("assets").join("image.png")]
        );
    }

    #[test]
    fn changing_search_dirs_does_not_reuse_same_named_local_resource() {
        let root = std::env::temp_dir().join(format!(
            "svg-renderer-resource-cache-{}",
            std::process::id()
        ));
        let first_dir = root.join("first");
        let second_dir = root.join("second");
        fs::create_dir_all(&first_dir).unwrap();
        fs::create_dir_all(&second_dir).unwrap();
        fs::write(first_dir.join("image.bin"), b"first").unwrap();
        fs::write(second_dir.join("image.bin"), b"second").unwrap();

        let provider = CachedResourceProvider::new(FontMgr::default());
        provider.set_search_dirs([first_dir]);
        let first = provider.load("", "image.bin").unwrap();

        provider.set_search_dirs([second_dir]);
        let second = provider.load("", "image.bin").unwrap();

        assert_eq!(first.as_bytes(), b"first");
        assert_eq!(second.as_bytes(), b"second");

        let _ = fs::remove_dir_all(root);
    }
}
