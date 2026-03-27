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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderProfileSet {
    pub default_profile_name: String,
    pub profiles: Vec<ProviderProfile>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRouteIntent {
    pub capability_name: String,
    pub preferred_profile_name: Option<String>,
    pub requested_model: String,
}

impl ProviderRouteIntent {
    pub fn llm_inference(requested_model: &str) -> Self {
        Self {
            capability_name: "loom.llm.inference.v1".to_string(),
            preferred_profile_name: env_trimmed(ENV_PROVIDER_PROFILE),
            requested_model: requested_model.trim().to_string(),
        }
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

pub fn resolve_llm_route(root: Option<&Path>, intent: &ProviderRouteIntent) -> LoomResult<ResolvedProviderRoute> {
    let profiles = load_provider_profiles(root)?;
    let selected_name = intent
        .preferred_profile_name
        .clone()
        .unwrap_or_else(|| profiles.default_profile_name.clone());
    let profile = profiles
        .profiles
        .iter()
        .find(|candidate| candidate.name == selected_name)
        .ok_or_else(|| {
            let available = profiles
                .profiles
                .iter()
                .map(|profile| profile.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "provider profile '{}' was not found (available: {})",
                selected_name, available
            )
        })?;
    let endpoint_url = normalize_endpoint_url(&profile.base_url)?;
    let model = if intent.requested_model.trim().is_empty() {
        profile.default_model.clone()
    } else {
        intent.requested_model.trim().to_string()
    };
    Ok(ResolvedProviderRoute {
        capability_name: intent.capability_name.clone(),
        profile_name: profile.name.clone(),
        profile_kind: profile.kind.clone(),
        endpoint_url,
        model,
        auth: profile.auth.clone(),
        source: profiles.source,
        note: profile.note.clone(),
    })
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
        ]
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
                name: profile_name,
                kind: inferred_kind,
                base_url,
                default_model: explicit_model,
                auth,
                note: "resolved from environment-backed provider defaults".to_string(),
            }],
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
                default_model: explicit_model,
                auth: ProviderAuthMode::BearerEnv {
                    env_var: DEFAULT_BEARER_ENV.to_string(),
                },
                note: "resolved from OPENAI_API_KEY-backed defaults".to_string(),
            }],
            source: "env_defaults".to_string(),
        };
    }
    ProviderProfileSet {
        default_profile_name: DEFAULT_LOCAL_PROFILE_NAME.to_string(),
        profiles: vec![ProviderProfile {
            name: DEFAULT_LOCAL_PROFILE_NAME.to_string(),
            kind: ProviderKind::LocalOllama,
            base_url: DEFAULT_LOCAL_OLLAMA_ENDPOINT.to_string(),
            default_model: explicit_model,
            auth: ProviderAuthMode::None,
            note: "resolved from Meridian local Ollama defaults".to_string(),
        }],
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
  ]
}"#;
        let profiles = parse_provider_profiles_json(raw).expect("parse provider profiles");
        assert_eq!(profiles.profiles.len(), 2);
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
            &ProviderRouteIntent {
                capability_name: "loom.llm.inference.v1".to_string(),
                preferred_profile_name: Some(DEFAULT_LOCAL_PROFILE_NAME.to_string()),
                requested_model: "custom-alias".to_string(),
            },
        )
        .expect("resolve route");
        assert_eq!(route.profile_name, DEFAULT_LOCAL_PROFILE_NAME);
        assert_eq!(route.model, "custom-alias");
        assert_eq!(route.endpoint_url.as_str(), DEFAULT_LOCAL_OLLAMA_ENDPOINT);
    }
}
