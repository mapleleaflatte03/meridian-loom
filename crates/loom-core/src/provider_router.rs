use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use url::Url;

type LoomResult<T> = Result<T, String>;

pub const DEFAULT_PROVIDER_PROFILES_PATH: &str = "providers/profiles.json";
const DEFAULT_LOCAL_PROFILE_NAME: &str = "local_ollama";
const DEFAULT_OPENAI_PROFILE_NAME: &str = "openai_default";
const DEFAULT_CUSTOM_PROFILE_NAME: &str = "custom_endpoint";
const DEFAULT_LOCAL_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434/v1/chat/completions";
const DEFAULT_OPENAI_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MODEL_ALIAS: &str = "gpt-3.5-turbo";
const DEFAULT_BEARER_ENV: &str = "OPENAI_API_KEY";
const ENV_PROVIDER_PROFILE: &str = "LLM_PROVIDER_PROFILE";
const ENV_PROVIDER_PROFILES_PATH: &str = "LLM_PROVIDER_PROFILES_PATH";
const ENV_PROVIDER_KIND: &str = "LLM_PROVIDER_KIND";
const ENV_AUTH_MODE: &str = "LLM_AUTH_MODE";
const ENV_AUTH_ENV_VAR: &str = "LLM_AUTH_ENV_VAR";
const ENV_AUTH_HEADER_NAME: &str = "LLM_AUTH_HEADER_NAME";
const ENV_LLM_BASE_URL: &str = "LLM_BASE_URL";
const ENV_LLM_MODEL: &str = "LLM_MODEL";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    LocalOllama,
    OpenAiCompatible,
    CustomEndpoint,
}

impl ProviderKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LocalOllama => "local_ollama",
            Self::OpenAiCompatible => "openai_compatible",
            Self::CustomEndpoint => "custom_endpoint",
        }
    }

    fn from_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "local_ollama" | "ollama" => Some(Self::LocalOllama),
            "openai_compatible" | "openai-compatible" | "openai" => Some(Self::OpenAiCompatible),
            "custom_endpoint" | "custom-endpoint" | "custom" => Some(Self::CustomEndpoint),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderAuthMode {
    None,
    BearerEnv { env_var: String },
    StaticHeaderEnv { header_name: String, env_var: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderProfile {
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub default_model: String,
    pub auth: ProviderAuthMode,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProviderRoutePolicy {
    pub profile_name: Option<String>,
    pub default_model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProviderRoutingTable {
    pub capabilities: BTreeMap<String, ProviderRoutePolicy>,
    pub agents: BTreeMap<String, ProviderRoutePolicy>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderProfileSet {
    pub default_profile_name: String,
    pub profiles: Vec<ProviderProfile>,
    pub routing: ProviderRoutingTable,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRouteIntent {
    pub capability_name: String,
    pub preferred_profile_name: Option<String>,
    pub requested_model: String,
    pub agent_id: Option<String>,
    pub org_id: Option<String>,
}

impl ProviderRouteIntent {
    pub fn for_capability(capability_name: &str, requested_model: &str) -> Self {
        Self {
            capability_name: if capability_name.trim().is_empty() {
                "loom.llm.inference.v1".to_string()
            } else {
                capability_name.trim().to_string()
            },
            preferred_profile_name: env_trimmed(ENV_PROVIDER_PROFILE),
            requested_model: requested_model.trim().to_string(),
            agent_id: None,
            org_id: None,
        }
    }

    pub fn llm_inference(requested_model: &str) -> Self {
        Self::for_capability("loom.llm.inference.v1", requested_model)
    }

    pub fn with_agent_id(mut self, agent_id: &str) -> Self {
        self.agent_id = trim_to_option(agent_id);
        self
    }

    pub fn with_org_id(mut self, org_id: &str) -> Self {
        self.org_id = trim_to_option(org_id);
        self
    }

    pub fn with_preferred_profile_name(mut self, profile_name: &str) -> Self {
        self.preferred_profile_name = trim_to_option(profile_name);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProviderRoute {
    pub capability_name: String,
    pub profile_name: String,
    pub profile_kind: ProviderKind,
    pub endpoint_url: Url,
    pub model: String,
    pub auth: ProviderAuthMode,
    pub source: String,
    pub note: String,
    pub matched_rule: String,
    pub agent_id: Option<String>,
    pub org_id: Option<String>,
}

impl ResolvedProviderRoute {
    pub fn resolve_auth_header(&self) -> LoomResult<Option<(String, String)>> {
        match &self.auth {
            ProviderAuthMode::None => Ok(None),
            ProviderAuthMode::BearerEnv { env_var } => {
                let value = std::env::var(env_var)
                    .ok()
                    .map(|raw| raw.trim().to_string())
                    .filter(|raw| !raw.is_empty())
                    .ok_or_else(|| {
                        format!(
                            "provider profile '{}' requires bearer token env {}",
                            self.profile_name, env_var
                        )
                    })?;
                Ok(Some(("authorization".to_string(), format!("Bearer {value}"))))
            }
            ProviderAuthMode::StaticHeaderEnv { header_name, env_var } => {
                let value = std::env::var(env_var)
                    .ok()
                    .map(|raw| raw.trim().to_string())
                    .filter(|raw| !raw.is_empty())
                    .ok_or_else(|| {
                        format!(
                            "provider profile '{}' requires header env {}",
                            self.profile_name, env_var
                        )
                    })?;
                Ok(Some((header_name.clone(), value)))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderPlaneSummary {
    pub profiles_path: PathBuf,
    pub source: String,
    pub default_profile_name: String,
    pub profile_count: usize,
    pub capability_route_count: usize,
    pub agent_route_count: usize,
}

pub fn ensure_provider_profiles_scaffold(root: &Path) -> LoomResult<PathBuf> {
    let path = provider_profiles_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    if !path.exists() {
        fs::write(&path, render_default_provider_profiles()).map_err(io_err)?;
    }
    Ok(path)
}

pub fn provider_profiles_runtime_path(root: Option<&Path>) -> LoomResult<PathBuf> {
    let root = resolve_runtime_root(root)?;
    Ok(provider_profiles_path(&root))
}

pub fn provider_plane_summary(root: Option<&Path>) -> LoomResult<ProviderPlaneSummary> {
    let root = resolve_runtime_root(root)?;
    let profiles = load_provider_profiles(Some(&root))?;
    Ok(ProviderPlaneSummary {
        profiles_path: provider_profiles_path(&root),
        source: profiles.source,
        default_profile_name: profiles.default_profile_name,
        profile_count: profiles.profiles.len(),
        capability_route_count: profiles.routing.capabilities.len(),
        agent_route_count: profiles.routing.agents.len(),
    })
}

pub fn load_provider_profiles(root: Option<&Path>) -> LoomResult<ProviderProfileSet> {
    let root = resolve_runtime_root(root)?;
    let path = provider_profiles_path(&root);
    if path.exists() {
        let raw = fs::read_to_string(&path).map_err(io_err)?;
        let mut profiles = parse_provider_profiles_json(&raw)?;
        profiles.source = path.display().to_string();
        return Ok(profiles);
    }
    Ok(env_default_provider_profiles())
}

pub fn resolve_provider_route(root: Option<&Path>, intent: &ProviderRouteIntent) -> LoomResult<ResolvedProviderRoute> {
    let profiles = load_provider_profiles(root)?;
    let agent_policy = intent
        .agent_id
        .as_ref()
        .and_then(|agent_id| profiles.routing.agents.get(agent_id));
    let capability_policy = profiles.routing.capabilities.get(&intent.capability_name);
    let explicit_profile = intent.preferred_profile_name.clone();
    let profile_name = explicit_profile
        .clone()
        .or_else(|| agent_policy.and_then(|policy| policy.profile_name.clone()))
        .or_else(|| capability_policy.and_then(|policy| policy.profile_name.clone()))
        .unwrap_or_else(|| profiles.default_profile_name.clone());
    let matched_rule = if explicit_profile.is_some() {
        "explicit_profile".to_string()
    } else if let Some(agent_id) = &intent.agent_id {
        if agent_policy.and_then(|policy| policy.profile_name.clone()).is_some() {
            format!("agent:{}", agent_id)
        } else if capability_policy.and_then(|policy| policy.profile_name.clone()).is_some() {
            format!("capability:{}", intent.capability_name)
        } else {
            "default_profile".to_string()
        }
    } else if capability_policy.and_then(|policy| policy.profile_name.clone()).is_some() {
        format!("capability:{}", intent.capability_name)
    } else {
        "default_profile".to_string()
    };
    let profile = profiles
        .profiles
        .iter()
        .find(|candidate| candidate.name == profile_name)
        .ok_or_else(|| {
            let available = profiles
                .profiles
                .iter()
                .map(|profile| profile.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "provider profile '{}' was not found (available: {})",
                profile_name, available
            )
        })?;
    let endpoint_url = normalize_endpoint_url(&profile.base_url)?;
    let model = if !intent.requested_model.trim().is_empty() {
        intent.requested_model.trim().to_string()
    } else if let Some(policy) = agent_policy {
        policy
            .default_model
            .clone()
            .or_else(|| capability_policy.and_then(|item| item.default_model.clone()))
            .unwrap_or_else(|| profile.default_model.clone())
    } else if let Some(policy) = capability_policy {
        policy.default_model.clone().unwrap_or_else(|| profile.default_model.clone())
    } else {
        profile.default_model.clone()
    };
    let note = match matched_rule.as_str() {
        "explicit_profile" => "resolved from explicit provider profile override".to_string(),
        "default_profile" => "resolved from runtime default provider profile".to_string(),
        _ if matched_rule.starts_with("agent:") => {
            format!("resolved from agent-scoped provider routing rule ({})", matched_rule)
        }
        _ if matched_rule.starts_with("capability:") => {
            format!("resolved from capability-scoped provider routing rule ({})", matched_rule)
        }
        _ => profile.note.clone(),
    };
    Ok(ResolvedProviderRoute {
        capability_name: intent.capability_name.clone(),
        profile_name: profile.name.clone(),
        profile_kind: profile.kind.clone(),
        endpoint_url,
        model,
        auth: profile.auth.clone(),
        source: profiles.source,
        note,
        matched_rule,
        agent_id: intent.agent_id.clone(),
        org_id: intent.org_id.clone(),
    })
}

pub fn resolve_llm_route(root: Option<&Path>, intent: &ProviderRouteIntent) -> LoomResult<ResolvedProviderRoute> {
    resolve_provider_route(root, intent)
}

pub fn render_provider_plane_human(summary: &ProviderPlaneSummary) -> String {
    format!(
        "profiles_path:      {}
provider_source:    {}
default_profile:    {}
profile_count:      {}
capability_routes:  {}
agent_routes:       {}
",
        summary.profiles_path.display(),
        summary.source,
        summary.default_profile_name,
        summary.profile_count,
        summary.capability_route_count,
        summary.agent_route_count,
    )
}

pub fn render_provider_plane_json(summary: &ProviderPlaneSummary) -> String {
    format!(
        "{{\n  \"profiles_path\": {},\n  \"provider_source\": {},\n  \"default_profile\": {},\n  \"profile_count\": {},\n  \"capability_routes\": {},\n  \"agent_routes\": {}\n}}\n",
        json_string(&summary.profiles_path.display().to_string()),
        json_string(&summary.source),
        json_string(&summary.default_profile_name),
        summary.profile_count,
        summary.capability_route_count,
        summary.agent_route_count,
    )
}

pub fn render_provider_route_human(route: &ResolvedProviderRoute) -> String {
    format!(
        "capability:         {}
profile:            {} ({})
endpoint:           {}
model:              {}
matched_rule:       {}
source:             {}
note:               {}
",
        route.capability_name,
        route.profile_name,
        route.profile_kind.label(),
        route.endpoint_url,
        route.model,
        route.matched_rule,
        route.source,
        route.note,
    )
}

pub fn render_provider_route_json(route: &ResolvedProviderRoute) -> String {
    format!(
        "{{\n  \"capability\": {},\n  \"profile\": {},\n  \"profile_kind\": {},\n  \"endpoint\": {},\n  \"model\": {},\n  \"matched_rule\": {},\n  \"source\": {},\n  \"note\": {},\n  \"agent_id\": {},\n  \"org_id\": {}\n}}\n",
        json_string(&route.capability_name),
        json_string(&route.profile_name),
        json_string(route.profile_kind.label()),
        json_string(route.endpoint_url.as_str()),
        json_string(&route.model),
        json_string(&route.matched_rule),
        json_string(&route.source),
        json_string(&route.note),
        json_option(route.agent_id.as_deref()),
        json_option(route.org_id.as_deref()),
    )
}

fn resolve_runtime_root(root: Option<&Path>) -> LoomResult<PathBuf> {
    match root {
        Some(path) => Ok(path.to_path_buf()),
        None => {
            let current_dir = std::env::current_dir().map_err(io_err)?;
            if current_dir.join("loom.toml").exists() {
                Ok(current_dir)
            } else {
                default_app_home()
            }
        }
    }
}

fn default_app_home() -> LoomResult<PathBuf> {
    if let Ok(value) = std::env::var("LOOM_ROOT") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(value) = std::env::var("XDG_DATA_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(
                PathBuf::from(trimmed)
                    .join("meridian-loom")
                    .join("runtime")
                    .join("default"),
            );
        }
    }
    let home = std::env::var("HOME")
        .map_err(|_| "HOME is not set and no Loom runtime root was provided".to_string())?;
    Ok(PathBuf::from(home)
        .join(".local/share/meridian-loom")
        .join("runtime")
        .join("default"))
}

fn provider_profiles_path(root: &Path) -> PathBuf {
    match env_trimmed(ENV_PROVIDER_PROFILES_PATH) {
        Some(raw) => PathBuf::from(raw),
        None => root.join(DEFAULT_PROVIDER_PROFILES_PATH),
    }
}

fn render_default_provider_profiles() -> String {
    serde_json::to_string_pretty(&json!({
        "default_profile": DEFAULT_LOCAL_PROFILE_NAME,
        "profiles": [
            {
                "name": DEFAULT_LOCAL_PROFILE_NAME,
                "kind": ProviderKind::LocalOllama.label(),
                "base_url": DEFAULT_LOCAL_OLLAMA_ENDPOINT,
                "model": DEFAULT_MODEL_ALIAS,
                "auth": {
                    "mode": "none"
                },
                "note": "seeded Meridian local inference route for the bounded LLM host call"
            }
        ],
        "routing": {
            "capabilities": {
                "loom.llm.inference.v1": {
                    "profile": DEFAULT_LOCAL_PROFILE_NAME,
                    "default_model": DEFAULT_MODEL_ALIAS
                }
            },
            "agents": {}
        }
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

fn parse_provider_profiles_json(raw: &str) -> LoomResult<ProviderProfileSet> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("invalid provider profiles json: {error}"))?;
    let default_profile_name = value_string_or(value.get("default_profile"), DEFAULT_LOCAL_PROFILE_NAME);
    let profiles = value
        .get("profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| "provider profiles file must define a profiles array".to_string())?;
    if profiles.is_empty() {
        return Err("provider profiles file must define at least one profile".to_string());
    }
    let mut parsed_profiles = Vec::with_capacity(profiles.len());
    for raw_profile in profiles {
        parsed_profiles.push(parse_provider_profile(raw_profile)?);
    }
    Ok(ProviderProfileSet {
        default_profile_name,
        profiles: parsed_profiles,
        routing: parse_provider_routing(value.get("routing"))?,
        source: "runtime_provider_file".to_string(),
    })
}

fn parse_provider_profile(value: &Value) -> LoomResult<ProviderProfile> {
    let name = value_string(value.get("name"), "provider profile name")?;
    let kind = ProviderKind::from_str(&value_string_or(value.get("kind"), "openai_compatible"))
        .ok_or_else(|| format!("provider profile '{}' has unsupported kind", name))?;
    let base_url = value_string(value.get("base_url"), "provider profile base_url")?;
    let default_model = value_string_or(value.get("model"), DEFAULT_MODEL_ALIAS);
    let note = value_string_or(value.get("note"), "");
    let auth = parse_provider_auth(value.get("auth"))?;
    Ok(ProviderProfile {
        name,
        kind,
        base_url,
        default_model,
        auth,
        note,
    })
}

fn parse_provider_routing(value: Option<&Value>) -> LoomResult<ProviderRoutingTable> {
    let Some(value) = value else {
        return Ok(ProviderRoutingTable::default());
    };
    Ok(ProviderRoutingTable {
        capabilities: parse_policy_map(value.get("capabilities"))?,
        agents: parse_policy_map(value.get("agents"))?,
    })
}

fn parse_policy_map(value: Option<&Value>) -> LoomResult<BTreeMap<String, ProviderRoutePolicy>> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| "provider routing entries must be objects".to_string())?;
    let mut policies = BTreeMap::new();
    for (key, raw_policy) in object {
        policies.insert(key.to_string(), parse_route_policy(raw_policy)?);
    }
    Ok(policies)
}

fn parse_route_policy(value: &Value) -> LoomResult<ProviderRoutePolicy> {
    if let Some(profile_name) = value.as_str() {
        return Ok(ProviderRoutePolicy {
            profile_name: trim_to_option(profile_name),
            default_model: None,
        });
    }
    let Some(object) = value.as_object() else {
        return Err("provider route policy must be a string or object".to_string());
    };
    let profile_name = object
        .get("profile")
        .and_then(Value::as_str)
        .or_else(|| object.get("default_profile").and_then(Value::as_str))
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty());
    let default_model = object
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| object.get("default_model").and_then(Value::as_str))
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty());
    if profile_name.is_none() && default_model.is_none() {
        return Err("provider route policy object must define profile/default_profile or model/default_model".to_string());
    }
    Ok(ProviderRoutePolicy {
        profile_name,
        default_model,
    })
}

fn parse_provider_auth(value: Option<&Value>) -> LoomResult<ProviderAuthMode> {
    let Some(value) = value else {
        return Ok(ProviderAuthMode::None);
    };
    let mode = value_string_or(value.get("mode"), "none");
    match mode.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(ProviderAuthMode::None),
        "bearer_env" | "bearer-env" => {
            let env_var = value_string_or(value.get("env_var"), DEFAULT_BEARER_ENV);
            Ok(ProviderAuthMode::BearerEnv { env_var })
        }
        "static_header_env" | "static-header-env" => {
            let header_name = value_string(value.get("header_name"), "static_header_env.header_name")?;
            let env_var = value_string(value.get("env_var"), "static_header_env.env_var")?;
            Ok(ProviderAuthMode::StaticHeaderEnv { header_name, env_var })
        }
        _ => Err(format!("unsupported provider auth mode '{}'", mode)),
    }
}

fn env_default_provider_profiles() -> ProviderProfileSet {
    let explicit_base_url = env_trimmed(ENV_LLM_BASE_URL);
    let explicit_model = env_trimmed(ENV_LLM_MODEL).unwrap_or_else(|| DEFAULT_MODEL_ALIAS.to_string());
    if let Some(base_url) = explicit_base_url {
        let inferred_kind = ProviderKind::from_str(
            &env_trimmed(ENV_PROVIDER_KIND)
                .unwrap_or_else(|| infer_provider_kind_from_base_url(&base_url).label().to_string()),
        )
        .unwrap_or_else(|| infer_provider_kind_from_base_url(&base_url));
        let profile_name = env_trimmed(ENV_PROVIDER_PROFILE).unwrap_or_else(|| match inferred_kind {
            ProviderKind::LocalOllama => DEFAULT_LOCAL_PROFILE_NAME.to_string(),
            ProviderKind::OpenAiCompatible => DEFAULT_OPENAI_PROFILE_NAME.to_string(),
            ProviderKind::CustomEndpoint => DEFAULT_CUSTOM_PROFILE_NAME.to_string(),
        });
        let auth = env_default_auth_mode(&base_url);
        return ProviderProfileSet {
            default_profile_name: profile_name.clone(),
            profiles: vec![ProviderProfile {
                name: profile_name.clone(),
                kind: inferred_kind,
                base_url,
                default_model: explicit_model.clone(),
                auth,
                note: "resolved from environment-backed provider defaults".to_string(),
            }],
            routing: ProviderRoutingTable {
                capabilities: BTreeMap::from([(
                    "loom.llm.inference.v1".to_string(),
                    ProviderRoutePolicy {
                        profile_name: Some(profile_name),
                        default_model: Some(explicit_model),
                    },
                )]),
                agents: BTreeMap::new(),
            },
            source: "env_defaults".to_string(),
        };
    }
    if env_trimmed(DEFAULT_BEARER_ENV).is_some() {
        return ProviderProfileSet {
            default_profile_name: DEFAULT_OPENAI_PROFILE_NAME.to_string(),
            profiles: vec![ProviderProfile {
                name: DEFAULT_OPENAI_PROFILE_NAME.to_string(),
                kind: ProviderKind::OpenAiCompatible,
                base_url: DEFAULT_OPENAI_ENDPOINT.to_string(),
                default_model: explicit_model.clone(),
                auth: ProviderAuthMode::BearerEnv {
                    env_var: DEFAULT_BEARER_ENV.to_string(),
                },
                note: "resolved from OPENAI_API_KEY-backed defaults".to_string(),
            }],
            routing: ProviderRoutingTable {
                capabilities: BTreeMap::from([(
                    "loom.llm.inference.v1".to_string(),
                    ProviderRoutePolicy {
                        profile_name: Some(DEFAULT_OPENAI_PROFILE_NAME.to_string()),
                        default_model: Some(explicit_model),
                    },
                )]),
                agents: BTreeMap::new(),
            },
            source: "env_defaults".to_string(),
        };
    }
    ProviderProfileSet {
        default_profile_name: DEFAULT_LOCAL_PROFILE_NAME.to_string(),
        profiles: vec![ProviderProfile {
            name: DEFAULT_LOCAL_PROFILE_NAME.to_string(),
            kind: ProviderKind::LocalOllama,
            base_url: DEFAULT_LOCAL_OLLAMA_ENDPOINT.to_string(),
            default_model: explicit_model.clone(),
            auth: ProviderAuthMode::None,
            note: "resolved from Meridian local Ollama defaults".to_string(),
        }],
        routing: ProviderRoutingTable {
            capabilities: BTreeMap::from([(
                "loom.llm.inference.v1".to_string(),
                ProviderRoutePolicy {
                    profile_name: Some(DEFAULT_LOCAL_PROFILE_NAME.to_string()),
                    default_model: Some(explicit_model),
                },
            )]),
            agents: BTreeMap::new(),
        },
        source: "env_defaults".to_string(),
    }
}

fn env_default_auth_mode(base_url: &str) -> ProviderAuthMode {
    match env_trimmed(ENV_AUTH_MODE)
        .unwrap_or_else(|| {
            if endpoint_is_local(base_url) {
                "none".to_string()
            } else {
                "bearer_env".to_string()
            }
        })
        .to_ascii_lowercase()
        .as_str()
    {
        "none" => ProviderAuthMode::None,
        "static_header_env" | "static-header-env" => ProviderAuthMode::StaticHeaderEnv {
            header_name: env_trimmed(ENV_AUTH_HEADER_NAME)
                .unwrap_or_else(|| "x-api-key".to_string()),
            env_var: env_trimmed(ENV_AUTH_ENV_VAR)
                .unwrap_or_else(|| DEFAULT_BEARER_ENV.to_string()),
        },
        _ => ProviderAuthMode::BearerEnv {
            env_var: env_trimmed(ENV_AUTH_ENV_VAR)
                .unwrap_or_else(|| DEFAULT_BEARER_ENV.to_string()),
        },
    }
}

fn normalize_endpoint_url(raw: &str) -> LoomResult<Url> {
    let mut url = Url::parse(raw.trim())
        .map_err(|error| format!("provider base_url is invalid: {error}"))?;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/v1/chat/completions");
    }
    Ok(url)
}

fn endpoint_is_local(raw: &str) -> bool {
    Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(|value| value.to_string()))
        .map(|host| matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1"))
        .unwrap_or(false)
}

fn infer_provider_kind_from_base_url(base_url: &str) -> ProviderKind {
    if endpoint_is_local(base_url) {
        ProviderKind::LocalOllama
    } else if base_url.contains("openai.com") {
        ProviderKind::OpenAiCompatible
    } else {
        ProviderKind::CustomEndpoint
    }
}

fn env_trimmed(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn trim_to_option(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn value_string(value: Option<&Value>, label: &str) -> LoomResult<String> {
    value
        .and_then(Value::as_str)
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| format!("{label} must not be empty"))
}

fn value_string_or(value: Option<&Value>, fallback: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn io_err(error: std::io::Error) -> String {
    error.to_string()
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn json_option(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
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
    fn scaffold_writes_local_ollama_default_profile() {
        let root = temp_path("loom-provider-scaffold");
        let path = ensure_provider_profiles_scaffold(&root).expect("scaffold provider profiles");
        assert!(path.exists());
        let profiles = load_provider_profiles(Some(&root)).expect("load profiles");
        assert_eq!(profiles.default_profile_name, DEFAULT_LOCAL_PROFILE_NAME);
        assert_eq!(profiles.profiles.len(), 1);
        assert_eq!(profiles.profiles[0].kind, ProviderKind::LocalOllama);
        assert_eq!(profiles.routing.capabilities.len(), 1);
    }

    #[test]
    fn parser_supports_bearer_and_static_header_auth_modes() {
        let raw = r#"{
  "default_profile": "custom",
  "profiles": [
    {
      "name": "custom",
      "kind": "custom_endpoint",
      "base_url": "https://gateway.example.test",
      "model": "route-model",
      "auth": {
        "mode": "static_header_env",
        "header_name": "x-runtime-key",
        "env_var": "RUNTIME_KEY"
      }
    },
    {
      "name": "openai",
      "kind": "openai_compatible",
      "base_url": "https://api.openai.com/v1/chat/completions",
      "model": "gpt-4.1-mini",
      "auth": {
        "mode": "bearer_env",
        "env_var": "OPENAI_API_KEY"
      }
    }
  ],
  "routing": {
    "capabilities": {
      "loom.llm.inference.v1": { "profile": "openai", "default_model": "gpt-4.1-mini" }
    },
    "agents": {
      "agent_atlas": "custom"
    }
  }
}"#;
        let profiles = parse_provider_profiles_json(raw).expect("parse provider profiles");
        assert_eq!(profiles.profiles.len(), 2);
        assert_eq!(profiles.routing.capabilities.len(), 1);
        assert_eq!(profiles.routing.agents.len(), 1);
        assert_eq!(
            profiles.profiles[0].auth,
            ProviderAuthMode::StaticHeaderEnv {
                header_name: "x-runtime-key".to_string(),
                env_var: "RUNTIME_KEY".to_string(),
            }
        );
        assert_eq!(
            profiles.profiles[1].auth,
            ProviderAuthMode::BearerEnv {
                env_var: "OPENAI_API_KEY".to_string(),
            }
        );
    }

    #[test]
    fn resolved_route_uses_requested_model_override() {
        let root = temp_path("loom-provider-override");
        ensure_provider_profiles_scaffold(&root).expect("scaffold provider profiles");
        let route = resolve_llm_route(
            Some(&root),
            &ProviderRouteIntent::llm_inference("custom-alias"),
        )
        .expect("resolve route");
        assert_eq!(route.profile_name, DEFAULT_LOCAL_PROFILE_NAME);
        assert_eq!(route.model, "custom-alias");
        assert_eq!(route.endpoint_url.as_str(), DEFAULT_LOCAL_OLLAMA_ENDPOINT);
        assert_eq!(route.matched_rule, "capability:loom.llm.inference.v1");
    }

    #[test]
    fn agent_route_overrides_capability_route_when_no_explicit_profile_is_set() {
        let root = temp_path("loom-provider-agent-route");
        let path = ensure_provider_profiles_scaffold(&root).expect("scaffold provider profiles");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "default_profile": "local_ollama",
                "profiles": [
                    {
                        "name": "local_ollama",
                        "kind": "local_ollama",
                        "base_url": DEFAULT_LOCAL_OLLAMA_ENDPOINT,
                        "model": DEFAULT_MODEL_ALIAS,
                        "auth": { "mode": "none" }
                    },
                    {
                        "name": "atlas_local",
                        "kind": "local_ollama",
                        "base_url": DEFAULT_LOCAL_OLLAMA_ENDPOINT,
                        "model": "atlas-special",
                        "auth": { "mode": "none" }
                    }
                ],
                "routing": {
                    "capabilities": {
                        "loom.llm.inference.v1": { "profile": "local_ollama", "default_model": "cap-default" }
                    },
                    "agents": {
                        "agent_atlas": { "profile": "atlas_local", "default_model": "atlas-route-model" }
                    }
                }
            }))
            .expect("encode provider routing"),
        )
        .expect("write provider profiles");
        let route = resolve_provider_route(
            Some(&root),
            &ProviderRouteIntent::for_capability("loom.llm.inference.v1", "")
                .with_agent_id("agent_atlas")
                .with_org_id("org_demo"),
        )
        .expect("resolve provider route");
        assert_eq!(route.profile_name, "atlas_local");
        assert_eq!(route.model, "atlas-route-model");
        assert_eq!(route.matched_rule, "agent:agent_atlas");
        assert_eq!(route.agent_id.as_deref(), Some("agent_atlas"));
        assert_eq!(route.org_id.as_deref(), Some("org_demo"));
    }
}
