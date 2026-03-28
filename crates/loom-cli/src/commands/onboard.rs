use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::*;
use loom_core::agent_runtime::agent_runtime_overview;
use loom_core::channels::sync_channel_registry;
use loom_core::skills::sync_skill_registry;
use loom_core::onboarding::{
    derive_service_http_address, ensure_onboard_manifest, load_onboard_manifest,
    onboard_manifest_path, onboard_overview, write_onboard_manifest, OnboardManifest,
};
use loom_core::provider_router::{
    provider_auth_status, provider_plane_summary, resolve_provider_route, ProviderRouteIntent,
};
use serde_json::{json, Value};

pub(crate) fn handle_onboard(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let requested_mode = take_value(args, "--mode").unwrap_or_else(|| "embedded".to_string());
    let requested_org_id = take_value(args, "--org-id").unwrap_or_else(|| "default".to_string());
    let kernel_path = take_value(args, "--kernel-path");
    let interactive = format == "human"
        && !has_flag(args, "--non-interactive")
        && std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal();

    let had_existing_config = root.join("loom.toml").exists();
    let mut config_action = take_value(args, "--config-action").unwrap_or_else(|| {
        if had_existing_config {
            "keep".to_string()
        } else {
            "init".to_string()
        }
    });
    if had_existing_config && has_setup_overrides(args) && config_action == "keep" {
        config_action = "modify".to_string();
    }
    if interactive && had_existing_config && !has_setup_overrides(args) {
        config_action = prompt_choice(
            "Existing Loom runtime detected. Choose action",
            &config_action,
            &["keep", "modify", "reset"],
        )?;
    }

    if had_existing_config && config_action == "reset" {
        let _ = fs::remove_file(root.join("loom.toml"));
        let _ = fs::remove_file(onboard_manifest_path(&root));
    }

    let config_status = if root.join("loom.toml").exists() {
        if config_action == "modify" {
            "modified"
        } else {
            "reused"
        }
    } else {
        "initialized"
    };

    let mut config = if root.join("loom.toml").exists() {
        read_config(&root)?
    } else {
        init_workspace(&root, &requested_mode, kernel_path.as_deref(), &requested_org_id)?
    };

    if config_action != "keep" {
        if let Some(mode) = take_value(args, "--mode") {
            config.mode = mode;
        }
        if let Some(org_id) = take_value(args, "--org-id") {
            config.org_id = org_id;
        }
        if let Some(kernel_path) = kernel_path.as_deref() {
            config.kernel_path = kernel_path.to_string();
        }
    }

    let _ = ensure_onboard_manifest(&root, &config)?;
    let mut manifest = load_onboard_manifest(&root)?;

    if interactive && config_action != "keep" {
        apply_interactive_overrides(&mut manifest)?;
    }
    apply_cli_overrides(args, &mut manifest)?;

    if manifest.gateway_auth_mode == "none" && manifest.gateway_token_env.trim().is_empty() {
        manifest.gateway_token_env = config.service_token_env.clone();
    }
    manifest.last_action = if had_existing_config {
        config_action.clone()
    } else {
        "init".to_string()
    };
    manifest.last_run_at = current_unix();
    manifest.last_run_mode = config.mode.clone();

    config.service_http_address =
        derive_service_http_address(&manifest.gateway_bind, manifest.gateway_port);
    if manifest.gateway_auth_mode != "none" {
        config.service_token_env = manifest.gateway_token_env.clone();
    }
    loom_core::write_config(&root, &config)?;
    let manifest_path = write_onboard_manifest(&root, &manifest)?;
    let channel_summary = sync_channel_registry(&root)?;
    let skill_summary = sync_skill_registry(&root)?;

    let provider_summary = provider_plane_summary(Some(&root))?;
    let runtime_overview = agent_runtime_overview(&root)?;
    let codex_status = provider_auth_status(Some(&root), Some("manager_frontier")).ok();
    let manager_route = resolve_provider_route(
        Some(&root),
        &ProviderRouteIntent::llm_inference("").with_agent_id("leviathann"),
    )
    .ok();
    let pulse_route = resolve_provider_route(
        Some(&root),
        &ProviderRouteIntent::llm_inference("").with_agent_id("pulse"),
    )
    .ok();
    let start_daemon_requested = if interactive && config_action != "keep" && manifest.daemon_enabled {
        prompt_bool("Start supervisor daemon now", has_flag(args, "--start-daemon"))?
    } else {
        has_flag(args, "--start-daemon")
    };
    let daemon_snapshot = if start_daemon_requested && manifest.daemon_enabled && manifest.daemon_manager == "supervisor" {
        let snapshot = start_supervisor_daemon(&root, kernel_path.as_deref())?;
        manifest.daemon_state = daemon_state_from_snapshot(&snapshot);
        write_onboard_manifest(&root, &manifest)?;
        Some(snapshot)
    } else {
        None
    };

    let health_requested = if interactive && config_action != "keep" {
        prompt_bool("Run post-setup health check", !has_flag(args, "--skip-health-check"))?
    } else {
        !has_flag(args, "--skip-health-check")
    };
    let health_snapshot = if health_requested {
        let (healthy, report) = health(&root)?;
        Some((healthy, report))
    } else {
        None
    };

    let overview = onboard_overview(&root)?;

    let codex_ready = codex_status.as_ref().map(|status| status.ready).unwrap_or(false);
    let codex_path = codex_status.as_ref().and_then(|status| status.credential_path.clone());
    let codex_detail = codex_status
        .as_ref()
        .map(|status| status.detail.clone())
        .unwrap_or_else(|| "Codex OAuth is not configured yet".to_string());

    if format == "json" {
        print!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "root": root.display().to_string(),
                "config_status": config_status,
                "config_action": config_action,
                "mode": config.mode,
                "org_id": config.org_id,
                "provider_profiles": provider_summary.profile_count,
                "agent_profiles": runtime_overview.profile_count,
                "codex_auth_ready": codex_ready,
                "codex_credential_path": codex_path,
                "codex_detail": codex_detail,
                "manifest_path": manifest_path.display().to_string(),
                "onboard": {
                    "gateway": overview.gateway_summary,
                    "telegram": overview.telegram_summary,
                    "daemon": overview.daemon_summary,
                    "skills": overview.skills_summary,
                    "remote_mode": overview.remote_mode,
                    "channels": {
                        "total_count": channel_summary.total_count,
                        "enabled_count": channel_summary.enabled_count,
                        "channel_ids": channel_summary.channel_ids.clone(),
                    },
                    "skills_runtime": {
                        "total_count": skill_summary.total_count,
                        "enabled_count": skill_summary.enabled_count,
                        "default_count": skill_summary.default_count,
                        "imported_count": skill_summary.imported_count,
                        "skill_ids": skill_summary.skill_ids.clone(),
                    },
                },
                "manager_route": manager_route.as_ref().map(route_json),
                "pulse_route": pulse_route.as_ref().map(route_json),
                "daemon": daemon_snapshot,
                "health": health_snapshot.as_ref().map(|(healthy, report)| json!({
                    "status": if *healthy { "healthy" } else { "degraded" },
                    "report": report,
                })),
            }))
            .map_err(|error| error.to_string())?
        );
        println!();
        return Ok(());
    }

    let manager_profile = manager_route
        .as_ref()
        .map(|route| format!("{} ({})", route.profile_name, route.profile_kind.label()))
        .unwrap_or_else(|| "(unresolved)".to_string());
    let manager_endpoint = manager_route
        .as_ref()
        .map(|route| route.endpoint_url.to_string())
        .unwrap_or_else(|| "(unresolved)".to_string());
    let manager_model = manager_route
        .as_ref()
        .map(|route| route.model.clone())
        .unwrap_or_else(|| "(unresolved)".to_string());
    let pulse_profile = pulse_route
        .as_ref()
        .map(|route| format!("{} ({})", route.profile_name, route.profile_kind.label()))
        .unwrap_or_else(|| "(unresolved)".to_string());
    let pulse_model = pulse_route
        .as_ref()
        .map(|route| route.model.clone())
        .unwrap_or_else(|| "(unresolved)".to_string());
    let daemon_summary = daemon_snapshot
        .as_ref()
        .map(render_daemon_summary)
        .unwrap_or_else(|| overview.daemon_summary.clone());
    let health_summary = health_snapshot
        .as_ref()
        .map(|(healthy, _)| if *healthy { "healthy" } else { "degraded" }.to_string())
        .unwrap_or_else(|| "skipped".to_string());

    print_startup_banner();
    print_human(&format!(
        "Meridian Loom // ONBOARD
=========================
root:                {}
config_status:       {}
config_action:       {}
mode:                {}
org_id:              {}
provider_profiles:   {}
agent_profiles:      {}
codex_auth_ready:    {}
codex_auth_path:     {}
codex_detail:        {}
gateway:             {}
telegram:            {}
channels:            total={} enabled={} ids={}
skills:              {}
skills_runtime:      total={} enabled={} defaults={} imported={} ids={}
daemon:              {}
health:              {}
manager_route:       {}
manager_endpoint:    {}
manager_model:       {}
pulse_route:         {}
pulse_model:         {}
manifest:            {}
next_step:           loom doctor --root {} --format human
",
        root.display(),
        config_status,
        config_action,
        config.mode,
        config.org_id,
        provider_summary.profile_count,
        runtime_overview.profile_count,
        if codex_ready { "yes" } else { "no" },
        codex_path.as_deref().unwrap_or("(none)"),
        codex_detail,
        overview.gateway_summary,
        overview.telegram_summary,
        channel_summary.total_count,
        channel_summary.enabled_count,
        if channel_summary.channel_ids.is_empty() { "(none)".to_string() } else { channel_summary.channel_ids.join(",") },
        overview.skills_summary,
        skill_summary.total_count,
        skill_summary.enabled_count,
        skill_summary.default_count,
        skill_summary.imported_count,
        if skill_summary.skill_ids.is_empty() { "(none)".to_string() } else { skill_summary.skill_ids.join(",") },
        daemon_summary,
        health_summary,
        manager_profile,
        manager_endpoint,
        manager_model,
        pulse_profile,
        pulse_model,
        manifest_path.display(),
        root.display(),
    ));

    Ok(())
}

fn has_setup_overrides(args: &[String]) -> bool {
    const FLAGS: &[&str] = &[
        "--gateway-port",
        "--gateway-bind",
        "--gateway-auth-mode",
        "--gateway-token-env",
        "--tailscale-mode",
        "--telegram-enabled",
        "--telegram-token-env",
        "--telegram-dm-policy",
        "--telegram-group-policy",
        "--telegram-streaming",
        "--dm-scope",
        "--daemon-enabled",
        "--daemon-manager",
        "--remote-mode",
        "--skills-node-manager",
        "--skills-entry",
        "--start-daemon",
        "--skip-health-check",
        "--mode",
        "--org-id",
        "--kernel-path",
    ];
    FLAGS.iter().any(|flag| has_flag(args, flag))
}

fn apply_cli_overrides(args: &[String], manifest: &mut OnboardManifest) -> LoomResult<()> {
    if let Some(port) = take_value(args, "--gateway-port") {
        manifest.gateway_port = parse_u16("--gateway-port", &port)?;
    }
    if let Some(bind) = take_value(args, "--gateway-bind") {
        manifest.gateway_bind = bind;
    }
    if let Some(mode) = take_value(args, "--gateway-auth-mode") {
        manifest.gateway_auth_mode = mode;
    }
    if let Some(token_env) = take_value(args, "--gateway-token-env") {
        manifest.gateway_token_env = token_env;
    }
    if let Some(mode) = take_value(args, "--tailscale-mode") {
        manifest.gateway_tailscale_mode = mode;
    }
    if let Some(value) = take_value(args, "--telegram-enabled") {
        manifest.telegram_enabled = parse_bool_flag("--telegram-enabled", &value)?;
    }
    if let Some(token_env) = take_value(args, "--telegram-token-env") {
        manifest.telegram_token_env = token_env;
    }
    if let Some(value) = take_value(args, "--telegram-dm-policy") {
        manifest.telegram_dm_policy = value;
    }
    if let Some(value) = take_value(args, "--telegram-group-policy") {
        manifest.telegram_group_policy = value;
    }
    if let Some(value) = take_value(args, "--telegram-streaming") {
        manifest.telegram_streaming = value;
    }
    if let Some(value) = take_value(args, "--dm-scope") {
        manifest.session_dm_scope = value;
    }
    if let Some(value) = take_value(args, "--daemon-enabled") {
        manifest.daemon_enabled = parse_bool_flag("--daemon-enabled", &value)?;
        if !manifest.daemon_enabled {
            manifest.daemon_state = "disabled".to_string();
        }
    }
    if let Some(value) = take_value(args, "--daemon-manager") {
        manifest.daemon_manager = value;
    }
    if let Some(value) = take_value(args, "--remote-mode") {
        manifest.remote_mode = value;
    }
    if let Some(value) = take_value(args, "--skills-node-manager") {
        manifest.skills_node_manager = value;
    }
    let entries = take_values(args, "--skills-entry");
    if !entries.is_empty() {
        manifest.skills_entries = entries;
    }
    Ok(())
}

fn apply_interactive_overrides(manifest: &mut OnboardManifest) -> LoomResult<()> {
    manifest.remote_mode = prompt_text("Remote mode", &manifest.remote_mode)?;
    manifest.gateway_bind = prompt_choice(
        "Gateway bind",
        &manifest.gateway_bind,
        &["loopback", "all"],
    )?;
    manifest.gateway_port = parse_u16(
        "gateway port",
        &prompt_text("Gateway port", &manifest.gateway_port.to_string())?,
    )?;
    manifest.gateway_auth_mode = prompt_choice(
        "Gateway auth mode",
        &manifest.gateway_auth_mode,
        &["token", "none"],
    )?;
    manifest.gateway_token_env = prompt_text("Gateway token env", &manifest.gateway_token_env)?;
    manifest.gateway_tailscale_mode = prompt_choice(
        "Gateway tailscale mode",
        &manifest.gateway_tailscale_mode,
        &["off", "on"],
    )?;
    manifest.telegram_enabled = prompt_bool("Enable Telegram channel", manifest.telegram_enabled)?;
    if manifest.telegram_enabled {
        manifest.telegram_token_env = prompt_text("Telegram token env", &manifest.telegram_token_env)?;
        manifest.telegram_dm_policy = prompt_text("Telegram DM policy", &manifest.telegram_dm_policy)?;
        manifest.telegram_group_policy = prompt_text("Telegram group policy", &manifest.telegram_group_policy)?;
        manifest.telegram_streaming = prompt_text("Telegram streaming", &manifest.telegram_streaming)?;
    }
    manifest.session_dm_scope = prompt_text("Session DM scope", &manifest.session_dm_scope)?;
    manifest.daemon_enabled = prompt_bool("Enable supervisor daemon", manifest.daemon_enabled)?;
    manifest.daemon_manager = prompt_choice(
        "Daemon manager",
        &manifest.daemon_manager,
        &["supervisor"],
    )?;
    manifest.skills_node_manager = prompt_choice(
        "Skills node manager",
        &manifest.skills_node_manager,
        &["npm", "pnpm", "bun"],
    )?;
    let skill_entries = prompt_text(
        "Default skills entries (comma separated)",
        &manifest.skills_entries.join(","),
    )?;
    manifest.skills_entries = skill_entries
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect();
    Ok(())
}

fn prompt_text(label: &str, default: &str) -> LoomResult<String> {
    print!("{} [{}]: ", label, default);
    io::stdout().flush().map_err(|error| error.to_string())?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|error| error.to_string())?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn prompt_choice(label: &str, default: &str, allowed: &[&str]) -> LoomResult<String> {
    let rendered = allowed.join("/");
    loop {
        let value = prompt_text(&format!("{} ({})", label, rendered), default)?;
        if allowed.iter().any(|candidate| *candidate == value) {
            return Ok(value);
        }
        eprintln!("invalid value '{}'; expected one of {}", value, rendered);
    }
}

fn prompt_bool(label: &str, default: bool) -> LoomResult<bool> {
    let default_rendered = if default { "yes" } else { "no" };
    loop {
        let value = prompt_text(&format!("{} (yes/no)", label), default_rendered)?;
        match parse_bool_flag(label, &value) {
            Ok(parsed) => return Ok(parsed),
            Err(error) => eprintln!("{}", error),
        }
    }
}

fn parse_bool_flag(label: &str, raw: &str) -> LoomResult<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" | "enabled" => Ok(true),
        "0" | "false" | "no" | "n" | "off" | "disabled" => Ok(false),
        _ => Err(format!("{} expects yes/no/true/false", label)),
    }
}

fn parse_u16(label: &str, raw: &str) -> LoomResult<u16> {
    raw.trim()
        .parse::<u16>()
        .map_err(|error| format!("{} expects a valid u16: {}", label, error))
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn route_json(route: &loom_core::provider_router::ResolvedProviderRoute) -> Value {
    json!({
        "profile": route.profile_name,
        "kind": route.profile_kind.label(),
        "endpoint": route.endpoint_url.as_str(),
        "model": route.model,
        "matched_rule": route.matched_rule,
    })
}

fn start_supervisor_daemon(root: &Path, kernel_path: Option<&str>) -> LoomResult<Value> {
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let mut command = Command::new(exe);
    command
        .arg("supervisor")
        .arg("daemon")
        .arg("start")
        .arg("--root")
        .arg(root)
        .arg("--format")
        .arg("json");
    if let Some(kernel_path) = kernel_path {
        command.arg("--kernel-path").arg(kernel_path);
    }
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if !stderr.is_empty() { stderr } else { stdout });
    }
    serde_json::from_slice::<Value>(&output.stdout).map_err(|error| error.to_string())
}

fn daemon_state_from_snapshot(snapshot: &Value) -> String {
    if snapshot.get("running").and_then(Value::as_bool).unwrap_or(false) {
        "running".to_string()
    } else {
        snapshot
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("configured")
            .to_string()
    }
}

fn render_daemon_summary(snapshot: &Value) -> String {
    let status = daemon_state_from_snapshot(snapshot);
    let pid = snapshot
        .get("pid")
        .and_then(Value::as_u64)
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!("supervisor {} pid={}", status, pid)
}
