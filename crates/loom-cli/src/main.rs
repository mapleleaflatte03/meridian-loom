use loom_core::{
    build_action_envelope, build_action_envelope_with_options, capsule_inspect, contract_show, contract_verify, doctor, health,
    init_workspace, kernel_path_for, read_config, render_capsule_human, render_contract_human,
    render_config_human, render_contract_json, render_contract_verify_human,
    render_contract_verify_json, render_doctor_human, render_doctor_json,
    render_envelope_human, render_envelope_json, render_health_human, render_identity_human,
    render_identity_json, resolve_agent_identity, root_from, status_human,
    evaluate_reference_gates, Config, LoomResult, capability_shims::{generate_shim, render_shim_human, render_shim_json, validate_shim, LegacyToolSpec},
    capabilities::{
        find_capability_by_name, forge_capability, import_workspace_skill, load_capability_registry, promote_capability,
        load_capability_gap, capability_gap_replay_request, record_capability_gap, import_openclaw_plugin_skill_subset,
        render_capability_forge_human, render_capability_forge_json,
        render_capability_gap_human, render_capability_gap_json,
        render_capability_human, render_capability_import_human, render_capability_import_json,
        render_openclaw_plugin_import_human, render_openclaw_plugin_import_json,
        render_capability_json, render_capability_registry_human, render_capability_registry_json,
        render_capability_state_update_human, render_capability_state_update_json,
        scaffold_capability, timestamp_now as capability_timestamp_now,
        update_capability_gap_forge, update_capability_gap_promotion, update_capability_gap_verification,
        update_capability_verification, CapabilityForgeRequest, CapabilityGapRequest, CapabilityScaffoldRequest,
    },
    wasm_host::{
        render_host_config_human, render_host_config_json, run_wasm_guest, HostBackend,
        WasmExecutionRequest, WasmGuestSource, WasmHostBuilder,
    },
    wasm_limits::{default_limits, from_toml as parse_wasm_limits_toml, render_limits_human, render_limits_json, validate_limits},
    wasm_profiles::{profile_defaults_map, render_pooling_config_human, render_pooling_config_json, PoolingProfile},
};
use loom_shadow::{
    ack_queue_job, approve_job, consume_pending_queue, capture_decision, capture_preflight, capture_runtime_execution, compare_logs,
    decision_exit_code, enqueue_action, inspect_job, inspect_pending_queue, list_jobs, render_compare_human,
    render_compare_json, render_decision_human, render_decision_json,
    render_enqueued_action_human, render_enqueued_action_json, render_job_inspect_human,
    render_job_inspect_json, render_job_list_human, render_job_list_json, render_parity_report,
    render_queue_ack_human, render_queue_ack_json, render_queue_consume_human,
    render_queue_consume_json, render_queue_inspect_human, render_queue_inspect_json,
    render_queue_run_once_human, render_queue_run_once_json,
    render_queue_run_until_empty_human, render_queue_run_until_empty_json,
    render_queue_status_human, render_queue_status_json, queue_status,
    render_supervisor_lanes_human, render_supervisor_lanes_json,
    render_preflight_human, render_preflight_json, render_runtime_execution_human,
    render_runtime_execution_json, render_supervisor_daemon_human,
    render_supervisor_daemon_json, render_runtime_service_human,
    render_runtime_service_import_human, render_runtime_service_import_json,
    render_runtime_service_json, render_runtime_service_submit_human,
    render_runtime_service_submit_json, render_shadow_report, render_supervisor_run_human,
    render_supervisor_run_json, render_supervisor_status_human, render_supervisor_status_json,
    render_supervisor_watch_human, render_supervisor_watch_json, run_queue_once, run_queue_until_empty, run_supervisor,
    import_commitment_execution_requests,
    run_supervisor_daemon_loop, request_runtime_service_stop, request_supervisor_daemon_stop,
    run_runtime_service_loop, runtime_service_status, submit_runtime_service_action,
    supervisor_daemon_status, supervisor_status, watch_supervisor,
};
use serde_json::Value;
use std::env;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
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
        "version" | "-V" | "--version" => {
            print_human(&format!(
                "Meridian Loom // VERSION\n=========================\nversion:     {}\nboundary:    local-first runtime surface; OpenClaw replacement not claimed\n",
                env!("CARGO_PKG_VERSION")
            ));
            Ok(())
        }
        "init" => handle_init(&args[1..]),
        "doctor" => handle_doctor(&args[1..]),
        "health" => handle_health(&args[1..]),
        "status" => handle_status(&args[1..]),
        "start" => handle_start(&args[1..]),
        "stop" => handle_stop(&args[1..]),
        "restart" => handle_restart(&args[1..]),
        "logs" => handle_logs(&args[1..]),
        "config" => handle_config(&args[1..]),
        "contract" => handle_contract(&args[1..]),
        "capsule" => handle_capsule(&args[1..]),
        "capability" => handle_capability(&args[1..]),
        "job" => handle_job(&args[1..]),
        "queue" => handle_queue(&args[1..]),
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
        "Meridian Loom // INIT\n====================\nroot:        {}\nconfig:      {}\nmode:        {}\norg_id:      {}\nstate_dir:   {}\nrun_dir:     {}\nlog_dir:     {}\nartifact_dir: {}\nkernel_path: {}\nstatus:      initialized local-first runtime root\nnext_step:   loom doctor --root {} --format human\n",
        root.display(),
        root.join("loom.toml").display(),
        config.mode,
        config.org_id,
        config.state_dir,
        config.run_dir,
        config.log_dir,
        config.artifact_dir,
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
    let base = status_human(&root)?;
    let service = runtime_service_status(&root, take_value(args, "--socket").as_deref())?;
    print_human_block(&[base, render_runtime_service_human(&service)]);
    Ok(())
}

fn handle_start(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_start_help();
        return Ok(());
    }
    start_service_with_mode(args)
}

fn handle_stop(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_stop_help();
        return Ok(());
    }
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

fn handle_restart(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_restart_help();
        return Ok(());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let socket_path = take_value(args, "--socket");
    let status = runtime_service_status(&root, socket_path.as_deref())?;
    if status.running {
        let _ = request_runtime_service_stop(&root, socket_path.as_deref())?;
        for _ in 0..40 {
            let snapshot = runtime_service_status(&root, socket_path.as_deref())?;
            if !snapshot.running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    start_service_with_mode(args)
}

fn handle_logs(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_logs_help();
        return Ok(());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let lines = take_value(args, "--lines")
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(100);
    let follow = has_flag(args, "--follow");
    let config = read_config(&root)?;
    let log_path = root.join(&config.log_dir).join("service.log");
    if !log_path.exists() {
        return Err(format!(
            "service log not found at {}; start the runtime service first",
            log_path.display()
        ));
    }
    print_human(&format!(
        "Meridian Loom // LOGS\n=====================\npath:        {}\nmode:        {}\nlines:       {}\n\n",
        log_path.display(),
        if follow { "follow" } else { "tail" },
        lines,
    ));
    let mut offset = print_last_lines(&log_path, lines)?;
    if follow {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            offset = print_new_bytes(&log_path, offset)?;
        }
    }
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
    if args.is_empty() || has_flag(args, "--help") || has_flag(args, "-h") {
        print_capability_help();
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("list") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let registry = load_capability_registry(&root, &config)?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            if format == "json" {
                print!("{}", render_capability_registry_json(&registry));
            } else {
                print_human(&render_capability_registry_human(&root, &config, &registry));
            }
            Ok(())
        }
        Some("show") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let name = required_flag(args, "--name")?;
            let capability = find_capability_by_name(&root, &config, &name)?
                .ok_or_else(|| format!("capability '{}' not found", name))?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            if format == "json" {
                print!("{}", render_capability_show_json(&root, &capability)?);
            } else {
                print_human_block(&[
                    render_capability_human(&capability),
                    render_capability_evidence_human(&root, &capability),
                ]);
            }
            Ok(())
        }
        Some("scaffold") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let request = CapabilityScaffoldRequest {
                name: required_flag(args, "--name")?,
                description: take_value(args, "--description").unwrap_or_default(),
                action_type: required_flag(args, "--action-type")?,
                resource: required_flag(args, "--resource")?,
                worker_kind: take_value(args, "--worker-kind")
                    .unwrap_or_else(|| "python".to_string()),
                worker_entry: take_value(args, "--worker-entry").unwrap_or_default(),
                wasm_module: take_value(args, "--wasm-module").unwrap_or_default(),
                payload_mode: take_value(args, "--payload-mode")
                    .unwrap_or_else(|| "json".to_string()),
            };
            let result = scaffold_capability(&root, &config, &request)?;
            print_human(&format!(
                "Meridian Loom // CAPABILITY SCAFFOLD\n====================================\nmanifest:     {}\nworker_path:  {}\nname:         {}\nworker_kind:  {}\naction_type:  {}\nresource:     {}\nnote:         capability scaffolded into the local runtime root\n",
                result.manifest_path.display(),
                result.worker_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "(none)".to_string()),
                result.capability.name,
                result.capability.worker_kind,
                result.capability.action_type,
                result.capability.resource,
            ));
            Ok(())
        }
        Some("forge") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let gap_id = take_value(args, "--gap-id");
            let (gap_class, goal, name) = if let Some(ref gap_id) = gap_id {
                let gap = load_capability_gap(&root, &config, gap_id)?;
                (
                    gap.gap_class.clone(),
                    gap.goal.clone(),
                    gap.proposed_capability_name.clone(),
                )
            } else {
                let gap_class = take_value(args, "--gap-class").unwrap_or_default();
                let goal = take_value(args, "--goal").unwrap_or_default();
                let name = forge_name_from_args(args, &gap_class, &goal)?;
                (gap_class, goal, name)
            };
            let request = CapabilityForgeRequest {
                name,
                description: take_value(args, "--description").unwrap_or_default(),
                template: take_value(args, "--template").unwrap_or_default(),
                gap_class,
                goal,
            };
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let result = forge_capability(&root, &config, &request)?;
            let gap_update = if let Some(gap_id) = gap_id.as_deref() {
                Some(update_capability_gap_forge(
                    &root,
                    &config,
                    gap_id,
                    &result.manifest_path,
                    "gap candidate forged into Loom capability runtime",
                )?)
            } else {
                None
            };
            if format == "json" {
                if let Some(gap_update) = gap_update {
                    print!(
                        "{{\"forge\":{},\"gap\":{}}}\n",
                        render_capability_forge_json(&result).trim(),
                        render_capability_gap_json(&gap_update).trim()
                    );
                } else {
                    print!("{}", render_capability_forge_json(&result));
                }
            } else {
                if let Some(gap_update) = gap_update {
                    print_human_block(&[
                        render_capability_forge_human(&result),
                        render_capability_gap_human(&gap_update),
                    ]);
                } else {
                    print_human(&render_capability_forge_human(&result));
                }
            }
            Ok(())
        }
        Some("import-workspace-skill") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let skill_root = PathBuf::from(required_flag(args, "--skill-root")?);
            let capability_name = take_value(args, "--name");
            let entrypoint = take_value(args, "--entrypoint");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let result = import_workspace_skill(
                &root,
                &config,
                &skill_root,
                entrypoint.as_deref(),
                capability_name.as_deref(),
            )?;
            if format == "json" {
                print!("{}", render_capability_import_json(&result));
            } else {
                print_human(&render_capability_import_human(&result));
            }
            Ok(())
        }
        Some("import-openclaw-plugin-skill-subset") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let plugin_root = PathBuf::from(required_flag(args, "--plugin-root")?);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let result = import_openclaw_plugin_skill_subset(&root, &config, &plugin_root)?;
            if format == "json" {
                print!("{}", render_openclaw_plugin_import_json(&result));
            } else {
                print_human(&render_openclaw_plugin_import_human(&result));
            }
            Ok(())
        }
        Some("verify") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let name = required_flag(args, "--name")?;
            let agent_id = required_flag(args, "--agent-id")?;
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let payload_json = take_value(args, "--payload-json");
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let expect_summary_contains = take_value(args, "--expect-summary-contains");
            let expect_result_fields = take_values(args, "--expect-result-field");
            let gap_id = take_value(args, "--gap-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let capability = find_capability_by_name(&root, &config, &name)?
                .ok_or_else(|| format!("capability '{}' not found", name))?;
            let identity =
                resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope_with_options(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &capability.action_type,
                &capability.resource,
                estimated_cost_usd,
                None,
                None,
                Some(&name),
                payload_json.as_deref(),
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
            let verification_execution_id = read_runtime_event_execution_id(&capture.runtime_event_path)?;
            let worker_result = if capture.worker_result_path.exists() {
                Some(read_json_file(&capture.worker_result_path)?)
            } else {
                None
            };
            let expectation_failures = verify_capability_expectations(
                worker_result.as_ref(),
                expect_summary_contains.as_deref(),
                &expect_result_fields,
            )?;
            let base_verified =
                capture.runtime_outcome == "worker_executed" && capture.worker_status == "completed";
            let verification_status = if base_verified && expectation_failures.is_empty() {
                "verified"
            } else {
                "failed"
            };
            let mut verification_notes = vec![format!(
                "runtime_outcome={} worker_status={} effective_stage={}",
                capture.runtime_outcome, capture.worker_status, capture.effective_stage
            )];
            if !expectation_failures.is_empty() {
                verification_notes.push(format!(
                    "expectation_failures={}",
                    expectation_failures.join("; ")
                ));
            } else if expect_summary_contains.is_some() || !expect_result_fields.is_empty() {
                verification_notes.push("expectations=matched".to_string());
            }
            let verification_note = verification_notes.join(" | ");
            let updated = update_capability_verification(
                &root,
                &config,
                &name,
                verification_status,
                &capability_timestamp_now(),
                &capture.input_hash,
                &verification_execution_id,
                &verification_note,
            )?;
            let gap_update = if let Some(gap_id) = gap_id.as_deref() {
                Some(update_capability_gap_verification(
                    &root,
                    &config,
                    gap_id,
                    verification_status,
                    &capture.input_hash,
                    &verification_execution_id,
                    &verification_note,
                )?)
            } else {
                None
            };
            if format == "json" {
                if let Some(gap_update) = gap_update {
                    print!(
                        "{{\"verification\":{},\"gap\":{}}}\n",
                        render_capability_state_update_json(&updated).trim(),
                        render_capability_gap_json(&gap_update).trim()
                    );
                } else {
                    print!("{}", render_capability_state_update_json(&updated));
                }
            } else {
                let mut blocks = vec![
                    render_runtime_execution_human(&capture),
                    render_capability_state_update_human("Meridian Loom // CAPABILITY VERIFY", &updated),
                ];
                if let Some(gap_update) = gap_update {
                    blocks.push(render_capability_gap_human(&gap_update));
                }
                print_human_block(&blocks);
            }
            Ok(())
        }
        Some("promote") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let name = required_flag(args, "--name")?;
            let gap_id = take_value(args, "--gap-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let updated = promote_capability(&root, &config, &name, &capability_timestamp_now())?;
            let gap_update = if let Some(gap_id) = gap_id.as_deref() {
                Some(update_capability_gap_promotion(
                    &root,
                    &config,
                    gap_id,
                    "promoted",
                    "gap candidate promoted after verification",
                )?)
            } else {
                None
            };
            if format == "json" {
                if let Some(gap_update) = gap_update {
                    print!(
                        "{{\"promotion\":{},\"gap\":{}}}\n",
                        render_capability_state_update_json(&updated).trim(),
                        render_capability_gap_json(&gap_update).trim()
                    );
                } else {
                    print!("{}", render_capability_state_update_json(&updated));
                }
            } else {
                if let Some(gap_update) = gap_update {
                    print_human_block(&[
                        render_capability_state_update_human(
                            "Meridian Loom // CAPABILITY PROMOTE",
                            &updated,
                        ),
                        render_capability_gap_human(&gap_update),
                    ]);
                } else {
                    print_human(&render_capability_state_update_human(
                        "Meridian Loom // CAPABILITY PROMOTE",
                        &updated,
                    ));
                }
            }
            Ok(())
        }

        Some("gap") => {
            match args.get(1).map(String::as_str) {
                Some("show") => {
                    let root = root_from(take_value(args, "--root").as_deref())?;
                    let config = read_config(&root)?;
                    let gap_id = required_flag(args, "--gap-id")?;
                    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
                    let gap = load_capability_gap(&root, &config, &gap_id)?;
                    let gap_path = root
                        .join(&config.capabilities_dir)
                        .join("gaps")
                        .join(format!("{}.json", sanitize_token(&gap_id)));
                    let update = loom_core::capabilities::CapabilityGapUpdateResult { gap_path, gap };
                    if format == "json" {
                        print!("{}", render_capability_gap_json(&update));
                    } else {
                        print_human(&render_capability_gap_human(&update));
                    }
                    Ok(())
                }
                Some("replay") => {
                    let root = root_from(take_value(args, "--root").as_deref())?;
                    let config = read_config(&root)?;
                    let gap_id = required_flag(args, "--gap-id")?;
                    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
                    let gap = load_capability_gap(&root, &config, &gap_id)?;
                    let replay_request = capability_gap_replay_request(&gap)?;
                    run_action_execute_request(
                        &root,
                        &config,
                        &replay_request.agent_id,
                        Some(replay_request.capability_name.as_str()),
                        replay_request.action_type,
                        replay_request.resource,
                        0.0,
                        if replay_request.kernel_path.trim().is_empty() {
                            None
                        } else {
                            Some(replay_request.kernel_path.as_str())
                        },
                        if replay_request.org_id.trim().is_empty() {
                            None
                        } else {
                            Some(replay_request.org_id.as_str())
                        },
                        if replay_request.run_id.trim().is_empty() {
                            None
                        } else {
                            Some(replay_request.run_id.as_str())
                        },
                        if replay_request.session_id.trim().is_empty() {
                            None
                        } else {
                            Some(replay_request.session_id.as_str())
                        },
                        if replay_request.payload_json.trim().is_empty() {
                            None
                        } else {
                            Some(replay_request.payload_json.as_str())
                        },
                        &format,
                    )
                }
                _ => Err("capability gap supports 'show' and 'replay'".to_string()),
            }
        }
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
        _ => Err("capability supports 'list', 'show', 'scaffold', 'forge', 'import-workspace-skill', 'import-openclaw-plugin-skill-subset', 'verify', 'promote', and 'shim'".to_string()),
    }
}

fn handle_queue(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("inspect") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let limit = take_value(args, "--limit")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(0);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let records = inspect_pending_queue(&root, limit)?;
            if format == "json" {
                print!("{}", render_queue_inspect_json(&root, &records, limit));
            } else {
                print_human(&render_queue_inspect_human(&root, &records, limit));
            }
            Ok(())
        }
        Some("consume") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = consume_pending_queue(&root, kernel_path.as_deref(), max_jobs)?;
            if format == "json" {
                print!("{}", render_queue_consume_json(&summary));
            } else {
                print_human(&render_queue_consume_human(&summary));
            }
            Ok(())
        }
        Some("run-once") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = run_queue_once(&root, kernel_path.as_deref())?;
            if format == "json" {
                print!("{}", render_queue_run_once_json(&summary));
            } else {
                print_human(&render_queue_run_once_human(&summary));
            }
            Ok(())
        }
        Some("run-until-empty") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let max_passes = take_value(args, "--max-passes")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(25);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = run_queue_until_empty(&root, kernel_path.as_deref(), max_jobs, max_passes)?;
            if format == "json" {
                print!("{}", render_queue_run_until_empty_json(&summary));
            } else {
                print_human(&render_queue_run_until_empty_human(&summary));
            }
            Ok(())
        }
        Some("status") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = queue_status(&root)?;
            if format == "json" {
                print!("{}", render_queue_status_json(&snapshot));
            } else {
                print_human(&render_queue_status_human(&snapshot));
            }
            Ok(())
        }
        Some("ack") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let job_id = required_flag(args, "--job-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let capture = ack_queue_job(&root, &job_id)?;
            if format == "json" {
                print!("{}", render_queue_ack_json(&capture));
            } else {
                print_human(&render_queue_ack_human(&capture));
            }
            Ok(())
        }
        _ => Err("queue supports 'inspect', 'consume', 'run-once', 'run-until-empty', 'status', and 'ack'".to_string()),
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
        Some("approve") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let job_id = required_flag(args, "--job-id")?;
            let result = approve_job(&root, &job_id)?;
            println!("{}", result);
            Ok(())
        }
        _ => Err("job supports 'list', 'inspect', and 'approve'".to_string()),
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


fn run_action_execute_request(
    root: &Path,
    config: &Config,
    agent_id: &str,
    capability_name: Option<&str>,
    mut action_type: String,
    mut resource: String,
    estimated_cost_usd: f64,
    kernel_path: Option<&str>,
    org_id: Option<&str>,
    run_id: Option<&str>,
    session_id: Option<&str>,
    payload_json: Option<&str>,
    format: &str,
) -> LoomResult<()> {
    let identity = resolve_agent_identity(&root, kernel_path, agent_id, org_id)?;
    if let Some(name) = capability_name {
        match find_capability_by_name(&root, config, name)? {
            Some(capability) => {
                if action_type.is_empty() {
                    action_type = capability.action_type;
                }
                if resource.is_empty() {
                    resource = capability.resource;
                }
            }
            None => return Err(format!("capability '{}' not found", name)),
        }
    }
    if action_type.trim().is_empty() || resource.trim().is_empty() {
        return Err("action execute requires --action-type and --resource, or a resolvable --capability".to_string());
    }

    let envelope = build_action_envelope_with_options(
        &root,
        kernel_path,
        agent_id,
        org_id,
        &action_type,
        &resource,
        estimated_cost_usd,
        run_id,
        session_id,
        capability_name,
        payload_json,
    )?;
    let reference = evaluate_reference_gates(&root, kernel_path, &identity, &envelope)?;
    let decision = capture_decision(&root, &identity, &envelope, &reference)?;
    let effective_kernel_path = kernel_path_for(&root, kernel_path)?;
    let capture = capture_runtime_execution(&root, &effective_kernel_path, &envelope, &reference, &decision)?;
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
fn handle_action(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("enqueue") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let config = read_config(&root)?;
            let capability_name = take_value(args, "--capability");
            let mut action_type = take_value(args, "--action-type").unwrap_or_default();
            let mut resource = take_value(args, "--resource").unwrap_or_default();
            let payload_json = take_value(args, "--payload-json");
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            if let Some(name) = capability_name.as_deref() {
                let capability = find_capability_by_name(&root, &config, name)?
                    .ok_or_else(|| format!("capability '{}' not found", name))?;
                if action_type.is_empty() {
                    action_type = capability.action_type;
                }
                if resource.is_empty() {
                    resource = capability.resource;
                }
            }
            if action_type.trim().is_empty() || resource.trim().is_empty() {
                return Err("action enqueue requires --action-type and --resource, or a resolvable --capability".to_string());
            }

            let envelope = build_action_envelope_with_options(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
                capability_name.as_deref(),
                payload_json.as_deref(),
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
            let config = read_config(&root)?;
            let capability_name = take_value(args, "--capability");
            let gap_class = take_value(args, "--gap-class").unwrap_or_default();
            let gap_goal = take_value(args, "--goal").unwrap_or_default();
            let mut action_type = take_value(args, "--action-type").unwrap_or_default();
            let mut resource = take_value(args, "--resource").unwrap_or_default();
            let payload_json = take_value(args, "--payload-json");
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            if let Some(name) = capability_name.as_deref() {
                match find_capability_by_name(&root, &config, name)? {
                    Some(capability) => {
                        if action_type.is_empty() {
                            action_type = capability.action_type;
                        }
                        if resource.is_empty() {
                            resource = capability.resource;
                        }
                    }
                    None if !gap_class.is_empty() => {
                        let gap = record_capability_gap(
                            &root,
                            &config,
                            &CapabilityGapRequest {
                                requested_via: "action_execute".to_string(),
                                capability_name: name.to_string(),
                                gap_class,
                                goal: gap_goal,
                                proposed_capability_name: name.to_string(),
                                agent_id: agent_id.clone(),
                                org_id: org_id.clone().unwrap_or_else(|| config.org_id.clone()),
                                request_id: String::new(),
                                kernel_path: kernel_path.clone().unwrap_or_default(),
                                action_type: action_type.clone(),
                                resource: resource.clone(),
                                payload_json: payload_json.clone().unwrap_or_default(),
                                run_id: run_id.clone().unwrap_or_default(),
                                session_id: session_id.clone().unwrap_or_default(),
                                original_request_json: String::new(),
                            },
                        )?;
                        if format == "json" {
                            print!("{}", render_capability_gap_json(&gap));
                        } else {
                            print_human(&render_capability_gap_human(&gap));
                        }
                        return Ok(());
                    }
                    None => return Err(format!("capability '{}' not found", name)),
                }
            }
            if action_type.trim().is_empty() || resource.trim().is_empty() {
                return Err("action execute requires --action-type and --resource, or a resolvable --capability".to_string());
            }

            let identity =
                resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope_with_options(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
                capability_name.as_deref(),
                payload_json.as_deref(),
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

fn handle_supervisor_daemon_start(args: &[String]) -> LoomResult<()> {
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

fn handle_supervisor_daemon_loop(args: &[String]) -> LoomResult<()> {
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

fn handle_supervisor_daemon_status(args: &[String]) -> LoomResult<()> {
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

fn handle_supervisor_daemon_stop(args: &[String]) -> LoomResult<()> {
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

fn handle_supervisor_daemon(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("start") => handle_supervisor_daemon_start(args),
        Some("loop") => handle_supervisor_daemon_loop(args),
        Some("status") => handle_supervisor_daemon_status(args),
        Some("stop") => handle_supervisor_daemon_stop(args),
        _ => Err("supervisor daemon supports 'start', 'loop', 'status', and 'stop'".to_string()),
    }
}

fn handle_service(args: &[String]) -> LoomResult<()> {
    if args.is_empty() || matches!(args.first().map(String::as_str), Some("help" | "--help" | "-h")) {
        print_service_help();
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("start") => start_service_with_mode(&args[1..]),
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
        Some("stop") => handle_stop(&args[1..]),
        Some("submit") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let agent_id = required_flag(args, "--agent-id")?;
            let capability_name = take_value(args, "--capability");
            let gap_class = take_value(args, "--gap-class").unwrap_or_default();
            let gap_goal = take_value(args, "--goal").unwrap_or_default();
            let mut action_type = take_value(args, "--action-type").unwrap_or_default();
            let mut resource = take_value(args, "--resource").unwrap_or_default();
            let payload_json = take_value(args, "--payload-json");
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let socket_path = take_value(args, "--socket");
            let http_url = take_value(args, "--http-url");
            let service_token = effective_service_token(&config, take_value(args, "--service-token"))?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            if let Some(name) = capability_name.as_deref() {
                match find_capability_by_name(&root, &config, name)? {
                    Some(capability) => {
                        if action_type.is_empty() {
                            action_type = capability.action_type;
                        }
                        if resource.is_empty() {
                            resource = capability.resource;
                        }
                    }
                    None if !gap_class.is_empty() => {
                        let gap = record_capability_gap(
                            &root,
                            &config,
                            &CapabilityGapRequest {
                                requested_via: "service_submit".to_string(),
                                capability_name: name.to_string(),
                                gap_class,
                                goal: gap_goal,
                                proposed_capability_name: name.to_string(),
                                agent_id: agent_id.clone(),
                                org_id: org_id.clone().unwrap_or_else(|| config.org_id.clone()),
                                request_id: String::new(),
                                kernel_path: kernel_path.clone().unwrap_or_default(),
                                action_type: action_type.clone(),
                                resource: resource.clone(),
                                payload_json: payload_json.clone().unwrap_or_default(),
                                run_id: run_id.clone().unwrap_or_default(),
                                session_id: session_id.clone().unwrap_or_default(),
                                original_request_json: String::new(),
                            },
                        )?;
                        if format == "json" {
                            print!("{}", render_capability_gap_json(&gap));
                        } else {
                            print_human(&render_capability_gap_human(&gap));
                        }
                        return Ok(());
                    }
                    None => return Err(format!("capability '{}' not found", name)),
                }
            }
            if action_type.trim().is_empty() || resource.trim().is_empty() {
                return Err("service submit requires --action-type and --resource, or a resolvable --capability".to_string());
            }

            let envelope = build_action_envelope_with_options(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
                capability_name.as_deref(),
                payload_json.as_deref(),
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

fn start_service_with_mode(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let config = read_config(&root)?;
    let kernel_path = take_value(args, "--kernel-path");
    let socket_path = take_value(args, "--socket");
    let http_address = if has_flag(args, "--no-http") {
        None
    } else {
        take_value(args, "--http-address").or_else(|| {
            let value = config.service_http_address.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
    };
    let service_token = effective_service_token(&config, take_value(args, "--service-token"))?;
    if http_address.is_some()
        && service_token
            .as_deref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    {
        return Err(
            "service-token is required when the local HTTP control plane is enabled".to_string(),
        );
    }
    let commitments_source = take_value(args, "--commitments-source");
    let workspace_token = take_value(args, "--workspace-token");
    let max_jobs = take_value(args, "--max-jobs")
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(config.service_max_jobs);
    let poll_seconds = take_value(args, "--poll-seconds")
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(config.service_poll_seconds);
    let iterations = take_value(args, "--iterations")
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(config.service_max_iterations);
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let foreground = has_flag(args, "--foreground");
    let current = runtime_service_status(&root, socket_path.as_deref())?;
    if current.running {
        if format == "json" {
            print!("{}", render_runtime_service_json(&current));
        } else {
            print_human(&render_runtime_service_human(&current));
        }
        return Ok(());
    }

    if foreground {
        let session_id = format!("service-{}", chrono_like_timestamp());
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
            if iterations == 0 { usize::MAX } else { iterations },
            &session_id,
        )?;
        if format == "json" {
            print!("{}", render_runtime_service_json(&snapshot));
        } else {
            print_human(&render_runtime_service_human(&snapshot));
        }
        return Ok(());
    }

    let service_dir = root.join(&config.run_dir).join("service");
    std::fs::create_dir_all(&service_dir).map_err(|e| e.to_string())?;
    let log_dir = root.join(&config.log_dir);
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let stdout_log_path = log_dir.join("service.log");
    rotate_log_file_if_needed(&stdout_log_path, config.log_max_bytes, config.log_max_files)?;
    let stdout = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&stdout_log_path)
        .map_err(|e| e.to_string())?;
    set_private_permissions_if_supported(&stdout_log_path, 0o600)?;
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
        .arg(if iterations == 0 { usize::MAX.to_string() } else { iterations.to_string() })
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
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let note = format!(
        "runtime service start requested; pid={} session_id={} log={}",
        child.id(),
        session_id,
        stdout_log_path.display()
    );
    let mut snapshot_result = runtime_service_status(&root, socket_path.as_deref());
    let mut early_exit = None;
    for _ in 0..30 {
        if let Ok(snapshot) = &snapshot_result {
            if snapshot.available && snapshot.running && snapshot.session_id == session_id {
                break;
            }
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            early_exit = Some(status);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        snapshot_result = runtime_service_status(&root, socket_path.as_deref());
    }
    if let Some(status) = early_exit {
        return Err(format!(
            "runtime service exited before becoming healthy (session_id={} status={})",
            session_id, status
        ));
    }
    let mut snapshot = snapshot_result.unwrap_or_else(|_| current);
    if snapshot.note.is_empty() {
        snapshot.note = note;
    }
    if format == "json" {
        print!("{}", render_runtime_service_json(&snapshot));
    } else {
        print_human(&render_runtime_service_human(&snapshot));
    }
    Ok(())
}

fn effective_service_token(config: &loom_core::Config, explicit: Option<String>) -> LoomResult<Option<String>> {
    if let Some(explicit) = explicit {
        return Ok(Some(explicit));
    }
    if config.service_token_env.trim().is_empty() {
        return Ok(None);
    }
    Ok(env::var(&config.service_token_env)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn take_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn take_values(args: &[String], flag: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
        .collect()
}

fn forge_name_from_args(args: &[String], gap_class: &str, goal: &str) -> LoomResult<String> {
    if let Some(name) = take_value(args, "--name") {
        return Ok(name);
    }
    if gap_class.trim().is_empty() {
        return Err("capability forge requires --name or --gap-class".to_string());
    }
    let goal_token = if goal.trim().is_empty() {
        "candidate".to_string()
    } else {
        sanitize_token(goal)
    };
    Ok(format!(
        "loomforge.{}.{}.v0",
        sanitize_token(gap_class),
        goal_token
    ))
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|value| value == flag)
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

fn read_runtime_event_execution_id(path: &PathBuf) -> LoomResult<String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let job_id = value
        .get("job_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let execution_id = value
        .get("execution_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if job_id.is_empty() || execution_id.is_empty() {
        return Err(format!(
            "runtime event at {} missing job_id or execution_id",
            path.display()
        ));
    }
    Ok(execution_id)
}

fn read_json_file(path: &PathBuf) -> LoomResult<Value> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

fn render_capability_evidence_human(
    root: &std::path::Path,
    capability: &loom_core::capabilities::CapabilityDescriptor,
) -> String {
    let manifest_path = root
        .join("capabilities")
        .join("custom")
        .join(format!("{}.json", sanitize_token(&capability.name)));
    if capability.last_verification_job_id.is_empty() {
        return format!(
            "Verification evidence\n=====================\nmanifest:          {}\nverification_job:  (none)\nexpectation_summary: capability has not been verified through Loom yet\n",
            manifest_path.display(),
        );
    }
    match inspect_job(root, &capability.last_verification_job_id) {
        Ok(job) => {
            let worker_result_path = job
                .job_path
                .parent()
                .map(|parent| parent.join("result.json"))
                .unwrap_or_else(|| root.join("state/runtime/jobs/result.json"));
            format!(
                "Verification evidence\n=====================\nmanifest:          {}\nverification_job:  {}\nverification_exec: {}\nexpectation_summary: {}\njob_path:          {}\njob_status:        {}\njob_stage:         {}\nruntime_outcome:   {}\nworker_status:     {}\nbudget_status:     {}\nfailure_reason:    {}\njob_note:          {}\nworker_result:     {}\nevent_path:        {}\naudit_log:         {}\nparity_report:     {}\n",
                manifest_path.display(),
                capability.last_verification_job_id,
                if capability.last_verification_execution_id.is_empty() {
                    "(none)"
                } else {
                    &capability.last_verification_execution_id
                },
                if capability.verification_note.is_empty() {
                    "(none)"
                } else {
                    &capability.verification_note
                },
                job.job_path.display(),
                job.status,
                job.stage,
                job.runtime_outcome,
                job.worker_status,
                job.budget_reservation_status,
                if job.budget_reservation_reason.is_empty() {
                    if job.note.is_empty() {
                        "(none)"
                    } else {
                        &job.note
                    }
                } else {
                    &job.budget_reservation_reason
                },
                if job.note.is_empty() { "(none)" } else { &job.note },
                worker_result_path.display(),
                job.event_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "(none)".to_string()),
                job.audit_log_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "(none)".to_string()),
                job.parity_report_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "(none)".to_string()),
            )
        }
        Err(error) => format!(
            "Verification evidence\n=====================\nmanifest:          {}\nverification_job:  {}\nverification_exec: {}\nexpectation_summary: {}\nlookup_error:      {}\n",
            manifest_path.display(),
            capability.last_verification_job_id,
            if capability.last_verification_execution_id.is_empty() {
                "(none)"
            } else {
                &capability.last_verification_execution_id
            },
            if capability.verification_note.is_empty() {
                "(none)"
            } else {
                &capability.verification_note
            },
            error
        ),
    }
}

fn capability_verification_evidence_value(
    root: &std::path::Path,
    capability: &loom_core::capabilities::CapabilityDescriptor,
) -> LoomResult<Value> {
    let manifest_path = root
        .join("capabilities")
        .join("custom")
        .join(format!("{}.json", sanitize_token(&capability.name)));
    if capability.last_verification_job_id.is_empty() {
        return Ok(serde_json::json!({
            "manifest": manifest_path.display().to_string(),
            "verification_job": Value::Null,
            "expectation_summary": if capability.verification_note.is_empty() {
                "capability has not been verified through Loom yet"
            } else {
                capability.verification_note.as_str()
            },
        }));
    }

    match inspect_job(root, &capability.last_verification_job_id) {
        Ok(job) => {
            let worker_result_path = job
                .job_path
                .parent()
                .map(|parent| parent.join("result.json"))
                .unwrap_or_else(|| root.join("state/runtime/jobs/result.json"));
            Ok(serde_json::json!({
                "manifest": manifest_path.display().to_string(),
                "verification_job": capability.last_verification_job_id,
                "verification_exec": if capability.last_verification_execution_id.is_empty() {
                    Value::Null
                } else {
                    Value::String(capability.last_verification_execution_id.clone())
                },
                "expectation_summary": if capability.verification_note.is_empty() {
                    Value::Null
                } else {
                    Value::String(capability.verification_note.clone())
                },
                "job_path": job.job_path.display().to_string(),
                "job_status": job.status,
                "job_stage": job.stage,
                "runtime_outcome": job.runtime_outcome,
                "worker_status": job.worker_status,
                "budget_status": job.budget_reservation_status,
                "failure_reason": if job.budget_reservation_reason.is_empty() {
                    if job.note.is_empty() {
                        Value::Null
                    } else {
                        Value::String(job.note.clone())
                    }
                } else {
                    Value::String(job.budget_reservation_reason)
                },
                "job_note": if job.note.is_empty() {
                    Value::Null
                } else {
                    Value::String(job.note)
                },
                "worker_result": worker_result_path.display().to_string(),
                "event_path": job.event_path.map(|path| path.display().to_string()),
                "audit_log": job.audit_log_path.map(|path| path.display().to_string()),
                "parity_report": job.parity_report_path.map(|path| path.display().to_string()),
            }))
        }
        Err(error) => Ok(serde_json::json!({
            "manifest": manifest_path.display().to_string(),
            "verification_job": capability.last_verification_job_id,
            "verification_exec": if capability.last_verification_execution_id.is_empty() {
                Value::Null
            } else {
                Value::String(capability.last_verification_execution_id.clone())
            },
            "expectation_summary": if capability.verification_note.is_empty() {
                Value::Null
            } else {
                Value::String(capability.verification_note.clone())
            },
            "lookup_error": error,
        })),
    }
}

fn render_capability_show_json(
    root: &std::path::Path,
    capability: &loom_core::capabilities::CapabilityDescriptor,
) -> LoomResult<String> {
    let mut value: Value = serde_json::from_str(&render_capability_json(capability))
        .map_err(|error| format!("failed to parse capability json: {}", error))?;
    let evidence = capability_verification_evidence_value(root, capability)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("verification_evidence".to_string(), evidence);
    }
    Ok(format!("{}\n", value))
}

fn verify_capability_expectations(


    worker_result: Option<&Value>,
    expect_summary_contains: Option<&str>,
    expect_result_fields: &[String],
) -> LoomResult<Vec<String>> {
    if expect_summary_contains.is_none() && expect_result_fields.is_empty() {
        return Ok(Vec::new());
    }
    let Some(worker_result) = worker_result else {
        return Ok(vec!["worker result missing while expectations were requested".to_string()]);
    };
    let mut failures = Vec::new();
    if let Some(fragment) = expect_summary_contains {
        let summary = worker_result
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !summary.contains(fragment) {
            failures.push(format!(
                "summary missing fragment {:?} (actual: {:?})",
                fragment, summary
            ));
        }
    }
    for expectation in expect_result_fields {
        let Some((path, expected_raw)) = expectation.split_once('=') else {
            failures.push(format!(
                "invalid --expect-result-field {:?}; expected PATH=VALUE",
                expectation
            ));
            continue;
        };
        let Some(actual) = lookup_json_path(worker_result, path) else {
            failures.push(format!("result field {:?} not found", path));
            continue;
        };
        if !json_value_matches(actual, expected_raw) {
            failures.push(format!(
                "result field {:?} expected {:?} but was {}",
                path,
                expected_raw,
                json_value_to_string(actual)
            ));
        }
    }
    Ok(failures)
}

fn lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            return None;
        }
        current = match current {
            Value::Object(_) => current.get(trimmed)?,
            Value::Array(items) => {
                let index = trimmed.parse::<usize>().ok()?;
                items.get(index)?
            }
            _ => return None,
        };
    }
    Some(current)
}

fn json_value_matches(actual: &Value, expected_raw: &str) -> bool {
    if let Ok(expected_json) = serde_json::from_str::<Value>(expected_raw) {
        return actual == &expected_json;
    }
    json_value_to_string(actual) == expected_raw
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::String(raw) => raw.clone(),
        _ => value.to_string(),
    }
}

fn sanitize_token(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn print_last_lines(path: &PathBuf, lines: usize) -> LoomResult<u64> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let collected = contents.lines().collect::<Vec<_>>();
    let start = collected.len().saturating_sub(lines);
    for line in &collected[start..] {
        println!("{}", line);
    }
    Ok(contents.len() as u64)
}

fn print_new_bytes(path: &PathBuf, offset: u64) -> LoomResult<u64> {
    let contents = std::fs::read(path).map_err(|e| e.to_string())?;
    let start = offset.min(contents.len() as u64) as usize;
    if start < contents.len() {
        let new_bytes = &contents[start..];
        print!("{}", String::from_utf8_lossy(new_bytes));
    }
    Ok(contents.len() as u64)
}

fn rotate_log_file_if_needed(path: &PathBuf, max_bytes: usize, max_files: usize) -> LoomResult<()> {
    if max_bytes == 0 || max_files == 0 || !path.exists() {
        return Ok(());
    }
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len() as usize;
    if size < max_bytes {
        return Ok(());
    }
    let base_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("cannot rotate log path {}", path.display()))?
        .to_string();
    let parent = path
        .parent()
        .ok_or_else(|| format!("cannot rotate log path {}", path.display()))?;
    if max_files > 1 {
        for index in (1..max_files).rev() {
            let source = parent.join(format!("{}.{}", base_name, index));
            let target = parent.join(format!("{}.{}", base_name, index + 1));
            if source.exists() {
                if target.exists() {
                    let _ = std::fs::remove_file(&target);
                }
                std::fs::rename(&source, &target).map_err(|e| e.to_string())?;
            }
        }
        let first = parent.join(format!("{}.1", base_name));
        if first.exists() {
            let _ = std::fs::remove_file(&first);
        }
        std::fs::rename(path, &first).map_err(|e| e.to_string())?;
        set_private_permissions_if_supported(&first, 0o600)?;
    } else {
        std::fs::write(path, "").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn json_string(input: &str) -> String {
    format!("{:?}", input)
}

#[cfg(unix)]
fn set_private_permissions_if_supported(path: &std::path::Path, mode: u32) -> LoomResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path).map_err(|e| e.to_string())?.permissions();
    permissions.set_mode(mode);
    match std::fs::set_permissions(path, permissions) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(not(unix))]
fn set_private_permissions_if_supported(_path: &std::path::Path, _mode: u32) -> LoomResult<()> {
    Ok(())
}

fn print_start_help() {
    print_human(
        "Meridian Loom // START HELP\n\
=============================\n\
usage: loom start [--root PATH] [--kernel-path PATH] [--http-address HOST:PORT] [--service-token TOKEN] [--max-jobs N] [--poll-seconds N] [--iterations N] [--foreground] [--format human|json]\n\
\n\
notes:\n\
  - starts the local-first runtime service for a single host\n\
  - background mode writes logs to <root>/logs/service.log\n\
  - foreground mode keeps the service attached to the current terminal\n\
  - --service-token is required whenever the HTTP control plane is enabled\n",
    );
}

fn print_stop_help() {
    print_human(
        "Meridian Loom // STOP HELP\n\
============================\n\
usage: loom stop [--root PATH] [--socket PATH] [--format human|json]\n\
\n\
notes:\n\
  - requests a clean local service shutdown\n\
  - safe to run repeatedly; duplicate stop requests are treated as idempotent local lifecycle operations\n",
    );
}

fn print_restart_help() {
    print_human(
        "Meridian Loom // RESTART HELP\n\
===============================\n\
usage: loom restart [--root PATH] [--kernel-path PATH] [--http-address HOST:PORT] [--service-token TOKEN] [--max-jobs N] [--poll-seconds N] [--iterations N] [--foreground] [--format human|json]\n\
\n\
notes:\n\
  - stops the current local service if it is running\n\
  - starts a new local service session for the same runtime root\n",
    );
}

fn print_logs_help() {
    print_human(
        "Meridian Loom // LOGS HELP\n\
============================\n\
usage: loom logs [--root PATH] [--lines N] [--follow]\n\
\n\
notes:\n\
  - tails <root>/logs/service.log\n\
  - use --follow for a streaming local operator view\n",
    );
}

fn print_service_help() {
    print_human(
        "Meridian Loom // SERVICE HELP\n\
===============================\n\
usage:\n\
  loom service start [--root PATH] [--kernel-path PATH] [--socket PATH] [--http-address HOST:PORT] [--service-token TOKEN] [--commitments-source PATH|URL] [--workspace-token TOKEN] [--max-jobs N] [--poll-seconds N] [--iterations N] [--foreground] [--format human|json]\n\
  loom service status [--root PATH] [--socket PATH] [--format human|json]\n\
  loom service submit --agent-id ID [--capability NAME] [--action-type TYPE] [--resource RESOURCE] [--payload-json JSON] [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--socket PATH] [--http-url URL] [--service-token TOKEN] [--format human|json]\n\
  loom service import-commitments --root PATH --commitments-source PATH|URL [--kernel-path PATH] [--workspace-token TOKEN] [--format human|json]\n\
  loom service stop [--root PATH] [--socket PATH] [--format human|json]\n\
\n\
http surface:\n\
  GET  /status\n\
  GET  /health\n\
  GET  /metrics\n\
  GET  /config\n\
  GET  /jobs/<id>\n\
  POST /submit\n\
  POST /import-commitments\n\
  POST /stop\n\
  POST /mcp/tools/list\n\
  POST /mcp/tools/call\n\
  GET  /.well-known/agent.json\n",
    );
}

fn print_help() {
    print_human(
        "Meridian Loom // HELP\n\
======================\n\
phase:       production-oriented local runtime surface\n\
boundary:    local-first service is real; hosted replacement is not\n\
\n\
Bootstrap\n\
---------\n\
  loom version\n\
  loom init --mode <embedded|shadow|standalone> [--kernel-path PATH] [--root PATH] [--org-id ID]\n\
  loom doctor [--root PATH] [--format json|human]\n\
  loom health [--root PATH] [--format json|human]\n\
  loom status [--root PATH]\n\
  loom start [--root PATH] [--kernel-path PATH] [--http-address HOST:PORT] [--service-token TOKEN] [--foreground]\n\
  loom stop [--root PATH]\n\
  loom restart [--root PATH] [--kernel-path PATH] [--http-address HOST:PORT] [--service-token TOKEN] [--foreground]\n\
  loom logs [--root PATH] [--lines N] [--follow]\n\
  loom config show [--root PATH]\n\
\n\
Governance surfaces\n\
-------------------\n\
  loom contract show [--root PATH] [--kernel-path PATH] [--format human|json]\n\
  loom contract verify [--root PATH] [--kernel-path PATH] [--agent-id ID] [--org-id ORG] [--format human|json]\n\
  loom capsule inspect [--root PATH]\n\
  loom capability list [--root PATH] [--format human|json]\n\
  loom capability show --name NAME [--root PATH] [--format human|json]\n\
  loom capability gap show --gap-id ID [--root PATH] [--format human|json]\n\
  loom capability gap replay --gap-id ID [--root PATH] [--format human|json]\n\
  loom capability scaffold --name NAME --action-type TYPE --resource RESOURCE [--description TEXT] [--worker-kind python|wasm] [--worker-entry PATH] [--wasm-module builtin:minimal|wasm:PATH] [--payload-mode json|none] [--root PATH]\n\
  loom capability forge [--name NAME] [--gap-id ID] [--template echo_json_v0|artifact_inspect_v0|url_bundle_v0] [--gap-class artifact_triage|url_collection|response_echo] [--goal TEXT] [--description TEXT] [--root PATH] [--format human|json]\n\
  loom capability import-workspace-skill --skill-root PATH [--entrypoint PATH] [--name NAME] [--root PATH] [--format human|json]\n  loom capability import-openclaw-plugin-skill-subset --plugin-root PATH [--root PATH] [--format human|json]\n\
  loom capability verify --name NAME --agent-id ID --kernel-path PATH [--gap-id ID] [--org-id ORG] [--payload-json JSON] [--estimated-cost-usd USD] [--expect-summary-contains TEXT] [--expect-result-field PATH=VALUE]... [--root PATH] [--format human|json]\n\
  loom capability promote --name NAME [--gap-id ID] [--root PATH] [--format human|json]\n\
  loom capability shim --tool-name NAME --input-schema JSON --output-schema JSON [--version SEMVER] [--format human|json]\n\
  loom job list [--root PATH] [--status STATUS] [--limit N] [--format human|json]\n\
  loom job inspect --job-id HASH [--root PATH] [--format human|json]\n\
  loom job approve --job-id HASH [--root PATH]\n\
  loom agent resolve --agent-id ID [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom envelope build --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom wasm limits [--config-file loom.toml.example] [--format human|json]\n\
  loom wasm profile show [--profile minimal|standard|heavy] [--format human|json]\n\
  loom wasm host show [--profile minimal|standard|heavy|custom] [--backend preview_only|wasmtime_ready] [--config-file loom.toml.example] [--format human|json]\n\
  loom wasm run [--module builtin:minimal|PATH] [--entrypoint NAME] [--entrypoint-arg I32] [--fuel-budget N] [--profile minimal|standard|heavy|custom] [--backend wasmtime_ready] [--config-file loom.toml.example] [--format human|json]\n\
\n\
Runtime rehearsal\n\
-----------------\n\
  loom action enqueue --agent-id ID [--capability NAME] [--action-type TYPE] [--resource RESOURCE] [--payload-json JSON] [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom action execute --agent-id ID [--capability NAME] [--gap-class CLASS] [--goal TEXT] [--action-type TYPE] [--resource RESOURCE] [--payload-json JSON] [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom service start [--root PATH] [--kernel-path PATH] [--socket PATH] [--http-address HOST:PORT] [--service-token TOKEN] [--commitments-source PATH|URL] [--workspace-token TOKEN] [--max-jobs N] [--poll-seconds N] [--iterations N] [--foreground] [--format human|json]\n\
  loom service status [--root PATH] [--socket PATH] [--format human|json]\n\
  loom service submit --agent-id ID [--capability NAME] [--gap-class CLASS] [--goal TEXT] [--action-type TYPE] [--resource RESOURCE] [--payload-json JSON] [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--socket PATH] [--http-url URL] [--service-token TOKEN] [--format human|json]\n\
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
\n\
Next\n\
----\n\
  1. loom init --mode embedded --root \"$HOME/.local/share/meridian-loom/runtime/default\" --kernel-path /tmp/meridian-kernel\n\
  2. export LOOM_SERVICE_TOKEN=loom-local-token\n\
  3. loom start --root \"$HOME/.local/share/meridian-loom/runtime/default\" --kernel-path /tmp/meridian-kernel --http-address 127.0.0.1:18910 --service-token \"$LOOM_SERVICE_TOKEN\"\n\
  4. curl -sS -H 'Authorization: Bearer loom-local-token' http://127.0.0.1:18910/status\n\
  5. loom logs --root \"$HOME/.local/share/meridian-loom/runtime/default\" --lines 50\n",
    );
}

fn print_queue_help() {
    print_human(
        "Meridian Loom // QUEUE HELP
============================
usage:
  loom queue inspect [--root PATH] [--limit N] [--format human|json]
  loom queue consume [--root PATH] [--kernel-path PATH] [--max-jobs N] [--format human|json]
  loom queue run-once [--root PATH] [--kernel-path PATH] [--format human|json]
  loom queue run-until-empty [--root PATH] [--kernel-path PATH] [--max-jobs N] [--max-passes N] [--format human|json]
  loom queue status [--root PATH] [--format human|json]
  loom queue ack --job-id HASH [--root PATH]
notes:
  - inspect reads pending local queue records without mutating them
  - consume runs the local supervisor over pending queue records and writes filesystem ack receipts
  - run-once is the bounded pipeline step: it performs one local consume pass and records a progress artifact
  - run-until-empty repeatedly consumes bounded passes until the queue drains or the pass cap is reached, and writes a journal plus summary artifact
  - status reports policy-class queue depth without mutating any queue state
  - ack records a terminal job acknowledgement for an already completed, failed, denied, or cancelled job
",
    );
}

fn print_capability_help() {
    print_human(
        "Meridian Loom // CAPABILITY HELP\n\
=================================\n\
Commands\n\
--------\n\
  loom capability list [--root PATH] [--format human|json]\n\
  loom capability show --name NAME [--root PATH] [--format human|json]\n\
  loom capability gap show --gap-id ID [--root PATH] [--format human|json]\n\
  loom capability gap replay --gap-id ID [--root PATH] [--format human|json]\n\
  loom capability scaffold --name NAME --action-type TYPE --resource RESOURCE [--description TEXT] [--worker-kind python|wasm] [--worker-entry PATH] [--wasm-module builtin:minimal|wasm:PATH] [--payload-mode json|none] [--root PATH]\n\
  loom capability forge [--name NAME] [--gap-id ID] [--template echo_json_v0|artifact_inspect_v0|url_bundle_v0] [--gap-class artifact_triage|url_collection|response_echo] [--goal TEXT] [--description TEXT] [--root PATH] [--format human|json]\n\
  loom capability import-workspace-skill --skill-root PATH [--entrypoint PATH] [--name NAME] [--root PATH] [--format human|json]\n  loom capability import-openclaw-plugin-skill-subset --plugin-root PATH [--root PATH] [--format human|json]\n\
  loom capability verify --name NAME --agent-id ID --kernel-path PATH [--gap-id ID] [--org-id ORG] [--payload-json JSON] [--estimated-cost-usd USD] [--expect-summary-contains TEXT] [--expect-result-field PATH=VALUE]... [--root PATH] [--format human|json]\n\
  loom capability promote --name NAME [--gap-id ID] [--root PATH] [--format human|json]\n\
  loom capability shim --tool-name NAME --input-schema JSON --output-schema JSON [--version SEMVER] [--format human|json]\n\
\n\
Notes\n\
-----\n\
  - forge creates a candidate Loom-native capability from either a bounded template, a bounded gap-class, or a recorded capability gap.\n\
  - import-workspace-skill supports a bounded clawfamily contract v0 subset: workspace python entrypoint skills and bundle-manifest python skills. Workspace imports can disambiguate multi-script trees with --entrypoint or entrypoint: front matter.\n  - import-openclaw-plugin-skill-subset imports only immediate child skill dirs under the declared OpenClaw plugin skills roots and reports every unsupported source surface explicitly.\n\
  - verify executes the capability through Loom's runtime path, can assert expectations over the worker result, and writes verification state back into the custom manifest.\n\
  - promote is only for custom/imported capabilities that have already been verified.\n\
  - action execute / service submit with --capability plus --gap-class records a bounded gap object instead of pretending the capability exists.\n\
  - gap replay currently reissues only recorded action_execute gaps; service_submit gaps fail explicitly until their transport-side replay fields are persisted.\n\
  - imported workspace skills still run through Loom queue, job, worker, audit, and artifact paths.\n\
  - this is a local-first compatibility seam, not an OpenClaw runtime dependency or hosted cutover.\n",
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use loom_core::capabilities::ensure_capability_registry_scaffold;

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

    fn sample_config() -> loom_core::Config {
        loom_core::Config {
            mode: "embedded".to_string(),
            kernel_path: String::new(),
            org_id: "local_foundry".to_string(),
            state_dir: "state".to_string(),
            run_dir: "run".to_string(),
            log_dir: "logs".to_string(),
            artifact_dir: "artifacts".to_string(),
            capabilities_dir: "capabilities".to_string(),
            python_path: "workers/python".to_string(),
            typescript_path: "workers/typescript".to_string(),
            wasm_dir: "workers/wasm".to_string(),
            service_http_address: "127.0.0.1:18910".to_string(),
            service_token_env: "LOOM_SERVICE_TOKEN".to_string(),
            service_max_jobs: 8,
            service_poll_seconds: 1,
            service_max_iterations: 0,
            log_level: "info".to_string(),
            log_format: "jsonl".to_string(),
            log_max_bytes: 1024,
            log_max_files: 3,
            handoff_mode: "off".to_string(),
            delivery_queue: loom_core::DEFAULT_DELIVERY_QUEUE.to_string(),
        }
    }

    fn write_job_snapshot(root: &Path, job_id: &str, job_json: &str) {
        let job_dir = root.join("state/runtime/jobs").join(job_id);
        fs::create_dir_all(&job_dir).expect("create job dir");
        fs::write(job_dir.join("job.json"), job_json).expect("write job snapshot");
    }

    #[test]
    fn verify_expectations_accepts_matching_summary_and_fields() {
        let result = json!({
            "summary": "artifact suspicious.exe size=258",
            "artifact_exists": true,
            "artifact_name": "suspicious.exe",
            "meta": {"count": 1}
        });
        let failures = verify_capability_expectations(
            Some(&result),
            Some("suspicious.exe"),
            &[
                "artifact_exists=true".to_string(),
                "artifact_name=suspicious.exe".to_string(),
                "meta.count=1".to_string(),
            ],
        )
        .expect("verify expectations");
        assert!(failures.is_empty());
    }

    #[test]
    fn verify_expectations_reports_failures() {
        let result = json!({
            "summary": "artifact sample.bin size=99",
            "artifact_exists": false
        });
        let failures = verify_capability_expectations(
            Some(&result),
            Some("missing-fragment"),
            &["artifact_exists=true".to_string()],
        )
        .expect("verify expectations");
        assert_eq!(failures.len(), 2);
        assert!(failures.iter().any(|item| item.contains("summary missing fragment")));
        assert!(failures.iter().any(|item| item.contains("artifact_exists")));
    }

    #[test]
    fn verify_expectations_supports_array_paths() {
        let result = json!({
            "skill_output": {
                "blocked": [
                    {"url": "http://127.0.0.1/", "reason": "host resolves to non-public address: 127.0.0.1"}
                ]
            }
        });
        let failures = verify_capability_expectations(
            Some(&result),
            None,
            &[
                "skill_output.blocked.0.url=http://127.0.0.1/".to_string(),
                "skill_output.blocked.0.reason=host resolves to non-public address: 127.0.0.1".to_string(),
            ],
        )
        .expect("verify expectations");
        assert!(failures.is_empty());
    }

    #[test]
    fn capability_show_exposes_verification_evidence_and_reject_reasons() {
        let root = temp_path("loom-cap-show-evidence");
        let config = sample_config();
        ensure_capability_registry_scaffold(&root, &config).expect("registry scaffold");
        scaffold_capability(
            &root,
            &config,
            &CapabilityScaffoldRequest {
                name: "local.custom.reject".to_string(),
                description: "custom reject".to_string(),
                action_type: "respond".to_string(),
                resource: "capability:local.custom.reject".to_string(),
                worker_kind: "python".to_string(),
                worker_entry: String::new(),
                wasm_module: String::new(),
                payload_mode: "json".to_string(),
            },
        )
        .expect("scaffold");

        let job_id = "job::reject";
        let execution_id = "execution::reject";
        let job_path = root.join("state/runtime/jobs").join(job_id).join("job.json");
        write_job_snapshot(
            &root,
            job_id,
            &json!({
                "job_id": job_id,
                "job_path": job_path.display().to_string(),
                "job_status": "failed",
                "job_stage": "rejected",
                "queue_bucket": "reject",
                "queued_at": "1234567890",
                "updated_at": "1234567891",
                "agent_id": "agent_tutorial",
                "org_id": "org_tutorial",
                "action_type": "respond",
                "resource": "capability:local.custom.reject",
                "estimated_cost_usd": "0.050000",
                "runtime_outcome": "worker_rejected",
                "budget_reservation_id": null,
                "budget_reservation_status": "denied",
                "budget_reservation_reason": "policy reject: missing fixture set",
                "worker_status": "rejected",
                "queue_path": null,
                "decision_path": null,
                "execution_path": null,
                "event_path": null,
                "event_stream_path": null,
                "audit_log_path": null,
                "parity_report_path": null,
                "reservation_id": null,
                "reservation_state": "denied",
                "attempt_count": 1,
                "note": "reject reason: missing fixture set"
            })
            .to_string(),
        );

        update_capability_verification(
            &root,
            &config,
            "local.custom.reject",
            "failed",
            "1234567892",
            job_id,
            execution_id,
            "runtime_outcome=worker_rejected | expectation_failures=summary missing fragment: suspicious.exe",
        )
        .expect("update verification");

        let capability = find_capability_by_name(&root, &config, "local.custom.reject")
            .expect("resolve capability")
            .expect("capability present");

        let json_output = render_capability_show_json(&root, &capability).expect("render json");
        let value: Value = serde_json::from_str(&json_output).expect("parse show json");
        let evidence = value
            .get("verification_evidence")
            .and_then(Value::as_object)
            .expect("verification evidence");

        assert_eq!(evidence.get("job_status").and_then(Value::as_str), Some("failed"));
        assert_eq!(evidence.get("job_stage").and_then(Value::as_str), Some("rejected"));
        assert_eq!(
            evidence.get("expectation_summary").and_then(Value::as_str),
            Some("runtime_outcome=worker_rejected | expectation_failures=summary missing fragment: suspicious.exe")
        );
        assert_eq!(
            evidence.get("failure_reason").and_then(Value::as_str),
            Some("policy reject: missing fixture set")
        );
        assert_eq!(
            evidence.get("job_note").and_then(Value::as_str),
            Some("reject reason: missing fixture set")
        );

        let human = render_capability_evidence_human(&root, &capability);
        assert!(human.contains("expectation_summary: runtime_outcome=worker_rejected"));
        assert!(human.contains("failure_reason:    policy reject: missing fixture set"));
        assert!(human.contains("job_note:          reject reason: missing fixture set"));
    }

}
