use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{io_err, unix_now, Config, LoomResult};

pub const DEFAULT_ONBOARD_MANIFEST_PATH: &str = "state/onboard.json";
const DEFAULT_GATEWAY_BIND: &str = "loopback";
const DEFAULT_GATEWAY_AUTH_MODE: &str = "token";
const DEFAULT_GATEWAY_TAILSCALE_MODE: &str = "off";
const DEFAULT_TELEGRAM_TOKEN_ENV: &str = "MERIDIAN_TELEGRAM_BOT_TOKEN";
const DEFAULT_TELEGRAM_DM_POLICY: &str = "open";
const DEFAULT_TELEGRAM_GROUP_POLICY: &str = "allowlist";
const DEFAULT_TELEGRAM_STREAMING: &str = "partial";
const DEFAULT_SESSION_DM_SCOPE: &str = "per-channel-peer";
const DEFAULT_DAEMON_MANAGER: &str = "supervisor";
const DEFAULT_DAEMON_STATE: &str = "configured";
const DEFAULT_SKILLS_NODE_MANAGER: &str = "npm";
const DEFAULT_REMOTE_MODE: &str = "local";
const DEFAULT_WIZARD_VERSION: &str = "meridian-onboard-v1";
const DEFAULT_SKILL_ENTRIES: [&str; 4] = [
    "browser",
    "telegram_bridge",
    "web_bridge",
    "governed_memory",
];
const DEFAULT_RECURRING_ENTRIES: [&str; 4] = [
    "night_shift_kickoff",
    "night_shift_research",
    "night_shift_write",
    "morning_brief",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnboardManifest {
    pub wizard_version: String,
    pub last_action: String,
    pub last_run_at: u64,
    pub last_run_mode: String,
    pub remote_mode: String,
    pub gateway_port: u16,
    pub gateway_bind: String,
    pub gateway_auth_mode: String,
    pub gateway_token_env: String,
    pub gateway_tailscale_mode: String,
    pub telegram_enabled: bool,
    pub telegram_token_env: String,
    pub telegram_dm_policy: String,
    pub telegram_group_policy: String,
    pub telegram_streaming: String,
    pub session_dm_scope: String,
    pub daemon_enabled: bool,
    pub daemon_manager: String,
    pub daemon_state: String,
    pub skills_node_manager: String,
    pub skills_install_defaults: bool,
    pub skills_entries: Vec<String>,
    pub recurring_install_defaults: bool,
    pub recurring_entries: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnboardOverview {
    pub manifest_path: PathBuf,
    pub wizard_version: String,
    pub last_action: String,
    pub gateway_summary: String,
    pub telegram_summary: String,
    pub daemon_summary: String,
    pub skills_summary: String,
    pub recurring_summary: String,
    pub remote_mode: String,
}

pub fn onboard_manifest_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_ONBOARD_MANIFEST_PATH)
}

pub fn ensure_onboard_manifest(root: &Path, config: &Config) -> LoomResult<PathBuf> {
    let manifest_path = onboard_manifest_path(root);
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    if !manifest_path.exists() {
        let manifest = OnboardManifest::from_config(config, "initialized");
        write_onboard_manifest(root, &manifest)?;
    }
    Ok(manifest_path)
}

pub fn load_onboard_manifest(root: &Path) -> LoomResult<OnboardManifest> {
    let raw = fs::read_to_string(onboard_manifest_path(root)).map_err(io_err)?;
    parse_onboard_manifest(&raw)
}

pub fn write_onboard_manifest(root: &Path, manifest: &OnboardManifest) -> LoomResult<PathBuf> {
    let path = onboard_manifest_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    fs::write(&path, render_onboard_manifest(manifest)).map_err(io_err)?;
    Ok(path)
}

pub fn onboard_overview(root: &Path) -> LoomResult<OnboardOverview> {
    let manifest = load_onboard_manifest(root)?;
    Ok(OnboardOverview {
        manifest_path: onboard_manifest_path(root),
        wizard_version: manifest.wizard_version.clone(),
        last_action: manifest.last_action.clone(),
        gateway_summary: format!(
            "{}:{} auth={} tailscale={}",
            bind_host_for(&manifest.gateway_bind),
            manifest.gateway_port,
            manifest.gateway_auth_mode,
            manifest.gateway_tailscale_mode
        ),
        telegram_summary: if manifest.telegram_enabled {
            format!(
                "enabled env={} dm={} group={} streaming={}",
                manifest.telegram_token_env,
                manifest.telegram_dm_policy,
                manifest.telegram_group_policy,
                manifest.telegram_streaming
            )
        } else {
            "disabled".to_string()
        },
        daemon_summary: format!(
            "{} {}",
            manifest.daemon_manager,
            if manifest.daemon_enabled {
                manifest.daemon_state.as_str()
            } else {
                "disabled"
            }
        ),
        skills_summary: format!(
            "node_manager={} defaults={} entries={}",
            manifest.skills_node_manager,
            if manifest.skills_install_defaults { "yes" } else { "no" },
            manifest.skills_entries.join(",")
        ),
        recurring_summary: format!(
            "defaults={} entries={}",
            if manifest.recurring_install_defaults { "yes" } else { "no" },
            manifest.recurring_entries.join(",")
        ),
        remote_mode: manifest.remote_mode,
    })
}

/// Describes the user's setup state for user-first onboarding paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupState {
    /// No loom.toml yet — completely fresh workspace.
    FreshWorkspace,
    /// Workspace initialized but no provider credentials detected.
    FreshNoAuth { provider_count: usize },
    /// Credentials are local-only (Ollama), no frontier auth.
    LocalOnly { ollama_available: bool, agent_count: usize },
    /// At least one frontier provider credential found.
    FrontierAvailable { profiles: Vec<String>, agent_count: usize },
    /// Workspace is fully configured with agents, providers, and credentials.
    FullyConfigured { agent_count: usize, provider_count: usize },
}

/// Detect the current setup state to guide the user-first onboarding path.
pub fn detect_setup_state(root: &Path) -> SetupState {
    if !root.join("loom.toml").exists() {
        return SetupState::FreshWorkspace;
    }
    let profiles = match crate::provider_router::load_provider_profiles(Some(root)) {
        Ok(ps) => ps,
        Err(_) => return SetupState::FreshNoAuth { provider_count: 0 },
    };
    let agent_count = crate::agent_runtime::agent_runtime_overview(root).map(|o| o.profile_count).unwrap_or(0);
    let provider_count = profiles.profiles.len();
    if provider_count == 0 {
        return SetupState::FreshNoAuth { provider_count: 0 };
    }
    let has_frontier = profiles.profiles.iter().any(|p| {
        let label = p.kind.label();
        label != "local_ollama" && label != "unknown"
    });
    let has_ollama = profiles.profiles.iter().any(|p| p.kind.label() == "local_ollama");
    if !has_frontier {
        let ollama_available = has_ollama && crate::provider_router::provider_auth_status(Some(root), Some("local_ollama"))
            .map(|s| s.ready)
            .unwrap_or(false);
        return SetupState::LocalOnly { ollama_available, agent_count };
    }
    let ready_profiles: Vec<String> = profiles.profiles.iter()
        .filter(|p| {
            let label = p.kind.label();
            label != "local_ollama" && label != "unknown"
        })
        .map(|p| p.name.clone())
        .collect();
    if ready_profiles.is_empty() || agent_count == 0 {
        return SetupState::FrontierAvailable { profiles: ready_profiles, agent_count };
    }
    SetupState::FullyConfigured { agent_count, provider_count }
}

/// Returns a human-readable action hint appropriate for the given setup state.
pub fn onboard_path_hint(state: &SetupState) -> String {
    match state {
        SetupState::FreshWorkspace => {
            "No workspace found. Run: loom init --root <path>".to_string()
        }
        SetupState::FreshNoAuth { .. } => {
            "No provider credentials configured.\n  Option 1 (local):    install Ollama and configure local_ollama profile\n  Option 2 (frontier): set your API key env and run loom onboard --manager-lane frontier".to_string()
        }
        SetupState::LocalOnly { ollama_available, .. } => {
            if *ollama_available {
                "Local Ollama provider is ready. To add a frontier provider run loom onboard --manager-lane frontier.".to_string()
            } else {
                "Local Ollama profile found but credentials not detected. Start Ollama and ensure the endpoint is reachable.".to_string()
            }
        }
        SetupState::FrontierAvailable { profiles, agent_count } => {
            if *agent_count == 0 {
                format!("Frontier provider ready (profiles: {}). No agents configured yet — run loom agent to add one.", profiles.join(", "))
            } else {
                format!("Frontier provider ready (profiles: {}). Run loom doctor to verify the full stack.", profiles.join(", "))
            }
        }
        SetupState::FullyConfigured { agent_count, provider_count } => {
            format!("Runtime fully configured: {} agent(s), {} provider(s). Run loom doctor to inspect health.", agent_count, provider_count)
        }
    }
}

pub fn bind_host_for(bind: &str) -> &'static str {
    match bind.trim().to_ascii_lowercase().as_str() {
        "all" | "any" | "public" => "0.0.0.0",
        _ => "127.0.0.1",
    }
}

pub fn derive_service_http_address(bind: &str, port: u16) -> String {
    format!("{}:{}", bind_host_for(bind), port)
}

impl OnboardManifest {
    pub fn from_config(config: &Config, last_action: &str) -> Self {
        let (gateway_bind, gateway_port) = parse_service_address(&config.service_http_address);
        Self {
            wizard_version: DEFAULT_WIZARD_VERSION.to_string(),
            last_action: normalized_or(last_action, "initialized"),
            last_run_at: unix_now(),
            last_run_mode: config.mode.clone(),
            remote_mode: DEFAULT_REMOTE_MODE.to_string(),
            gateway_port,
            gateway_bind,
            gateway_auth_mode: DEFAULT_GATEWAY_AUTH_MODE.to_string(),
            gateway_token_env: config.service_token_env.clone(),
            gateway_tailscale_mode: DEFAULT_GATEWAY_TAILSCALE_MODE.to_string(),
            telegram_enabled: false,
            telegram_token_env: DEFAULT_TELEGRAM_TOKEN_ENV.to_string(),
            telegram_dm_policy: DEFAULT_TELEGRAM_DM_POLICY.to_string(),
            telegram_group_policy: DEFAULT_TELEGRAM_GROUP_POLICY.to_string(),
            telegram_streaming: DEFAULT_TELEGRAM_STREAMING.to_string(),
            session_dm_scope: DEFAULT_SESSION_DM_SCOPE.to_string(),
            daemon_enabled: false,
            daemon_manager: DEFAULT_DAEMON_MANAGER.to_string(),
            daemon_state: DEFAULT_DAEMON_STATE.to_string(),
            skills_node_manager: DEFAULT_SKILLS_NODE_MANAGER.to_string(),
            skills_install_defaults: true,
            skills_entries: DEFAULT_SKILL_ENTRIES.iter().map(|entry| entry.to_string()).collect(),
            recurring_install_defaults: true,
            recurring_entries: DEFAULT_RECURRING_ENTRIES.iter().map(|entry| entry.to_string()).collect(),
        }
    }

    pub fn as_json(&self) -> Value {
        json!({
            "wizard": {
                "version": self.wizard_version,
                "lastAction": self.last_action,
                "lastRunAt": self.last_run_at,
                "lastRunMode": self.last_run_mode,
                "remoteMode": self.remote_mode
            },
            "gateway": {
                "port": self.gateway_port,
                "bind": self.gateway_bind,
                "auth": {
                    "mode": self.gateway_auth_mode,
                    "tokenEnv": self.gateway_token_env
                },
                "tailscale": {
                    "mode": self.gateway_tailscale_mode
                }
            },
            "channels": {
                "telegram": {
                    "enabled": self.telegram_enabled,
                    "tokenEnv": self.telegram_token_env,
                    "dmPolicy": self.telegram_dm_policy,
                    "groupPolicy": self.telegram_group_policy,
                    "streaming": self.telegram_streaming
                }
            },
            "session": {
                "dmScope": self.session_dm_scope
            },
            "daemon": {
                "enabled": self.daemon_enabled,
                "manager": self.daemon_manager,
                "state": self.daemon_state
            },
            "skills": {
                "nodeManager": self.skills_node_manager,
                "installDefaults": self.skills_install_defaults,
                "entries": self.skills_entries
            },
            "recurring": {
                "installDefaults": self.recurring_install_defaults,
                "entries": self.recurring_entries
            }
        })
    }
}

fn render_onboard_manifest(manifest: &OnboardManifest) -> String {
    let mut rendered = serde_json::to_string_pretty(&manifest.as_json())
        .unwrap_or_else(|_| "{}".to_string());
    rendered.push('\n');
    rendered
}

fn parse_onboard_manifest(raw: &str) -> LoomResult<OnboardManifest> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("invalid onboard manifest json: {error}"))?;
    let wizard = value
        .get("wizard")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing wizard object".to_string())?;
    let gateway = value
        .get("gateway")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing gateway object".to_string())?;
    let gateway_auth = gateway
        .get("auth")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing gateway.auth object".to_string())?;
    let gateway_tailscale = gateway
        .get("tailscale")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing gateway.tailscale object".to_string())?;
    let channels = value
        .get("channels")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing channels object".to_string())?;
    let telegram = channels
        .get("telegram")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing channels.telegram object".to_string())?;
    let session = value
        .get("session")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing session object".to_string())?;
    let daemon = value
        .get("daemon")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing daemon object".to_string())?;
    let skills = value
        .get("skills")
        .and_then(Value::as_object)
        .ok_or_else(|| "onboard manifest missing skills object".to_string())?;
    let recurring = value
        .get("recurring")
        .and_then(Value::as_object);
    let skill_entries = skills
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let recurring_entries = recurring
        .and_then(|section| section.get("entries"))
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(OnboardManifest {
        wizard_version: value_string(wizard.get("version"), DEFAULT_WIZARD_VERSION),
        last_action: value_string(wizard.get("lastAction"), "initialized"),
        last_run_at: wizard.get("lastRunAt").and_then(Value::as_u64).unwrap_or_else(unix_now),
        last_run_mode: value_string(wizard.get("lastRunMode"), "embedded"),
        remote_mode: value_string(wizard.get("remoteMode"), DEFAULT_REMOTE_MODE),
        gateway_port: gateway.get("port").and_then(Value::as_u64).unwrap_or(18910) as u16,
        gateway_bind: value_string(gateway.get("bind"), DEFAULT_GATEWAY_BIND),
        gateway_auth_mode: value_string(gateway_auth.get("mode"), DEFAULT_GATEWAY_AUTH_MODE),
        gateway_token_env: value_string(gateway_auth.get("tokenEnv"), "LOOM_SERVICE_TOKEN"),
        gateway_tailscale_mode: value_string(gateway_tailscale.get("mode"), DEFAULT_GATEWAY_TAILSCALE_MODE),
        telegram_enabled: telegram.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        telegram_token_env: value_string(telegram.get("tokenEnv"), DEFAULT_TELEGRAM_TOKEN_ENV),
        telegram_dm_policy: value_string(telegram.get("dmPolicy"), DEFAULT_TELEGRAM_DM_POLICY),
        telegram_group_policy: value_string(telegram.get("groupPolicy"), DEFAULT_TELEGRAM_GROUP_POLICY),
        telegram_streaming: value_string(telegram.get("streaming"), DEFAULT_TELEGRAM_STREAMING),
        session_dm_scope: value_string(session.get("dmScope"), DEFAULT_SESSION_DM_SCOPE),
        daemon_enabled: daemon.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        daemon_manager: value_string(daemon.get("manager"), DEFAULT_DAEMON_MANAGER),
        daemon_state: value_string(daemon.get("state"), DEFAULT_DAEMON_STATE),
        skills_node_manager: value_string(skills.get("nodeManager"), DEFAULT_SKILLS_NODE_MANAGER),
        skills_install_defaults: skills.get("installDefaults").and_then(Value::as_bool).unwrap_or(true),
        skills_entries: if skill_entries.is_empty() {
            DEFAULT_SKILL_ENTRIES.iter().map(|entry| entry.to_string()).collect()
        } else {
            skill_entries
        },
        recurring_install_defaults: recurring
            .and_then(|section| section.get("installDefaults"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        recurring_entries: if recurring_entries.is_empty() {
            DEFAULT_RECURRING_ENTRIES.iter().map(|entry| entry.to_string()).collect()
        } else {
            recurring_entries
        },
    })
}

fn parse_service_address(raw: &str) -> (String, u16) {
    let trimmed = raw.trim();
    let Some((host, port)) = trimmed.rsplit_once(':') else {
        return (DEFAULT_GATEWAY_BIND.to_string(), 18910);
    };
    let bind = match host.trim() {
        "0.0.0.0" => "all",
        _ => DEFAULT_GATEWAY_BIND,
    };
    let port = port.trim().parse::<u16>().unwrap_or(18910);
    (bind.to_string(), port)
}

fn normalized_or(raw: &str, default: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn value_string(value: Option<&Value>, default: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init_workspace, read_config};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ensure_manifest_scaffold_uses_runtime_defaults() {
        let root = temp_path("loom-onboard-defaults");
        let config = init_workspace(&root, "embedded", None, "org_demo").expect("init");
        let path = ensure_onboard_manifest(&root, &config).expect("manifest");
        assert!(path.exists());
        let manifest = load_onboard_manifest(&root).expect("load");
        assert_eq!(manifest.gateway_auth_mode, "token");
        assert_eq!(manifest.gateway_bind, "loopback");
        assert_eq!(manifest.gateway_port, 18910);
        assert!(!manifest.telegram_enabled);
        assert_eq!(manifest.gateway_token_env, read_config(&root).expect("config").service_token_env);
    }

    #[test]
    fn derive_service_http_address_tracks_bind_and_port() {
        assert_eq!(derive_service_http_address("loopback", 18789), "127.0.0.1:18789");
        assert_eq!(derive_service_http_address("all", 18789), "0.0.0.0:18789");
    }

    fn temp_path(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{}-{}", label, timestamp))
    }
}
