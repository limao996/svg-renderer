use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use skia_safe::{
    Data, FontMgr, Typeface,
    resources::{ResourceProvider, UReqResourceProvider},
};

#[derive(Debug, Clone)]
pub(crate) struct CachedResourceProvider {
    inner: Arc<UReqResourceProvider>,
    cache: Arc<Mutex<HashMap<String, Data>>>,
    search_dirs: Arc<Mutex<Vec<PathBuf>>>,
}

impl CachedResourceProvider {
    pub(crate) fn new(font_mgr: impl Into<FontMgr>) -> Self {
        Self {
            inner: Arc::new(UReqResourceProvider::new(font_mgr)),
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

    fn cache_key(resource_path: &str, resource_name: &str) -> String {
        if resource_path.is_empty() {
            resource_name.to_owned()
        } else {
            format!("{resource_path}\0{resource_name}")
        }
    }

    fn load_local(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        for path in self.local_candidates(resource_path, resource_name) {
            if let Ok(bytes) = fs::read(path) {
                return Some(Data::new_copy(&bytes));
            }
        }

        None
    }

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

fn is_url_like(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("file://")
}

impl ResourceProvider for CachedResourceProvider {
    fn load(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        let key = Self::cache_key(resource_path, resource_name);

        if let Some(data) = self.cache.lock().ok()?.get(&key).cloned() {
            return Some(data);
        }

        let data = self
            .load_local(resource_path, resource_name)
            .or_else(|| self.inner.load(resource_path, resource_name))?;
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
}
