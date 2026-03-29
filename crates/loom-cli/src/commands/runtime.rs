use std::collections::BTreeMap;
use std::fs;
use std::io::IsTerminal;

use crate::*;
use loom_core::agent_runtime::{
    agent_memory_summary, agent_runtime_summary, agent_session_summary,
    commit_agent_session, open_agent_session, render_agent_memory_human, render_agent_memory_json,
    render_agent_runtime_human, render_agent_runtime_json, render_agent_session_human,
    render_agent_session_json, write_agent_memory_snapshot,
};
use loom_core::context_engine::{context_bundle, render_context_bundle_human, render_context_bundle_json};
use loom_core::{
    bindings, channels, gateway_runtime, onboarding, pipeline, provider_auth_store,
    provider_router, recurring, recurring_executor, schedules, service_ingress_runtime,
    service_runtime, session_provenance, skill_lifecycle, skills,
};

pub(crate) fn handle_init(args: &[String]) -> LoomResult<()> {
    let mode = take_value(args, "--mode").unwrap_or_else(|| "standalone".to_string());
    let kernel_path = take_value(args, "--kernel-path");
    let root = root_from(take_value(args, "--root").as_deref())?;
    let org_id = take_value(args, "--org-id").unwrap_or_else(|| "local_foundry".to_string());
    let config = init_workspace(&root, &mode, kernel_path.as_deref(), &org_id)?;
    print_human(&format!(
        "Meridian Loom // INIT\n====================\nroot:        {}\nmode:        {}\norg_id:      {}\nkernel_path: {}\nstatus:      local runtime root ready\n\nQuick start\n-----------\n1. loom onboard --root {} --format human\n2. loom provider login --source loom --device-auth\n3. loom doctor --root {} --format human\n",
        root.display(),
        config.mode,
        config.org_id,
        if config.kernel_path.is_empty() { "(not set)" } else { &config.kernel_path },
        root.display(),
        root.display()
    ));
    Ok(())
}


pub(crate) fn handle_doctor(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_doctor_help();
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
    let fix = args.iter().any(|a| a == "--fix");

    let mut checks = doctor(&root)?;

    let mut fix_results: Vec<String> = Vec::new();
    if fix {
        fix_results.extend(apply_safe_doctor_fixes(&root)?);
        if fix_results.is_empty() {
            fix_results.push("fix: no safe remediations to apply".to_string());
        }
        checks = doctor(&root)?;
    }

    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_doctor_human(&checks));
            for msg in &fix_results {
                println!("{}", msg);
            }
        }
        _ => {
            if fix_results.is_empty() {
                print!("{}", render_doctor_json(&checks));
            } else {
                // Append fix results to JSON output
                let checks_json = render_doctor_json(&checks);
                let trimmed = checks_json.trim_end();
                if trimmed.ends_with(']') {
                    // Wrap in object with fix_results
                    print!("{{\"checks\":{},\"fix_results\":{:?}}}\n", trimmed, fix_results);
                } else {
                    print!("{}", checks_json);
                    for msg in &fix_results {
                        eprintln!("{}", msg);
                    }
                }
            }
        }
    }
    Ok(())
}

fn print_doctor_help() {
    print_human(
        "Meridian Loom // DOCTOR HELP
===============================
USAGE: loom doctor [OPTIONS]

PURPOSE:
  Inspect the local runtime for configuration gaps, provider readiness,
  service health, session/pipeline state, memory/context health, and proof surfaces.

OPTIONS:
  --root PATH            Runtime root to inspect.
  --format human|json    Output format.
  --fix                  Apply only safe scaffold and registry repairs.

NOTES:
  - --fix does not perform destructive changes.
  - --fix does not claim a service is running unless it can verify it.
  - JSON output is stable for automation and external probe tooling.
",
    );
}

fn apply_safe_doctor_fixes(root: &std::path::Path) -> LoomResult<Vec<String>> {
    let mut results = Vec::new();
    let config = read_config(root)?;
    fs::create_dir_all(root.join(&config.state_dir)).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join(&config.run_dir)).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join(&config.log_dir)).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join(&config.artifact_dir)).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join(&config.capabilities_dir)).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("state/memory")).map_err(|error| error.to_string())?;
    results.push("fix: ensured runtime directories exist".to_string());

    onboarding::ensure_onboard_manifest(root, &config)?;
    provider_router::ensure_provider_profiles_scaffold(root)?;
    provider_auth_store::ensure_provider_auth_store_scaffold(root)?;
    provider_auth_store::sync_provider_auth_store(root)?;
    gateway_runtime::ensure_gateway_runtime_scaffold(root)?;
    service_runtime::ensure_service_runtime_scaffold(root)?;
    service_ingress_runtime::ensure_service_ingress_runtime_scaffold(root)?;
    loom_core::agent_runtime::ensure_agent_runtime_scaffold(root)?;
    loom_core::context_engine::ensure_context_engine_scaffold(root)?;
    recurring::ensure_heartbeat_runtime_scaffold(root)?;
    schedules::ensure_schedule_runtime_scaffold(root)?;
    channels::ensure_channel_runtime_scaffold(root)?;
    bindings::ensure_binding_runtime_scaffold(root)?;
    skills::ensure_skill_runtime_scaffold(root)?;
    session_provenance::ensure_session_provenance_scaffold(root)?;
    skill_lifecycle::ensure_skill_lifecycle_scaffold(root)?;
    recurring_executor::ensure_recurring_executor_scaffold(root)?;
    pipeline::ensure_pipeline_scaffold(root)?;
    results.push("fix: refreshed safe scaffold files and runtime registries".to_string());

    Ok(results)
}


pub(crate) fn handle_runtime_info(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let config = read_config(&root)?;
    let binary_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());
    let info = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "binary_path": binary_path,
        "runtime_root": root.display().to_string(),
        "mode": config.mode,
        "org_id": config.org_id,
        "service_http_address": config.service_http_address,
        "service_token_env": config.service_token_env,
        "python_path": config.python_path,
        "kernel_path": config.kernel_path,
        "state_dir": root.join(&config.state_dir).display().to_string(),
        "log_dir": root.join(&config.log_dir).display().to_string(),
    });
    print!("{}", serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?);
    println!();
    Ok(())
}


pub(crate) fn handle_health(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| "json".to_string());
    let (healthy, json) = health(&root)?;
    if format == "human" {
        print_human(&render_health_human(healthy, &json));
    } else {
        print!("{}", json);
    }
    Ok(())
}


pub(crate) fn handle_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let base = status_human(&root)?;
    let service = runtime_service_status(&root, take_value(args, "--socket").as_deref())?;
    print_startup_banner();
    print_human_block(&[base, render_runtime_service_human(&service)]);
    Ok(())
}


pub(crate) fn handle_config(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("show") {
        return Err("config only supports 'show' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let config = read_config(&root)?;
    print_human(&render_config_human(&config, &root));
    Ok(())
}


pub(crate) fn handle_contract(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("show") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = contract_show(&root, kernel_path.as_deref())?;
            if format == "json" {
                print!("{}", render_contract_json(&snapshot));
            } else {
                print_human(&render_contract_human(&snapshot));
            }
            Ok(())
        }
        Some("verify") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let agent_id = take_value(args, "--agent-id")
                .unwrap_or_else(|| "agent_tutorial".to_string());
            let org_id = take_value(args, "--org-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let result = contract_verify(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
            )?;
            if format == "json" {
                print!("{}", render_contract_verify_json(&result));
            } else {
                print_human(&render_contract_verify_human(&result));
            }
            if result.passed < result.total {
                std::process::exit(1);
            }
            Ok(())
        }
        _ => Err("contract supports 'show' and 'verify'".to_string()),
    }
}


pub(crate) fn handle_capsule(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("inspect") {
        return Err("capsule only supports 'inspect' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let inspection = capsule_inspect(&root)?;
    print_human(&render_capsule_human(&inspection));
    Ok(())
}


pub(crate) fn handle_agent(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("resolve") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let identity = resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            if format == "json" {
                print!("{}", render_identity_json(&identity));
            } else {
                print_human(&render_identity_human(&identity));
            }
            Ok(())
        }
        Some("runtime") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = agent_runtime_summary(&root, &agent_id)?;
            if format == "json" {
                print!("{}", render_agent_runtime_json(&summary));
            } else {
                print_human(&render_agent_runtime_human(&summary));
            }
            Ok(())
        }
        Some("session") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let task_kind = take_value(args, "--task-kind");
            let status = take_value(args, "--status");
            let summary_text = take_value(args, "--summary");
            let summary = if has_flag(args, "--new") {
                open_agent_session(&root, &agent_id, task_kind.as_deref())?
            } else if status.is_some() || summary_text.is_some() || task_kind.is_some() {
                commit_agent_session(
                    &root,
                    &agent_id,
                    status.as_deref(),
                    summary_text.as_deref(),
                    task_kind.as_deref(),
                )?
            } else {
                agent_session_summary(&root, &agent_id)?
            };
            if format == "json" {
                print!("{}", render_agent_session_json(&summary));
            } else {
                print_human(&render_agent_session_human(&summary));
            }
            Ok(())
        }
        Some("memory") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let mut updates = BTreeMap::new();
            for entry in take_values(args, "--set") {
                let Some((key, value)) = entry.split_once('=') else {
                    return Err(format!("invalid --set '{}': expected key=value", entry));
                };
                let key = key.trim();
                let value = value.trim();
                if key.is_empty() || value.is_empty() {
                    return Err(format!("invalid --set '{}': expected key=value", entry));
                }
                updates.insert(key.to_string(), value.to_string());
            }
            let summary = if updates.is_empty() {
                agent_memory_summary(&root, &agent_id)?
            } else {
                write_agent_memory_snapshot(&root, &agent_id, &updates)?
            };
            if format == "json" {
                print!("{}", render_agent_memory_json(&summary));
            } else {
                print_human(&render_agent_memory_human(&summary));
            }
            Ok(())
        }
        Some("context") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let session_id = take_value(args, "--session-id");
            let bundle = context_bundle(&root, &agent_id, session_id.as_deref())?;
            if format == "json" {
                print!("{}", render_context_bundle_json(&bundle));
            } else {
                print_human(&render_context_bundle_human(&bundle));
            }
            Ok(())
        }
        _ => Err("agent supports 'resolve', 'runtime', 'session', 'memory', and 'context'".to_string()),
    }
}


pub(crate) fn handle_envelope(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("build") {
        return Err("envelope only supports 'build' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let agent_id = required_flag(args, "--agent-id")?;
    let action_type = required_flag(args, "--action-type")?;
    let resource = required_flag(args, "--resource")?;
    let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
    let kernel_path = take_value(args, "--kernel-path");
    let org_id = take_value(args, "--org-id");
    let run_id = take_value(args, "--run-id");
    let session_id = take_value(args, "--session-id");
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

    let envelope = build_action_envelope(
        &root,
        kernel_path.as_deref(),
        &agent_id,
        org_id.as_deref(),
        &action_type,
        &resource,
        estimated_cost_usd,
        run_id.as_deref(),
        session_id.as_deref(),
    )?;
    if format == "json" {
        print!("{}", render_envelope_json(&envelope));
    } else {
        print_human(&render_envelope_human(&envelope));
    }
    Ok(())
}


pub(crate) fn handle_shadow(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("report") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            print_human(&render_shadow_report(&root)?);
            Ok(())
        }
        Some("preflight") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let action_type = required_flag(args, "--action-type")?;
            let resource = required_flag(args, "--resource")?;
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

            let identity = resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
            )?;
            let reference =
                evaluate_reference_gates(&root, kernel_path.as_deref(), &identity, &envelope)?;
            let effective_kernel_path = kernel_path_for(&root, kernel_path.as_deref())?;
            let capture =
                capture_preflight(&root, &effective_kernel_path, &identity, &envelope, &reference)?;
            if format == "json" {
                print!("{}", render_preflight_json(&capture));
            } else {
                print_human_block(&[
                    render_identity_human(&identity),
                    render_envelope_human(&envelope),
                    render_preflight_human(&capture),
                ]);
            }
            Ok(())
        }
        Some("decide") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let action_type = required_flag(args, "--action-type")?;
            let resource = required_flag(args, "--resource")?;
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

            let identity =
                resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
            )?;
            let reference =
                evaluate_reference_gates(&root, kernel_path.as_deref(), &identity, &envelope)?;
            let capture = capture_decision(&root, &identity, &envelope, &reference)?;
            if format == "json" {
                print!("{}", render_decision_json(&capture));
            } else {
                print_human(&render_decision_human(&capture));
            }
            Ok(())
        }
        Some("enforce") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let action_type = required_flag(args, "--action-type")?;
            let resource = required_flag(args, "--resource")?;
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

            let identity =
                resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
            )?;
            let reference =
                evaluate_reference_gates(&root, kernel_path.as_deref(), &identity, &envelope)?;
            let capture = capture_decision(&root, &identity, &envelope, &reference)?;
            if format == "json" {
                print!("{}", render_decision_json(&capture));
            } else {
                print_human(&render_decision_human(&capture));
            }
            std::process::exit(decision_exit_code(&capture, 0, 2));
        }
        Some("compare") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let primary = PathBuf::from(required_flag(args, "--primary")?);
            let shadow = take_value(args, "--shadow")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(&config.artifact_dir).join("shadow/events.jsonl"));
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = compare_logs(Some(&root), &primary, &shadow)?;
            if format == "json" {
                print!("{}", render_compare_json(&summary));
            } else {
                print_human(&render_compare_human(&summary));
            }
            Ok(())
        }
        _ => Err("shadow supports 'preflight', 'decide', 'enforce', 'compare', and 'report'".to_string()),
    }
}



pub(crate) fn handle_parity(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("report") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            print_human(&render_parity_report(&root)?);
            Ok(())
        }
        _ => Err("parity supports 'report'".to_string()),
    }
}
