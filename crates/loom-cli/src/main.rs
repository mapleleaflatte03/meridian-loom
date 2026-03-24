use loom_core::{
    build_action_envelope, capsule_inspect, contract_show, doctor, health, init_workspace,
    kernel_path_for, read_config, render_capsule_human, render_contract_human,
    render_config_human, render_contract_json, render_doctor_human, render_doctor_json,
    render_envelope_human, render_envelope_json, render_health_human, render_identity_human,
    render_identity_json, resolve_agent_identity, root_from, status_human,
    evaluate_reference_gates, LoomResult, capability_shims::{generate_shim, render_shim_human, render_shim_json, validate_shim, LegacyToolSpec},
    wasm_host::{
        render_host_config_human, render_host_config_json, run_wasm_guest, HostBackend,
        WasmExecutionRequest, WasmGuestSource, WasmHostBuilder,
    },
    wasm_limits::{default_limits, from_toml as parse_wasm_limits_toml, render_limits_human, render_limits_json, validate_limits},
    wasm_profiles::{profile_defaults_map, render_pooling_config_human, render_pooling_config_json, PoolingProfile},
};
use loom_shadow::{
    capture_decision, capture_preflight, capture_runtime_execution, compare_logs,
    decision_exit_code, enqueue_action, inspect_job, list_jobs, render_compare_human,
    render_compare_json, render_decision_human, render_decision_json,
    render_enqueued_action_human, render_enqueued_action_json, render_job_inspect_human,
    render_job_inspect_json, render_job_list_human, render_job_list_json, render_parity_report,
    render_supervisor_lanes_human, render_supervisor_lanes_json,
    render_preflight_human, render_preflight_json, render_runtime_execution_human,
    render_runtime_execution_json, render_supervisor_daemon_human,
    render_supervisor_daemon_json, render_runtime_service_human,
    render_runtime_service_import_human, render_runtime_service_import_json,
    render_runtime_service_json, render_runtime_service_submit_human,
    render_runtime_service_submit_json, render_shadow_report, render_supervisor_run_human,
    render_supervisor_run_json, render_supervisor_status_human, render_supervisor_status_json,
    render_supervisor_watch_human, render_supervisor_watch_json, run_supervisor,
    import_commitment_execution_requests,
    run_supervisor_daemon_loop, request_runtime_service_stop, request_supervisor_daemon_stop,
    run_runtime_service_loop, runtime_service_status, submit_runtime_service_action,
    supervisor_daemon_status, supervisor_status, watch_supervisor,
};
use std::env;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("loom: {}", error);
            ExitCode::FAILURE
        }
    }
}

fn run() -> LoomResult<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "init" => handle_init(&args[1..]),
        "doctor" => handle_doctor(&args[1..]),
        "health" => handle_health(&args[1..]),
        "status" => handle_status(&args[1..]),
        "config" => handle_config(&args[1..]),
        "contract" => handle_contract(&args[1..]),
        "capsule" => handle_capsule(&args[1..]),
        "capability" => handle_capability(&args[1..]),
        "job" => handle_job(&args[1..]),
        "agent" => handle_agent(&args[1..]),
        "envelope" => handle_envelope(&args[1..]),
        "action" => handle_action(&args[1..]),
        "supervisor" => handle_supervisor(&args[1..]),
        "service" => handle_service(&args[1..]),
        "shadow" => handle_shadow(&args[1..]),
        "parity" => handle_parity(&args[1..]),
        "wasm" => handle_wasm(&args[1..]),
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command '{}'", other)),
    }
}

fn handle_init(args: &[String]) -> LoomResult<()> {
    let mode = take_value(args, "--mode").unwrap_or_else(|| "standalone".to_string());
    let kernel_path = take_value(args, "--kernel-path");
    let root = root_from(take_value(args, "--root").as_deref())?;
    let org_id = take_value(args, "--org-id").unwrap_or_else(|| "local_foundry".to_string());
    let config = init_workspace(&root, &mode, kernel_path.as_deref(), &org_id)?;
    print_human(&format!(
        "Meridian Loom // INIT\n====================\nroot:        {}\nmode:        {}\norg_id:      {}\nstate_dir:   {}\nkernel_path: {}\nstatus:      initialized experimental scaffold\nnext_step:   loom doctor --root {} --format human\n",
        root.display(),
        config.mode,
        config.org_id,
        config.state_dir,
        if config.kernel_path.is_empty() { "(not set)" } else { &config.kernel_path },
        root.display()
    ));
    Ok(())
}

fn handle_doctor(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| "json".to_string());
    let checks = doctor(&root)?;
    match format.as_str() {
        "human" => print_human(&render_doctor_human(&checks)),
        _ => print!("{}", render_doctor_json(&checks)),
    }
    Ok(())
}

fn handle_health(args: &[String]) -> LoomResult<()> {
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

fn handle_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    print_human(&status_human(&root)?);
    Ok(())
}

fn handle_config(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("show") {
        return Err("config only supports 'show' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let config = read_config(&root)?;
    print_human(&render_config_human(&config, &root));
    Ok(())
}

fn handle_contract(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("show") {
        return Err("contract only supports 'show' in this scaffold".to_string());
    }
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

fn handle_capsule(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("inspect") {
        return Err("capsule only supports 'inspect' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let inspection = capsule_inspect(&root)?;
    print_human(&render_capsule_human(&inspection));
    Ok(())
}

fn handle_capability(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("shim") => {
            let tool_name = required_flag(args, "--tool-name")?;
            let input_schema = required_flag(args, "--input-schema")?;
            let output_schema = required_flag(args, "--output-schema")?;
            let version = take_value(args, "--version");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let spec = LegacyToolSpec {
                name: tool_name,
                version,
                input_schema,
                output_schema,
            };
            let shim = generate_shim(&spec);
            if let Err(errors) = validate_shim(&shim) {
                return Err(format!("generated invalid shim: {}", errors.join("; ")));
            }
            if format == "json" {
                print!("{}", render_shim_json(&shim));
            } else {
                print_human(&render_shim_human(&shim));
            }
            Ok(())
        }
        _ => Err("capability supports 'shim'".to_string()),
    }
}

fn handle_job(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("list") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let status_filter = take_value(args, "--status");
            let limit = take_value(args, "--limit")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(20);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let jobs = list_jobs(&root, status_filter.as_deref(), limit)?;
            if format == "json" {
                print!("{}", render_job_list_json(&jobs));
            } else {
                print_human(&render_job_list_human(&root, &jobs, status_filter.as_deref()));
            }
            Ok(())
        }
        Some("inspect") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let job_id = required_flag(args, "--job-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = inspect_job(&root, &job_id)?;
            if format == "json" {
                print!("{}", render_job_inspect_json(&snapshot));
            } else {
                print_human(&render_job_inspect_human(&snapshot));
            }
            Ok(())
        }
        _ => Err("job supports 'list' and 'inspect'".to_string()),
    }
}

fn handle_agent(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("resolve") {
        return Err("agent only supports 'resolve' in this scaffold".to_string());
    }
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

fn handle_envelope(args: &[String]) -> LoomResult<()> {
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

fn handle_shadow(args: &[String]) -> LoomResult<()> {
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
            let primary = PathBuf::from(required_flag(args, "--primary")?);
            let shadow = take_value(args, "--shadow")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(".loom/shadow/events.jsonl"));
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

fn handle_action(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("enqueue") => {
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
            let effective_kernel_path = kernel_path_for(&root, kernel_path.as_deref())?;
            let capture = enqueue_action(&root, &effective_kernel_path, &envelope)?;
            if format == "json" {
                print!("{}", render_enqueued_action_json(&capture));
            } else {
                print_human_block(&[
                    render_envelope_human(&envelope),
                    render_enqueued_action_human(&capture),
                ]);
            }
            Ok(())
        }
        Some("execute") => {
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
            let decision = capture_decision(&root, &identity, &envelope, &reference)?;
            let effective_kernel_path = kernel_path_for(&root, kernel_path.as_deref())?;
            let capture = capture_runtime_execution(
                &root,
                &effective_kernel_path,
                &envelope,
                &reference,
                &decision,
            )?;
            if format == "json" {
                print!("{}", render_runtime_execution_json(&capture));
            } else {
                print_human_block(&[
                    render_identity_human(&identity),
                    render_envelope_human(&envelope),
                    render_decision_human(&decision),
                    render_runtime_execution_human(&capture),
                ]);
            }
            std::process::exit(decision_exit_code(&decision, 0, 2));
        }
        _ => Err("action supports 'enqueue' and 'execute'".to_string()),
    }
}

fn handle_parity(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("report") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            print_human(&render_parity_report(&root)?);
            Ok(())
        }
        _ => Err("parity supports 'report'".to_string()),
    }
}

fn handle_wasm_limits(args: &[String]) -> LoomResult<()> {
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let raw = if let Some(config_path) = take_value(args, "--config-file") {
        std::fs::read_to_string(&config_path)
            .map_err(|error| format!("failed to read {}: {}", config_path, error))?
    } else {
        String::new()
    };
    let limits = if raw.is_empty() {
        default_limits()
    } else {
        parse_wasm_limits_toml(&raw)?
    };
    if let Err(errors) = validate_limits(&limits) {
        return Err(format!("invalid wasm limits: {}", errors.join("; ")));
    }
    if format == "json" {
        print!("{}", render_limits_json(&limits));
    } else {
        print_human(&render_limits_human(&limits));
    }
    Ok(())
}

fn handle_wasm_profile(args: &[String]) -> LoomResult<()> {
    if args.get(1).map(String::as_str) != Some("show") {
        return Err("wasm profile supports 'show'".to_string());
    }
    let profile_name = take_value(args, "--profile").unwrap_or_else(|| "standard".to_string());
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let profile = profile_defaults_map()
        .remove(&profile_name)
        .ok_or_else(|| format!("unknown wasm profile '{}'", profile_name))?;
    if format == "json" {
        print!("{}", render_pooling_config_json(&profile));
    } else {
        print_human(&render_pooling_config_human(&profile));
    }
    Ok(())
}

fn handle_wasm_host(args: &[String]) -> LoomResult<()> {
    if args.get(1).map(String::as_str) != Some("show") {
        return Err("wasm host supports 'show'".to_string());
    }
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let backend = match take_value(args, "--backend")
        .unwrap_or_else(|| "preview_only".to_string())
        .as_str()
    {
        "preview_only" => HostBackend::PreviewOnly,
        "wasmtime_ready" => HostBackend::WasmtimeReady,
        other => return Err(format!("unknown wasm host backend '{}'", other)),
    };
    let profile_name = take_value(args, "--profile").unwrap_or_else(|| "standard".to_string());
    let profile = match profile_name.as_str() {
        "minimal" => PoolingProfile::Minimal,
        "standard" => PoolingProfile::Standard,
        "heavy" => PoolingProfile::Heavy,
        "custom" => PoolingProfile::Custom,
        other => return Err(format!("unknown wasm pooling profile '{}'", other)),
    };
    let raw = if let Some(config_path) = take_value(args, "--config-file") {
        std::fs::read_to_string(&config_path)
            .map_err(|error| format!("failed to read {}: {}", config_path, error))?
    } else {
        String::new()
    };
    let limits = if raw.is_empty() {
        default_limits()
    } else {
        parse_wasm_limits_toml(&raw)?
    };
    let config = WasmHostBuilder::new()
        .with_profile_name(format!("host/{}", profile_name))
        .with_backend(backend)
        .with_pooling_profile(profile)
        .with_store_limits(limits)
        .build()
        .map_err(|errors| format!("invalid wasm host config: {}", errors.join("; ")))?;
    if format == "json" {
        print!("{}", render_host_config_json(&config));
    } else {
        print_human(&render_host_config_human(&config));
    }
    Ok(())
}

fn handle_wasm_run(args: &[String]) -> LoomResult<()> {
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let backend = match take_value(args, "--backend")
        .unwrap_or_else(|| "wasmtime_ready".to_string())
        .as_str()
    {
        "preview_only" => HostBackend::PreviewOnly,
        "wasmtime_ready" => HostBackend::WasmtimeReady,
        other => return Err(format!("unknown wasm host backend '{}'", other)),
    };
    let profile_name = take_value(args, "--profile").unwrap_or_else(|| "standard".to_string());
    let profile = match profile_name.as_str() {
        "minimal" => PoolingProfile::Minimal,
        "standard" => PoolingProfile::Standard,
        "heavy" => PoolingProfile::Heavy,
        "custom" => PoolingProfile::Custom,
        other => return Err(format!("unknown wasm pooling profile '{}'", other)),
    };
    let raw = if let Some(config_path) = take_value(args, "--config-file") {
        std::fs::read_to_string(&config_path)
            .map_err(|error| format!("failed to read {}: {}", config_path, error))?
    } else {
        String::new()
    };
    let limits = if raw.is_empty() {
        default_limits()
    } else {
        parse_wasm_limits_toml(&raw)?
    };
    let config = WasmHostBuilder::new()
        .with_profile_name(format!("host/{}", profile_name))
        .with_backend(backend)
        .with_pooling_profile(profile)
        .with_store_limits(limits)
        .build()
        .map_err(|errors| format!("invalid wasm host config: {}", errors.join("; ")))?;
    let module_source = take_value(args, "--module").unwrap_or_else(|| "builtin:minimal".to_string());
    let source = if module_source == "builtin:minimal" {
        WasmGuestSource::WasmBytes {
            name: "builtin:minimal".to_string(),
            bytes: builtin_minimal_wasm_module(),
        }
    } else {
        WasmGuestSource::WasmBytes {
            name: module_source.clone(),
            bytes: std::fs::read(&module_source)
                .map_err(|error| format!("failed to read {}: {}", module_source, error))?,
        }
    };
    let entrypoint = take_value(args, "--entrypoint").unwrap_or_else(|| "run".to_string());
    let entrypoint_args = take_value(args, "--entrypoint-arg")
        .map(|raw| raw.parse::<i32>().map(|value| vec![value]).map_err(|error| {
            format!("invalid --entrypoint-arg '{}': {}", raw, error)
        }))
        .transpose()?
        .unwrap_or_default();
    let fuel_budget = take_value(args, "--fuel-budget")
        .map(|raw| raw.parse::<u64>().map_err(|error| format!("invalid --fuel-budget '{}': {}", raw, error)))
        .transpose()?
        .unwrap_or(100_000);
    let result = run_wasm_guest(&WasmExecutionRequest {
        host: config,
        source,
        entrypoint,
        entrypoint_args,
        memory_probe: None,
        fuel_budget,
    })?;
    if format == "json" {
        print!("{}", render_wasm_run_json(&result));
    } else {
        print_human(&render_wasm_run_human(&result));
    }
    Ok(())
}

fn handle_wasm(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("limits") => handle_wasm_limits(args),
        Some("profile") => handle_wasm_profile(args),
        Some("host") => handle_wasm_host(args),
        Some("run") => handle_wasm_run(args),
        _ => Err("wasm supports 'limits', 'profile show', 'host show', and 'run'".to_string()),
    }
}

fn handle_supervisor(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("daemon") => handle_supervisor_daemon(&args[1..]),
        Some("run") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = run_supervisor(&root, kernel_path.as_deref(), max_jobs)?;
            if format == "json" {
                print!("{}", render_supervisor_run_json(&summary));
            } else {
                print_human(&render_supervisor_run_human(&summary));
            }
            Ok(())
        }
        Some("watch") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(2);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = watch_supervisor(
                &root,
                kernel_path.as_deref(),
                max_jobs,
                iterations,
                poll_seconds,
            )?;
            if format == "json" {
                print!("{}", render_supervisor_watch_json(&summary));
            } else {
                print_human(&render_supervisor_watch_human(&summary));
            }
            Ok(())
        }
        Some("status") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = supervisor_status(&root)?;
            if format == "json" {
                print!("{}", render_supervisor_status_json(&snapshot));
            } else {
                print_human(&render_supervisor_status_human(&snapshot));
            }
            Ok(())
        }
        Some("lanes") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            if format == "json" {
                print!("{}", render_supervisor_lanes_json(&root)?);
            } else {
                print_human(&render_supervisor_lanes_human(&root)?);
            }
            Ok(())
        }
        _ => Err("supervisor supports 'run', 'watch', 'status', 'lanes', and 'daemon'".to_string()),
    }
}

fn handle_supervisor_daemon(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("start") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(60);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let supervisor_dir = root.join(".loom/runtime/supervisor");
            std::fs::create_dir_all(&supervisor_dir).map_err(|e| e.to_string())?;
            let stdout_log_path = supervisor_dir.join("daemon.log");
            let stdout = std::fs::File::create(&stdout_log_path).map_err(|e| e.to_string())?;
            let stderr = stdout.try_clone().map_err(|e| e.to_string())?;
            let session_id = format!("daemon-{}", chrono_like_timestamp());
            let exe = env::current_exe().map_err(|e| e.to_string())?;
            let mut command = Command::new(exe);
            command
                .arg("supervisor")
                .arg("daemon")
                .arg("loop")
                .arg("--root")
                .arg(&root)
                .arg("--max-jobs")
                .arg(max_jobs.to_string())
                .arg("--poll-seconds")
                .arg(poll_seconds.to_string())
                .arg("--iterations")
                .arg(iterations.to_string())
                .arg("--session-id")
                .arg(&session_id)
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            if let Some(kernel_path) = kernel_path.as_deref() {
                command.arg("--kernel-path").arg(kernel_path);
            }
            let child = command.spawn().map_err(|e| e.to_string())?;
            let note = format!(
                "daemon start requested; pid={} session_id={} log={}",
                child.id(),
                session_id,
                stdout_log_path.display()
            );
            let fallback_note = note.clone();
            let mut snapshot_result = supervisor_daemon_status(&root);
            for _ in 0..10 {
                if let Ok(snapshot) = &snapshot_result {
                    if snapshot.available {
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                snapshot_result = supervisor_daemon_status(&root);
            }
            let snapshot = snapshot_result.unwrap_or_else(|_| {
                loom_shadow::SupervisorDaemonSnapshot {
                    root: root.clone(),
                    supervisor_dir,
                    runtime_state_path: root.join(".loom/runtime/supervisor/runtime_state.json"),
                    stop_request_path: root.join(".loom/runtime/supervisor/stop.requested"),
                    stdout_log_path,
                    available: true,
                    session_id: session_id.clone(),
                    pid: child.id(),
                    running: true,
                    status: "starting".to_string(),
                    updated_at: String::new(),
                    booted_at: String::new(),
                    stopped_at: String::new(),
                    poll_seconds,
                    max_jobs,
                    max_iterations: iterations,
                    iterations_completed: 0,
                    processed: 0,
                    allowed: 0,
                    denied: 0,
                    failed: 0,
                    pending_jobs: 0,
                    processed_jobs: 0,
                    failed_jobs: 0,
                    heartbeat_entries: 0,
                    note: fallback_note,
                }
            });
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                let mut snapshot = snapshot;
                if snapshot.note.is_empty() {
                    snapshot.note = note;
                }
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        Some("loop") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(60);
            let session_id =
                take_value(args, "--session-id").unwrap_or_else(|| format!("daemon-{}", chrono_like_timestamp()));
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = run_supervisor_daemon_loop(
                &root,
                kernel_path.as_deref(),
                max_jobs,
                poll_seconds,
                iterations,
                &session_id,
            )?;
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        Some("status") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = supervisor_daemon_status(&root)?;
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        Some("stop") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = request_supervisor_daemon_stop(&root)?;
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        _ => Err("supervisor daemon supports 'start', 'loop', 'status', and 'stop'".to_string()),
    }
}

fn handle_service(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("start") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let socket_path = take_value(args, "--socket");
            let http_address = take_value(args, "--http-address");
            let service_token = take_value(args, "--service-token");
            let commitments_source = take_value(args, "--commitments-source");
            let workspace_token = take_value(args, "--workspace-token");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(120);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let service_dir = root.join(".loom/runtime/service");
            std::fs::create_dir_all(&service_dir).map_err(|e| e.to_string())?;
            let stdout_log_path = service_dir.join("service.log");
            let stdout = std::fs::File::create(&stdout_log_path).map_err(|e| e.to_string())?;
            let stderr = stdout.try_clone().map_err(|e| e.to_string())?;
            let session_id = format!("service-{}", chrono_like_timestamp());
            let exe = env::current_exe().map_err(|e| e.to_string())?;
            let mut command = Command::new(exe);
            command
                .arg("service")
                .arg("loop")
                .arg("--root")
                .arg(&root)
                .arg("--max-jobs")
                .arg(max_jobs.to_string())
                .arg("--poll-seconds")
                .arg(poll_seconds.to_string())
                .arg("--iterations")
                .arg(iterations.to_string())
                .arg("--session-id")
                .arg(&session_id)
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            if let Some(kernel_path) = kernel_path.as_deref() {
                command.arg("--kernel-path").arg(kernel_path);
            }
            if let Some(socket_path) = socket_path.as_deref() {
                command.arg("--socket").arg(socket_path);
            }
            if let Some(http_address) = http_address.as_deref() {
                command.arg("--http-address").arg(http_address);
            }
            if let Some(service_token) = service_token.as_deref() {
                command.arg("--service-token").arg(service_token);
            }
            if let Some(commitments_source) = commitments_source.as_deref() {
                command.arg("--commitments-source").arg(commitments_source);
            }
            if let Some(workspace_token) = workspace_token.as_deref() {
                command.arg("--workspace-token").arg(workspace_token);
            }
            let child = command.spawn().map_err(|e| e.to_string())?;
            let note = format!(
                "runtime service start requested; pid={} session_id={} log={}",
                child.id(),
                session_id,
                stdout_log_path.display()
            );
            let fallback_note = note.clone();
            let mut snapshot_result = runtime_service_status(&root, socket_path.as_deref());
            for _ in 0..10 {
                if let Ok(snapshot) = &snapshot_result {
                    if snapshot.available {
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                snapshot_result = runtime_service_status(&root, socket_path.as_deref());
            }
            let snapshot = snapshot_result.unwrap_or_else(|_| loom_shadow::RuntimeServiceSnapshot {
                root: root.clone(),
                service_dir,
                socket_path: socket_path
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| root.join(".loom/runtime/service/runtime.sock")),
                http_address: http_address.clone().unwrap_or_default(),
                http_token_required: service_token
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false),
                runtime_state_path: root.join(".loom/runtime/service/runtime_state.json"),
                stop_request_path: root.join(".loom/runtime/service/stop.requested"),
                stdout_log_path,
                event_log_path: root.join(".loom/runtime/service/service_events.jsonl"),
                ingress_stream_path: root.join(".loom/runtime/ingress/stream.jsonl"),
                available: true,
                session_id: session_id.clone(),
                pid: child.id(),
                running: true,
                status: "starting".to_string(),
                updated_at: String::new(),
                booted_at: String::new(),
                stopped_at: String::new(),
                poll_seconds,
                max_jobs,
                max_iterations: iterations,
                iterations_completed: 0,
                submitted: 0,
                processed: 0,
                allowed: 0,
                denied: 0,
                failed: 0,
                pending_jobs: 0,
                processed_jobs: 0,
                failed_jobs: 0,
                last_request_id: String::new(),
                last_job_id: String::new(),
                note: fallback_note,
            });
            if format == "json" {
                print!("{}", render_runtime_service_json(&snapshot));
            } else {
                let mut snapshot = snapshot;
                if snapshot.note.is_empty() {
                    snapshot.note = note;
                }
                print_human(&render_runtime_service_human(&snapshot));
            }
            Ok(())
        }
        Some("loop") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let socket_path = take_value(args, "--socket");
            let http_address = take_value(args, "--http-address");
            let service_token = take_value(args, "--service-token");
            let commitments_source = take_value(args, "--commitments-source");
            let workspace_token = take_value(args, "--workspace-token");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(120);
            let session_id =
                take_value(args, "--session-id").unwrap_or_else(|| format!("service-{}", chrono_like_timestamp()));
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = run_runtime_service_loop(
                &root,
                kernel_path.as_deref(),
                socket_path.as_deref(),
                http_address.as_deref(),
                service_token.as_deref(),
                commitments_source.as_deref(),
                workspace_token.as_deref(),
                max_jobs,
                poll_seconds,
                iterations,
                &session_id,
            )?;
            if format == "json" {
                print!("{}", render_runtime_service_json(&snapshot));
            } else {
                print_human(&render_runtime_service_human(&snapshot));
            }
            Ok(())
        }
        Some("status") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let socket_path = take_value(args, "--socket");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = runtime_service_status(&root, socket_path.as_deref())?;
            if format == "json" {
                print!("{}", render_runtime_service_json(&snapshot));
            } else {
                print_human(&render_runtime_service_human(&snapshot));
            }
            Ok(())
        }
        Some("stop") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let socket_path = take_value(args, "--socket");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = request_runtime_service_stop(&root, socket_path.as_deref())?;
            if format == "json" {
                print!("{}", render_runtime_service_json(&snapshot));
            } else {
                print_human(&render_runtime_service_human(&snapshot));
            }
            Ok(())
        }
        Some("submit") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let action_type = required_flag(args, "--action-type")?;
            let resource = required_flag(args, "--resource")?;
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let socket_path = take_value(args, "--socket");
            let http_url = take_value(args, "--http-url");
            let service_token = take_value(args, "--service-token");
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
            let effective_kernel_path = kernel_path_for(&root, kernel_path.as_deref())?;
            let capture = submit_runtime_service_action(
                &root,
                socket_path.as_deref(),
                http_url.as_deref(),
                service_token.as_deref(),
                &effective_kernel_path,
                &envelope,
            )?;
            if format == "json" {
                print!("{}", render_runtime_service_submit_json(&capture));
            } else {
                print_human_block(&[
                    render_envelope_human(&envelope),
                    render_runtime_service_submit_human(&capture),
                ]);
            }
            Ok(())
        }
        Some("import-commitments") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let commitments_source = required_flag(args, "--commitments-source")?;
            let workspace_token = take_value(args, "--workspace-token");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let capture = import_commitment_execution_requests(
                &root,
                kernel_path.as_deref(),
                &commitments_source,
                workspace_token.as_deref(),
            )?;
            if format == "json" {
                print!("{}", render_runtime_service_import_json(&capture));
            } else {
                print_human(&render_runtime_service_import_human(&capture));
            }
            Ok(())
        }
        _ => Err("service supports 'start', 'loop', 'status', 'submit', 'import-commitments', and 'stop'".to_string()),
    }
}

fn take_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn required_flag(args: &[String], flag: &str) -> LoomResult<String> {
    take_value(args, flag).ok_or_else(|| format!("missing required flag {}", flag))
}

fn parse_f64_flag(args: &[String], flag: &str) -> Option<f64> {
    take_value(args, flag).and_then(|raw| raw.parse::<f64>().ok())
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn json_string(input: &str) -> String {
    format!("{:?}", input)
}

fn print_help() {
    print_human(
        "Meridian Loom // HELP\n\
======================\n\
phase:       public experimental scaffold\n\
boundary:    operator shape is real; governed runtime is not\n\
\n\
Bootstrap\n\
---------\n\
  loom init --mode <embedded|shadow|standalone> [--kernel-path PATH] [--root PATH] [--org-id ID]\n\
  loom doctor [--root PATH] [--format json|human]\n\
  loom health [--root PATH] [--format json|human]\n\
  loom status [--root PATH]\n\
  loom config show [--root PATH]\n\
\n\
Governance surfaces\n\
-------------------\n\
  loom contract show [--root PATH] [--kernel-path PATH] [--format human|json]\n\
  loom capsule inspect [--root PATH]\n\
  loom capability shim --tool-name NAME --input-schema JSON --output-schema JSON [--version SEMVER] [--format human|json]\n\
  loom job list [--root PATH] [--status STATUS] [--limit N] [--format human|json]\n\
  loom job inspect --job-id HASH [--root PATH] [--format human|json]\n\
  loom agent resolve --agent-id ID [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom envelope build --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom wasm limits [--config-file loom.toml.example] [--format human|json]\n\
  loom wasm profile show [--profile minimal|standard|heavy] [--format human|json]\n\
  loom wasm host show [--profile minimal|standard|heavy|custom] [--backend preview_only|wasmtime_ready] [--config-file loom.toml.example] [--format human|json]\n\
  loom wasm run [--module builtin:minimal|PATH] [--entrypoint NAME] [--entrypoint-arg I32] [--fuel-budget N] [--profile minimal|standard|heavy|custom] [--backend wasmtime_ready] [--config-file loom.toml.example] [--format human|json]\n\
\n\
Runtime rehearsal\n\
-----------------\n\
  loom action enqueue --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom action execute --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom service start [--root PATH] [--kernel-path PATH] [--socket PATH] [--http-address HOST:PORT] [--service-token TOKEN] [--commitments-source PATH|URL] [--workspace-token TOKEN] [--max-jobs N] [--poll-seconds N] [--iterations N] [--format human|json]\n\
  loom service status [--root PATH] [--socket PATH] [--format human|json]\n\
  loom service submit --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--socket PATH] [--http-url URL] [--service-token TOKEN] [--format human|json]\n\
  loom service import-commitments --commitments-source PATH|URL [--workspace-token TOKEN] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom service stop [--root PATH] [--socket PATH] [--format human|json]\n\
  loom supervisor run [--root PATH] [--kernel-path PATH] [--max-jobs N] [--format human|json]\n\
  loom supervisor watch [--root PATH] [--kernel-path PATH] [--max-jobs N] [--iterations N] [--poll-seconds N] [--format human|json]\n\
  loom supervisor status [--root PATH] [--format human|json]\n\
  loom supervisor lanes [--root PATH] [--format human|json]\n\
  loom supervisor daemon start [--root PATH] [--kernel-path PATH] [--max-jobs N] [--poll-seconds N] [--iterations N] [--format human|json]\n\
  loom supervisor daemon status [--root PATH] [--format human|json]\n\
  loom supervisor daemon stop [--root PATH] [--format human|json]\n\
  loom shadow preflight --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom shadow decide --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom shadow enforce --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom shadow compare --primary FILE [--shadow FILE] [--root PATH] [--format human|json]\n\
  loom shadow report [--root PATH]\n\
  loom parity report [--root PATH]\n\
\n\
Next\n\
----\n\
  1. loom init --mode embedded --root /tmp/loom-rehearsal --kernel-path /tmp/meridian-kernel\n\
  2. loom service start --root /tmp/loom-rehearsal --kernel-path /tmp/meridian-kernel --http-address 127.0.0.1:0 --max-jobs 1 --poll-seconds 1 --iterations 20\n\
  3. loom service submit --root /tmp/loom-rehearsal --agent-id agent_atlas --action-type research --resource web_search --estimated-cost-usd 0.05\n\
  4. loom service import-commitments --root /tmp/loom-rehearsal --commitments-source /tmp/commitments.json\n\
  5. loom parity report --root /tmp/loom-rehearsal\n",
    );
}

fn render_wasm_run_human(result: &loom_core::wasm_host::WasmExecutionResult) -> String {
    let mut out = format!(
        "Meridian Loom // WASM RUN\n==========================\nphase:       experimental local guest lane\nboundary:    local Wasmtime execution is real; hosted capability runtime is not\n\nRuntime\n=======\nmodule:      {}\nentrypoint:  {}\nhost_backend:{}\nruntime_path:{}\nprofile:     {}\nentrypoint_result: {}\nstore_limit: {}\npooling:     {}\n",
        result.module_name,
        result.entrypoint,
        result.host_backend,
        result.runtime_path,
        result.host_profile_name,
        result
            .entrypoint_result
            .map(|value| value.to_string())
            .unwrap_or_else(|| "(none)".to_string()),
        result.store_memory_limit_bytes,
        result.pooling_profile,
    );
    if let Some(export_name) = result.memory_probe_export.as_ref() {
        out.push_str(&format!(
            "memory_probe:{} => {} pages_after={}\n",
            export_name,
            result
                .memory_probe_result
                .map(|value| value.to_string())
                .unwrap_or_else(|| "(none)".to_string()),
            result
                .memory_pages_after
                .map(|value| value.to_string())
                .unwrap_or_else(|| "(unknown)".to_string()),
        ));
    }
    out.push_str("\nHost hints\n==========\n");
    for (key, value) in &result.host_hints {
        out.push_str(&format!("{:<18} {}\n", format!("{}:", key), value));
    }
    out.push_str("\nNotes\n=====\n");
    for note in &result.notes {
        out.push_str(&format!("- {}\n", note));
    }
    out
}

fn render_wasm_run_json(result: &loom_core::wasm_host::WasmExecutionResult) -> String {
    let host_hints = result
        .host_hints
        .iter()
        .map(|(key, value)| format!("    {}: {}", json_string(key), json_string(value)))
        .collect::<Vec<_>>()
        .join(",\n");
    let notes = result
        .notes
        .iter()
        .map(|note| format!("    {}", json_string(note)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"status\": \"wasm_guest_executed\",\n  \"module_name\": {},\n  \"entrypoint\": {},\n  \"entrypoint_result\": {},\n  \"host_backend\": {},\n  \"host_profile_name\": {},\n  \"runtime_path\": {},\n  \"memory_probe_export\": {},\n  \"memory_probe_result\": {},\n  \"memory_pages_after\": {},\n  \"store_memory_limit_bytes\": {},\n  \"pooling_profile\": {},\n  \"host_hints\": {{\n{}\n  }},\n  \"notes\": [\n{}\n  ]\n}}\n",
        json_string(&result.module_name),
        json_string(&result.entrypoint),
        result
            .entrypoint_result
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        json_string(&result.host_backend),
        json_string(&result.host_profile_name),
        json_string(&result.runtime_path),
        result
            .memory_probe_export
            .as_ref()
            .map(|value| json_string(value))
            .unwrap_or_else(|| "null".to_string()),
        result
            .memory_probe_result
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        result
            .memory_pages_after
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        result.store_memory_limit_bytes,
        json_string(&result.pooling_profile),
        host_hints,
        notes,
    )
}

fn builtin_minimal_wasm_module() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x05, 0x01, 0x60, 0x00, 0x01,
        0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00,
        0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x07, 0x0b,
    ]
}

fn print_human(output: &str) {
    if stdout_supports_color() {
        print!("{}", style_human_output(output));
    } else {
        print!("{}", output);
    }
}

fn print_human_block(parts: &[String]) {
    let merged = parts.join("\n\n");
    print_human(&merged);
}

fn stdout_supports_color() -> bool {
    if env::var_os("FORCE_COLOR").is_some() {
        return true;
    }
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }
    io::stdout().is_terminal()
}

fn style_human_output(output: &str) -> String {
    let mut styled = output
        .lines()
        .map(style_human_line)
        .collect::<Vec<_>>()
        .join("\n");
    if output.ends_with('\n') {
        styled.push('\n');
    }
    styled
}

fn style_human_line(line: &str) -> String {
    const RESET: &str = "\x1b[0m";
    const CYAN: &str = "\x1b[38;5;81m";
    const BLUE: &str = "\x1b[38;5;111m";
    const GREEN: &str = "\x1b[38;5;114m";
    const YELLOW: &str = "\x1b[38;5;221m";
    const RED: &str = "\x1b[38;5;203m";
    const DIM: &str = "\x1b[2m";
    const BOLD: &str = "\x1b[1m";

    if line.starts_with("Meridian Loom //") {
        return format!("{BOLD}{CYAN}{line}{RESET}");
    }
    if !line.is_empty() && line.chars().all(|c| c == '=' || c == '-') {
        return format!("{DIM}{line}{RESET}");
    }
    if line.starts_with("[OK") {
        return format!("{GREEN}{line}{RESET}");
    }
    let lower = line.to_ascii_lowercase();
    if lower.contains("deny") || lower.contains("blocked") || lower.contains("failed") {
        return format!("{RED}{line}{RESET}");
    }
    if lower.contains("warn") || lower.contains("degraded") || lower.contains("divergence") {
        return format!("{YELLOW}{line}{RESET}");
    }
    if line.starts_with("phase:")
        || line.starts_with("boundary:")
        || line == "Decision"
        || line == "Checks"
        || line == "Current state"
        || line == "Next"
    {
        return format!("{BOLD}{BLUE}{line}{RESET}");
    }
    line.to_string()
}
