use std::io::IsTerminal;

use crate::*;
use loom_core::provider_auth_store::{
    list_provider_auth_profiles, mark_provider_auth_profile_failure,
    mark_provider_auth_profile_used, render_provider_auth_profile_human,
    render_provider_auth_profile_json, render_provider_auth_profiles_human,
    render_provider_auth_profiles_json, render_provider_auth_store_human,
    render_provider_auth_store_json, provider_auth_store_overview,
};
use loom_core::provider_router::{
    provider_auth_status, provider_plane_summary, render_provider_auth_human,
    render_provider_auth_json, render_provider_plane_human, render_provider_plane_json,
    render_provider_route_human, render_provider_route_json, resolve_provider_route,
    ProviderRouteIntent,
};

pub(crate) fn handle_provider(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("status") => handle_provider_status(&args[1..]),
        Some("route") => handle_provider_route(&args[1..]),
        Some("auth") => handle_provider_auth(&args[1..]),
        Some("profiles") => handle_provider_profiles(&args[1..]),
        Some("mark-used") => handle_provider_mark_used(&args[1..]),
        Some("mark-failure") => handle_provider_mark_failure(&args[1..]),
        _ => Err("provider supports 'status', 'route', 'auth', 'profiles', 'mark-used', and 'mark-failure'".to_string()),
    }
}

fn output_format(args: &[String]) -> String {
    take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    })
}

fn handle_provider_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let summary = provider_plane_summary(Some(&root))?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_provider_plane_human(&summary));
        }
        _ => print!("{}", render_provider_plane_json(&summary)),
    }
    Ok(())
}

fn handle_provider_route(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let capability = take_value(args, "--capability").unwrap_or_else(|| "loom.llm.inference.v1".to_string());
    let requested_model = take_value(args, "--model").unwrap_or_default();
    let mut intent = ProviderRouteIntent::for_capability(&capability, &requested_model);
    if let Some(agent_id) = take_value(args, "--agent-id") {
        intent = intent.with_agent_id(&agent_id);
    }
    if let Some(org_id) = take_value(args, "--org-id") {
        intent = intent.with_org_id(&org_id);
    }
    if let Some(profile) = take_value(args, "--profile") {
        intent = intent.with_preferred_profile_name(&profile);
    }
    let route = resolve_provider_route(Some(&root), &intent)?;
    if format == "json" {
        print!("{}", render_provider_route_json(&route));
    } else {
        print_startup_banner();
        print_human(&render_provider_route_human(&route));
    }
    Ok(())
}

fn handle_provider_auth(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let profile = take_value(args, "--profile");
    let status = provider_auth_status(Some(&root), profile.as_deref())?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_provider_auth_human(&status));
        }
        _ => print!("{}", render_provider_auth_json(&status)),
    }
    Ok(())
}

fn handle_provider_profiles(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let store = provider_auth_store_overview(&root)?;
    let records = list_provider_auth_profiles(&root)?;
    if let Some(profile_name) = take_value(args, "--profile") {
        let record = records
            .into_iter()
            .find(|record| record.profile_name == profile_name)
            .ok_or_else(|| format!("provider auth profile '{}' was not found", profile_name))?;
        match format.as_str() {
            "human" => {
                print_startup_banner();
                print_human(&render_provider_auth_profile_human(&record));
            }
            _ => print!("{}", render_provider_auth_profile_json(&record)),
        }
        return Ok(());
    }

    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_provider_auth_store_human(&store));
            print_human(&render_provider_auth_profiles_human(&records));
        }
        _ => {
            let json = serde_json::json!({
                "store": serde_json::from_str::<serde_json::Value>(&render_provider_auth_store_json(&store)).unwrap_or_else(|_| serde_json::json!({})),
                "profiles": serde_json::from_str::<serde_json::Value>(&render_provider_auth_profiles_json(&records)).unwrap_or_else(|_| serde_json::json!([])),
            });
            print!("{}\n", serde_json::to_string_pretty(&json).map_err(|error| error.to_string())?);
        }
    }
    Ok(())
}

fn handle_provider_mark_used(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let profile = required_flag(args, "--profile")?;
    let format = output_format(args);
    let record = mark_provider_auth_profile_used(&root, &profile)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_provider_auth_profile_human(&record));
        }
        _ => print!("{}", render_provider_auth_profile_json(&record)),
    }
    Ok(())
}

fn handle_provider_mark_failure(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let profile = required_flag(args, "--profile")?;
    let format = output_format(args);
    let reason = take_value(args, "--reason");
    let cooldown_ms = take_value(args, "--cooldown-ms").map(|raw| {
        raw.parse::<u64>()
            .map_err(|_| format!("invalid --cooldown-ms '{}'", raw))
    }).transpose()?;
    let record = mark_provider_auth_profile_failure(&root, &profile, reason.as_deref(), cooldown_ms)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_provider_auth_profile_human(&record));
        }
        _ => print!("{}", render_provider_auth_profile_json(&record)),
    }
    Ok(())
}
