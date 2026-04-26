use alloc::{string::String, sync::Arc};
use core::sync::atomic::{AtomicU64, Ordering};
use hashbrown::HashMap;
use spin::RwLock;

#[derive(Debug, Clone)]
pub struct ProcessData {
    // shared data
    pub(super) env: Arc<RwLock<HashMap<String, String, ahash::RandomState>>>,
    // bonus 1: track user ELF image pages
    pub(super) code_pages: Arc<AtomicU64>,
}

impl Default for ProcessData {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessData {
    pub fn new() -> Self {
        Self {
            env: Arc::new(RwLock::new(HashMap::default())),
            code_pages: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn env(&self, key: &str) -> Option<String> {
        self.env.read().get(key).cloned()
    }

    pub fn set_env(&mut self, key: &str, val: &str) {
        self.env.write().insert(key.into(), val.into());
    }

    pub fn code_pages(&self) -> u64 {
        self.code_pages.load(Ordering::Relaxed)
    }

    pub fn set_code_pages(&mut self, pages: u64) {
        self.code_pages.store(pages, Ordering::Relaxed);
    }
}
