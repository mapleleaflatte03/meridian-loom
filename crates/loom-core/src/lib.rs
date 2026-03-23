use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub type LoomResult<T> = Result<T, String>;

const DEFAULT_STATE_DIR: &str = ".loom";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub mode: String,
    pub kernel_path: String,
    pub org_id: String,
    pub state_dir: String,
    pub python_path: String,
    pub typescript_path: String,
    pub wasm_dir: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub level: &'static str,
    pub label: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractSnapshot {
    pub kernel_path: PathBuf,
    pub runtime_status: String,
    pub local_scaffold: String,
    pub notes: String,
    pub hooks: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsuleInspection {
    pub org_id: String,
    pub manifest_path: PathBuf,
    pub state_dir: PathBuf,
    pub files: Vec<String>,
}

pub fn init_workspace(
    root: &Path,
    mode: &str,
    kernel_path: Option<&str>,
    org_id: &str,
) -> LoomResult<Config> {
    let mode = normalize_mode(mode)?;
    let root = ensure_root(root)?;
    let config_path = root.join("loom.toml");
    if config_path.exists() {
        return Err(format!(
            "refusing to overwrite existing config at {}",
            config_path.display()
        ));
    }

    let state_dir = root.join(DEFAULT_STATE_DIR);
    let capsule_dir = state_dir.join("capsules").join(org_id);
    let shadow_dir = state_dir.join("shadow");
    let workers_python = root.join("workers/python");
    let workers_typescript = root.join("workers/typescript");
    let workers_wasm = root.join("workers/wasm");

    fs::create_dir_all(&capsule_dir).map_err(io_err)?;
    fs::create_dir_all(&shadow_dir).map_err(io_err)?;
    fs::create_dir_all(&workers_python).map_err(io_err)?;
    fs::create_dir_all(&workers_typescript).map_err(io_err)?;
    fs::create_dir_all(&workers_wasm).map_err(io_err)?;

    let kernel_path = kernel_path.unwrap_or_default().to_string();
    let config = Config {
        mode,
        kernel_path,
        org_id: org_id.to_string(),
        state_dir: DEFAULT_STATE_DIR.to_string(),
        python_path: "workers/python".to_string(),
        typescript_path: "workers/typescript".to_string(),
        wasm_dir: "workers/wasm".to_string(),
    };

    fs::write(&config_path, render_config(&config)).map_err(io_err)?;
    fs::write(
        state_dir.join("state.json"),
        format!(
            "{{\n  \"org_id\": {},\n  \"mode\": {},\n  \"created_at\": {},\n  \"status\": \"initialized\"\n}}\n",
            json_string(&config.org_id),
            json_string(&config.mode),
            unix_now()
        ),
    )
    .map_err(io_err)?;
    fs::write(
        state_dir.join("audit.log"),
        format!(
            "{} init mode={} org_id={}\n",
            unix_now(),
            config.mode,
            config.org_id
        ),
    )
    .map_err(io_err)?;
    fs::write(
        capsule_dir.join("manifest.json"),
        format!(
            "{{\n  \"org_id\": {},\n  \"state\": \"local_embedded_capsule\",\n  \"provenance\": \"experimental_scaffold\",\n  \"created_at\": {},\n  \"files\": [\"state.json\", \"audit.log\"]\n}}\n",
            json_string(&config.org_id),
            unix_now()
        ),
    )
    .map_err(io_err)?;
    fs::write(
        shadow_dir.join("latest.json"),
        "{\n  \"status\": \"not_started\",\n  \"events_compared\": 0,\n  \"divergences\": 0,\n  \"note\": \"shadow mode is not implemented in this scaffold\"\n}\n",
    )
    .map_err(io_err)?;

    Ok(config)
}

pub fn read_config(root: &Path) -> LoomResult<Config> {
    let root = ensure_root(root)?;
    let contents = fs::read_to_string(root.join("loom.toml")).map_err(io_err)?;
    let mut values = BTreeMap::new();
    for raw in contents.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(
            key.trim().to_string(),
            value.trim().trim_matches('"').to_string(),
        );
    }

    let config = Config {
        mode: values
            .get("mode")
            .cloned()
            .ok_or_else(|| "loom.toml missing runtime.mode".to_string())?,
        kernel_path: values.get("kernel_path").cloned().unwrap_or_default(),
        org_id: values
            .get("org_id")
            .cloned()
            .unwrap_or_else(|| "local_foundry".to_string()),
        state_dir: values
            .get("state_dir")
            .cloned()
            .unwrap_or_else(|| DEFAULT_STATE_DIR.to_string()),
        python_path: values
            .get("python_path")
            .cloned()
            .unwrap_or_else(|| "workers/python".to_string()),
        typescript_path: values
            .get("typescript_path")
            .cloned()
            .unwrap_or_else(|| "workers/typescript".to_string()),
        wasm_dir: values
            .get("wasm_dir")
            .cloned()
            .unwrap_or_else(|| "workers/wasm".to_string()),
    };

    normalize_mode(&config.mode)?;
    Ok(config)
}

pub fn doctor(root: &Path) -> LoomResult<Vec<Check>> {
    let root = ensure_root(root)?;
    let config = read_config(&root)?;
    let mut checks = Vec::new();

    checks.push(Check {
        level: "OK",
        label: "config",
        detail: format!("loaded {}", root.join("loom.toml").display()),
    });

    let state_dir = root.join(&config.state_dir);
    push_path_check(
        &mut checks,
        "state_dir",
        &state_dir,
        true,
        "state directory present",
    );
    push_path_check(
        &mut checks,
        "python_workers",
        &root.join(&config.python_path),
        true,
        "python worker path present",
    );
    push_path_check(
        &mut checks,
        "typescript_workers",
        &root.join(&config.typescript_path),
        true,
        "typescript worker path present",
    );
    push_path_check(
        &mut checks,
        "wasm_modules",
        &root.join(&config.wasm_dir),
        true,
        "wasm module path present",
    );
    push_path_check(
        &mut checks,
        "capsule_manifest",
        &state_dir
            .join("capsules")
            .join(&config.org_id)
            .join("manifest.json"),
        true,
        "capsule manifest present",
    );

    let kernel_required = config.mode == "shadow" || config.mode == "standalone";
    let kernel_path = if config.kernel_path.is_empty() {
        None
    } else {
        Some(PathBuf::from(&config.kernel_path))
    };
    match (kernel_required, kernel_path) {
        (true, None) => checks.push(Check {
            level: "CRITICAL",
            label: "kernel_path",
            detail: "standalone/shadow mode requires --kernel-path".to_string(),
        }),
        (_, Some(path)) => {
            push_path_check(
                &mut checks,
                "kernel_path",
                &path,
                true,
                "kernel path present",
            );
            let registry = path.join("kernel/runtimes.json");
            push_path_check(
                &mut checks,
                "runtime_registry",
                &registry,
                true,
                "Meridian runtime registry available",
            );
        }
        (false, None) => checks.push(Check {
            level: "WARN",
            label: "kernel_path",
            detail: "embedded mode can run without kernel_path; contract inspection needs it".to_string(),
        }),
    }

    Ok(checks)
}

pub fn render_doctor_human(checks: &[Check]) -> String {
    let mut out = String::from("Meridian Loom doctor\n====================\n");
    for check in checks {
        out.push_str(&format!("[{:<8}] {:<18} {}\n", check.level, check.label, check.detail));
    }
    out
}

pub fn render_doctor_json(checks: &[Check]) -> String {
    let parts: Vec<String> = checks
        .iter()
        .map(|check| {
            format!(
                "{{\"level\":{},\"label\":{},\"detail\":{}}}",
                json_string(check.level),
                json_string(check.label),
                json_string(&check.detail)
            )
        })
        .collect();
    format!("[{}]\n", parts.join(","))
}

pub fn health(root: &Path) -> LoomResult<(bool, String)> {
    let checks = doctor(root)?;
    let degraded = checks
        .iter()
        .any(|check| check.level == "CRITICAL" || check.level == "WARN");
    let config = read_config(root)?;
    let status = if degraded { "degraded" } else { "healthy" };
    let json = format!(
        "{{\n  \"status\": {},\n  \"mode\": {},\n  \"org_id\": {},\n  \"checks\": {}\n}}\n",
        json_string(status),
        json_string(&config.mode),
        json_string(&config.org_id),
        render_doctor_json(&checks).trim()
    );
    Ok((!degraded, json))
}

pub fn status_human(root: &Path) -> LoomResult<String> {
    let root = ensure_root(root)?;
    let config = read_config(&root)?;
    let state_dir = root.join(&config.state_dir);
    let manifest = state_dir
        .join("capsules")
        .join(&config.org_id)
        .join("manifest.json");
    Ok(format!(
        "Meridian Loom status\n====================\nmode:        {}\norg_id:      {}\nstate_dir:   {}\nkernel_path: {}\ncapsule:     {}\nshadow:      {}\n",
        config.mode,
        config.org_id,
        state_dir.display(),
        if config.kernel_path.is_empty() { "(not set)" } else { &config.kernel_path },
        manifest.display(),
        state_dir.join("shadow/latest.json").display()
    ))
}

pub fn contract_show(root: &Path, override_kernel_path: Option<&str>) -> LoomResult<ContractSnapshot> {
    let config = read_config(root)?;
    let kernel_path = override_kernel_path
        .map(PathBuf::from)
        .or_else(|| {
            if config.kernel_path.is_empty() {
                None
            } else {
                Some(PathBuf::from(config.kernel_path))
            }
        })
        .ok_or_else(|| "kernel path is required for contract inspection".to_string())?;
    let registry_path = kernel_path.join("kernel/runtimes.json");
    let contents = fs::read_to_string(&registry_path).map_err(io_err)?;
    let start = contents
        .find("\"meridian_loom\"")
        .ok_or_else(|| format!("meridian_loom not found in {}", registry_path.display()))?;
    let section = &contents[start..];

    let runtime_status = extract_json_string(section, "\"status\"")
        .ok_or_else(|| "runtime status not found".to_string())?;
    let notes = extract_json_string(section, "\"notes\"").unwrap_or_default();
    let hook_names = [
        "agent_identity",
        "action_envelope",
        "cost_attribution",
        "approval_hook",
        "audit_emission",
        "sanction_controls",
        "budget_gate",
    ];
    let hooks = hook_names
        .iter()
        .map(|hook| {
            let key = format!("\"{}\"", hook);
            let value = extract_json_literal(section, &key).unwrap_or_else(|| "unknown".to_string());
            ((*hook).to_string(), value)
        })
        .collect();

    Ok(ContractSnapshot {
        kernel_path,
        runtime_status,
        local_scaffold: "experimental_scaffold_present".to_string(),
        notes,
        hooks,
    })
}

pub fn render_contract_human(snapshot: &ContractSnapshot) -> String {
    let mut out = format!(
        "Meridian Loom contract state\n============================\nkernel: {}\nstatus: {}\nlocal_scaffold: {}\n\n",
        snapshot.kernel_path.display(),
        snapshot.runtime_status
        ,
        snapshot.local_scaffold,
    );
    for (hook, value) in &snapshot.hooks {
        out.push_str(&format!("{:<18} {}\n", hook, value));
    }
    out.push_str(&format!("\nnotes: {}\n", snapshot.notes));
    out
}

pub fn render_contract_json(snapshot: &ContractSnapshot) -> String {
    let hooks = snapshot
        .hooks
        .iter()
        .map(|(hook, value)| format!("    {}: {}", json_string(hook), json_string(value)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"kernel_path\": {},\n  \"status\": {},\n  \"local_scaffold\": {},\n  \"hooks\": {{\n{}\n  }},\n  \"notes\": {}\n}}\n",
        json_string(&snapshot.kernel_path.display().to_string()),
        json_string(&snapshot.runtime_status),
        json_string(&snapshot.local_scaffold),
        hooks,
        json_string(&snapshot.notes)
    )
}

pub fn capsule_inspect(root: &Path) -> LoomResult<CapsuleInspection> {
    let config = read_config(root)?;
    let state_dir = root.join(&config.state_dir);
    let capsule_dir = state_dir.join("capsules").join(&config.org_id);
    let manifest_path = capsule_dir.join("manifest.json");
    let mut files = Vec::new();
    for entry in fs::read_dir(&capsule_dir).map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        files.push(entry.file_name().to_string_lossy().to_string());
    }
    files.sort();
    Ok(CapsuleInspection {
        org_id: config.org_id,
        manifest_path,
        state_dir,
        files,
    })
}

pub fn render_capsule_human(inspection: &CapsuleInspection) -> String {
    format!(
        "Capsule inspection\n==================\norg_id:       {}\nstate_dir:    {}\nmanifest:     {}\nfiles:        {}\n",
        inspection.org_id,
        inspection.state_dir.display(),
        inspection.manifest_path.display(),
        inspection.files.join(", ")
    )
}

pub fn root_from(opt: Option<&str>) -> LoomResult<PathBuf> {
    let root = opt
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(io_err)?;
    ensure_root(&root)
}

fn ensure_root(root: &Path) -> LoomResult<PathBuf> {
    fs::create_dir_all(root).map_err(io_err)?;
    Ok(root.to_path_buf())
}

fn render_config(config: &Config) -> String {
    format!(
        "[runtime]\nmode = {}\nkernel_path = {}\norg_id = {}\nstate_dir = {}\n\n[workers]\npython_path = {}\ntypescript_path = {}\nwasm_dir = {}\n",
        json_string(&config.mode),
        json_string(&config.kernel_path),
        json_string(&config.org_id),
        json_string(&config.state_dir),
        json_string(&config.python_path),
        json_string(&config.typescript_path),
        json_string(&config.wasm_dir),
    )
}

fn normalize_mode(mode: &str) -> LoomResult<String> {
    match mode {
        "embedded" | "shadow" | "standalone" => Ok(mode.to_string()),
        other => Err(format!("unsupported mode '{}'", other)),
    }
}

fn push_path_check(
    checks: &mut Vec<Check>,
    label: &'static str,
    path: &Path,
    required: bool,
    success: &'static str,
) {
    if path.exists() {
        checks.push(Check {
            level: "OK",
            label,
            detail: format!("{} ({})", success, path.display()),
        });
    } else if required {
        checks.push(Check {
            level: "CRITICAL",
            label,
            detail: format!("missing {}", path.display()),
        });
    } else {
        checks.push(Check {
            level: "WARN",
            label,
            detail: format!("optional path missing {}", path.display()),
        });
    }
}

fn extract_json_string(section: &str, key: &str) -> Option<String> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let first_quote = after.find('"')?;
    let rest = &after[first_quote + 1..];
    let end_quote = rest.find('"')?;
    Some(rest[..end_quote].to_string())
}

fn extract_json_literal(section: &str, key: &str) -> Option<String> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let end = rest.find([',', '\n', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('"').to_string())
}

fn json_string(input: &str) -> String {
    format!("{:?}", input)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn io_err(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("loom-core-{}-{}", name, unix_now()));
        if path.exists() {
            let _ = fs::remove_dir_all(&path);
        }
        path
    }

    #[test]
    fn init_and_read_config_round_trip() {
        let root = temp_root("roundtrip");
        let config = init_workspace(&root, "embedded", Some("/tmp/meridian-kernel"), "test_org")
            .expect("init workspace");
        let loaded = read_config(&root).expect("read config");
        assert_eq!(config, loaded);
    }

    #[test]
    fn init_refuses_overwrite() {
        let root = temp_root("overwrite");
        init_workspace(&root, "embedded", None, "org").expect("first init");
        let error = init_workspace(&root, "embedded", None, "org").expect_err("second init fails");
        assert!(error.contains("refusing to overwrite"));
    }
}
