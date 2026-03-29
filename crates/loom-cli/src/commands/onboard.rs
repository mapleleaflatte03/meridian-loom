use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::*;
use loom_core::agent_runtime::agent_runtime_overview;
use loom_core::bindings::sync_binding_registry;
use loom_core::channels::sync_channel_registry;
use loom_core::gateway_runtime::sync_gateway_runtime;
use loom_core::skills::sync_skill_registry;
use loom_core::service_runtime::sync_service_runtime;
use loom_core::service_ingress_runtime::sync_service_ingress_runtime;
use loom_core::schedules::{schedule_overview, sync_schedule_registry};
use loom_core::onboarding::{
    derive_service_http_address, detect_setup_state, ensure_onboard_manifest, load_onboard_manifest,
    onboard_manifest_path, onboard_overview, onboard_path_hint, write_onboard_manifest,
    OnboardManifest, SetupState,
};
use loom_core::provider_auth_store::{provider_auth_store_overview, sync_provider_auth_store};
use loom_core::provider_router::{
    configure_onboard_provider_profile, configure_onboard_provider_routes,
    default_codex_auth_path_hint, load_provider_profiles, provider_auth_status,
    provider_plane_summary, resolve_provider_route, shared_codex_auth_path_hint,
    OnboardProviderRouteConfig, ProviderAuthMode, ProviderKind, ProviderRouteIntent,
};
use serde_json::{json, Value};

pub(crate) fn handle_onboard(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_onboard_help();
        return Ok(());
    }

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

    let setup_state = detect_setup_state(&root);
    let had_existing_config = root.join("loom.toml").exists();
    let had_existing_manifest = onboard_manifest_path(&root).exists();
    let existing_manifest_action = if had_existing_manifest {
        load_onboard_manifest(&root)
            .ok()
            .map(|manifest| manifest.last_action)
    } else {
        None
    };
    let is_fresh_runtime = had_existing_config
        && existing_manifest_action.as_deref() == Some("initialized");
    let mut config_action = take_value(args, "--config-action").unwrap_or_else(|| {
        if is_fresh_runtime {
            "setup".to_string()
        } else if had_existing_config {
            "keep".to_string()
        } else {
            "init".to_string()
        }
    });
    if had_existing_config && has_setup_overrides(args) && config_action == "keep" {
        config_action = "modify".to_string();
    }
    if interactive && had_existing_config && had_existing_manifest && !is_fresh_runtime && !has_setup_overrides(args) {
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
        if !had_existing_config || is_fresh_runtime {
            "initialized"
        } else if config_action == "modify" {
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
    let current_brain = current_manager_brain(&root, &manifest);
    let explicit_manager_model = take_value(args, "--manager-model");
    let explicit_codex_auth_source = take_value(args, "--codex-auth-source");
    let explicit_codex_auth_path = take_value(args, "--codex-auth-path");
    let mut manager_lane = take_value(args, "--manager-lane")
        .unwrap_or_else(|| current_brain.lane.clone());
    let mut manager_model = explicit_manager_model
        .clone()
        .unwrap_or_else(|| current_brain.model.clone());
    let mut codex_auth_source = explicit_codex_auth_source
        .clone()
        .unwrap_or_else(|| current_brain.codex_auth_source.clone());
    let mut codex_auth_path = if explicit_codex_auth_path.is_some() {
        explicit_codex_auth_path.clone()
    } else if explicit_codex_auth_source.is_some() {
        None
    } else {
        current_brain.codex_auth_path.clone()
    };
    if explicit_codex_auth_path.is_some() && explicit_codex_auth_source.is_none() {
        codex_auth_source = "path".to_string();
    }
    let mut banner_rendered = false;
    let mut interactive_provider_config: Option<OnboardProviderRouteConfig> = None;

    if interactive {
        print_startup_banner();
        banner_rendered = true;
        print_human(&format!(
            "Meridian Loom // SETUP
=======================
Choose your manager brain, edge bindings, and runtime defaults. Meridian will scaffold the governed runtime, but the end-user still owns the final configuration.

setup_state:         {}
hint:                {}

",
            setup_state_label(&setup_state),
            onboard_path_hint(&setup_state),
        ));
    } else if format == "human" {
        print_startup_banner();
        banner_rendered = true;
        print_human(&format!(
            "Meridian Loom // SETUP (non-interactive)\n\
setup_state:         {}\n\
hint:                {}\n\n",
            setup_state_label(&setup_state),
            onboard_path_hint(&setup_state),
        ));
    }

    if interactive && config_action != "keep" {
        // Security acknowledgment
        print_human(
            "SECURITY NOTICE\n\
             ===============\n\
             Meridian Loom governs autonomous agent actions on your behalf.\n\
             All pipeline runs are subject to the constitutional contract.\n\
             Actions are audited and cost-attributed to the owning org.\n\
             Provider credentials are stored locally under the runtime root.\n\
             No telemetry leaves this host without explicit configuration.\n",
        );
        let ack = prompt_bool("Acknowledge and continue", true)?;
        if !ack {
            return Err("Setup cancelled — security acknowledgment declined.".to_string());
        }

        // Quickstart vs manual mode
        let setup_mode = prompt_choice(
            "Setup mode",
            "quickstart",
            &["quickstart", "manual"],
        )?;
        let default_provider_choice = default_provider_choice_for_state(
            &manager_lane,
            current_brain.provider_kind.as_ref(),
        );

        if setup_mode == "quickstart" {
            print_quickstart_summary_card(&manifest, &manager_model);
            print_setup_stage(
                1,
                5,
                "Provider",
                "Choose the inference path Meridian should wire now. QuickStart keeps the edge, daemon, and recurring defaults intact.",
            );
            let provider_choice = prompt_choice(
                "Inference provider",
                default_provider_choice,
                &[
                    "loom_codex",
                    "local_ollama",
                    "openai_compatible",
                    "custom_endpoint",
                    "local_only",
                ],
            )?;
            let selection = prompt_provider_setup(
                &provider_choice,
                &manager_model,
                &codex_auth_source,
                codex_auth_path.clone(),
                false,
            )?;
            manager_lane = selection.manager_lane;
            manager_model = selection.manager_model;
            codex_auth_source = selection.codex_auth_source;
            codex_auth_path = selection.codex_auth_path;
            interactive_provider_config = selection.provider_config;
            print_human(
                "QuickStart defaults
-------------------
Meridian will now confirm the local edge, daemon, and built-in runtime defaults before writing the runtime root.

",
            );
        } else {
            // Manual: full interactive flow
            print_setup_stage(
                1,
                5,
                "Manager provider",
                "Pick the provider, model, and account Meridian should use for Leviathann.",
            );
            let provider_choice = prompt_choice(
                "Inference provider",
                default_provider_choice,
                &[
                    "loom_codex",
                    "local_ollama",
                    "openai_compatible",
                    "custom_endpoint",
                    "local_only",
                ],
            )?;
            let selection = prompt_provider_setup(
                &provider_choice,
                &manager_model,
                &codex_auth_source,
                codex_auth_path.clone(),
                true,
            )?;
            manager_lane = selection.manager_lane;
            manager_model = selection.manager_model;
            codex_auth_source = selection.codex_auth_source;
            codex_auth_path = selection.codex_auth_path;
            interactive_provider_config = selection.provider_config;
    }
    apply_interactive_overrides(&mut manifest)?;
    }
    apply_cli_overrides(args, &mut manifest)?;
    let uses_codex_auth = interactive_provider_config
        .as_ref()
        .map(|config| matches!(config.kind, ProviderKind::OpenAiCodex))
        .unwrap_or(true);
    let (codex_auth_source, codex_auth_path) = if uses_codex_auth {
        normalize_codex_auth_selection(&manager_lane, &codex_auth_source, codex_auth_path)?
    } else {
        ("none".to_string(), None)
    };
    manifest.manager_lane = manager_lane.clone();
    manifest.manager_model = manager_model.clone();
    manifest.codex_auth_source = codex_auth_source.clone();
    manifest.codex_auth_path = codex_auth_path.clone().unwrap_or_default();
    let provider_profiles_path = if let Some(provider_config) = interactive_provider_config.as_ref()
    {
        configure_onboard_provider_profile(&root, provider_config)?
    } else {
        configure_onboard_provider_routes(
            &root,
            &manager_lane,
            Some(&manager_model),
            codex_auth_path.as_deref(),
        )?
    };
    let provider_auth_sync = sync_provider_auth_store(&root)?;

    if manifest.gateway_auth_mode == "none" && manifest.gateway_token_env.trim().is_empty() {
        manifest.gateway_token_env = config.service_token_env.clone();
    }
    manifest.last_action = if is_fresh_runtime {
        "setup".to_string()
    } else if had_existing_config {
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
    let gateway_runtime = sync_gateway_runtime(&root)?;
    let service_runtime = sync_service_runtime(&root)?;
    let ingress_runtime = sync_service_ingress_runtime(&root)?;
    let binding_summary = sync_binding_registry(&root)?;
    let skill_summary = sync_skill_registry(&root)?;
    let schedule_sync = sync_schedule_registry(&root)?;
    let schedule_runtime = schedule_overview(&root, current_unix_ms())?;
    fs::create_dir_all(root.join("state/memory")).map_err(|error| error.to_string())?;

    let provider_summary = provider_plane_summary(Some(&root))?;
    let provider_auth_summary = provider_auth_store_overview(&root)?;
    let runtime_overview = agent_runtime_overview(&root)?;
    let manager_route = resolve_provider_route(
        Some(&root),
        &ProviderRouteIntent::llm_inference("").with_agent_id("leviathann"),
    )
    .ok();
    let configured_manager_provider = configured_manager_provider(&root).ok();
    let codex_status = manager_route
        .as_ref()
        .filter(|route| matches!(route.profile_kind, ProviderKind::OpenAiCodex))
        .and_then(|route| provider_auth_status(Some(&root), Some(&route.profile_name)).ok());
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
    let codex_detail = if let Some(status) = codex_status.as_ref() {
        status.detail.clone()
    } else if manager_route
        .as_ref()
        .map(|route| matches!(route.profile_kind, ProviderKind::OpenAiCodex))
        .unwrap_or(false)
    {
        "Codex OAuth is not configured yet".to_string()
    } else {
        "current manager provider does not use Codex OAuth".to_string()
    };

    if format == "json" {
        print!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "root": root.display().to_string(),
                "setup_state": setup_state_label(&setup_state),
                "setup_hint": onboard_path_hint(&setup_state),
                "config_status": config_status,
                "config_action": config_action,
                "mode": config.mode,
                "org_id": config.org_id,
                "provider_profiles": provider_summary.profile_count,
                "provider_profiles_path": provider_profiles_path.display().to_string(),
                "provider_auth_runtime": {
                    "profile_count": provider_auth_summary.profile_count,
                    "ready_count": provider_auth_summary.ready_count,
                    "last_good_count": provider_auth_summary.last_good_count,
                    "usage_stats_count": provider_auth_summary.usage_stats_count,
                    "profiles": provider_auth_summary.profile_names.clone(),
                    "sync_ready_count": provider_auth_sync.ready_count,
                },
                "agent_profiles": runtime_overview.profile_count,
                "manager_lane": manager_lane,
                "manager_provider_profile": manager_route
                    .as_ref()
                    .map(|route| route.profile_name.clone())
                    .or_else(|| configured_manager_provider.as_ref().map(|route| route.profile_name.clone())),
                "manager_transport_kind": manager_route
                    .as_ref()
                    .map(|route| route.transport_kind().to_string())
                    .or_else(|| configured_manager_provider.as_ref().map(|route| route.transport_kind.clone())),
                "manager_model_config": manager_model,
                "codex_auth_source": manifest.codex_auth_source,
                "configured_codex_auth_path": if manifest.codex_auth_path.trim().is_empty() { None } else { Some(manifest.codex_auth_path.clone()) },
                "codex_auth_ready": codex_ready,
                "codex_credential_path": codex_path,
                "codex_detail": codex_detail,
                "manifest_path": manifest_path.display().to_string(),
                "onboard": {
                    "brain": overview.brain_summary,
                    "gateway": overview.gateway_summary,
                    "telegram": overview.telegram_summary,
                    "daemon": overview.daemon_summary,
                    "skills": overview.skills_summary,
                    "recurring": overview.recurring_summary,
                    "remote_mode": overview.remote_mode,
                    "channels": {
                        "total_count": channel_summary.total_count,
                        "enabled_count": channel_summary.enabled_count,
                        "channel_ids": channel_summary.channel_ids.clone(),
                    },
                    "gateway_runtime": {
                        "gateway_id": gateway_runtime.gateway_id.clone(),
                        "endpoint": gateway_runtime.endpoint.clone(),
                        "auth_mode": gateway_runtime.auth_mode.clone(),
                        "remote_mode": gateway_runtime.remote_mode.clone(),
                        "daemon_summary": gateway_runtime.daemon_summary.clone(),
                        "total_channel_count": gateway_runtime.total_channel_count,
                        "enabled_channel_count": gateway_runtime.enabled_channel_count,
                        "channel_ids": gateway_runtime.channel_ids.clone(),
                    },
                    "service_runtime": {
                        "service_health": service_runtime.service_health.clone(),
                        "service_http_address": service_runtime.service_http_address.clone(),
                        "service_pending_jobs": service_runtime.service_pending_jobs,
                        "service_processed_jobs": service_runtime.service_processed_jobs,
                        "supervisor_health": service_runtime.supervisor_health.clone(),
                        "supervisor_pending_jobs": service_runtime.supervisor_pending_jobs,
                        "supervisor_processed_jobs": service_runtime.supervisor_processed_jobs,
                    },
                    "service_ingress_runtime": {
                        "total_requests": ingress_runtime.total_requests,
                        "accepted_count": ingress_runtime.accepted_count,
                        "pending_count": ingress_runtime.pending_count,
                        "last_request_id": ingress_runtime.last_request_id.clone(),
                        "last_job_id": ingress_runtime.last_job_id.clone(),
                    },
                    "bindings_runtime": {
                        "total_count": binding_summary.total_count,
                        "enabled_count": binding_summary.enabled_count,
                        "binding_ids": binding_summary.binding_ids.clone(),
                    },
                    "skills_runtime": {
                        "total_count": skill_summary.total_count,
                        "enabled_count": skill_summary.enabled_count,
                        "default_count": skill_summary.default_count,
                        "imported_count": skill_summary.imported_count,
                        "skill_ids": skill_summary.skill_ids.clone(),
                    },
                    "schedules_runtime": {
                        "total_count": schedule_sync.total_count,
                        "enabled_count": schedule_sync.enabled_count,
                        "due_count": schedule_runtime.due_count,
                        "job_ids": schedule_sync.job_ids.clone(),
                    },
                },
                "manager_route": manager_route.as_ref().map(route_json),
                "manager_route_configured": configured_manager_provider.as_ref().map(configured_route_json),
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

    let manager_endpoint = manager_route
        .as_ref()
        .map(|route| route.endpoint_url.to_string())
        .or_else(|| configured_manager_provider.as_ref().map(|route| route.endpoint.clone()))
        .unwrap_or_else(|| "(pending provider setup)".to_string());
    let manager_route_model = manager_route
        .as_ref()
        .map(|route| route.model.clone())
        .or_else(|| configured_manager_provider.as_ref().map(|route| route.model.clone()))
        .unwrap_or_else(|| "(pending provider setup)".to_string());
    let manager_route_status = if manager_route.is_some() {
        "ready".to_string()
    } else if configured_manager_provider.is_some() {
        "configured but not ready — action required".to_string()
    } else {
        "not connected — action required".to_string()
    };
    let pulse_profile = pulse_route
        .as_ref()
        .map(|route| format!("{} ({})", route.profile_name, route.profile_kind.label()))
        .unwrap_or_else(|| "not connected — action required".to_string());
    let pulse_model = pulse_route
        .as_ref()
        .map(|route| route.model.clone())
        .unwrap_or_else(|| "(pending provider setup)".to_string());
    let daemon_summary = daemon_snapshot
        .as_ref()
        .map(render_daemon_summary)
        .unwrap_or_else(|| overview.daemon_summary.clone());
    let health_summary = health_snapshot
        .as_ref()
        .map(|(healthy, _)| if *healthy { "healthy" } else { "degraded" }.to_string())
        .unwrap_or_else(|| "skipped".to_string());

    if !banner_rendered {
        print_startup_banner();
    }
    print_human(&format!(
        "Meridian Loom // ONBOARD
=========================
root:                {}
setup_state:         {}
config_status:       {}
config_action:       {}
mode:                {}
org_id:              {}
provider_profiles:   {}
provider_cfg:        {}
provider_auth:       profiles={} ready={} last_good={} usage_stats={}
agent_profiles:      {}
brain:               {}
manager_lane:        {}
manager_provider:    {}
manager_transport:   {}
manager_model_cfg:   {}
codex_auth_source:   {}
configured_auth:     {}
codex_auth_ready:    {}
codex_auth_path:     {}
codex_detail:        {}
gateway:             {}
telegram:            {}
channels:            total={} enabled={} ids={}
gateway_runtime:     endpoint={} auth={} remote={} daemon={} channels={}/{}
service_runtime:     service={} jobs={}/{} supervisor={} jobs={}/{}
ingress_runtime:     requests={} accepted={} pending={} last_request={} last_job={}
bindings_runtime:    total={} enabled={} ids={}
skills:              {}
recurring:           {}
skills_runtime:      total={} enabled={} defaults={} imported={} ids={}
schedules_runtime:   total={} enabled={} due={} ids={}
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
        setup_state_label(&setup_state),
        config_status,
        config_action,
        config.mode,
        config.org_id,
        provider_summary.profile_count,
        provider_profiles_path.display(),
        provider_auth_summary.profile_count,
        provider_auth_summary.ready_count,
        provider_auth_summary.last_good_count,
        provider_auth_summary.usage_stats_count,
        runtime_overview.profile_count,
        overview.brain_summary,
        manager_lane,
        manager_route
            .as_ref()
            .map(|route| route.profile_name.as_str())
            .or_else(|| configured_manager_provider.as_ref().map(|route| route.profile_name.as_str()))
            .unwrap_or("(pending provider setup)"),
        manager_route
            .as_ref()
            .map(|route| route.transport_kind())
            .or_else(|| configured_manager_provider.as_ref().map(|route| route.transport_kind.as_str()))
            .unwrap_or("(pending provider setup)"),
        manager_model,
        manifest.codex_auth_source,
        if manifest.codex_auth_path.trim().is_empty() { "(none)" } else { manifest.codex_auth_path.as_str() },
        if codex_ready { "yes" } else { "no" },
        codex_path.as_deref().unwrap_or("(none)"),
        codex_detail,
        overview.gateway_summary,
        overview.telegram_summary,
        channel_summary.total_count,
        channel_summary.enabled_count,
        if channel_summary.channel_ids.is_empty() { "(none)".to_string() } else { channel_summary.channel_ids.join(",") },
        gateway_runtime.endpoint,
        gateway_runtime.auth_mode,
        gateway_runtime.remote_mode,
        gateway_runtime.daemon_summary,
        gateway_runtime.enabled_channel_count,
        gateway_runtime.total_channel_count,
        service_runtime.service_health,
        service_runtime.service_pending_jobs,
        service_runtime.service_processed_jobs,
        service_runtime.supervisor_health,
        service_runtime.supervisor_pending_jobs,
        service_runtime.supervisor_processed_jobs,
        ingress_runtime.total_requests,
        ingress_runtime.accepted_count,
        ingress_runtime.pending_count,
        if ingress_runtime.last_request_id.is_empty() { "(none)".to_string() } else { ingress_runtime.last_request_id.clone() },
        if ingress_runtime.last_job_id.is_empty() { "(none)".to_string() } else { ingress_runtime.last_job_id.clone() },
        binding_summary.total_count,
        binding_summary.enabled_count,
        if binding_summary.binding_ids.is_empty() { "(none)".to_string() } else { binding_summary.binding_ids.join(",") },
        overview.skills_summary,
        overview.recurring_summary,
        skill_summary.total_count,
        skill_summary.enabled_count,
        skill_summary.default_count,
        skill_summary.imported_count,
        if skill_summary.skill_ids.is_empty() { "(none)".to_string() } else { skill_summary.skill_ids.join(",") },
        schedule_sync.total_count,
        schedule_sync.enabled_count,
        schedule_runtime.due_count,
        if schedule_sync.job_ids.is_empty() { "(none)".to_string() } else { schedule_sync.job_ids.join(",") },
        daemon_summary,
        health_summary,
        manager_route_status,
        manager_endpoint,
        manager_route_model,
        pulse_profile,
        pulse_model,
        manifest_path.display(),
        root.display(),
    ));

    Ok(())
}

fn setup_state_label(state: &SetupState) -> &'static str {
    match state {
        SetupState::FreshWorkspace => "fresh_workspace",
        SetupState::FreshNoAuth { .. } => "fresh_no_auth",
        SetupState::LocalOnly { .. } => "local_only",
        SetupState::FrontierAvailable { .. } => "frontier_available",
        SetupState::FullyConfigured { .. } => "fully_configured",
    }
}

fn has_setup_overrides(args: &[String]) -> bool {
    const FLAGS: &[&str] = &[
        "--manager-lane",
        "--manager-model",
        "--codex-auth-source",
        "--codex-auth-path",
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
        "--recurring-install-defaults",
        "--recurring-entry",
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
    if let Some(value) = take_value(args, "--recurring-install-defaults") {
        manifest.recurring_install_defaults = parse_bool_flag("--recurring-install-defaults", &value)?;
    }
    let recurring_entries = take_values(args, "--recurring-entry");
    if !recurring_entries.is_empty() {
        manifest.recurring_entries = recurring_entries;
    }
    Ok(())
}

fn apply_interactive_overrides(manifest: &mut OnboardManifest) -> LoomResult<()> {
    print_setup_stage(
        2,
        5,
        "Gateway edge",
        "Shape the local web edge before Meridian exposes the service.",
    );
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
    print_setup_stage(
        3,
        5,
        "Telegram edge",
        "Configure delivery without borrowing someone else's bot state.",
    );
    manifest.telegram_enabled = prompt_bool("Enable Telegram channel", manifest.telegram_enabled)?;
    if manifest.telegram_enabled {
        manifest.telegram_token_env = prompt_text("Telegram token env", &manifest.telegram_token_env)?;
        manifest.telegram_dm_policy = prompt_text("Telegram DM policy", &manifest.telegram_dm_policy)?;
        manifest.telegram_group_policy = prompt_text("Telegram group policy", &manifest.telegram_group_policy)?;
        manifest.telegram_streaming = prompt_text("Telegram streaming", &manifest.telegram_streaming)?;
    }
    print_setup_stage(
        4,
        5,
        "Runtime daemon",
        "Choose how Meridian stays alive after setup finishes.",
    );
    manifest.session_dm_scope = prompt_text("Session DM scope", &manifest.session_dm_scope)?;
    manifest.daemon_enabled = prompt_bool("Enable supervisor daemon", manifest.daemon_enabled)?;
    manifest.daemon_manager = prompt_choice(
        "Daemon manager",
        &manifest.daemon_manager,
        &["supervisor"],
    )?;
    print_setup_stage(
        5,
        5,
        "Skills and defaults",
        "Finish by seeding the built-in skills Meridian expects on day one.",
    );
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
    manifest.recurring_install_defaults = prompt_bool(
        "Install default recurring jobs",
        manifest.recurring_install_defaults,
    )?;
    let recurring_entries = prompt_text(
        "Recurring job entries (comma separated)",
        &manifest.recurring_entries.join(","),
    )?;
    manifest.recurring_entries = recurring_entries
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect();
    Ok(())
}

#[derive(Clone, Debug)]
struct ManagerBrainSelection {
    lane: String,
    model: String,
    codex_auth_source: String,
    codex_auth_path: Option<String>,
    provider_kind: Option<ProviderKind>,
}

fn current_manager_brain(root: &Path, manifest: &OnboardManifest) -> ManagerBrainSelection {
    let route = resolve_provider_route(
        Some(root),
        &ProviderRouteIntent::llm_inference("").with_agent_id("leviathann"),
    )
    .ok();
    let lane = route
        .as_ref()
        .map(|resolved| {
            if resolved.profile_name == "local_ollama" || resolved.profile_kind.label() == "local_ollama" {
                "local".to_string()
            } else {
                "frontier".to_string()
            }
        })
        .unwrap_or_else(|| default_manager_lane(manifest));
    let model = route
        .as_ref()
        .map(|resolved| resolved.model.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_manager_model(&lane, manifest));
    let (codex_auth_source, codex_auth_path) = current_codex_auth_selection(root)
        .unwrap_or_else(|_| manifest_codex_auth_selection(manifest, &lane));
    let provider_kind = route.as_ref().map(|resolved| resolved.profile_kind.clone());
    let (codex_auth_source, codex_auth_path) = match provider_kind.as_ref() {
        Some(ProviderKind::OpenAiCodex) => (codex_auth_source, codex_auth_path),
        _ => ("none".to_string(), None),
    };
    ManagerBrainSelection {
        lane,
        model,
        codex_auth_source,
        codex_auth_path,
        provider_kind,
    }
}

#[derive(Clone, Debug)]
struct ProviderSetupSelection {
    manager_lane: String,
    manager_model: String,
    codex_auth_source: String,
    codex_auth_path: Option<String>,
    provider_config: Option<OnboardProviderRouteConfig>,
}

#[derive(Clone, Debug)]
struct ConfiguredManagerRoute {
    profile_name: String,
    profile_kind: ProviderKind,
    endpoint: String,
    model: String,
    transport_kind: String,
}

fn default_provider_choice_for_state(
    lane: &str,
    provider_kind: Option<&ProviderKind>,
) -> &'static str {
    match provider_kind {
        Some(ProviderKind::OpenAiCompatible) => "openai_compatible",
        Some(ProviderKind::CustomEndpoint) => "custom_endpoint",
        Some(ProviderKind::LocalOllama) => "local_ollama",
        _ if lane.eq_ignore_ascii_case("local") => "local_ollama",
        _ => "loom_codex",
    }
}

fn print_quickstart_summary_card(manifest: &OnboardManifest, manager_model: &str) {
    print_human(&format!(
        "QuickStart Summary
------------------
gateway:            {}:{} auth={} tailscale={}
telegram:           {}
daemon:             {} ({})
skills defaults:    {}
recurring defaults: {}
manager default:    {}

",
        manifest.gateway_bind,
        manifest.gateway_port,
        manifest.gateway_auth_mode,
        manifest.gateway_tailscale_mode,
        if manifest.telegram_enabled { "enabled" } else { "disabled" },
        if manifest.daemon_enabled { "enabled" } else { "disabled" },
        manifest.daemon_manager,
        manifest.skills_entries.join(","),
        if manifest.recurring_install_defaults {
            "enabled"
        } else {
            "disabled"
        },
        manager_model,
    ));
}

fn prompt_provider_setup(
    provider_choice: &str,
    current_model: &str,
    current_codex_auth_source: &str,
    current_codex_auth_path: Option<String>,
    detailed_flow: bool,
) -> LoomResult<ProviderSetupSelection> {
    match provider_choice {
        "loom_codex" => {
            if looks_remote_headless() {
                print_human(
                    "Remote OAuth
------------
This host appears to be headless or remote. Keep the Loom-managed auth path on this runtime, then finish device auth in your local browser when prompted. Meridian will keep the Loom-managed auth file separate from the shared operator login.

",
                );
            }
            let manager_model = prompt_text("Manager model", current_model)?;
            let codex_auth_source = prompt_choice(
                "Codex auth source",
                if current_codex_auth_source == "none" {
                    "loom"
                } else {
                    current_codex_auth_source
                },
                &["loom", "cli", "path"],
            )?;
            let codex_auth_path = match codex_auth_source.as_str() {
                "loom" => None,
                "cli" => None,
                _ => {
                    let hint = current_codex_auth_path.unwrap_or_else(|| {
                        default_codex_auth_path_hint()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|_| "~/.meridian/auth/codex/auth.json".to_string())
                    });
                    Some(prompt_text("Custom Codex auth.json path", &hint)?)
                }
            };
            Ok(ProviderSetupSelection {
                manager_lane: "frontier".to_string(),
                manager_model,
                codex_auth_source,
                codex_auth_path,
                provider_config: None,
            })
        }
        "local_ollama" | "local_only" => {
            if provider_choice == "local_only" {
                print_human(
                    "Local-only setup
----------------
Meridian will stay on the local inference path and skip remote provider setup for now.

",
                );
            }
            let default_model = if current_model.trim().is_empty() {
                "qwen2.5:7b"
            } else {
                current_model
            };
            let manager_model = prompt_text("Local model", default_model)?;
            Ok(ProviderSetupSelection {
                manager_lane: "local".to_string(),
                manager_model,
                codex_auth_source: "none".to_string(),
                codex_auth_path: None,
                provider_config: None,
            })
        }
        "openai_compatible" => {
            if detailed_flow {
                print_human(
                    "Provider branch
---------------
Meridian will route the manager through a standard OpenAI-compatible chat completions endpoint using a bearer token environment variable.

",
                );
            }
            let default_model = if current_model.trim().is_empty() {
                "gpt-5.4"
            } else {
                current_model
            };
            let manager_model = prompt_text("Remote model", default_model)?;
            let base_url = prompt_text(
                "OpenAI-compatible base URL",
                "https://api.openai.com/v1/chat/completions",
            )?;
            let env_var = prompt_text("Bearer token env var", "OPENAI_API_KEY")?;
            Ok(ProviderSetupSelection {
                manager_lane: "frontier".to_string(),
                manager_model: manager_model.clone(),
                codex_auth_source: "none".to_string(),
                codex_auth_path: None,
                provider_config: Some(OnboardProviderRouteConfig {
                    profile_name: "openai_default".to_string(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url,
                    default_model: manager_model,
                    auth: ProviderAuthMode::BearerEnv { env_var },
                    note: "seeded Meridian OpenAI-compatible route for manager reasoning"
                        .to_string(),
                    make_default: true,
                }),
            })
        }
        "custom_endpoint" => {
            let default_model = if current_model.trim().is_empty() {
                "gpt-5.4"
            } else {
                current_model
            };
            let manager_model = prompt_text("Remote model", default_model)?;
            let base_url = prompt_text(
                "Custom endpoint base URL",
                "https://api.example.test/v1/chat/completions",
            )?;
            let auth_mode = prompt_choice(
                "Custom endpoint auth mode",
                "bearer_env",
                &["bearer_env", "static_header_env", "none"],
            )?;
            let auth = match auth_mode.as_str() {
                "none" => ProviderAuthMode::None,
                "static_header_env" => {
                    let header_name = prompt_text("Header name", "x-api-key")?;
                    let env_var = prompt_text("Header env var", "MERIDIAN_CUSTOM_LLM_KEY")?;
                    ProviderAuthMode::StaticHeaderEnv { header_name, env_var }
                }
                _ => {
                    let env_var = prompt_text("Bearer token env var", "MERIDIAN_CUSTOM_LLM_KEY")?;
                    ProviderAuthMode::BearerEnv { env_var }
                }
            };
            Ok(ProviderSetupSelection {
                manager_lane: "frontier".to_string(),
                manager_model: manager_model.clone(),
                codex_auth_source: "none".to_string(),
                codex_auth_path: None,
                provider_config: Some(OnboardProviderRouteConfig {
                    profile_name: "custom_endpoint".to_string(),
                    kind: ProviderKind::CustomEndpoint,
                    base_url,
                    default_model: manager_model,
                    auth,
                    note: "seeded Meridian custom endpoint route for manager reasoning"
                        .to_string(),
                    make_default: true,
                }),
            })
        }
        other => Err(format!("unsupported provider choice '{}'", other)),
    }
}

fn default_manager_lane(manifest: &OnboardManifest) -> String {
    let lane = manifest.manager_lane.trim();
    if lane.is_empty() {
        "frontier".to_string()
    } else {
        lane.to_string()
    }
}

fn default_manager_model(lane: &str, manifest: &OnboardManifest) -> String {
    let model = manifest.manager_model.trim();
    if !model.is_empty() {
        return model.to_string();
    }
    if lane.eq_ignore_ascii_case("local") {
        "qwen2.5:7b".to_string()
    } else {
        "gpt-5.4".to_string()
    }
}

fn manifest_codex_auth_selection(
    manifest: &OnboardManifest,
    lane: &str,
) -> (String, Option<String>) {
    if lane.eq_ignore_ascii_case("local") {
        return ("none".to_string(), None);
    }
    let source = match manifest.codex_auth_source.trim() {
        "" => "loom".to_string(),
        value => value.to_string(),
    };
    let path = trimmed_string_option(&manifest.codex_auth_path);
    (source, path)
}

fn current_codex_auth_selection(root: &Path) -> LoomResult<(String, Option<String>)> {
    let profiles = load_provider_profiles(Some(root))?;
    let profile = profiles
        .profiles
        .iter()
        .find(|candidate| candidate.name == "manager_frontier")
        .ok_or_else(|| "manager_frontier provider profile was not found".to_string())?;
    match &profile.auth {
        ProviderAuthMode::CodexAuthJson { path } => {
            let loom_path = default_codex_auth_path_hint()?.display().to_string();
            let cli_path = shared_codex_auth_path_hint()?.display().to_string();
            match path.as_deref() {
                None => Ok(("cli".to_string(), Some(cli_path))),
                Some(raw) if auth_paths_match(raw, &loom_path) => {
                    Ok(("loom".to_string(), Some(loom_path)))
                }
                Some(raw) if auth_paths_match(raw, &cli_path) => {
                    Ok(("cli".to_string(), Some(cli_path)))
                }
                Some(raw) => Ok(("path".to_string(), Some(raw.trim().to_string()))),
            }
        }
        _ => Ok(("none".to_string(), None)),
    }
}

fn configured_manager_provider(root: &Path) -> LoomResult<ConfiguredManagerRoute> {
    let profiles = load_provider_profiles(Some(root))?;
    let configured_name = profiles
        .routing
        .agents
        .get("leviathann")
        .and_then(|policy| policy.profile_name.clone())
        .unwrap_or_else(|| profiles.default_profile_name.clone());
    let profile = profiles
        .profiles
        .iter()
        .find(|candidate| candidate.name == configured_name)
        .ok_or_else(|| format!("configured provider profile '{}' was not found", configured_name))?;
    Ok(ConfiguredManagerRoute {
        profile_name: profile.name.clone(),
        profile_kind: profile.kind.clone(),
        endpoint: profile.base_url.clone(),
        model: profile.default_model.clone(),
        transport_kind: transport_kind_for_profile(&profile.kind).to_string(),
    })
}

fn normalize_codex_auth_selection(
    manager_lane: &str,
    codex_auth_source: &str,
    codex_auth_path: Option<String>,
) -> LoomResult<(String, Option<String>)> {
    if manager_lane.eq_ignore_ascii_case("local") {
        return Ok(("none".to_string(), None));
    }
    let source = if codex_auth_source.trim().is_empty() {
        "loom".to_string()
    } else {
        codex_auth_source.trim().to_ascii_lowercase()
    };
    let explicit_path = codex_auth_path.and_then(|value| trimmed_string_option(&value));
    match source.as_str() {
        "loom" => {
            let expected = default_codex_auth_path_hint()?.display().to_string();
            if let Some(raw) = explicit_path.as_deref() {
                if !auth_paths_match(raw, &expected) {
                    return Err(
                        "--codex-auth-source loom cannot be combined with a different --codex-auth-path; use --codex-auth-source path for a custom Loom account file"
                            .to_string(),
                    );
                }
            }
            Ok(("loom".to_string(), Some(expected)))
        }
        "cli" | "shared" => {
            let expected = shared_codex_auth_path_hint()?.display().to_string();
            if let Some(raw) = explicit_path.as_deref() {
                if !auth_paths_match(raw, &expected) {
                    return Err(
                        "--codex-auth-source cli cannot be combined with a different --codex-auth-path; use --codex-auth-source path for a custom shared auth file"
                            .to_string(),
                    );
                }
            }
            Ok(("cli".to_string(), Some(expected)))
        }
        "path" => Ok((
            "path".to_string(),
            Some(explicit_path.ok_or_else(|| {
                "--codex-auth-source path requires --codex-auth-path PATH".to_string()
            })?),
        )),
        other => Err(format!(
            "unsupported Codex auth source '{}'; expected 'loom', 'cli', or 'path'",
            other
        )),
    }
}

fn auth_paths_match(raw: &str, expected: &str) -> bool {
    let normalized_expected = expected.trim();
    expand_auth_path(raw)
        .map(|path| path.display().to_string())
        .map(|path| path == normalized_expected)
        .unwrap_or_else(|| raw.trim() == normalized_expected)
}

fn expand_auth_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        return Some(path);
    }
    env::var("HOME").ok().map(|home| PathBuf::from(home).join(path))
}

fn looks_remote_headless() -> bool {
    env::var("SSH_CONNECTION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        || env::var("SSH_CLIENT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
        || env::var("DISPLAY")
            .ok()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
}

fn transport_kind_for_profile(kind: &ProviderKind) -> &'static str {
    match kind {
        ProviderKind::LocalOllama => "ollama_local",
        ProviderKind::OpenAiCompatible => "openai_rest",
        ProviderKind::OpenAiCodex => "codex_session",
        ProviderKind::CustomEndpoint => "custom_http",
    }
}

fn trimmed_string_option(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn print_setup_stage(step: usize, total: usize, title: &str, detail: &str) {
    print_human(&format!(
        "Stage {}/{} // {}
----------------------
{}

",
        step, total, title, detail
    ));
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

fn configured_route_json(route: &ConfiguredManagerRoute) -> Value {
    json!({
        "profile": route.profile_name,
        "kind": route.profile_kind.label(),
        "endpoint": route.endpoint,
        "model": route.model,
        "transport_kind": route.transport_kind,
        "matched_rule": "configured",
    })
}

fn print_onboard_help() {
    print_human(
        "Meridian Loom // ONBOARD HELP
================================
USAGE: loom onboard [OPTIONS]

PURPOSE:
  Configure a Loom runtime root, choose a manager provider, and materialize
  the local runtime state Meridian expects.

OPTIONS:
  --root PATH                 Runtime root to initialize or modify.
  --format human|json         Output format. Human becomes interactive on a TTY.
  --non-interactive           Skip prompts and apply CLI flags plus safe defaults.
  --config-action ACTION      keep | modify | reset
  --manager-lane LANE         frontier | local
  --manager-model MODEL       Manager model alias or provider-native name.
  --codex-auth-source SRC     loom | cli | path | none
  --codex-auth-path PATH      Custom auth.json path when using path mode.
  --gateway-bind MODE         loopback | all
  --gateway-port PORT         Gateway port.
  --gateway-auth-mode MODE    token | none
  --gateway-token-env NAME    Gateway token environment variable.
  --tailscale-mode MODE       off | on
  --start-daemon              Start the supervisor daemon after setup when enabled.
  --skip-health-check         Skip the post-setup health check.

NOTES:
  - Interactive setup offers QuickStart and Manual paths.
  - Provider selection and model selection are separate.
  - Loom-managed OAuth stays under the runtime-owned auth path.
  - Non-interactive onboarding still writes real runtime state.
",
    );
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

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
