use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod capability_shims;
pub mod wasm_host;
pub mod wasm_limits;
pub mod wasm_profiles;

pub type LoomResult<T> = Result<T, String>;

const DEFAULT_STATE_DIR: &str = ".loom";
const DEFAULT_PYTHON_WORKER_FILE: &str = "loom_runtime_worker.py";
const DEFAULT_PYTHON_WORKER_SOURCE: &str = r#"#!/usr/bin/env python3
import argparse
import json
from datetime import datetime, timezone


def main():
    parser = argparse.ArgumentParser(description="Meridian Loom experimental worker")
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    with open(args.input, "r", encoding="utf-8") as handle:
        payload = json.load(handle)

    envelope = payload.get("envelope", {})
    decision = payload.get("decision", {})
    result = {
        "status": "completed",
        "worker_kind": "python_reference_worker",
        "completed_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "agent_id": envelope.get("agent_id", ""),
        "org_id": envelope.get("org_id", ""),
        "action_type": envelope.get("action_type", ""),
        "resource": envelope.get("resource", ""),
        "input_hash": payload.get("input_hash", ""),
        "decision": decision.get("overall_decision", ""),
        "effective_stage": decision.get("effective_stage", ""),
        "summary": f"experimental worker handled {envelope.get('action_type', 'unknown')}::{envelope.get('resource', 'unknown')}",
    }

    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(result, handle, indent=2, sort_keys=True)
        handle.write("\n")

    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
"#;
const EXPERIMENTAL_PRELIGHT_HOOKS: [&str; 7] = [
    "agent_identity",
    "action_envelope",
    "cost_attribution",
    "approval_hook",
    "audit_emission",
    "sanction_controls",
    "budget_gate",
];

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
    pub experimental_hooks: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsuleInspection {
    pub org_id: String,
    pub manifest_path: PathBuf,
    pub state_dir: PathBuf,
    pub files: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentIdentityResolution {
    pub agent_id: String,
    pub agent_name: String,
    pub org_id: String,
    pub role: String,
    pub economy_key: String,
    pub approval_required: bool,
    pub max_per_run_usd: Option<f64>,
    pub restrictions: Vec<String>,
    pub sanction_decision: String,
    pub runtime_id: String,
    pub runtime_label: String,
    pub bound_org_id: String,
    pub boundary_name: String,
    pub identity_model: String,
    pub runtime_registered: bool,
    pub registration_status: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionEnvelope {
    pub agent_id: String,
    pub agent_name: String,
    pub org_id: String,
    pub runtime_id: String,
    pub runtime_label: String,
    pub action_type: String,
    pub resource: String,
    pub estimated_cost_usd: f64,
    pub run_id: String,
    pub session_id: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceGateCheck {
    pub allowed: bool,
    pub stage: String,
    pub reason: String,
    pub restrictions: Vec<String>,
    pub sanction_gate_decision: String,
    pub approval_gate_decision: String,
    pub budget_gate_decision: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSanctionPreview {
    pub allowed: bool,
    pub decision: String,
    pub reason: String,
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
    ensure_runtime_worker_scaffold(&root, &config)?;

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
    fs::write(shadow_dir.join("events.jsonl"), "").map_err(io_err)?;

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

pub fn ensure_runtime_worker_scaffold(root: &Path, config: &Config) -> LoomResult<PathBuf> {
    let worker_path = root
        .join(&config.python_path)
        .join(DEFAULT_PYTHON_WORKER_FILE);
    if let Some(parent) = worker_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    if !worker_path.exists() {
        fs::write(&worker_path, DEFAULT_PYTHON_WORKER_SOURCE).map_err(io_err)?;
    }
    Ok(worker_path)
}

pub fn runtime_worker_entry(root: &Path, config: &Config) -> PathBuf {
    root.join(&config.python_path)
        .join(DEFAULT_PYTHON_WORKER_FILE)
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
            let agent_registry = path.join("kernel/agent_registry.py");
            push_path_check(
                &mut checks,
                "agent_registry",
                &agent_registry,
                true,
                "Meridian agent registry CLI available",
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
    let mut out = String::from(
        "Meridian Loom // DOCTOR\n=======================\nphase:       experimental runtime rehearsal\nboundary:    public scaffold, not governed runtime\n\nChecks\n------\n",
    );
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
        "{{\n  \"status\": {},\n  \"mode\": {},\n  \"org_id\": {},\n  \"checks\": {},\n  \"experimental_hooks\": {}\n}}\n",
        json_string(status),
        json_string(&config.mode),
        json_string(&config.org_id),
        render_doctor_json(&checks).trim(),
        render_json_string_array(&EXPERIMENTAL_PRELIGHT_HOOKS)
    );
    Ok((!degraded, json))
}

pub fn render_health_human(healthy: bool, json: &str) -> String {
    let status = if healthy { "healthy" } else { "degraded" };
    let mode = extract_json_string(json, "\"mode\"").unwrap_or_else(|| "unknown".to_string());
    let org_id = extract_json_string(json, "\"org_id\"").unwrap_or_else(|| "unknown".to_string());
    let check_count = json.matches("\"label\"").count();
    format!(
        "Meridian Loom // HEALTH\n=======================\nstatus:      {}\nmode:        {}\norg_id:      {}\nchecks:      {}\nsource:      doctor-derived health summary\nnext_step:   loom doctor --root <path> --format human\n",
        status,
        mode,
        org_id,
        check_count
    )
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
        "Meridian Loom // STATUS\n=======================\nmode:        {}\norg_id:      {}\nstate_dir:   {}\nkernel_path: {}\ncapsule:     {}\nshadow:      {}\nqueue:       {}\nruntime:     experimental local queue supervisor + one-shot rehearsal\nexperimental_hooks: {}\n",
        config.mode,
        config.org_id,
        state_dir.display(),
        if config.kernel_path.is_empty() { "(not set)" } else { &config.kernel_path },
        manifest.display(),
        state_dir.join("shadow/latest.json").display(),
        state_dir.join("runtime/queue/pending").display(),
        EXPERIMENTAL_PRELIGHT_HOOKS.join(", ")
    ))
}

pub fn render_config_human(config: &Config, root: &Path) -> String {
    format!(
        "Meridian Loom // CONFIG\n=======================\nroot:        {}\nmode:        {}\norg_id:      {}\nstate_dir:   {}\nkernel_path: {}\npython_path: {}\ntypescript:  {}\nwasm_dir:    {}\nboundary:    local config only; experimental queue supervisor is available\n",
        root.display(),
        config.mode,
        config.org_id,
        config.state_dir,
        if config.kernel_path.is_empty() {
            "(not set)"
        } else {
            &config.kernel_path
        },
        config.python_path,
        config.typescript_path,
        config.wasm_dir,
    )
}

pub fn contract_show(root: &Path, override_kernel_path: Option<&str>) -> LoomResult<ContractSnapshot> {
    let config = read_config(root)?;
    let kernel_path = resolve_kernel_path(root, override_kernel_path, Some(&config))?;
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

    let experimental_hooks = EXPERIMENTAL_PRELIGHT_HOOKS
        .iter()
        .map(|hook| ((*hook).to_string(), "experimental_preflight_path".to_string()))
        .collect();

    Ok(ContractSnapshot {
        kernel_path,
        runtime_status,
        local_scaffold: "experimental_scaffold_present".to_string(),
        notes,
        hooks,
        experimental_hooks,
    })
}

pub fn render_contract_human(snapshot: &ContractSnapshot) -> String {
    let mut out = format!(
        "Meridian Loom // CONTRACT\n=========================\nkernel: {}\nstatus: {}\nlocal_scaffold: {}\n\nregistry_declared_hooks\n----------------------\n",
        snapshot.kernel_path.display(),
        snapshot.runtime_status,
        snapshot.local_scaffold,
    );
    for (hook, value) in &snapshot.hooks {
        out.push_str(&format!("{:<18} {}\n", hook, value));
    }
    out.push_str("\nexperimental_hook_paths\n-----------------------\n");
    for (hook, value) in &snapshot.experimental_hooks {
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
    let experimental = snapshot
        .experimental_hooks
        .iter()
        .map(|(hook, value)| format!("    {}: {}", json_string(hook), json_string(value)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"kernel_path\": {},\n  \"status\": {},\n  \"local_scaffold\": {},\n  \"hooks\": {{\n{}\n  }},\n  \"experimental_hooks\": {{\n{}\n  }},\n  \"notes\": {}\n}}\n",
        json_string(&snapshot.kernel_path.display().to_string()),
        json_string(&snapshot.runtime_status),
        json_string(&snapshot.local_scaffold),
        hooks,
        experimental,
        json_string(&snapshot.notes)
    )
}

pub fn resolve_agent_identity(
    root: &Path,
    override_kernel_path: Option<&str>,
    agent_ref: &str,
    org_hint: Option<&str>,
) -> LoomResult<AgentIdentityResolution> {
    let config = read_config(root)?;
    let kernel_path = resolve_kernel_path(root, override_kernel_path, Some(&config))?;
    let script = kernel_path.join("kernel/agent_registry.py");
    if !script.exists() {
        return Err(format!("missing {}", script.display()));
    }

    let normalized_agent = agent_ref.trim();
    if normalized_agent.is_empty() {
        return Err("agent_ref is required".to_string());
    }

    let explicit_org_hint = org_hint
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let stdout = run_agent_registry_lookup(&script, normalized_agent, explicit_org_hint.as_deref())?;

    let runtime_binding = find_named_object(&stdout, "\"runtime_binding\"")
        .ok_or_else(|| "runtime_binding missing from agent record".to_string())?;
    let agent_id = extract_json_string(&stdout, "\"id\"")
        .ok_or_else(|| "agent id missing".to_string())?;
    let agent_name = extract_json_string(&stdout, "\"name\"")
        .ok_or_else(|| "agent name missing".to_string())?;
    let org_id = extract_json_string(&stdout, "\"org_id\"")
        .ok_or_else(|| "org_id missing".to_string())?;
    let role = extract_json_string(&stdout, "\"role\"").unwrap_or_default();
    let economy_key = extract_json_string(&stdout, "\"economy_key\"").unwrap_or_default();
    let approval_required = extract_json_bool(&stdout, "\"approval_required\"").unwrap_or(false);
    let max_per_run_usd = find_named_object(&stdout, "\"budget\"")
        .and_then(|budget| extract_json_f64(&budget, "\"max_per_run_usd\""));
    let snapshot_restrictions = extract_json_string_array(&stdout, "\"restrictions\"");
    let snapshot_sanction_decision = extract_json_string(&stdout, "\"sanction_decision\"");
    let restriction_lookup = if economy_key.is_empty() {
        agent_id.as_str()
    } else {
        economy_key.as_str()
    };
    let (restrictions, sanction_decision) = if snapshot_restrictions.is_empty() {
        query_agent_restrictions(&kernel_path, restriction_lookup, Some(&org_id))
    } else {
        let decision = snapshot_sanction_decision
            .unwrap_or_else(|| derive_sanction_decision(&snapshot_restrictions).to_string());
        (snapshot_restrictions, decision)
    };

    Ok(AgentIdentityResolution {
        agent_id,
        agent_name,
        org_id,
        role,
        economy_key,
        approval_required,
        max_per_run_usd,
        restrictions,
        sanction_decision,
        runtime_id: extract_json_string(&runtime_binding, "\"runtime_id\"")
            .ok_or_else(|| "runtime_id missing".to_string())?,
        runtime_label: extract_json_string(&runtime_binding, "\"runtime_label\"").unwrap_or_default(),
        bound_org_id: extract_json_string(&runtime_binding, "\"bound_org_id\"").unwrap_or_default(),
        boundary_name: extract_json_string(&runtime_binding, "\"boundary_name\"").unwrap_or_default(),
        identity_model: extract_json_string(&runtime_binding, "\"identity_model\"").unwrap_or_default(),
        runtime_registered: extract_json_bool(&runtime_binding, "\"runtime_registered\"").unwrap_or(true),
        registration_status: extract_json_string(&runtime_binding, "\"registration_status\"")
            .unwrap_or_else(|| "registered".to_string()),
        source: "kernel_agent_registry".to_string(),
    })
}

fn run_agent_registry_lookup(
    script: &Path,
    agent_ref: &str,
    org_hint: Option<&str>,
) -> LoomResult<String> {
    let mut cmd = Command::new("python3");
    cmd.arg(script).arg("get").arg("--agent_id").arg(agent_ref);
    if let Some(org_id) = org_hint {
        cmd.arg("--org_id").arg(org_id);
    }

    let output = cmd.output().map_err(io_err)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "agent_registry lookup failed: {}",
            if stderr.is_empty() { stdout } else { stderr }
        ));
    }
    if stdout.starts_with("Not found:") {
        return Err(stdout);
    }
    if !stdout.starts_with('{') {
        return Err(format!(
            "agent_registry returned non-JSON output: {}",
            stdout.lines().next().unwrap_or_default()
        ));
    }
    Ok(stdout)
}

fn query_agent_restrictions(
    kernel_path: &Path,
    agent_lookup: &str,
    org_hint: Option<&str>,
) -> (Vec<String>, String) {
    let kernel_dir = kernel_path.join("kernel");
    let script = r#"import sys
kernel_dir = sys.argv[1]
agent_id = sys.argv[2]
org_id = sys.argv[3] if len(sys.argv) > 3 and sys.argv[3] else None
sys.path.insert(0, kernel_dir)
from court import get_restrictions
for item in get_restrictions(agent_id, org_id=org_id) or []:
    print(item)
"#;

    let mut cmd = Command::new("python3");
    cmd.arg("-c")
        .arg(script)
        .arg(&kernel_dir)
        .arg(agent_lookup);
    if let Some(org_id) = org_hint {
        cmd.arg(org_id);
    }

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let restrictions = stdout
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|line| line.to_string())
                .collect::<Vec<_>>();
            let decision = derive_sanction_decision(&restrictions);
            (restrictions, decision.to_string())
        }
        _ => (Vec::new(), "unknown".to_string()),
    }
}

fn derive_sanction_decision(restrictions: &[String]) -> &'static str {
    if restrictions.is_empty() {
        "clear"
    } else if restrictions
        .iter()
        .any(|value| value == "execute" || value == "remediation_only")
    {
        "restricted_execute"
    } else {
        "restricted"
    }
}

pub fn render_identity_human(identity: &AgentIdentityResolution) -> String {
    format!(
        "Meridian Loom // AGENT IDENTITY\n================================\nagent_id:            {}\nagent_name:          {}\norg_id:              {}\nrole:                {}\neconomy_key:         {}\napproval_required:   {}\nmax_per_run_usd:     {}\nrestrictions:        {}\nsanction_decision:   {}\nruntime_id:          {}\nruntime_label:       {}\nbound_org_id:        {}\nboundary_name:       {}\nidentity_model:      {}\nruntime_registered:  {}\nregistration_status: {}\nsource:              {}\n",
        identity.agent_id,
        identity.agent_name,
        identity.org_id,
        identity.role,
        if identity.economy_key.is_empty() { "(none)" } else { &identity.economy_key },
        identity.approval_required,
        identity
            .max_per_run_usd
            .map(|value| format!("{:.4}", value))
            .unwrap_or_else(|| "(unknown)".to_string()),
        if identity.restrictions.is_empty() {
            "(none)".to_string()
        } else {
            identity.restrictions.join(", ")
        },
        identity.sanction_decision,
        identity.runtime_id,
        identity.runtime_label,
        identity.bound_org_id,
        identity.boundary_name,
        identity.identity_model,
        identity.runtime_registered,
        identity.registration_status,
        identity.source,
    )
}

pub fn render_identity_json(identity: &AgentIdentityResolution) -> String {
    format!(
        "{{\n  \"agent_id\": {},\n  \"agent_name\": {},\n  \"org_id\": {},\n  \"role\": {},\n  \"economy_key\": {},\n  \"approval_required\": {},\n  \"max_per_run_usd\": {},\n  \"restrictions\": {},\n  \"sanction_decision\": {},\n  \"runtime_id\": {},\n  \"runtime_label\": {},\n  \"bound_org_id\": {},\n  \"boundary_name\": {},\n  \"identity_model\": {},\n  \"runtime_registered\": {},\n  \"registration_status\": {},\n  \"source\": {}\n}}\n",
        json_string(&identity.agent_id),
        json_string(&identity.agent_name),
        json_string(&identity.org_id),
        json_string(&identity.role),
        json_string(&identity.economy_key),
        if identity.approval_required { "true" } else { "false" },
        identity
            .max_per_run_usd
            .map(|value| format!("{:.6}", value))
            .unwrap_or_else(|| "null".to_string()),
        render_json_string_array(&identity.restrictions),
        json_string(&identity.sanction_decision),
        json_string(&identity.runtime_id),
        json_string(&identity.runtime_label),
        json_string(&identity.bound_org_id),
        json_string(&identity.boundary_name),
        json_string(&identity.identity_model),
        if identity.runtime_registered { "true" } else { "false" },
        json_string(&identity.registration_status),
        json_string(&identity.source),
    )
}

pub fn build_action_envelope(
    root: &Path,
    override_kernel_path: Option<&str>,
    agent_ref: &str,
    org_hint: Option<&str>,
    action_type: &str,
    resource: &str,
    estimated_cost_usd: f64,
    run_id: Option<&str>,
    session_id: Option<&str>,
) -> LoomResult<ActionEnvelope> {
    let action_type = action_type.trim();
    let resource = resource.trim();
    if action_type.is_empty() {
        return Err("action_type is required".to_string());
    }
    if resource.is_empty() {
        return Err("resource is required".to_string());
    }
    if estimated_cost_usd < 0.0 {
        return Err("estimated_cost_usd must be non-negative".to_string());
    }

    let identity = resolve_agent_identity(root, override_kernel_path, agent_ref, org_hint)?;
    Ok(ActionEnvelope {
        agent_id: identity.agent_id,
        agent_name: identity.agent_name,
        org_id: identity.org_id,
        runtime_id: identity.runtime_id,
        runtime_label: identity.runtime_label,
        action_type: action_type.to_string(),
        resource: resource.to_string(),
        estimated_cost_usd,
        run_id: run_id.unwrap_or("").trim().to_string(),
        session_id: session_id.unwrap_or("").trim().to_string(),
        source: "loom_experimental_preflight".to_string(),
    })
}

pub fn preview_local_sanction_controls(identity: &AgentIdentityResolution) -> LocalSanctionPreview {
    if identity
        .restrictions
        .iter()
        .any(|value| value == "execute" || value == "remediation_only")
    {
        return LocalSanctionPreview {
            allowed: false,
            decision: "deny".to_string(),
            reason: format!("Agent {} is restricted from execute", identity.agent_id),
        };
    }

    LocalSanctionPreview {
        allowed: true,
        decision: "allow".to_string(),
        reason: "no execute restriction".to_string(),
    }
}

pub fn evaluate_reference_gates(
    root: &Path,
    override_kernel_path: Option<&str>,
    identity: &AgentIdentityResolution,
    envelope: &ActionEnvelope,
) -> LoomResult<ReferenceGateCheck> {
    let config = read_config(root)?;
    let kernel_path = resolve_kernel_path(root, override_kernel_path, Some(&config))?;
    let kernel_dir = kernel_path.join("kernel");
    let script = r#"import json, sys
kernel_dir = sys.argv[1]
org_id = sys.argv[2]
envelope = json.loads(sys.argv[3])
sys.path.insert(0, kernel_dir)
from adapters.openclaw_compatible import pre_action_check
print(json.dumps(pre_action_check(org_id, envelope)))
"#;

    let reference_agent = if identity.economy_key.trim().is_empty() {
        envelope.agent_id.clone()
    } else {
        identity.economy_key.clone()
    };
    let reference_envelope = ActionEnvelope {
        agent_id: reference_agent,
        agent_name: envelope.agent_name.clone(),
        org_id: envelope.org_id.clone(),
        runtime_id: envelope.runtime_id.clone(),
        runtime_label: envelope.runtime_label.clone(),
        action_type: envelope.action_type.clone(),
        resource: envelope.resource.clone(),
        estimated_cost_usd: envelope.estimated_cost_usd,
        run_id: envelope.run_id.clone(),
        session_id: envelope.session_id.clone(),
        source: envelope.source.clone(),
    };
    let envelope_json = render_envelope_json(&reference_envelope);
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(&kernel_dir)
        .arg(&envelope.org_id)
        .arg(envelope_json)
        .output()
        .map_err(io_err)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "reference adapter pre_action_check failed: {}",
            if stderr.is_empty() { stdout } else { stderr }
        ));
    }
    if !stdout.starts_with('{') {
        return Err(format!(
            "reference adapter returned non-JSON output: {}",
            stdout.lines().next().unwrap_or_default()
        ));
    }

    let allowed = extract_json_bool(&stdout, "\"allowed\"")
        .ok_or_else(|| "reference adapter response missing allowed".to_string())?;
    let stage = extract_json_string(&stdout, "\"stage\"").unwrap_or_else(|| {
        if allowed {
            "ok".to_string()
        } else {
            "unknown".to_string()
        }
    });
    let reason = extract_json_string(&stdout, "\"reason\"").unwrap_or_default();
    let restrictions = extract_json_string_array(&stdout, "\"restrictions\"");
    let sanction_gate_decision = if stage == "sanction_controls" {
        "deny".to_string()
    } else {
        "allow".to_string()
    };
    let approval_gate_decision = match stage.as_str() {
        "sanction_controls" => "not_reached".to_string(),
        "approval_hook" => "deny".to_string(),
        _ => "allow".to_string(),
    };
    let budget_gate_decision = match stage.as_str() {
        "sanction_controls" | "approval_hook" => "not_reached".to_string(),
        "budget_gate" => "deny".to_string(),
        _ if envelope.estimated_cost_usd <= 0.0 => "skipped_zero_cost".to_string(),
        _ => "allow".to_string(),
    };

    Ok(ReferenceGateCheck {
        allowed,
        stage,
        reason,
        restrictions,
        sanction_gate_decision,
        approval_gate_decision,
        budget_gate_decision,
        source: "kernel_reference_adapter_read_only".to_string(),
    })
}

pub fn envelope_input_hash(envelope: &ActionEnvelope) -> String {
    let raw = format!(
        "{}|{}|{}|{}|{:.6}|{}|{}",
        envelope.agent_id,
        envelope.org_id,
        envelope.runtime_id,
        envelope.action_type,
        envelope.estimated_cost_usd,
        envelope.resource,
        envelope.source,
    );
    format!("{:016x}", fnv1a64(raw.as_bytes()))
}

pub fn render_envelope_human(envelope: &ActionEnvelope) -> String {
    format!(
        "Meridian Loom // ACTION ENVELOPE\n=================================\nagent_id:            {}\nagent_name:          {}\norg_id:              {}\nruntime_id:          {}\nruntime_label:       {}\naction_type:         {}\nresource:            {}\nestimated_cost_usd:  {:.4}\nrun_id:              {}\nsession_id:          {}\nsource:              {}\ninput_hash:          {}\n",
        envelope.agent_id,
        envelope.agent_name,
        envelope.org_id,
        envelope.runtime_id,
        envelope.runtime_label,
        envelope.action_type,
        envelope.resource,
        envelope.estimated_cost_usd,
        if envelope.run_id.is_empty() { "(none)" } else { &envelope.run_id },
        if envelope.session_id.is_empty() { "(none)" } else { &envelope.session_id },
        envelope.source,
        envelope_input_hash(envelope),
    )
}

pub fn render_envelope_json(envelope: &ActionEnvelope) -> String {
    format!(
        "{{\n  \"agent_id\": {},\n  \"agent_name\": {},\n  \"org_id\": {},\n  \"runtime_id\": {},\n  \"runtime_label\": {},\n  \"action_type\": {},\n  \"resource\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"run_id\": {},\n  \"session_id\": {},\n  \"source\": {},\n  \"input_hash\": {}\n}}\n",
        json_string(&envelope.agent_id),
        json_string(&envelope.agent_name),
        json_string(&envelope.org_id),
        json_string(&envelope.runtime_id),
        json_string(&envelope.runtime_label),
        json_string(&envelope.action_type),
        json_string(&envelope.resource),
        envelope.estimated_cost_usd,
        json_string(&envelope.run_id),
        json_string(&envelope.session_id),
        json_string(&envelope.source),
        json_string(&envelope_input_hash(envelope)),
    )
}

pub fn capsule_inspect(root: &Path) -> LoomResult<CapsuleInspection> {
    let config = read_config(root)?;
    let state_dir = root.join(&config.state_dir);
    let capsule_dir = state_dir.join("capsules").join(&config.org_id);
    let manifest_path = capsule_dir.join("manifest.json");
    let mut files: Vec<String> = fs::read_dir(&capsule_dir)
        .map_err(io_err)?
        .map(|entry| {
            let entry = entry.map_err(io_err)?;
            Ok(entry
                .file_name()
                .into_string()
                .unwrap_or_else(|os_str| os_str.to_string_lossy().into_owned()))
        })
        .collect::<LoomResult<Vec<String>>>()?;
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
        "Meridian Loom // CAPSULE INSPECT\n=================================\norg_id:       {}\nstate_dir:    {}\nmanifest:     {}\nfiles:        {}\n",
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

pub fn kernel_path_for(root: &Path, override_kernel_path: Option<&str>) -> LoomResult<PathBuf> {
    let config = read_config(root)?;
    resolve_kernel_path(root, override_kernel_path, Some(&config))
}

fn resolve_kernel_path(
    root: &Path,
    override_kernel_path: Option<&str>,
    config: Option<&Config>,
) -> LoomResult<PathBuf> {
    let from_override = override_kernel_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let from_config = config
        .map(|cfg| cfg.kernel_path.trim().to_string())
        .filter(|value| !value.is_empty());
    let kernel_path = from_override
        .or(from_config)
        .ok_or_else(|| format!("kernel path is required for {}", root.display()))?;
    Ok(PathBuf::from(kernel_path))
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

fn find_named_object(section: &str, key: &str) -> Option<String> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let brace_idx = after.find('{')?;
    let start = idx + key.len() + brace_idx;
    let end = find_matching_brace(section, start)?;
    Some(section[start..=end].to_string())
}

fn find_matching_brace(section: &str, start: usize) -> Option<usize> {
    let bytes = section.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, byte) in bytes.iter().enumerate().skip(start) {
        let ch = *byte as char;
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_json_string(section: &str, key: &str) -> Option<String> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let first_quote = after.find('"')?;
    let rest = &after[first_quote + 1..];
    let end_quote = rest.find('"')?;
    Some(rest[..end_quote].to_string())
}

fn extract_json_bool(section: &str, key: &str) -> Option<bool> {
    match extract_json_literal(section, key)?.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn extract_json_string_array(section: &str, key: &str) -> Vec<String> {
    let Some(idx) = section.find(key) else {
        return Vec::new();
    };
    let after = &section[idx + key.len()..];
    let Some(bracket_start) = after.find('[') else {
        return Vec::new();
    };
    let rest = &after[bracket_start + 1..];
    let Some(bracket_end) = rest.find(']') else {
        return Vec::new();
    };
    let body = &rest[..bracket_end];
    body.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.trim_matches('"').to_string())
        .collect()
}

fn extract_json_f64(section: &str, key: &str) -> Option<f64> {
    extract_json_literal(section, key)?.parse::<f64>().ok()
}

fn extract_json_literal(section: &str, key: &str) -> Option<String> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let end = rest.find([',', '\n', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('"').to_string())
}

fn render_json_string_array<T: AsRef<str>>(values: &[T]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string(value.as_ref()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn json_string(input: &str) -> String {
    format!("{:?}", input)
}

fn io_err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn init_refuses_overwrite() {
        let root = temp_path("loom-core-init-refuse");
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("loom.toml"), "existing").expect("existing config");
        let error = init_workspace(&root, "embedded", None, "org_demo").expect_err("should fail");
        assert!(error.contains("refusing to overwrite"));
    }

    #[test]
    fn init_and_read_config_round_trip() {
        let root = temp_path("loom-core-init");
        let config = init_workspace(&root, "embedded", Some("/tmp/meridian-kernel"), "org_demo")
            .expect("init workspace");
        assert_eq!(config.mode, "embedded");
        let loaded = read_config(&root).expect("read config");
        assert_eq!(loaded.org_id, "org_demo");
        assert_eq!(loaded.kernel_path, "/tmp/meridian-kernel");
        assert!(root.join(".loom/shadow/events.jsonl").exists());
    }

    #[test]
    fn resolve_identity_and_build_envelope_against_fake_kernel() {
        let kernel = fake_kernel_root("atlas");
        let root = temp_path("loom-core-envelope");
        init_workspace(&root, "shadow", Some(&kernel.display().to_string()), "org_demo")
            .expect("init workspace");

        let identity = resolve_agent_identity(&root, None, "atlas", None).expect("resolve identity");
        assert_eq!(identity.agent_id, "agent_atlas");
        assert_eq!(identity.max_per_run_usd, Some(0.5));
        assert!(!identity.approval_required);
        assert_eq!(identity.restrictions, vec!["lead".to_string()]);
        assert_eq!(identity.sanction_decision, "restricted");
        assert_eq!(identity.runtime_id, "local_kernel");
        assert!(identity.runtime_registered);

        let envelope = build_action_envelope(
            &root,
            None,
            "atlas",
            None,
            "research",
            "web_search",
            0.25,
            Some("run_1"),
            Some("session_1"),
        )
        .expect("build envelope");
        assert_eq!(envelope.agent_id, "agent_atlas");
        assert_eq!(envelope.org_id, "org_demo");
        assert_eq!(envelope.estimated_cost_usd, 0.25);
        assert!(!envelope_input_hash(&envelope).is_empty());
    }

    #[test]
    fn resolve_identity_prefers_snapshot_restrictions_when_present() {
        let kernel =
            fake_kernel_root_with_snapshot("sanction", &["execute"], Some("restricted_execute"), &[]);
        let root = temp_path("loom-core-snapshot");
        init_workspace(&root, "shadow", Some(&kernel.display().to_string()), "org_demo")
            .expect("init workspace");

        let identity =
            resolve_agent_identity(&root, None, "sanction", None).expect("resolve identity");
        assert_eq!(identity.agent_id, "agent_atlas");
        assert_eq!(identity.restrictions, vec!["execute".to_string()]);
        assert_eq!(identity.sanction_decision, "restricted_execute");
        let preview = preview_local_sanction_controls(&identity);
        assert!(!preview.allowed);
        assert_eq!(preview.decision, "deny");
    }

    #[test]
    fn evaluate_reference_gates_uses_kernel_reference_adapter() {
        let kernel = fake_kernel_root("atlas");
        let root = temp_path("loom-core-reference");
        init_workspace(&root, "shadow", Some(&kernel.display().to_string()), "org_demo")
            .expect("init workspace");

        let allowed = build_action_envelope(
            &root,
            None,
            "atlas",
            None,
            "research",
            "web_search",
            0.25,
            Some("run_1"),
            Some("session_1"),
        )
        .expect("allowed envelope");
        let identity = resolve_agent_identity(&root, None, "atlas", None).expect("identity");
        let allowed_check =
            evaluate_reference_gates(&root, None, &identity, &allowed).expect("allowed gates");
        assert!(allowed_check.allowed);
        assert_eq!(allowed_check.stage, "ok");
        assert_eq!(allowed_check.sanction_gate_decision, "allow");
        assert_eq!(allowed_check.approval_gate_decision, "allow");
        assert_eq!(allowed_check.budget_gate_decision, "allow");

        let denied = build_action_envelope(
            &root,
            None,
            "atlas",
            None,
            "research",
            "web_search",
            1.25,
            None,
            None,
        )
        .expect("denied envelope");
        let denied_check =
            evaluate_reference_gates(&root, None, &identity, &denied).expect("denied gates");
        assert!(!denied_check.allowed);
        assert_eq!(denied_check.stage, "budget_gate");
        assert_eq!(denied_check.budget_gate_decision, "deny");
    }

    #[test]
    fn build_envelope_rejects_negative_cost() {
        let kernel = fake_kernel_root("atlas");
        let root = temp_path("loom-core-envelope-negative");
        init_workspace(&root, "shadow", Some(&kernel.display().to_string()), "org_demo")
            .expect("init workspace");
        let error = build_action_envelope(
            &root,
            None,
            "atlas",
            None,
            "research",
            "web_search",
            -0.1,
            None,
            None,
        )
        .expect_err("negative cost should fail");
        assert!(error.contains("non-negative"));
    }

    #[test]
    fn resolve_identity_does_not_force_workspace_org_hint() {
        let kernel = fake_kernel_root("atlas");
        let root = temp_path("loom-core-org-fallback");
        init_workspace(&root, "embedded", Some(&kernel.display().to_string()), "org_local")
            .expect("init workspace");

        let identity = resolve_agent_identity(&root, None, "atlas", None).expect("resolve identity");
        assert_eq!(identity.agent_id, "agent_atlas");
        assert_eq!(identity.org_id, "org_demo");
        assert_eq!(identity.max_per_run_usd, Some(0.5));
        assert_eq!(identity.restrictions, vec!["lead".to_string()]);
    }

    #[test]
    fn local_sanction_preview_allows_non_execute_restrictions() {
        let identity = AgentIdentityResolution {
            agent_id: "agent_atlas".to_string(),
            agent_name: "Atlas".to_string(),
            org_id: "org_demo".to_string(),
            role: "analyst".to_string(),
            economy_key: "atlas".to_string(),
            approval_required: false,
            max_per_run_usd: Some(0.5),
            restrictions: vec!["lead".to_string()],
            sanction_decision: "restricted".to_string(),
            runtime_id: "local_kernel".to_string(),
            runtime_label: "Local Kernel Runtime".to_string(),
            bound_org_id: "org_demo".to_string(),
            boundary_name: "workspace".to_string(),
            identity_model: "session".to_string(),
            runtime_registered: true,
            registration_status: "registered".to_string(),
            source: "kernel_agent_registry".to_string(),
        };
        let preview = preview_local_sanction_controls(&identity);
        assert!(preview.allowed);
        assert_eq!(preview.decision, "allow");
        assert_eq!(preview.reason, "no execute restriction");
    }

    #[test]
    fn local_sanction_preview_denies_execute_and_remediation_only() {
        let mut identity = AgentIdentityResolution {
            agent_id: "agent_atlas".to_string(),
            agent_name: "Atlas".to_string(),
            org_id: "org_demo".to_string(),
            role: "analyst".to_string(),
            economy_key: "atlas".to_string(),
            approval_required: false,
            max_per_run_usd: Some(0.5),
            restrictions: vec!["execute".to_string()],
            sanction_decision: "restricted_execute".to_string(),
            runtime_id: "local_kernel".to_string(),
            runtime_label: "Local Kernel Runtime".to_string(),
            bound_org_id: "org_demo".to_string(),
            boundary_name: "workspace".to_string(),
            identity_model: "session".to_string(),
            runtime_registered: true,
            registration_status: "registered".to_string(),
            source: "kernel_agent_registry".to_string(),
        };
        let preview = preview_local_sanction_controls(&identity);
        assert!(!preview.allowed);
        assert_eq!(preview.decision, "deny");
        assert!(preview.reason.contains("restricted from execute"));

        identity.restrictions = vec!["remediation_only".to_string()];
        let remediation = preview_local_sanction_controls(&identity);
        assert!(!remediation.allowed);
        assert_eq!(remediation.decision, "deny");
    }

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}",
            prefix,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn fake_kernel_root(agent_lookup: &str) -> PathBuf {
        fake_kernel_root_with_snapshot(agent_lookup, &[], None, &["lead"])
    }

    fn fake_kernel_root_with_snapshot(
        agent_lookup: &str,
        snapshot_restrictions: &[&str],
        snapshot_sanction_decision: Option<&str>,
        court_restrictions: &[&str],
    ) -> PathBuf {
        let root = temp_path("loom-core-kernel");
        let kernel_dir = root.join("kernel");
        fs::create_dir_all(&kernel_dir).expect("kernel dir");
        let snapshot_restrictions_literal = format!(
            "[{}]",
            snapshot_restrictions
                .iter()
                .map(|value| format!("{:?}", value))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let snapshot_fields = if snapshot_restrictions.is_empty() {
            String::new()
        } else if let Some(decision) = snapshot_sanction_decision {
            format!(
                "'restrictions': {restrictions}, 'sanction_decision': {decision:?}, ",
                restrictions = snapshot_restrictions_literal,
                decision = decision
            )
        } else {
            format!(
                "'restrictions': {restrictions}, ",
                restrictions = snapshot_restrictions_literal
            )
        };
        let court_restrictions_literal = format!(
            "[{}]",
            court_restrictions
                .iter()
                .map(|value| format!("{:?}", value))
                .collect::<Vec<_>>()
                .join(", ")
        );
        fs::write(
            kernel_dir.join("runtimes.json"),
            format!(
                "{{\n  \"runtimes\": {{\n    \"local_kernel\": {{\"id\": \"local_kernel\", \"label\": \"Local Kernel Runtime\"}},\n    \"meridian_loom\": {{\"status\": \"planned\", \"notes\": \"test note\", \"contract_compliance\": {{\"agent_identity\": null, \"action_envelope\": null, \"cost_attribution\": null, \"approval_hook\": null, \"audit_emission\": null, \"sanction_controls\": null, \"budget_gate\": null}}}}\n  }}\n}}\n"
            ),
        )
        .expect("write runtimes");
        fs::write(
            kernel_dir.join("agent_registry.py"),
            format!(
                "import json, sys\nagent_id = sys.argv[sys.argv.index('--agent_id') + 1]\norg_id = 'org_demo'\nif '--org_id' in sys.argv:\n    org_id = sys.argv[sys.argv.index('--org_id') + 1]\nif agent_id in ('{lookup}', 'agent_atlas', 'Atlas'):\n    print(json.dumps({{'id': 'agent_atlas', 'name': 'Atlas', 'org_id': org_id, 'role': 'analyst', 'economy_key': '{lookup}', 'approval_required': False, {snapshot_fields}'budget': {{'max_per_run_usd': 0.5}}, 'runtime_binding': {{'runtime_id': 'local_kernel', 'runtime_label': 'Local Kernel Runtime', 'bound_org_id': org_id, 'boundary_name': 'workspace', 'identity_model': 'session', 'runtime_registered': True, 'registration_status': 'registered'}}}}, indent=2))\nelse:\n    print(f'Not found: {{agent_id}}')\n",
                lookup = agent_lookup,
                snapshot_fields = snapshot_fields
            ),
        )
        .expect("write agent_registry");
        fs::write(
            kernel_dir.join("court.py"),
            format!(
                "def get_restrictions(agent_id, org_id=None):\n    if agent_id in ('atlas', 'agent_atlas'):\n        return {restrictions}\n    return []\n",
                restrictions = court_restrictions_literal
            ),
        )
        .expect("write court");
        let adapters_dir = kernel_dir.join("adapters");
        fs::create_dir_all(&adapters_dir).expect("adapters dir");
        fs::write(adapters_dir.join("__init__.py"), "").expect("write adapters init");
        fs::write(
            kernel_dir.join("authority.py"),
            "def check_authority(agent_id, action, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write authority");
        fs::write(
            kernel_dir.join("treasury.py"),
            "def check_budget(agent_id, cost_usd, org_id=None):\n    if cost_usd > 0.5:\n        return False, 'below reserve'\n    return True, 'ok'\n",
        )
        .expect("write treasury");
        fs::write(
            kernel_dir.join("audit.py"),
            "def log_event(*args, **kwargs):\n    return 'evt_fake'\n",
        )
        .expect("write audit");
        fs::write(
            kernel_dir.join("metering.py"),
            "def record(*args, **kwargs):\n    return 'meter_fake'\n",
        )
        .expect("write metering");
        fs::write(
            adapters_dir.join("openclaw_compatible.py"),
            "from audit import log_event\nfrom authority import check_authority\nfrom court import get_restrictions\nfrom metering import record as meter_record\nfrom treasury import check_budget\n\n\
def pre_session_check(org_id, agent_id):\n    restrictions = list(get_restrictions(agent_id, org_id=org_id) or [])\n    if 'execute' in restrictions or 'remediation_only' in restrictions:\n        return {'allowed': False, 'reason': f'Agent {agent_id} is restricted from execute', 'restrictions': restrictions}\n    return {'allowed': True, 'reason': 'ok', 'restrictions': restrictions}\n\n\
def pre_action_check(org_id, envelope):\n    session_gate = pre_session_check(org_id, envelope['agent_id'])\n    if not session_gate['allowed']:\n        return {'allowed': False, 'reason': session_gate['reason'], 'stage': 'sanction_controls', 'envelope': envelope, 'restrictions': session_gate['restrictions']}\n    allowed, reason = check_authority(envelope['agent_id'], envelope['action_type'], org_id=org_id)\n    if not allowed:\n        return {'allowed': False, 'reason': reason, 'stage': 'approval_hook', 'envelope': envelope, 'restrictions': session_gate['restrictions']}\n    estimated_cost = envelope.get('estimated_cost_usd', 0.0)\n    if estimated_cost > 0:\n        allowed, reason = check_budget(envelope['agent_id'], estimated_cost, org_id=org_id)\n        if not allowed:\n            return {'allowed': False, 'reason': reason, 'stage': 'budget_gate', 'envelope': envelope, 'restrictions': session_gate['restrictions']}\n    return {'allowed': True, 'reason': 'ok', 'stage': 'ok', 'envelope': envelope, 'restrictions': session_gate['restrictions']}\n",
        )
        .expect("write adapter");
        root
    }
}
