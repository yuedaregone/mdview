//! Caching layer for parsed AST and rendered block heights
//!
//! Two-level cache:
//! 1. AST cache: parsed MarkdownDoc keyed by file path + content hash
//! 2. Block height cache: rendered heights keyed by content + render params hash

use std::collections::HashMap;
use std::path::PathBuf;

use crate::markdown::parser::MarkdownDoc;

/// Simple FNV-1a hash for content deduplication
fn content_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ─── AST Cache ──────────────────────────────────────────────────────────────

use std::sync::Arc;

struct AstCacheEntry {
    doc: Arc<MarkdownDoc>, // 使用 Arc 共享所有权
    hash: u64,
}

/// Cache for parsed Markdown documents
pub struct AstCache {
    entries: HashMap<PathBuf, AstCacheEntry>,
    max_entries: usize,
}

impl AstCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Get a cached AST, or parse and cache it
    pub fn get_or_parse(&mut self, path: &PathBuf, content: &str) -> Arc<MarkdownDoc> {
        let hash = content_hash(content);

        if let Some(entry) = self.entries.get(path) {
            if entry.hash == hash {
                return Arc::clone(&entry.doc); // 只增加引用计数，不拷贝数据
            }
        }

        let doc = Arc::new(crate::markdown::parser::parse_full(content));

        if self.entries.len() >= self.max_entries {
            if let Some(key) = self.entries.keys().next().cloned() {
                self.entries.remove(&key);
            }
        }

        self.entries.insert(
            path.clone(),
            AstCacheEntry {
                doc: Arc::clone(&doc),
                hash,
            },
        );

        doc
    }
}

impl Default for AstCache {
    fn default() -> Self {
        Self::new(16)
    }
}
