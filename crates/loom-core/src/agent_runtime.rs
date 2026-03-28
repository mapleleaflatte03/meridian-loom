use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub type LoomResult<T> = Result<T, String>;

pub const DEFAULT_AGENT_RUNTIME_REGISTRY_PATH: &str = "agents/registry.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRuntimeProfile {
    pub agent_id: String,
    pub display_name: String,
    pub role: String,
    pub workspace_root: String,
    pub memory_root: String,
    pub session_root: String,
    pub provider_profile: String,
    pub tool_scope: String,
    pub heartbeat_policy: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRuntimeOverview {
    pub registry_path: PathBuf,
    pub profile_count: usize,
    pub agent_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRuntimeSummary {
    pub registry_path: PathBuf,
    pub profile_count: usize,
    pub profile: AgentRuntimeProfile,
    pub workspace_path: PathBuf,
    pub memory_path: PathBuf,
    pub session_path: PathBuf,
}

pub fn agent_runtime_registry_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_AGENT_RUNTIME_REGISTRY_PATH)
}

pub fn ensure_agent_runtime_scaffold(root: &Path) -> LoomResult<PathBuf> {
    let registry_path = agent_runtime_registry_path(root);
    if let Some(parent) = registry_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    if !registry_path.exists() {
        fs::write(&registry_path, render_default_agent_runtime_registry()).map_err(io_err)?;
    }
    let profiles = load_agent_runtime_registry(root)?;
    for profile in &profiles {
        fs::create_dir_all(root.join(&profile.workspace_root)).map_err(io_err)?;
        fs::create_dir_all(root.join(&profile.memory_root)).map_err(io_err)?;
        fs::create_dir_all(root.join(&profile.session_root)).map_err(io_err)?;
    }
    Ok(registry_path)
}

pub fn load_agent_runtime_registry(root: &Path) -> LoomResult<Vec<AgentRuntimeProfile>> {
    let registry_path = agent_runtime_registry_path(root);
    let raw = fs::read_to_string(&registry_path).map_err(io_err)?;
    parse_agent_runtime_registry(&raw)
}

pub fn agent_runtime_overview(root: &Path) -> LoomResult<AgentRuntimeOverview> {
    let profiles = load_agent_runtime_registry(root)?;
    Ok(AgentRuntimeOverview {
        registry_path: agent_runtime_registry_path(root),
        profile_count: profiles.len(),
        agent_ids: profiles.into_iter().map(|profile| profile.agent_id).collect(),
    })
}

pub fn agent_runtime_summary(root: &Path, agent_id: &str) -> LoomResult<AgentRuntimeSummary> {
    let profiles = load_agent_runtime_registry(root)?;
    let normalized_agent_id = agent_id.trim();
    if normalized_agent_id.is_empty() {
        return Err("agent_id is required".to_string());
    }
    let profile = profiles
        .iter()
        .find(|profile| profile.agent_id == normalized_agent_id)
        .cloned()
        .ok_or_else(|| {
            let available = profiles
                .iter()
                .map(|profile| profile.agent_id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "agent runtime profile '{}' was not found (available: {})",
                normalized_agent_id, available
            )
        })?;
    Ok(AgentRuntimeSummary {
        registry_path: agent_runtime_registry_path(root),
        profile_count: profiles.len(),
        workspace_path: root.join(&profile.workspace_root),
        memory_path: root.join(&profile.memory_root),
        session_path: root.join(&profile.session_root),
        profile,
    })
}

pub fn render_agent_runtime_human(summary: &AgentRuntimeSummary) -> String {
    format!(
        "registry_path:      {}
profile_count:      {}
agent_id:           {}
display_name:       {}
role:               {}
workspace_root:     {}
memory_root:        {}
session_root:       {}
provider_profile:   {}
tool_scope:         {}
heartbeat_policy:   {}
",
        summary.registry_path.display(),
        summary.profile_count,
        summary.profile.agent_id,
        summary.profile.display_name,
        summary.profile.role,
        summary.workspace_path.display(),
        summary.memory_path.display(),
        summary.session_path.display(),
        summary.profile.provider_profile,
        summary.profile.tool_scope,
        summary.profile.heartbeat_policy,
    )
}

pub fn render_agent_runtime_json(summary: &AgentRuntimeSummary) -> String {
    serde_json::to_string_pretty(&json!({
        "registry_path": summary.registry_path.display().to_string(),
        "profile_count": summary.profile_count,
        "agent_id": summary.profile.agent_id,
        "display_name": summary.profile.display_name,
        "role": summary.profile.role,
        "workspace_root": summary.workspace_path.display().to_string(),
        "memory_root": summary.memory_path.display().to_string(),
        "session_root": summary.session_path.display().to_string(),
        "provider_profile": summary.profile.provider_profile,
        "tool_scope": summary.profile.tool_scope,
        "heartbeat_policy": summary.profile.heartbeat_policy,
    }))
    .unwrap_or_else(|_| "{}".to_string()) + "\n"
}

fn render_default_agent_runtime_registry() -> String {
    serde_json::to_string_pretty(&json!({
        "agents": [
            default_profile("leviathann", "Leviathann", "manager", "manager_frontier", "manager_scope", "managed"),
            default_profile("atlas", "Atlas", "research", "research_frontier", "research_scope", "managed"),
            default_profile("quill", "Quill", "writer", "writer_general", "writer_scope", "managed"),
            default_profile("aegis", "Aegis", "qa_gate", "qa_frontier", "qa_scope", "managed"),
            default_profile("sentinel", "Sentinel", "verifier", "verifier_frontier", "verifier_scope", "managed"),
            default_profile("forge", "Forge", "executor", "executor_tooling", "executor_scope", "managed"),
            default_profile("pulse", "Pulse", "compressor", "local_ollama", "compression_scope", "cheap_background"),
        ]
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

fn default_profile(
    agent_id: &str,
    display_name: &str,
    role: &str,
    provider_profile: &str,
    tool_scope: &str,
    heartbeat_policy: &str,
) -> Value {
    json!({
        "agent_id": agent_id,
        "display_name": display_name,
        "role": role,
        "workspace_root": format!("agents/{}/workspace", agent_id),
        "memory_root": format!("agents/{}/memory", agent_id),
        "session_root": format!("agents/{}/sessions", agent_id),
        "provider_profile": provider_profile,
        "tool_scope": tool_scope,
        "heartbeat_policy": heartbeat_policy,
    })
}

fn parse_agent_runtime_registry(raw: &str) -> LoomResult<Vec<AgentRuntimeProfile>> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("invalid agent runtime registry json: {error}"))?;
    let agents = value
        .get("agents")
        .and_then(Value::as_array)
        .ok_or_else(|| "agent runtime registry must define an agents array".to_string())?;
    if agents.is_empty() {
        return Err("agent runtime registry must define at least one agent".to_string());
    }
    let mut profiles = Vec::with_capacity(agents.len());
    for agent in agents {
        profiles.push(parse_agent_runtime_profile(agent)?);
    }
    Ok(profiles)
}

fn parse_agent_runtime_profile(value: &Value) -> LoomResult<AgentRuntimeProfile> {
    Ok(AgentRuntimeProfile {
        agent_id: value_string(value.get("agent_id"), "agent_id")?,
        display_name: value_string(value.get("display_name"), "display_name")?,
        role: value_string(value.get("role"), "role")?,
        workspace_root: value_string(value.get("workspace_root"), "workspace_root")?,
        memory_root: value_string(value.get("memory_root"), "memory_root")?,
        session_root: value_string(value.get("session_root"), "session_root")?,
        provider_profile: value_string(value.get("provider_profile"), "provider_profile")?,
        tool_scope: value_string(value.get("tool_scope"), "tool_scope")?,
        heartbeat_policy: value_string(value.get("heartbeat_policy"), "heartbeat_policy")?,
    })
}

fn value_string(value: Option<&Value>, label: &str) -> LoomResult<String> {
    value
        .and_then(Value::as_str)
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| format!("{label} must not be empty"))
}

fn io_err(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{}-{}", label, unique));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp path");
        path
    }

    #[test]
    fn scaffold_writes_default_agent_registry_and_directories() {
        let root = temp_path("loom-agent-runtime-scaffold");
        let registry = ensure_agent_runtime_scaffold(&root).expect("scaffold agent runtime registry");
        assert!(registry.exists());
        let overview = agent_runtime_overview(&root).expect("agent runtime overview");
        assert_eq!(overview.profile_count, 7);
        assert!(root.join("agents/atlas/workspace").exists());
        assert!(root.join("agents/pulse/memory").exists());
    }

    #[test]
    fn summary_resolves_expected_agent_profile() {
        let root = temp_path("loom-agent-runtime-summary");
        ensure_agent_runtime_scaffold(&root).expect("scaffold agent runtime registry");
        let summary = agent_runtime_summary(&root, "pulse").expect("agent runtime summary");
        assert_eq!(summary.profile.display_name, "Pulse");
        assert_eq!(summary.profile.provider_profile, "local_ollama");
        assert!(summary.workspace_path.ends_with(Path::new("agents/pulse/workspace")));
    }
}
