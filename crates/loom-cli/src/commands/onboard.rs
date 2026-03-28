use std::io::IsTerminal;

use crate::*;
use loom_core::agent_runtime::agent_runtime_overview;
use loom_core::provider_router::{
    provider_auth_status, provider_plane_summary, resolve_provider_route, ProviderRouteIntent,
};
use serde_json::json;

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
    let org_id = take_value(args, "--org-id").unwrap_or_else(|| "default".to_string());
    let kernel_path = take_value(args, "--kernel-path");

    let existing = root.join("loom.toml").exists();
    let config = if existing {
        read_config(&root)?
    } else {
        init_workspace(&root, &requested_mode, kernel_path.as_deref(), &org_id)?
    };

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

    let codex_ready = codex_status.as_ref().map(|status| status.ready).unwrap_or(false);
    let codex_path = codex_status
        .as_ref()
        .and_then(|status| status.credential_path.clone());
    let codex_detail = codex_status
        .as_ref()
        .map(|status| status.detail.clone())
        .unwrap_or_else(|| "Codex OAuth is not configured yet".to_string());

    if format == "json" {
        print!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "root": root.display().to_string(),
                "config_status": if existing { "reused" } else { "initialized" },
                "mode": config.mode,
                "org_id": config.org_id,
                "provider_profiles": provider_summary.profile_count,
                "agent_profiles": runtime_overview.profile_count,
                "codex_auth_ready": codex_ready,
                "codex_credential_path": codex_path,
                "codex_detail": codex_detail,
                "manager_route": manager_route.as_ref().map(|route| json!({
                    "profile": route.profile_name,
                    "kind": route.profile_kind.label(),
                    "endpoint": route.endpoint_url.as_str(),
                    "model": route.model,
                })),
                "pulse_route": pulse_route.as_ref().map(|route| json!({
                    "profile": route.profile_name,
                    "kind": route.profile_kind.label(),
                    "endpoint": route.endpoint_url.as_str(),
                    "model": route.model,
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

    print_startup_banner();
    print_human(&format!(
        "Meridian Loom // ONBOARD
=========================
root:                {}
config_status:       {}
mode:                {}
org_id:              {}
provider_profiles:   {}
agent_profiles:      {}
codex_auth_ready:    {}
codex_auth_path:     {}
codex_detail:        {}
manager_route:       {}
manager_endpoint:    {}
manager_model:       {}
pulse_route:         {}
pulse_model:         {}
next_step:           loom doctor --root {} --format human
",
        root.display(),
        if existing { "reused" } else { "initialized" },
        config.mode,
        config.org_id,
        provider_summary.profile_count,
        runtime_overview.profile_count,
        if codex_ready { "yes" } else { "no" },
        codex_path.as_deref().unwrap_or("(none)"),
        codex_detail,
        manager_profile,
        manager_endpoint,
        manager_model,
        pulse_profile,
        pulse_model,
        root.display(),
    ));

    Ok(())
}
