use std::io::IsTerminal;

use crate::*;
use loom_core::provider_router::{
    provider_plane_summary, render_provider_plane_human, render_provider_plane_json,
    render_provider_route_human, render_provider_route_json, resolve_provider_route,
    ProviderRouteIntent,
};

pub(crate) fn handle_provider(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("status") => handle_provider_status(&args[1..]),
        Some("route") => handle_provider_route(&args[1..]),
        _ => Err("provider supports 'status' and 'route'".to_string()),
    }
}

fn handle_provider_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
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
    let requested_model = take_value(args, "--model").unwrap_or_else(|| "gpt-3.5-turbo".to_string());
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
