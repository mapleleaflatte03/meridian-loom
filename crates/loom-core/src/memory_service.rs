//! Memory service and repository seam.
//!
//! This module formalizes the boundary between memory storage (the repo — a
//! per-agent directory of governed memory files) and memory operations (the
//! service — search, write, prune, and governance checks).
//!
//! The repo is the truth. The service is the access pattern. Context engine
//! layers can read from the repo, but all writes go through the service to
//! enforce governance (retention policy, size limits, agent isolation).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::LoomResult;

const MEMORY_ROOT_DIR: &str = "state/memory";
const MEMORY_INDEX_FILE: &str = "index.json";
const DEFAULT_MAX_ENTRIES_PER_AGENT: usize = 500;
const DEFAULT_MAX_ENTRY_BYTES: usize = 16_384;
const DEFAULT_RETENTION_DAYS: u64 = 365;

// ───── Repo types ─────

/// A single memory entry in the repo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryEntry {
    pub entry_id: String,
    pub agent_id: String,
    pub category: String,
    pub key: String,
    pub content: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub source: String,
    pub governed: bool,
}

impl MemoryEntry {
    pub fn to_json(&self) -> Value {
        json!({
            "entry_id": self.entry_id,
            "agent_id": self.agent_id,
            "category": self.category,
            "key": self.key,
            "content": self.content,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "source": self.source,
            "governed": self.governed,
        })
    }

    pub fn from_json(value: &Value) -> LoomResult<Self> {
        Ok(Self {
            entry_id: value["entry_id"].as_str().unwrap_or("").to_string(),
            agent_id: value["agent_id"].as_str().unwrap_or("").to_string(),
            category: value["category"].as_str().unwrap_or("general").to_string(),
            key: value["key"].as_str().unwrap_or("").to_string(),
            content: value["content"].as_str().unwrap_or("").to_string(),
            created_at: value["created_at"].as_u64().unwrap_or(0),
            updated_at: value["updated_at"].as_u64().unwrap_or(0),
            source: value["source"].as_str().unwrap_or("unknown").to_string(),
            governed: value["governed"].as_bool().unwrap_or(true),
        })
    }

    pub fn byte_size(&self) -> usize {
        self.content.len() + self.key.len() + self.category.len()
    }
}

/// Index of all memory entries for one agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryIndex {
    pub agent_id: String,
    pub entry_count: usize,
    pub categories: Vec<String>,
    pub total_bytes: usize,
    pub oldest_entry: u64,
    pub newest_entry: u64,
}

// ───── Repo operations ─────

/// Memory repo — per-agent directory of governed memory files.
pub struct MemoryRepo {
    root: PathBuf,
}

impl MemoryRepo {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.join(MEMORY_ROOT_DIR),
        }
    }

    pub fn agent_dir(&self, agent_id: &str) -> PathBuf {
        self.root.join(agent_id)
    }

    pub fn agent_index_path(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join(MEMORY_INDEX_FILE)
    }

    pub fn ensure_scaffold(&self, agent_id: &str) -> LoomResult<PathBuf> {
        let dir = self.agent_dir(agent_id);
        fs::create_dir_all(&dir).map_err(|e| format!("memory scaffold: {}", e))?;
        Ok(dir)
    }

    pub fn list_agents(&self) -> LoomResult<Vec<String>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut agents = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|e| format!("memory root: {}", e))? {
            let entry = entry.map_err(|e| format!("memory readdir: {}", e))?;
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    agents.push(name.to_string());
                }
            }
        }
        agents.sort();
        Ok(agents)
    }

    pub fn read_entries(&self, agent_id: &str) -> LoomResult<Vec<MemoryEntry>> {
        let index_path = self.agent_index_path(agent_id);
        if !index_path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&index_path)
            .map_err(|e| format!("memory read: {}", e))?;
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(Vec::new());
        }
        let value: Value = serde_json::from_str(raw)
            .map_err(|e| format!("memory parse: {}", e))?;
        let entries_val = value.get("entries").and_then(|v| v.as_array());
        match entries_val {
            Some(arr) => arr.iter().map(MemoryEntry::from_json).collect(),
            None => Ok(Vec::new()),
        }
    }

    pub fn write_entries(&self, agent_id: &str, entries: &[MemoryEntry]) -> LoomResult<PathBuf> {
        self.ensure_scaffold(agent_id)?;
        let index_path = self.agent_index_path(agent_id);
        let now = current_unix();
        let entries_json: Vec<Value> = entries.iter().map(|e| e.to_json()).collect();
        let doc = json!({
            "agent_id": agent_id,
            "updated_at": now,
            "entry_count": entries.len(),
            "entries": entries_json,
        });
        let text = serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("memory serialize: {}", e))?;
        fs::write(&index_path, text).map_err(|e| format!("memory write: {}", e))?;
        Ok(index_path)
    }

    pub fn build_index(&self, agent_id: &str) -> LoomResult<MemoryIndex> {
        let entries = self.read_entries(agent_id)?;
        let mut categories: Vec<String> = entries.iter().map(|e| e.category.clone()).collect();
        categories.sort();
        categories.dedup();
        let total_bytes: usize = entries.iter().map(|e| e.byte_size()).sum();
        let oldest = entries.iter().map(|e| e.created_at).min().unwrap_or(0);
        let newest = entries.iter().map(|e| e.updated_at).max().unwrap_or(0);
        Ok(MemoryIndex {
            agent_id: agent_id.to_string(),
            entry_count: entries.len(),
            categories,
            total_bytes,
            oldest_entry: oldest,
            newest_entry: newest,
        })
    }
}

// ───── Service operations ─────

/// Memory service policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryPolicy {
    pub max_entries_per_agent: usize,
    pub max_entry_bytes: usize,
    pub retention_days: u64,
    pub agent_isolation: bool,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            max_entries_per_agent: DEFAULT_MAX_ENTRIES_PER_AGENT,
            max_entry_bytes: DEFAULT_MAX_ENTRY_BYTES,
            retention_days: DEFAULT_RETENTION_DAYS,
            agent_isolation: true,
        }
    }
}

/// Memory service — governs all access to the memory repo.
pub struct MemoryService {
    repo: MemoryRepo,
    policy: MemoryPolicy,
}

impl MemoryService {
    pub fn new(root: &Path, policy: MemoryPolicy) -> Self {
        Self {
            repo: MemoryRepo::new(root),
            policy,
        }
    }

    pub fn with_defaults(root: &Path) -> Self {
        Self::new(root, MemoryPolicy::default())
    }

    /// Write a memory entry, enforcing policy.
    pub fn write(
        &self,
        agent_id: &str,
        category: &str,
        key: &str,
        content: &str,
        source: &str,
    ) -> LoomResult<MemoryEntry> {
        let now = current_unix();
        let entry = MemoryEntry {
            entry_id: format!("mem_{}_{}", agent_id, now),
            agent_id: agent_id.to_string(),
            category: category.to_string(),
            key: key.to_string(),
            content: content.to_string(),
            created_at: now,
            updated_at: now,
            source: source.to_string(),
            governed: true,
        };

        // Policy: entry size
        if entry.byte_size() > self.policy.max_entry_bytes {
            return Err(format!(
                "memory entry exceeds size limit: {} > {}",
                entry.byte_size(),
                self.policy.max_entry_bytes
            ));
        }

        let mut entries = self.repo.read_entries(agent_id)?;

        // Policy: max entries (evict oldest if at limit)
        if entries.len() >= self.policy.max_entries_per_agent {
            entries.sort_by_key(|e| e.updated_at);
            entries.remove(0);
        }

        // Upsert by key
        if let Some(existing) = entries.iter_mut().find(|e| e.key == key && e.category == category) {
            existing.content = entry.content.clone();
            existing.updated_at = now;
            existing.source = entry.source.clone();
        } else {
            entries.push(entry.clone());
        }

        self.repo.write_entries(agent_id, &entries)?;
        Ok(entry)
    }

    /// Search memory entries for an agent by category and/or key prefix.
    pub fn search(
        &self,
        agent_id: &str,
        category: Option<&str>,
        key_prefix: Option<&str>,
    ) -> LoomResult<Vec<MemoryEntry>> {
        let entries = self.repo.read_entries(agent_id)?;
        let filtered: Vec<MemoryEntry> = entries
            .into_iter()
            .filter(|e| {
                if let Some(cat) = category {
                    if e.category != cat {
                        return false;
                    }
                }
                if let Some(prefix) = key_prefix {
                    if !e.key.starts_with(prefix) {
                        return false;
                    }
                }
                true
            })
            .collect();
        Ok(filtered)
    }

    /// Remove a specific entry by key.
    pub fn remove(&self, agent_id: &str, category: &str, key: &str) -> LoomResult<bool> {
        let mut entries = self.repo.read_entries(agent_id)?;
        let before = entries.len();
        entries.retain(|e| !(e.key == key && e.category == category));
        if entries.len() < before {
            self.repo.write_entries(agent_id, &entries)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Prune expired entries across all agents.
    pub fn prune(&self) -> LoomResult<usize> {
        let now = current_unix();
        let cutoff = now.saturating_sub(self.policy.retention_days * 86400);
        let agents = self.repo.list_agents()?;
        let mut total_pruned = 0;
        for agent_id in &agents {
            let mut entries = self.repo.read_entries(agent_id)?;
            let before = entries.len();
            entries.retain(|e| e.updated_at >= cutoff);
            if entries.len() < before {
                total_pruned += before - entries.len();
                self.repo.write_entries(agent_id, &entries)?;
            }
        }
        Ok(total_pruned)
    }

    /// Build overview for diagnostics.
    pub fn overview(&self) -> LoomResult<MemoryServiceOverview> {
        let agents = self.repo.list_agents()?;
        let mut indices = Vec::new();
        let mut total_entries = 0;
        let mut total_bytes = 0;
        for agent_id in &agents {
            let index = self.repo.build_index(agent_id)?;
            total_entries += index.entry_count;
            total_bytes += index.total_bytes;
            indices.push(index);
        }
        Ok(MemoryServiceOverview {
            agent_count: agents.len(),
            total_entries,
            total_bytes,
            indices,
            policy: self.policy.clone(),
        })
    }
}

/// Overview of the memory service state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryServiceOverview {
    pub agent_count: usize,
    pub total_entries: usize,
    pub total_bytes: usize,
    pub indices: Vec<MemoryIndex>,
    pub policy: MemoryPolicy,
}

impl MemoryServiceOverview {
    pub fn to_json(&self) -> Value {
        json!({
            "agent_count": self.agent_count,
            "total_entries": self.total_entries,
            "total_bytes": self.total_bytes,
            "policy": {
                "max_entries_per_agent": self.policy.max_entries_per_agent,
                "max_entry_bytes": self.policy.max_entry_bytes,
                "retention_days": self.policy.retention_days,
                "agent_isolation": self.policy.agent_isolation,
            },
            "agents": self.indices.iter().map(|idx| json!({
                "agent_id": idx.agent_id,
                "entry_count": idx.entry_count,
                "categories": idx.categories,
                "total_bytes": idx.total_bytes,
            })).collect::<Vec<_>>(),
        })
    }
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use std::sync::atomic::{AtomicU64, Ordering};
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_root() -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("loom_memory_test_{}_{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_memory_entry_roundtrip() {
        let entry = MemoryEntry {
            entry_id: "mem_1".to_string(),
            agent_id: "atlas".to_string(),
            category: "knowledge".to_string(),
            key: "test_key".to_string(),
            content: "test content".to_string(),
            created_at: 1000,
            updated_at: 2000,
            source: "user".to_string(),
            governed: true,
        };
        let json = entry.to_json();
        let parsed = MemoryEntry::from_json(&json).unwrap();
        assert_eq!(parsed.agent_id, "atlas");
        assert_eq!(parsed.content, "test content");
    }

    #[test]
    fn test_repo_write_and_read() {
        let root = temp_root();
        let repo = MemoryRepo::new(&root);
        let entries = vec![MemoryEntry {
            entry_id: "mem_1".to_string(),
            agent_id: "atlas".to_string(),
            category: "facts".to_string(),
            key: "k1".to_string(),
            content: "v1".to_string(),
            created_at: 1000,
            updated_at: 1000,
            source: "test".to_string(),
            governed: true,
        }];
        repo.write_entries("atlas", &entries).unwrap();
        let read = repo.read_entries("atlas").unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].key, "k1");
        cleanup(&root);
    }

    #[test]
    fn test_service_write_and_search() {
        let root = temp_root();
        let svc = MemoryService::with_defaults(&root);
        svc.write("atlas", "knowledge", "weather", "sunny", "user").unwrap();
        svc.write("atlas", "knowledge", "location", "home", "user").unwrap();
        svc.write("atlas", "preferences", "lang", "en", "user").unwrap();

        let all = svc.search("atlas", None, None).unwrap();
        assert_eq!(all.len(), 3);

        let knowledge = svc.search("atlas", Some("knowledge"), None).unwrap();
        assert_eq!(knowledge.len(), 2);

        let weather = svc.search("atlas", Some("knowledge"), Some("weather")).unwrap();
        assert_eq!(weather.len(), 1);
        assert_eq!(weather[0].content, "sunny");
        cleanup(&root);
    }

    #[test]
    fn test_service_upsert() {
        let root = temp_root();
        let svc = MemoryService::with_defaults(&root);
        svc.write("atlas", "facts", "k1", "v1", "test").unwrap();
        svc.write("atlas", "facts", "k1", "v2", "test").unwrap();

        let entries = svc.search("atlas", None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "v2");
        cleanup(&root);
    }

    #[test]
    fn test_service_remove() {
        let root = temp_root();
        let svc = MemoryService::with_defaults(&root);
        svc.write("atlas", "facts", "k1", "v1", "test").unwrap();
        assert!(svc.remove("atlas", "facts", "k1").unwrap());
        assert!(!svc.remove("atlas", "facts", "k1").unwrap());
        let entries = svc.search("atlas", None, None).unwrap();
        assert!(entries.is_empty());
        cleanup(&root);
    }

    #[test]
    fn test_service_size_limit() {
        let root = temp_root();
        let policy = MemoryPolicy {
            max_entry_bytes: 10,
            ..Default::default()
        };
        let svc = MemoryService::new(&root, policy);
        let result = svc.write("atlas", "facts", "k1", "this is way too long content", "test");
        assert!(result.is_err());
        cleanup(&root);
    }

    #[test]
    fn test_service_overview() {
        let root = temp_root();
        let svc = MemoryService::with_defaults(&root);
        svc.write("atlas", "facts", "k1", "v1", "test").unwrap();
        svc.write("sentinel", "rules", "r1", "no", "test").unwrap();

        let overview = svc.overview().unwrap();
        assert_eq!(overview.agent_count, 2);
        assert_eq!(overview.total_entries, 2);
        cleanup(&root);
    }

    #[test]
    fn test_agent_isolation() {
        let root = temp_root();
        let svc = MemoryService::with_defaults(&root);
        svc.write("atlas", "facts", "shared_key", "atlas_data", "test").unwrap();
        svc.write("sentinel", "facts", "shared_key", "sentinel_data", "test").unwrap();

        let atlas_entries = svc.search("atlas", None, None).unwrap();
        assert_eq!(atlas_entries.len(), 1);
        assert_eq!(atlas_entries[0].content, "atlas_data");

        let sentinel_entries = svc.search("sentinel", None, None).unwrap();
        assert_eq!(sentinel_entries.len(), 1);
        assert_eq!(sentinel_entries[0].content, "sentinel_data");
        cleanup(&root);
    }
}
